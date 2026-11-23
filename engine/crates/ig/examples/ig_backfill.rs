//! Carga histórica (#76): baja un rango de velas de IG (con su spread de cierre, #95) y lo pasa
//! por el circuito de ingesta completo (validación → PostgreSQL). No publica en Redis: no es tiempo real.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ig --example ig_backfill -- <INSTRUMENTO> <15m|1h> <desde UTC> <hasta UTC>
//! ```
//! Antes de pedir mira la base (#137): solo pide los tramos de velas cerradas que faltan según
//! el calendario de sesiones, y si no falta nada no se conecta a IG. Gasta del cupo semanal
//! ~1 punto por vela pedida, más el margen horario de cada tramo.

#[path = "common/allowance.rs"]
mod allowance;
#[path = "common/budget.rs"]
mod budget;
#[path = "common/gate.rs"]
mod gate;

use std::collections::BTreeSet;
use std::path::PathBuf;

use cfds_ig::{
    Credentials, IgConfig, IgProvider, ReqwestTransport, api_key_from_env, epics_from_catalog,
    estimate_points, parse_flags,
};
use cfds_ingest::{IngestRequest, ingest, intraday_gaps};
use cfds_market_data::{
    Catalog, ExchangeCalendar, SessionCalendar, StaticProvider, ValidationConfig,
};
use cfds_storage::PgStore;
use chrono::Utc;

const CONFIG: &str = include_str!("../../../../config/ig.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const VALIDATION: &str = include_str!("../../../../config/data_validation.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (args, flags) = parse_flags(std::env::args().skip(1).collect())?;
    let [instrument, tf, from, to] = args.as_slice() else {
        return Err(
            "uso: <INSTRUMENTO> <15m|1h> <desde UTC> <hasta UTC> [--dry-run] [--yes]".into(),
        );
    };
    let catalog = Catalog::from_toml(CATALOG)?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    // Con --dry-run, solo lectura (#132): no migra ni registra instrumentos.
    if flags.writes() {
        store.migrate().await?;
        for spec in catalog.instruments.values() {
            store.upsert_instrument(&spec.instrument).await?;
        }
    } else {
        println!("--dry-run: solo lectura de la base (no migra ni registra instrumentos)");
    }
    let (id, tf) = (instrument.parse()?, tf.parse()?);
    let (from, to) = (from.parse()?, to.parse()?);
    // Mirar la base antes de pedir (#137): con IG, una vela repetida igual gasta cupo.
    let expected = calendar
        .expected_starts(&id, tf, from, to)
        .ok_or("el calendario de sesiones no cubre ese rango: no se pide nada")?;
    let stored: BTreeSet<_> = store
        .candles(&id, tf, from, to)
        .await?
        .into_iter()
        .map(|c| c.start)
        .collect();
    let plan = intraday_gaps(&expected, &stored, tf, Utc::now());
    println!(
        "{id} {tf}: {} velas cerradas esperadas, {} ya guardadas, {} a pedir en {} tramo(s)",
        plan.expected,
        plan.stored,
        plan.missing(),
        plan.ranges.len()
    );
    if plan.ranges.is_empty() {
        println!("no falta nada: no se pide a IG (0 puntos)");
        return Ok(());
    }

    let cfg = IgConfig::from_toml(CONFIG)?;
    let estimated = estimate_points(&plan.ranges, tf.duration(), cfg.local_time_margin_hours);
    if !gate::check(estimated, flags, &cfg, &store, "ig_backfill").await? {
        return Ok(());
    }
    let validation = ValidationConfig::from_toml(VALIDATION)?;
    let transport = ReqwestTransport::new(&cfg, api_key_from_env())?;
    let ig = IgProvider::new(
        transport,
        cfg,
        Credentials::from_env()?,
        epics_from_catalog(&catalog),
    );
    let mut result = Ok(());
    for &(from, to) in &plan.ranges {
        let req = IngestRequest {
            instrument: id.clone(),
            timeframe: tf,
            from,
            to,
        };
        if let Err(e) = load_range(&ig, &calendar, &validation, &store, &req).await {
            result = Err(e);
            break;
        }
    }
    ig.logout().await?;
    allowance::record_and_report(&ig, Some(&store), "ig_backfill").await?;
    result
}

/// Un tramo: velas y spreads del mismo pedido (#95, el cupo se paga una vez), ingesta de
/// siempre (validación incluida) y spread de las velas cerradas.
async fn load_range(
    ig: &IgProvider<ReqwestTransport>,
    calendar: &ExchangeCalendar,
    validation: &ValidationConfig,
    store: &PgStore,
    req: &IngestRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let (candles, spreads) = ig
        .history_with_spreads(&req.instrument, req.timeframe, req.from, req.to)
        .await?;
    let now = Utc::now();
    let r = ingest(
        &StaticProvider::new(candles),
        calendar,
        validation,
        store,
        None,
        req,
        now,
    )
    .await?;
    println!(
        "{} → {}: {} recibidas, {} nuevas, {} ya estaban, {} abiertas descartadas",
        req.from, req.to, r.fetched, r.inserted, r.unchanged, r.skipped_open
    );
    let closed: Vec<_> = spreads
        .into_iter()
        .filter(|s| now >= s.start + s.timeframe.duration())
        .collect();
    let sr = store.insert_spreads(&closed).await?;
    println!(
        "  spreads: {} nuevos, {} ya estaban",
        sr.inserted, sr.unchanged
    );
    Ok(())
}

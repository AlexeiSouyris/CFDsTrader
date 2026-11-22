//! Carga de velas diarias (#102): baja de IG un rango de días de un índice, lo valida contra el
//! calendario de su bolsa (`validate_daily`) y lo guarda en PostgreSQL. Los días que faltan se
//! avisan y no se rellenan; cualquier otro problema bloquea la carga entera. No publica en Redis: no es tiempo real.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ig --example ig_backfill_daily -- <INSTRUMENTO> <desde AAAA-MM-DD> <hasta AAAA-MM-DD>
//! ```
//! Antes de pedir mira la base (#137): solo pide los tramos de días con sesión, ya cerrados,
//! que faltan, y si no falta nada no se conecta a IG. Gasta del cupo semanal ~1 punto por día
//! pedido. El día `hasta` no entra.

#[path = "common/allowance.rs"]
mod allowance;

use std::collections::BTreeSet;
use std::path::PathBuf;

use cfds_ig::{
    Credentials, IgConfig, IgProvider, ReqwestTransport, api_key_from_env, epics_from_catalog,
};
use cfds_ingest::{daily_gaps, session_days};
use cfds_market_data::{
    Catalog, DailyIssueKind, ExchangeCalendar, local_day_bounds, validate_daily,
};
use cfds_storage::PgStore;
use chrono::{NaiveDate, Utc};

const CONFIG: &str = include_str!("../../../../config/ig.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [instrument, from, to] = args.as_slice() else {
        return Err("uso: <INSTRUMENTO> <desde AAAA-MM-DD> <hasta AAAA-MM-DD>".into());
    };
    let id = instrument.parse()?;
    let (from, to): (NaiveDate, NaiveDate) = (from.parse()?, to.parse()?);
    let catalog = Catalog::from_toml(CATALOG)?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let session = calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    for spec in catalog.instruments.values() {
        store.upsert_instrument(&spec.instrument).await?;
    }
    let cfg = IgConfig::from_toml(CONFIG)?;
    let (tz, tolerance) = (cfg.prices_tz, cfg.daily_start_tolerance);

    // Mirar la base antes de pedir (#137): con IG, un día repetido igual gasta cupo.
    let expected = session_days(session, from, to)
        .ok_or("el calendario de sesiones no cubre ese rango: no se pide nada")?;
    let stored: BTreeSet<_> = store
        .daily_candles(&id, from, to)
        .await?
        .into_iter()
        .map(|c| c.day)
        .collect();
    // Primer día que todavía no cerró en la zona de la cuenta: desde ahí no se pide.
    let now = Utc::now();
    let mut closed_by = now.with_timezone(&tz).date_naive();
    while local_day_bounds(closed_by, tz).is_some_and(|(_, end)| end <= now) {
        closed_by = closed_by.succ_opt().ok_or("fecha fuera de rango")?;
    }
    let plan = daily_gaps(&expected, &stored, closed_by);
    println!(
        "{id}: {} días con sesión cerrados, {} ya guardados, {} a pedir en {} tramo(s)",
        plan.expected,
        plan.stored,
        plan.missing(),
        plan.ranges.len()
    );
    if plan.ranges.is_empty() {
        println!("no falta nada: no se pide a IG (0 puntos)");
        return Ok(());
    }

    let transport = ReqwestTransport::new(&cfg, api_key_from_env())?;
    let ig = IgProvider::new(
        transport,
        cfg,
        Credentials::from_env()?,
        epics_from_catalog(&catalog),
    );
    let mut result = Ok(());
    for &(from, to) in &plan.ranges {
        match ig.daily_history(&id, from, to).await {
            Ok(history) => {
                if let Err(e) = save(&store, &id, history, tz, tolerance, session).await {
                    result = Err(e);
                    break;
                }
            }
            Err(e) => {
                result = Err(e.into());
                break;
            }
        }
    }
    ig.logout().await?;
    allowance::record_and_report(&ig, Some(&store), "ig_backfill_daily").await?;
    result
}

/// Valida y guarda un tramo de días.
async fn save(
    store: &PgStore,
    id: &cfds_core::InstrumentId,
    history: cfds_ig::DailyHistory,
    tz: chrono_tz::Tz,
    tolerance: chrono::TimeDelta,
    session: &cfds_market_data::Session,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut daily = history.candles;
    // El día en curso todavía no cerró: no se guarda.
    let now = Utc::now();
    let open = daily.iter().filter(|c| !c.is_closed_at(now)).count();
    daily.retain(|c| c.is_closed_at(now));

    // Un día con sesión sin vela es un hueco del proveedor: se avisa y NO se rellena (no se
    // inventa). Quien lea la serie lo vuelve a ver con `validate_daily`. Cualquier otro
    // problema (precios, límites del día, orden) bloquea la carga entera.
    let issues = validate_daily(&daily, tz, tolerance, session);
    for i in &issues {
        eprintln!("⚠️  {id} {}: {}", i.day, i.kind);
    }
    let blocking = issues
        .iter()
        .filter(|i| !matches!(i.kind, DailyIssueKind::MissingDays { .. }))
        .count();
    if blocking > 0 {
        return Err(format!("{blocking} problemas en los datos: no se guarda nada").into());
    }
    if !history.empty_days.is_empty() {
        println!(
            "{id}: {} días sin operaciones (IG los manda vacíos, no son velas): {:?}…",
            history.empty_days.len(),
            &history.empty_days[..history.empty_days.len().min(5)]
        );
    }
    let r = store.insert_daily(&daily).await?;
    println!(
        "{id}: {} días recibidos ({} → {}), {} nuevos, {} ya estaban, {} sin cerrar descartados",
        daily.len(),
        daily.first().map(|c| c.day.to_string()).unwrap_or_default(),
        daily.last().map(|c| c.day.to_string()).unwrap_or_default(),
        r.inserted,
        r.unchanged,
        open
    );
    Ok(())
}

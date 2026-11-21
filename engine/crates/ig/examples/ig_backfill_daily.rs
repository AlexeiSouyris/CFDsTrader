//! Carga de velas diarias (#102): baja de IG un rango de días de un índice, lo valida contra el
//! calendario de su bolsa (`validate_daily`) y lo guarda en PostgreSQL. Los días que faltan se
//! avisan y no se rellenan; cualquier otro problema bloquea la carga entera. No publica en Redis: no es tiempo real.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ig --example ig_backfill_daily -- <INSTRUMENTO> <desde AAAA-MM-DD> <hasta AAAA-MM-DD>
//! ```
//! Gasta del cupo semanal de IG ~1 punto por día del rango. El día `hasta` no entra.

use std::path::PathBuf;

use cfds_ig::{
    Credentials, IgConfig, IgProvider, ReqwestTransport, api_key_from_env, epics_from_catalog,
};
use cfds_market_data::{Catalog, DailyIssueKind, ExchangeCalendar, validate_daily};
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
    let transport = ReqwestTransport::new(&cfg, api_key_from_env())?;
    let ig = IgProvider::new(
        transport,
        cfg,
        Credentials::from_env()?,
        epics_from_catalog(&catalog),
    );
    let fetched = ig.daily_history(&id, from, to).await;
    let allowance = ig.allowance().await;
    ig.logout().await?;
    let history = fetched?;
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
    if let Some((left, _)) = allowance {
        println!("cupo semanal restante: {left} puntos");
    }
    Ok(())
}

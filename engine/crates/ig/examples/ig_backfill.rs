//! Carga histórica (#76): baja un rango de velas de IG (con su spread de cierre, #95) y lo pasa
//! por el circuito de ingesta completo (validación → PostgreSQL). No publica en Redis: no es tiempo real.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ig --example ig_backfill -- <INSTRUMENTO> <15m|1h> <desde UTC> <hasta UTC>
//! ```
//! Gasta del cupo semanal de IG ~1 punto por vela del rango (más el margen horario).

use std::path::PathBuf;

use cfds_ig::{
    Credentials, IgConfig, IgProvider, ReqwestTransport, api_key_from_env, epics_from_catalog,
};
use cfds_ingest::{IngestRequest, ingest};
use cfds_market_data::{Catalog, ExchangeCalendar, StaticProvider, ValidationConfig};
use cfds_storage::PgStore;
use chrono::Utc;

const CONFIG: &str = include_str!("../../../../config/ig.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const VALIDATION: &str = include_str!("../../../../config/data_validation.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [instrument, tf, from, to] = args.as_slice() else {
        return Err("uso: <INSTRUMENTO> <15m|1h> <desde UTC> <hasta UTC>".into());
    };
    let catalog = Catalog::from_toml(CATALOG)?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    for spec in catalog.instruments.values() {
        store.upsert_instrument(&spec.instrument).await?;
    }
    let cfg = IgConfig::from_toml(CONFIG)?;
    let transport = ReqwestTransport::new(&cfg, api_key_from_env())?;
    let ig = IgProvider::new(
        transport,
        cfg,
        Credentials::from_env()?,
        epics_from_catalog(&catalog),
    );
    let req = IngestRequest {
        instrument: instrument.parse()?,
        timeframe: tf.parse()?,
        from: from.parse()?,
        to: to.parse()?,
    };
    // Velas y spreads del mismo pedido (#95): el cupo semanal se paga una vez.
    let fetched = ig
        .history_with_spreads(&req.instrument, req.timeframe, req.from, req.to)
        .await;
    ig.logout().await?;
    let (candles, spreads) = fetched?;
    let now = Utc::now();
    // Las velas pasan por la ingesta de siempre (validación incluida) desde la memoria.
    let r = ingest(
        &StaticProvider::new(candles),
        &calendar,
        &ValidationConfig::from_toml(VALIDATION)?,
        &store,
        None,
        &req,
        now,
    )
    .await?;
    println!(
        "{} {}: {} recibidas, {} nuevas, {} ya estaban, {} abiertas descartadas",
        req.instrument, req.timeframe, r.fetched, r.inserted, r.unchanged, r.skipped_open
    );
    // Solo el spread de las velas cerradas, que son las que se guardaron.
    let closed: Vec<_> = spreads
        .into_iter()
        .filter(|s| now >= s.start + s.timeframe.duration())
        .collect();
    let sr = store.insert_spreads(&closed).await?;
    println!(
        "spreads: {} nuevos, {} ya estaban",
        sr.inserted, sr.unchanged
    );
    if let Some((left, _)) = ig.allowance().await {
        println!("cupo semanal restante: {left} puntos");
    }
    Ok(())
}

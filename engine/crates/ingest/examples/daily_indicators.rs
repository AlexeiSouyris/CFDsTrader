//! Calcula y guarda los indicadores de las velas diarias (#148, spec §13) de todos los
//! instrumentos del catálogo que tengan diarias guardadas. No usa IG ni gasta cupo.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example daily_indicators
//! ```
//! Es idempotente: lo ya guardado e idéntico cuenta como "ya estaban". Si un valor recalculado
//! diera distinto, frena con error (sería repintado) y no guarda nada de ese instrumento.

use std::path::PathBuf;

use cfds_indicators::DailyConfig;
use cfds_ingest::daily_indicators;
use cfds_market_data::{Catalog, ExchangeCalendar};
use cfds_storage::PgStore;
use chrono::{NaiveDate, Utc};

const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const INDICATORS: &str = include_str!("../../../../config/indicators.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::from_toml(CATALOG)?;
    let cfg = DailyConfig::from_toml(INDICATORS)?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    let (from, to) = (
        NaiveDate::from_ymd_opt(2000, 1, 1).ok_or("fecha")?,
        Utc::now().date_naive().succ_opt().ok_or("fecha")?,
    );
    let now = Utc::now();
    for id in catalog.instruments.keys() {
        let days: Vec<_> = store
            .daily_candles(id, from, to)
            .await?
            .into_iter()
            .filter(|d| d.is_closed_at(now))
            .collect();
        if days.is_empty() {
            println!("{id}: sin velas diarias guardadas");
            continue;
        }
        let session = calendar.session_for(id).ok_or("instrumento sin bolsa")?;
        let rows = daily_indicators(&days, session, &cfg)
            .map_err(|d| format!("{id}: el calendario de sesiones no cubre el {d}"))?;
        let ready = rows
            .iter()
            .filter(|r| r.atr.is_some() && r.realized_vol.is_some())
            .count();
        let r = store.insert_daily_indicators(&rows).await?;
        println!(
            "{id}: {} días con sesión ({} con los dos indicadores) → {} nuevos, {} ya estaban",
            rows.len(),
            ready,
            r.inserted,
            r.unchanged
        );
    }
    Ok(())
}

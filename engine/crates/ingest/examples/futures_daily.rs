//! Arma y guarda las velas diarias de los futuros (#282) desde las velas de 15m ya armadas
//! (`futures_candles`, ejemplo `futures_candles`). No usa IG ni Databento: solo PostgreSQL.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example futures_daily
//! ```
//! - Misma ventana que las diarias de IG: de medianoche a medianoche en `daily_timezone`
//!   (`config/candle_sources.toml`), solo los días con sesión según el calendario del CFD
//!   (`config/sessions.toml` y los feriados), y solo días cerrados dentro de lo bajado.
//! - El día del vencimiento (calendario de #270) usa solo el contrato nuevo.
//! - Se guardan en `daily_candles` bajo el instrumento derivado (por ejemplo `US500.FUT`): nunca
//!   pisan las diarias de IG. Después, `daily_indicators -- --fuente futuros` les calcula los
//!   indicadores.
//! - Es idempotente: una diaria ya guardada e idéntica cuenta como "ya estaba". Una ya guardada
//!   con **otros** valores no se pisa (solo agregar) ni frena el resto (#409): queda la guardada,
//!   la nueva no se guarda y se avisa qué días son. Pasa, por ejemplo, con el primer día de una
//!   historia que después se extendió hacia atrás (ese día se había armado sin su primera hora).

use std::path::PathBuf;

use cfds_core::{InstrumentId, Timeframe};
use cfds_market_data::{
    CandleSource, CandleSources, Catalog, ExchangeCalendar, FuturesFeeds, futures_daily,
};
use cfds_storage::PgStore;
use chrono::Utc;

const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const SOURCES: &str = include_str!("../../../../config/candle_sources.toml");
const DATABENTO: &str = include_str!("../../../../config/databento.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::from_toml(CATALOG)?;
    let sources = CandleSources::from_toml(SOURCES)?;
    let feeds = FuturesFeeds::from_toml(DATABENTO)?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    let now = Utc::now();
    let mut failed = 0;
    for (id, spec) in &catalog.instruments {
        let Some(feed) = feeds.get(id) else {
            println!("{id}: sin futuro en config/databento.toml");
            continue;
        };
        let r: Result<(), Box<dyn std::error::Error>> = async {
            let Some((from, to)) = store
                .futures_loaded_range(&sources.futures_source, &feed.dataset, &feed.symbol, id)
                .await?
            else {
                println!("{id}: sin barras de futuros bajadas");
                return Ok(());
            };
            let until = to.min(now);
            let m15 = store
                .futures_candles(&sources.futures_source, id, Timeframe::M15, from, until)
                .await?;
            let session = calendar.session_for(id).ok_or("instrumento sin bolsa")?;
            let rolls = sources.roll_instants(id, session, &m15)?;
            let out: InstrumentId = sources.instrument_for(CandleSource::Futures, id)?;
            let daily = futures_daily(&m15, &out, session, sources.daily_tz, &rolls, until);
            if daily.is_empty() {
                println!("{out}: sin días cerrados con velas");
                return Ok(());
            }
            store
                .upsert_instrument(
                    &sources.derived_instrument(CandleSource::Futures, &spec.instrument)?,
                )
                .await?;
            // Solo agregar (#409): una diaria ya guardada con otros valores no se pisa ni frena el
            // resto; queda la guardada y se avisa.
            let (saved, kept) = store.insert_daily_keeping(&daily).await?;
            println!(
                "{out} ({}): {} diarias del {} al {} ({} vencimientos) → {} nuevas, {} ya estaban",
                sources.label(CandleSource::Futures),
                daily.len(),
                daily[0].day,
                daily[daily.len() - 1].day,
                rolls.len(),
                saved.inserted,
                saved.unchanged
            );
            if !kept.is_empty() {
                eprintln!(
                    "⚠️  {out}: {} diarias ya guardadas con otros valores, no se pisan (quedan las \
                     guardadas): {}",
                    kept.len(),
                    days_text(&kept)
                );
            }
            Ok(())
        }
        .await;
        if let Err(e) = r {
            failed += 1;
            eprintln!("⚠️  {id}: {e}");
        }
    }
    if failed > 0 {
        return Err(format!("{failed} instrumentos con errores").into());
    }
    Ok(())
}

/// Los días en texto: todos si son pocos; si no, cuántos y el primero y el último.
fn days_text(days: &[chrono::NaiveDate]) -> String {
    match days {
        [] => String::new(),
        [a, .., b] if days.len() > 5 => format!("{} días, del {a} al {b}", days.len()),
        _ => days
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

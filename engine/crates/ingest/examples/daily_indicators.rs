//! Calcula y guarda los indicadores de las velas diarias (#148, spec §13) y la caída desde el
//! máximo (#213, spec §9.1) y el tercio de la volatilidad (spec §13, spec ML §3) de todos los instrumentos del catálogo que tengan diarias guardadas. No usa IG ni gasta cupo.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example daily_indicators [-- --fuente futuros]
//! ```
//! Con `--fuente futuros` (#282) usa las diarias armadas desde los futuros (ejemplo
//! `futures_daily`), guardadas bajo el instrumento derivado (por ejemplo `US500.FUT`), y guarda
//! los indicadores bajo ese mismo id: mismo cálculo, sin mezclar con los del CFD de IG. La sesión
//! de cada día es la del CFD.
//! Es idempotente: lo ya guardado e idéntico cuenta como "ya estaban". Si un valor recalculado
//! diera distinto, frena con error (sería repintado) y no guarda nada de ese instrumento. Con
//! `--fuente futuros` (#409), en cambio, no pisa el guardado ni frena: guarda el resto y avisa qué
//! días quedaron con el valor viejo (la historia de los futuros se extendió hacia atrás).

use std::path::PathBuf;

use cfds_indicators::DailyConfig;
use cfds_ingest::{daily_drawdowns, daily_indicators, daily_vol_terciles};
use cfds_market_data::{CandleSource, CandleSources, Catalog, ExchangeCalendar};
use cfds_storage::PgStore;
use chrono::{NaiveDate, Utc};

const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const INDICATORS: &str = include_str!("../../../../config/indicators.toml");
const SOURCES: &str = include_str!("../../../../config/candle_sources.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::from_toml(CATALOG)?;
    let sources = CandleSources::from_toml(SOURCES)?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let source = match args.iter().position(|a| a == "--fuente") {
        None => CandleSource::Ig,
        Some(i) => args
            .get(i + 1)
            .and_then(|s| CandleSource::parse(s))
            .ok_or("uso: [--fuente ig|futuros]")?,
    };
    println!("diarias: {}", sources.label(source));
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
    for base in catalog.instruments.keys() {
        // Con futuros, las diarias y los indicadores son del instrumento derivado.
        let id = &sources.instrument_for(source, base)?;
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
        let session = calendar.session_for(base).ok_or("instrumento sin bolsa")?;
        let rows = daily_indicators(&days, session, &cfg)
            .map_err(|d| format!("{id}: el calendario de sesiones no cubre el {d}"))?;
        let ready = rows
            .iter()
            .filter(|r| r.atr.is_some() && r.realized_vol.is_some())
            .count();
        let r = save(
            source,
            id,
            "indicadores",
            if source == CandleSource::Futures {
                store.insert_daily_indicators_keeping(&rows).await?
            } else {
                (store.insert_daily_indicators(&rows).await?, Vec::new())
            },
        );
        println!(
            "{id}: {} días con sesión ({} con los dos indicadores) → {} nuevos, {} ya estaban",
            rows.len(),
            ready,
            r.inserted,
            r.unchanged
        );
        let dd = daily_drawdowns(&days, session, &cfg)
            .map_err(|d| format!("{id}: el calendario de sesiones no cubre el {d}"))?;
        let ready = dd.iter().filter(|r| r.drawdown.is_some()).count();
        let r = save(
            source,
            id,
            "caída desde el máximo",
            if source == CandleSource::Futures {
                store.insert_daily_drawdowns_keeping(&dd).await?
            } else {
                (store.insert_daily_drawdowns(&dd).await?, Vec::new())
            },
        );
        println!(
            "{id}: caída desde el máximo de {} velas: {ready} días con valor → {} nuevos, {} ya estaban",
            cfg.drawdown_window, r.inserted, r.unchanged
        );
        let terciles = daily_vol_terciles(&days, session, &cfg)
            .map_err(|d| format!("{id}: el calendario de sesiones no cubre el {d}"))?;
        let ready = terciles.iter().filter(|r| r.tercile.is_some()).count();
        let r = save(
            source,
            id,
            "tercio de la volatilidad",
            if source == CandleSource::Futures {
                store.insert_daily_vol_terciles_keeping(&terciles).await?
            } else {
                (
                    store.insert_daily_vol_terciles(&terciles).await?,
                    Vec::new(),
                )
            },
        );
        println!(
            "{id}: tercio de la volatilidad ({} años de historia): {ready} días con valor → {} nuevos, {} ya estaban",
            cfg.vol_tercile_years, r.inserted, r.unchanged
        );
    }
    Ok(())
}

/// Con futuros (#409), un valor ya guardado con otro valor no se pisa ni frena el resto: queda el
/// guardado y se avisa qué días son. Pasa cuando la historia se extendió hacia atrás: los días
/// que antes no tenían historia suficiente quedaron guardados sin valor. Con IG sigue siendo un
/// error (el `?` de quien llama).
fn save(
    source: CandleSource,
    id: &cfds_core::InstrumentId,
    what: &str,
    (report, kept): (cfds_storage::InsertReport, Vec<NaiveDate>),
) -> cfds_storage::InsertReport {
    if let (Some(a), Some(b)) = (kept.first(), kept.last()) {
        eprintln!(
            "⚠️  {id} ({what}, {source:?}): {} días ya guardados con otro valor, no se pisan \
             (quedan los guardados), del {a} al {b}",
            kept.len()
        );
    }
    report
}

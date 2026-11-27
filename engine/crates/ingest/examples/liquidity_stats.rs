//! Calcula y guarda las estadísticas de liquidez (spec §5.4.1, #231) de todos los instrumentos
//! del catálogo, con las velas ya guardadas en PostgreSQL: eventos de FVG de 15m, 1h y 1d, gaps
//! de apertura diarios, desequilibrio de 15m, 1h y 1d, y los agregados. No usa IG ni gasta cupo.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example liquidity_stats [-- --fuente futuros|ibkr]
//! ```
//! Con `--fuente futuros` (#259) usa las velas de 15m y 1h armadas desde los futuros de
//! Databento y guarda todo bajo el instrumento derivado (por ejemplo `US500.FUT`). Los futuros
//! no tienen diarias: con esa fuente no hay FVG ni desequilibrio de 1d ni gaps de apertura.
//! Siempre calcula desde la primera vela guardada (el ATR de Wilder depende de toda la serie),
//! solo con velas cerradas. Es idempotente:
//! - lo ya guardado e idéntico cuenta como "ya estaba", y un evento nuevo se agrega una vez;
//! - los agregados se guardan como una corrida nueva (`computed_at` = ahora) solo si cambiaron.
//!
//! Si algo ya guardado diera otro valor (repintado, o un cambio de `config/liquidity.toml`),
//! frena con error y no guarda nada de ese lote.

#[path = "common/source.rs"]
mod source;

use cfds_core::Timeframe;
use cfds_liquidity::{
    ImbalanceConfig, LiquidityConfig, StatsConfig, fvg_events, fvg_stats, gap_stats,
    imbalance_series, opening_gaps,
};
use cfds_market_data::{CandleSource, CandleSources, Catalog};
use cfds_storage::PgStore;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};

const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const LIQUIDITY: &str = include_str!("../../../../config/liquidity.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::from_toml(CATALOG)?;
    let sources = CandleSources::from_toml(source::SOURCES)?;
    let chosen = source::source_from_args()?;
    let atr_period = LiquidityConfig::from_toml(LIQUIDITY)?.atr_period;
    let stats_cfg = StatsConfig::from_toml(LIQUIDITY)?;
    let imbalance_cfg = ImbalanceConfig::from_toml(LIQUIDITY)?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    let from: DateTime<Utc> = "2000-01-01T00:00:00Z".parse()?;
    let now = Utc::now();
    let first_day = NaiveDate::from_ymd_opt(2000, 1, 1).ok_or("fecha")?;
    let tomorrow = now.date_naive().succ_opt().ok_or("fecha")?;
    for (id, spec) in &catalog.instruments {
        let Some((feed, out)) = source::feed(&sources, chosen, id)? else {
            println!("{id}: sin velas de {}", sources.label(chosen));
            continue;
        };
        let mut events = Vec::new();
        let mut imbalances = Vec::new();
        for tf in [Timeframe::M15, Timeframe::H1] {
            let candles: Vec<_> = store
                .candles_from(feed, id, &out, tf, from, now + TimeDelta::days(1))
                .await?
                .into_iter()
                .filter(|c| c.is_closed_at(now))
                .collect();
            events.extend(fvg_events(&candles, atr_period, &stats_cfg));
            imbalances.extend(imbalance_series(&candles, atr_period, &imbalance_cfg));
            println!(
                "{out} {tf}: {} velas ({})",
                candles.len(),
                sources.label(chosen)
            );
        }
        // Los futuros e IBKR no tienen diarias: con esas fuentes no hay 1d ni gaps de apertura.
        let daily: Vec<_> = match chosen {
            CandleSource::Ig => store
                .daily_candles(id, first_day, tomorrow)
                .await?
                .into_iter()
                .filter(|d| d.is_closed_at(now))
                .collect(),
            CandleSource::Futures | CandleSource::Ibkr => Vec::new(),
        };
        events.extend(fvg_events(&daily, atr_period, &stats_cfg));
        imbalances.extend(imbalance_series(&daily, atr_period, &imbalance_cfg));
        let gaps = opening_gaps(&daily, atr_period, &stats_cfg);
        println!("{out} 1d: {} velas", daily.len());
        if events.is_empty() && imbalances.is_empty() && gaps.is_empty() {
            continue;
        }
        source::register(&store, &sources, chosen, spec).await?;

        let f = store.insert_fvg_events(&events).await?;
        let g = store.insert_opening_gaps(&gaps).await?;
        let i = store.insert_imbalances(&imbalances).await?;
        let mut aggregates = fvg_stats(&events, &stats_cfg);
        aggregates.extend(gap_stats(&gaps, &stats_cfg));
        let a = store.insert_liquidity_stats(now, &aggregates).await?;
        println!(
            "{out}: FVG {} nuevos, {} eventos nuevos, {} ya estaban | gaps {} nuevos, {} eventos \
             nuevos, {} ya estaban | desequilibrio {} nuevos, {} ya estaban | agregados {} grupos \
             nuevos, {} sin cambios",
            f.new_rows,
            f.new_events,
            f.unchanged,
            g.new_rows,
            g.new_events,
            g.unchanged,
            i.new_rows,
            i.unchanged,
            a.new_groups,
            a.unchanged_groups
        );
    }
    Ok(())
}

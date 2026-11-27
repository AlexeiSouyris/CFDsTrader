//! Calcula y guarda el mapa de liquidez (spec §5.4.1, #231) de todos los instrumentos del
//! catálogo, con las velas de 15m ya guardadas en PostgreSQL. No usa IG ni gasta cupo.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example liquidity_levels [-- --fuente futuros|ibkr]
//! ```
//! Con `--fuente futuros` (#259) usa las velas de 15m armadas desde los futuros de Databento y
//! guarda los niveles bajo el instrumento derivado (por ejemplo `US500.FUT`): mismo cálculo,
//! nunca mezclado con lo del CFD de IG. Con `--fuente ibkr` (#630), igual con las barras de IBKR
//! de los instrumentos con mapeo (US100 → MNQ, US500 → MES), bajo `US100.IBKR`.
//! Siempre calcula desde la primera vela guardada (el ATR de Wilder depende de toda la serie),
//! solo con velas cerradas. Es idempotente: lo ya guardado e idéntico cuenta como "ya estaban",
//! y un barrido, ruptura o vencimiento nuevo se agrega una sola vez. Si un nivel ya guardado
//! diera otro estado, frena con error (sería repintado) y no guarda nada de ese instrumento.
//! Desde #504 también guarda los niveles de número redondo (`round_high` y `round_low`, #497) con
//! los pasos de `config/liquidity.toml [round_numbers]`; un instrumento sin pasos no tiene.

use std::path::PathBuf;

#[path = "common/source.rs"]
mod source;

use cfds_core::Timeframe;
use cfds_indicators::wilder_atr;
use cfds_liquidity::{LiquidityConfig, RoundNumbersConfig, liquidity_levels, round_number_levels};
use cfds_market_data::{CandleSources, Catalog, ExchangeCalendar};
use cfds_patterns::{PatternsConfig, confirmed_pivots};
use cfds_storage::PgStore;
use chrono::{DateTime, TimeDelta, Utc};

const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const LIQUIDITY: &str = include_str!("../../../../config/liquidity.toml");
const PATTERNS: &str = include_str!("../../../../config/patterns.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::from_toml(CATALOG)?;
    let sources = CandleSources::from_toml(source::SOURCES)?;
    let chosen = source::source_from_args()?;
    let cfg = LiquidityConfig::from_toml(LIQUIDITY)?;
    let round = RoundNumbersConfig::from_toml(LIQUIDITY)?;
    let strength = PatternsConfig::from_toml(PATTERNS)?.pivots.strength;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    let from: DateTime<Utc> = "2000-01-01T00:00:00Z".parse()?;
    let now = Utc::now();
    for (id, spec) in &catalog.instruments {
        let Some((feed, out)) = source::feed(&sources, chosen, id)? else {
            println!("{id}: sin velas de {}", sources.label(chosen));
            continue;
        };
        let candles: Vec<_> = store
            .candles_from(
                feed,
                id,
                &out,
                Timeframe::M15,
                from,
                now + TimeDelta::days(1),
            )
            .await?
            .into_iter()
            .filter(|c| c.is_closed_at(now))
            .collect();
        if candles.is_empty() {
            println!(
                "{out}: sin velas de 15m guardadas ({})",
                sources.label(chosen)
            );
            continue;
        }
        source::register(&store, &sources, chosen, spec).await?;
        let session = calendar.session_for(id).ok_or("instrumento sin bolsa")?;
        let atr = wilder_atr(&candles, cfg.atr_period);
        let pivots = confirmed_pivots(&candles, strength);
        let mut levels = liquidity_levels(&candles, &atr, &pivots, session, &cfg.levels);
        // Con los futuros (#259) se usan los pasos del instrumento base: mismos precios.
        let round_levels =
            round_number_levels(&candles, &atr, round.steps_for(id), cfg.levels.max_age_bars);
        let rounds = round_levels.len();
        levels.extend(round_levels);
        let active = levels
            .iter()
            .filter(|l| cfds_liquidity::is_active_at(l, candles[candles.len() - 1].end()))
            .count();
        let r = store.insert_liquidity_levels(&levels).await?;
        println!(
            "{out} ({}): {} velas de 15m → {} niveles ({rounds} redondos, {active} activos): {} nuevos, {} eventos nuevos, {} ya estaban",
            sources.label(chosen),
            candles.len(),
            levels.len(),
            r.new_levels,
            r.new_events,
            r.unchanged
        );
    }
    Ok(())
}

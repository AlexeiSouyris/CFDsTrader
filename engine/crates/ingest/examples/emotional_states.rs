//! Calcula y guarda los estados emocionales de precio de la spec §9.1 (#220) de todos los
//! instrumentos del catálogo, con las velas de 15m y 1h ya guardadas en PostgreSQL
//! (`emotional_states`, `emotional_state.v1`). No usa IG ni gasta cupo.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example emotional_states [-- --fuente futuros]
//! ```
//! FOMO y clímax de venta en 1h (`cfds_regime`), rechazo en 15m y 1h (`cfds_liquidity`), juntados
//! por `cfds_strategy::price_emotional_states`. Un registro por vela cerrada, también sin estados.
//! Siempre calcula desde la primera vela guardada (la EMA y el ATR dependen de toda la serie) y
//! solo con velas cerradas. Es idempotente: lo ya guardado e idéntico cuenta como "ya estaban";
//! si un momento ya guardado diera otros estados, frena con error (sería repintado) y no guarda
//! nada de ese instrumento.

use cfds_core::Timeframe;
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{CandleSources, Catalog};
use cfds_patterns::PatternsConfig;
use cfds_regime::EmotionConfig;
use cfds_storage::PgStore;
use cfds_strategy::{Configs, StrategyConfig, price_emotional_states};
use chrono::{DateTime, TimeDelta, Utc};

#[path = "common/source.rs"]
mod source;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let sources = CandleSources::from_toml(source::SOURCES)?;
    let chosen = source::source_from_args()?;
    let configs = Configs {
        patterns: PatternsConfig::from_toml(cfg!("patterns.toml"))?,
        liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml"))?,
        trend: TrendConfig::from_toml(cfg!("indicators.toml"))?,
        strategy: StrategyConfig::from_toml(cfg!("strategy.toml"))?,
    };
    let emotions = EmotionConfig::from_toml(cfg!("regime.toml"))?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    let from: DateTime<Utc> = "2000-01-01T00:00:00Z".parse()?;
    let now = Utc::now();
    for (id, spec) in &catalog.instruments {
        let Some((feed, out)) = source::feed(&sources, chosen, id)? else {
            println!("{id}: sin velas de {}", sources.label(chosen));
            continue;
        };
        let mut series = Vec::new();
        for tf in [Timeframe::M15, Timeframe::H1] {
            let candles: Vec<_> = store
                .candles_from(feed, id, &out, tf, from, now + TimeDelta::days(1))
                .await?
                .into_iter()
                .filter(|c| c.is_closed_at(now))
                .collect();
            series.push(candles);
        }
        let (m15, h1) = (&series[0], &series[1]);
        if m15.is_empty() && h1.is_empty() {
            println!(
                "{out}: sin velas de 15m ni de 1h ({})",
                sources.label(chosen)
            );
            continue;
        }
        source::register(&store, &sources, chosen, spec).await?;
        let states = price_emotional_states(m15, h1, &configs, &emotions);
        let active = states.iter().filter(|s| !s.states.is_empty()).count();
        let r = store.insert_emotional_states(&states).await?;
        println!(
            "{out} ({}): {} velas de 15m y {} de 1h → {active} con algún estado: {} nuevos, {} ya estaban",
            sources.label(chosen),
            m15.len(),
            h1.len(),
            r.inserted,
            r.unchanged
        );
    }
    Ok(())
}

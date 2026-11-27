//! Lo común de los ejemplos del programa 5 (#649): las velas de futuros con su contrato, los
//! eventos reales de FVG que calcula el motor (`cfds_liquidity::fvg_events`, filtrados con
//! `cfds_backtest::research::program5`) y las operaciones de la estrategia en uso en las ventanas
//! de validación (lo mismo que `diagnostico --fuente futuros --tramos`, con las ventanas de
//! validación del walk-forward como bloques). Solo orquestación y lectura de la base.
#![allow(dead_code, reason = "cada ejemplo usa una parte")]

use cfds_backtest::research::program5::{EventCounts, RealEvent, real_events};
use cfds_backtest::{EvaluationConfig, Inputs, TradeRecord, evaluate_period_with};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_liquidity::{LiquidityConfig, StatsConfig, fvg_events};
use cfds_market_data::CandleSources;
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::research_round::Loaded;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../../config/", $f))
    };
}

/// Una serie de un instrumento y timeframe con sus eventos reales.
pub struct Series {
    pub bars: Vec<Candle>,
    pub contracts: Vec<u64>,
    pub events: Vec<RealEvent>,
    pub counts: EventCounts,
}

/// Velas de `futures_candles` de `id` en `[load_from, to)` y sus eventos reales con toque en
/// `[from, to)` y `horizon` velas de reacción. `to` es el inicio del fuera de muestra: no se
/// carga ninguna vela desde ahí.
pub async fn series(
    store: &PgStore,
    sources: &CandleSources,
    id: &InstrumentId,
    tf: Timeframe,
    load_from: DateTime<Utc>,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
    horizon: usize,
) -> Result<Series, Box<dyn std::error::Error>> {
    let rows = store
        .futures_candles_with_contract(&sources.futures_source, id, tf, load_from, to)
        .await?;
    if rows.last().is_some_and(|c| c.candle.start >= to) {
        return Err("se cargó una vela del fuera de muestra".into());
    }
    let (bars, contracts): (Vec<Candle>, Vec<u64>) =
        rows.into_iter().map(|c| (c.candle, c.contract_id)).unzip();
    let liquidity = cfg!("liquidity.toml");
    let atr_period = LiquidityConfig::from_toml(liquidity)?.atr_period;
    let stats = StatsConfig::from_toml(liquidity)?;
    let all = fvg_events(&bars, atr_period, &stats);
    let (events, counts) = real_events(&bars, &contracts, &all, (from, to), horizon);
    Ok(Series {
        bars,
        contracts,
        events,
        counts,
    })
}

/// Operaciones de la estrategia en uso en cada ventana de validación (cada una desde el capital
/// inicial y con el calentamiento de `config/evaluation.toml`), con los índices llevados a
/// `l.p.m15`.
pub fn strategy_trades(l: &Loaded) -> Result<Vec<TradeRecord>, Box<dyn std::error::Error>> {
    let p = &l.p;
    let eval =
        EvaluationConfig::from_toml(cfg!("evaluation.toml")).map_err(|e| format!("{e:?}"))?;
    let inputs = Inputs {
        m15: &p.m15,
        h1: &p.h1,
        entries: &p.entries,
        force_close: &p.force_close,
        data_errors: &p.data_errors,
        news: &p.news,
        published_from: p.published_from,
    };
    let mut trades = Vec::new();
    for w in &l.split.windows {
        let period =
            cfds_backtest::research_period(w.validation, &l.split).map_err(|e| format!("{e:?}"))?;
        let keep = |_: &cfds_backtest::Slice, e: Vec<bool>| Ok(e);
        let (_, rep) =
            evaluate_period_with(&inputs, period, &p.setup, eval.warmup, Decimal::ZERO, keep)
                .map_err(|e| format!("{e:?}"))?;
        let Some(rep) = rep else { continue };
        let lo = p
            .m15
            .partition_point(|c| c.start < period.start - eval.warmup);
        trades.extend(rep.trades.into_iter().map(|mut t| {
            t.signal_index += lo;
            t.entry_index += lo;
            for e in &mut t.exits {
                e.index += lo;
            }
            t
        }));
    }
    Ok(trades)
}

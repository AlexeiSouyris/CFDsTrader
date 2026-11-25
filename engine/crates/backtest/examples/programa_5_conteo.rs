//! Programa 5 (#649), punto abierto 16 aprobado por el dueño: **conteo** antes de congelar de los
//! eventos reales de FVG (§3 y §7) y de las operaciones de la estrategia en uso que son "F" o "R"
//! (§5.2, punto 13), por año, en la validación concatenada del walk-forward. Sin P/L, sin
//! retornos, sin reacciones: solo cantidades. El fuera de muestra no se carga. Estudio: no guarda
//! nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example programa_5_conteo -- \
//!     US100 --escenario-financiacion EUR=<archivo.csv>
//! ```

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/program5.rs"]
mod program5;
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use std::collections::BTreeMap;

use cfds_backtest::research::program5::{is_fvg_trade, touch_closes};
use cfds_core::{InstrumentId, Timeframe};
use cfds_market_data::{CandleSources, SplitConfig};
use cfds_storage::PgStore;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// Horizonte principal (8 velas, §3 F1 y F3).
const HORIZON: usize = 8;
/// Mínimo de operaciones F por instrumento (punto abierto 15).
const MIN_F: usize = 100;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let from: DateTime<Utc> = "2011-01-01T00:00:00Z".parse()?;
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let id: InstrumentId = args
        .first()
        .ok_or("uso: US100 --escenario-financiacion …")?
        .parse()?;
    let l = research_round::load_opts(
        &id,
        &args,
        from,
        Decimal::from(10_000),
        &sources,
        &store,
        &split_cfg,
        false,
    )
    .await?;
    let (lo, hi) = (
        l.split.windows[0].validation.start,
        l.split.out_of_sample.start,
    );
    println!("══ conteo (toque en [{lo}, {hi})), sin resultados");
    let mut m15 = None;
    for tf in [Timeframe::M15, Timeframe::H1] {
        let s = program5::series(&store, &sources, &id, tf, from, (lo, hi), HORIZON).await?;
        let c = s.counts;
        println!(
            "{id} {}: {} velas; primeros toques en el período {}; F1 {}; F1 + F2 (eventos reales) {}",
            tf.as_str(),
            s.bars.len(),
            c.in_period,
            c.same_contract,
            c.real
        );
        let mut by_year: BTreeMap<i32, usize> = BTreeMap::new();
        for e in &s.events {
            *by_year.entry(s.bars[e.touch].start.year()).or_default() += 1;
        }
        println!("  por año: {by_year:?}");
        if tf == Timeframe::M15 {
            m15 = Some(s);
        }
    }
    let s = m15.ok_or("sin 15m")?;
    let touches = touch_closes(&s.bars, &s.events);
    let trades = program5::strategy_trades(&l)?;
    let mut by_year: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
    for t in &trades {
        let e = by_year.entry(t.signal_at.year()).or_default();
        if is_fvg_trade(&touches, t.signal_at, t.side) {
            e.0 += 1;
        } else {
            e.1 += 1;
        }
    }
    let f: usize = by_year.values().map(|x| x.0).sum();
    println!(
        "{id}: {} operaciones de la estrategia en las ventanas de validación; F {f}, R {}",
        trades.len(),
        trades.len() - f
    );
    for (y, (a, b)) in &by_year {
        println!("  {y}: F {a}, R {b}");
    }
    if f < MIN_F {
        println!("  ⚠️  menos de {MIN_F} operaciones F: H5.2 NO se congela (punto abierto 16)");
    }
    Ok(())
}

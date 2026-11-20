//! Backtest sobre velas reales guardadas en PostgreSQL (#76), con el `Setup` armado solo desde
//! la configuración. Corre la estrategia y comprar y mantener sobre el mismo período.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-backtest --example backtest_run -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> <capital inicial> <tamaño comprar y mantener>
//! ```
//! Si al instrumento le falta algún dato del broker (por ejemplo el tipo de referencia de la
//! financiación), no corre y dice qué falta.

use std::path::PathBuf;

use cfds_backtest::{BacktestConfig, buy_and_hold, run, setup_from_catalog};
use cfds_core::Timeframe;
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Catalog, ExchangeCalendar, TradingWindowConfig, trading_window};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_storage::PgStore;
use cfds_strategy::{Configs, StrategyConfig};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [instrument, from, to, equity, size] = args.as_slice() else {
        return Err("uso: <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <tamaño B&H>".into());
    };
    let id = instrument.parse()?;
    let (from, to) = (from.parse()?, to.parse()?);
    let (equity, size): (Decimal, Decimal) = (equity.parse()?, size.parse()?);

    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let strategy = Configs {
        patterns: PatternsConfig::from_toml(cfg!("patterns.toml"))?,
        liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml"))?,
        trend: TrendConfig::from_toml(cfg!("indicators.toml"))?,
        strategy: StrategyConfig::from_toml(cfg!("strategy.toml"))?,
    };
    let setup = setup_from_catalog(
        &catalog,
        &id,
        strategy,
        RiskConfig::from_toml(cfg!("risk.toml"))?,
        BacktestConfig::from_toml(cfg!("backtest.toml"))?,
        equity,
    )?;

    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let m15 = store.candles(&id, Timeframe::M15, from, to).await?;
    let h1 = store.candles(&id, Timeframe::H1, from, to).await?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let session = calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml"))?;
    let (entries, force_close) =
        trading_window::flags(session, &m15, &window).ok_or("fechas fuera del calendario")?;
    println!(
        "{id}: {} velas de 15m y {} de 1h, del {from} al {to}",
        m15.len(),
        h1.len()
    );

    let report = run(&m15, &h1, &entries, &force_close, &setup).map_err(|e| format!("{e:?}"))?;
    println!(
        "estrategia: {} operaciones, {} señales rechazadas por riesgo, capital final {}",
        report.trades.len(),
        report.rejected.len(),
        report.final_equity
    );
    for t in &report.trades {
        println!(
            "  {:?} entrada {} @ {} tamaño {} → neto {} (costos {})",
            t.side,
            t.entry_time,
            t.entry_price,
            t.size,
            t.net_pnl,
            t.total_costs()
        );
    }

    let bh = buy_and_hold(&m15, size, &setup).ok_or("sin velas")?;
    let (first, last) = (&m15[0], &m15[m15.len() - 1]);
    println!(
        "comprar y mantener ({size}): apertura {} = {} → cierre {} = {}",
        first.start, first.open, last.start, last.close
    );
    println!("  {bh:?}");
    Ok(())
}

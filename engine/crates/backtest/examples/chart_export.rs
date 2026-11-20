//! Exporta velas reales de PostgreSQL con sus indicadores, puntajes, pivotes y las operaciones
//! del backtest (`contracts/chart_export.v1`, #81). Lo leen la comparación con la referencia
//! (`tools/verify/`) y el visor; ninguno recalcula nada.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-backtest --example chart_export -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <archivo.json>
//! ```
//! El archivo tiene precios de IG: no se commitea (el repo es público).

use cfds_backtest::{BacktestConfig, chart_export, run, setup_from_catalog};
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
    let [instrument, from, to, equity, out] = args.as_slice() else {
        return Err("uso: <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <archivo.json>".into());
    };
    let id = instrument.parse()?;
    let (from, to) = (from.parse()?, to.parse()?);
    let equity: Decimal = equity.parse()?;
    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let setup = setup_from_catalog(
        &catalog,
        &id,
        Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml"))?,
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml"))?,
            trend: TrendConfig::from_toml(cfg!("indicators.toml"))?,
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml"))?,
        },
        RiskConfig::from_toml(cfg!("risk.toml"))?,
        BacktestConfig::from_toml(cfg!("backtest.toml"))?,
        equity,
    )?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let m15 = store.candles(&id, Timeframe::M15, from, to).await?;
    let h1 = store.candles(&id, Timeframe::H1, from, to).await?;
    let config_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let session = calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml"))?;
    let (entries, force_close) =
        trading_window::flags(session, &m15, &window).ok_or("fechas fuera del calendario")?;
    let report = run(&m15, &h1, &entries, &force_close, &setup).map_err(|e| format!("{e:?}"))?;
    let export = chart_export(&m15, &h1, &setup, &report).map_err(|e| format!("{e:?}"))?;
    std::fs::write(out, serde_json::to_string(&export)?)?;
    println!(
        "{out}: {} velas, {} pivotes, {} operaciones",
        export.candles.len(),
        export.pivots.len(),
        export.trades.len()
    );
    Ok(())
}

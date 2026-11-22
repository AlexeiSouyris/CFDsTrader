//! Backtest (Fase 3): salidas de una operación (spec §8 y §12), costos (§11) y el recorrido
//! completo de un instrumento con su registro de operaciones y comprar y mantener.

pub mod config;
pub mod costs;
pub mod exits;
pub mod export;
pub mod metrics;
pub mod portfolio;
pub mod rates;
pub mod runner;
pub mod setup;

pub use config::{BacktestConfig, CostConfig, ExitConfig, MetricsConfig};
pub use costs::{
    Commission, Execution, InstrumentCosts, commission, fill_price, funding, funding_cutoffs,
    funding_nights, funding_over,
};
pub use exits::{ExitReason, Fill, Outcome, Trade, simulate};
pub use export::{ChartExport, chart_export};
pub use metrics::{Metrics, evaluate};
pub use portfolio::{
    Market, PortfolioError, PortfolioRejected, PortfolioReport, PortfolioTrade, run_portfolio,
};
pub use rates::{DailyRates, FundingRates};
pub use runner::{
    BuyAndHold, CloseReason, ExitRecord, RejectedSignal, Report, RunError, Setup, TradeRecord,
    buy_and_hold, choose, execute, pick, run, run_checked, run_with_news,
};
pub use setup::{SetupError, setup_from_catalog};

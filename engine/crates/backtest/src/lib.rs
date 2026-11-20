//! Backtest (Fase 3): salidas de una operación (spec §8 y §12), costos (§11) y el recorrido
//! completo de un instrumento con su registro de operaciones y comprar y mantener.

pub mod config;
pub mod costs;
pub mod exits;
pub mod portfolio;
pub mod runner;

pub use config::{BacktestConfig, CostConfig, ExitConfig};
pub use costs::{
    Commission, Execution, InstrumentCosts, commission, fill_price, funding, funding_nights,
};
pub use exits::{ExitReason, Fill, Outcome, Trade, simulate};
pub use portfolio::{
    Market, PortfolioError, PortfolioRejected, PortfolioReport, PortfolioTrade, run_portfolio,
};
pub use runner::{
    BuyAndHold, CloseReason, ExitRecord, RejectedSignal, Report, RunError, Setup, TradeRecord,
    buy_and_hold, choose, execute, pick, run,
};

//! Backtest (Fase 3). Hoy: las salidas de una operación (spec §8 y §12) y los costos (§11).
//! El recorrido de toda la serie llega en un Issue aparte.

pub mod config;
pub mod costs;
pub mod exits;

pub use config::{BacktestConfig, CostConfig, ExitConfig};
pub use costs::{
    Commission, Execution, InstrumentCosts, commission, fill_price, funding, funding_nights,
};
pub use exits::{ExitReason, Fill, Outcome, Trade, simulate};

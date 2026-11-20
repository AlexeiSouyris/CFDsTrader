//! Backtest (Fase 3). Hoy: las salidas de una operación (spec §8 y §12). Los costos y el
//! recorrido de toda la serie llegan en Issues aparte.

pub mod config;
pub mod exits;

pub use config::ExitConfig;
pub use exits::{ExitReason, Fill, Outcome, Trade, simulate};

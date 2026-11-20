//! Patrones de precio (fase 3b). Definiciones objetivas y parametrizadas (spec §4 y §5,
//! `config/patterns.toml`): nada de "se ve como".
//!
//! Convención: todo se evalúa sobre velas **cerradas**, y un patrón en la vela `t` solo usa
//! las velas `0..=t`.

pub mod candles;
pub mod config;
pub mod pivots;

pub use candles::{CandlePattern, candle_patterns_at};
pub use config::{CandleConfig, PatternsConfig, PivotConfig};
pub use pivots::{Pivot, PivotKind, confirmed_pivots};

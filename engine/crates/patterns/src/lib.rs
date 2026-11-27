//! Patrones de precio (fase 3b). Definiciones objetivas y parametrizadas (spec §4 y §5,
//! `config/patterns.toml`): nada de "se ve como".
//!
//! Convención: todo se evalúa sobre velas **cerradas**, y un patrón en la vela `t` solo usa
//! las velas `0..=t`.

pub mod bottom;
pub mod candles;
pub mod config;
pub mod double;
pub mod fibonacci;
pub mod levels;
pub mod pivots;
pub mod wyckoff;

pub use bottom::{Bottom, BottomConfig, bottoms};
pub use candles::{CandlePattern, candle_patterns_at};
pub use config::{CandleConfig, DoubleConfig, LevelConfig, PatternsConfig, PivotConfig};
pub use double::{DoubleKind, DoublePattern, double_at};
pub use fibonacci::{
    FibConfig, FibLeg, LegDirection, PHI, PocketEvent, fib_legs, golden_pocket_events,
};
pub use levels::{Level, LevelKind, levels_at, rejections_at};
pub use pivots::{Pivot, PivotKind, confirmed_pivots};
pub use wyckoff::{WyckoffConfig, WyckoffEvent, WyckoffKind, wyckoff_events};

//! Indicadores técnicos. Cada indicador vive en su propio módulo, con la fórmula
//! exacta documentada (ROADMAP fase 3b). Python y la UI no los recalculan.
//!
//! Convención: cada función recibe una serie de velas **cerradas** y devuelve un
//! valor por vela, alineado por índice. `None` = todavía en calentamiento.

pub mod atr;
pub mod drawdown;
pub mod moving_average;
pub mod tercile;
pub mod trend;
pub mod volatility;
pub mod wilson;

pub use atr::{AtrSeed, Hlc, true_range, wilder_atr, wilder_atr_after};
pub use drawdown::drawdown_from_high;
pub use moving_average::{ema, ema_values, sma, sma_values};
pub use tercile::vol_terciles;
pub use trend::{Trend, TrendConfig, trend};
pub use volatility::{DailyConfig, realized_volatility};
pub use wilson::wilson;

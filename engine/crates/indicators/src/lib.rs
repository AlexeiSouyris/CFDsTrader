//! Indicadores técnicos. Cada indicador vive en su propio módulo, con la fórmula
//! exacta documentada (ROADMAP fase 3b). Python y la UI no los recalculan.
//!
//! Convención: cada función recibe una serie de velas **cerradas** y devuelve un
//! valor por vela, alineado por índice. `None` = todavía en calentamiento.

pub mod atr;
pub mod moving_average;
pub mod trend;
pub mod volatility;

pub use atr::{Hlc, true_range, wilder_atr};
pub use moving_average::{ema, sma};
pub use trend::{Trend, TrendConfig, trend};
pub use volatility::{DailyConfig, realized_volatility};

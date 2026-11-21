//! Spread al cierre de cada vela (#95). Contrato: `contracts/candle_spread.v1.schema.json`.
//!
//! Complementa a la vela (que guarda el precio medio): lo usa el filtro de spread de la spec
//! §7.3 (#90). Los costos del backtest siguen usando el spread típico del catálogo.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::{Timeframe, utc_z};
use crate::instrument::InstrumentId;

/// Fuente: `GET /prices` de la cuenta demo de IG.
pub const IG_DEMO: &str = "ig_demo";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandleSpread {
    pub instrument_id: InstrumentId,
    pub timeframe: Timeframe,
    /// Inicio de la vela, en UTC.
    #[serde(with = "utc_z")]
    pub start: DateTime<Utc>,
    /// `ask − bid` al cierre, en puntos de precio. Nunca negativo.
    #[serde(with = "crate::decimal_str")]
    pub spread: Decimal,
    #[serde(with = "crate::sentiment::label")]
    pub source: String,
}

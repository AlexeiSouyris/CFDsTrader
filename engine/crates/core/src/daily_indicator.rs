//! Indicadores de una vela diaria (#148). Contrato: `contracts/daily_indicator.v1.schema.json`.
//!
//! Los calcula el motor (`cfds-indicators`) y los lee Python para los análogos (spec §13).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyIndicator {
    pub instrument_id: InstrumentId,
    pub day: NaiveDate,
    /// Fin de la vela diaria: desde ahí existe el valor.
    #[serde(with = "utc_z")]
    pub published_at: DateTime<Utc>,
    pub atr_period: u32,
    /// `None` durante el calentamiento. El campo es obligatorio en el contrato (puede ser null).
    #[serde(with = "crate::decimal_str::option")]
    pub atr: Option<Decimal>,
    pub realized_vol_window: u32,
    #[serde(with = "crate::decimal_str::option")]
    pub realized_vol: Option<Decimal>,
}

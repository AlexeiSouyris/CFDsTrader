//! Caída desde el máximo de una vela diaria (#213, spec §9.1). Contrato:
//! `contracts/daily_drawdown.v1.schema.json`.
//!
//! La calcula el motor (`cfds-indicators`) y la lee Python para el índice de sentimiento.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyDrawdown {
    pub instrument_id: InstrumentId,
    pub day: NaiveDate,
    /// Fin de la vela diaria: desde ahí existe el valor.
    #[serde(with = "utc_z")]
    pub published_at: DateTime<Utc>,
    pub drawdown_window: u32,
    /// `None` durante el calentamiento. El campo es obligatorio en el contrato (puede ser null).
    #[serde(with = "crate::decimal_str::signed_option")]
    pub drawdown: Option<Decimal>,
}

//! Tercio de la volatilidad realizada de una vela diaria frente a su propia historia (spec §13,
//! spec ML §3). Contrato: `contracts/daily_vol_tercile.v1.schema.json`.
//!
//! Lo calcula el motor (`cfds-indicators`), lo guarda `cfds-ingest` y lo leen los análogos en
//! Python y las variables de ML (`cfds-journal`). Nadie más lo recalcula (CLAUDE.md §1).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolTercile {
    Low,
    Mid,
    High,
}

impl VolTercile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyVolTercile {
    pub instrument_id: InstrumentId,
    pub day: NaiveDate,
    /// Fin de la vela diaria: desde ahí existe el valor.
    #[serde(with = "utc_z")]
    pub published_at: DateTime<Utc>,
    pub realized_vol_window: u32,
    pub history_years: u32,
    /// `None` sin volatilidad ese día o sin `history_years` años completos de historia. El
    /// campo es obligatorio en el contrato (puede ser null): `deserialize_with` hace que falte
    /// sea un error, en vez de leerlo como `None`.
    #[serde(deserialize_with = "Option::deserialize")]
    pub tercile: Option<VolTercile>,
}

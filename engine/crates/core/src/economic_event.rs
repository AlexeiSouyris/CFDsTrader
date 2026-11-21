//! Eventos del calendario económico (#91). Contrato: `contracts/economic_event.v1.schema.json`.
//!
//! Los usa el filtro de noticias de la spec §7.2. La fuente y su vía legal de acceso son de la
//! fase 4b (spec §9, con su ADR): acá solo está la forma del dato.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Impact {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "Raw")]
pub struct EconomicEvent {
    #[serde(with = "crate::sentiment::label")]
    pub source: String,
    pub event: String,
    /// ISO 3166-1 alfa-2 (`EU` = zona euro). `None` si el evento es de un instrumento.
    pub country: Option<String>,
    pub instrument_id: Option<InstrumentId>,
    pub impact: Impact,
    #[serde(with = "utc_z")]
    pub scheduled_at: DateTime<Utc>,
    /// Desde cuándo estuvo en el calendario: antes no existe para el sistema.
    #[serde(with = "utc_z")]
    pub published_at: DateTime<Utc>,
}

/// La forma sin validar: el contrato exige los siete campos (aunque sean `null`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(with = "crate::sentiment::label")]
    source: String,
    event: String,
    #[serde(deserialize_with = "required")]
    country: Option<String>,
    #[serde(deserialize_with = "required")]
    instrument_id: Option<InstrumentId>,
    impact: Impact,
    #[serde(with = "utc_z")]
    scheduled_at: DateTime<Utc>,
    #[serde(with = "utc_z")]
    published_at: DateTime<Utc>,
}

fn required<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

impl TryFrom<Raw> for EconomicEvent {
    type Error = String;

    fn try_from(r: Raw) -> Result<Self, String> {
        let chars = r.event.chars().count();
        if chars == 0 || chars > 200 {
            return Err("event tiene que tener entre 1 y 200 caracteres".into());
        }
        if let Some(c) = &r.country
            && !(c.len() == 2 && c.chars().all(|x| x.is_ascii_uppercase()))
        {
            return Err(format!("country inválido: {c:?} (ISO 3166-1 alfa-2)"));
        }
        if r.country.is_none() && r.instrument_id.is_none() {
            return Err("hace falta country o instrument_id".into());
        }
        Ok(Self {
            source: r.source,
            event: r.event,
            country: r.country,
            instrument_id: r.instrument_id,
            impact: r.impact,
            scheduled_at: r.scheduled_at,
            published_at: r.published_at,
        })
    }
}

//! Observaciones de sentimiento. Contrato: `contracts/sentiment_observation.v1.schema.json`.
//!
//! Dato crudo de una fuente (por ejemplo el % de clientes de IG con posición larga, #27), tal
//! como se publicó. Rust solo lo **guarda**; interpretarlo le corresponde a Python (fase 4b).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

/// Fuente del sentimiento de clientes de IG (#27).
pub const IG_CLIENT: &str = "ig_client";
/// Serie: % de clientes de IG con posición larga (el corto es 100 − este valor).
pub const LONG_PCT: &str = "long_pct";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SentimentObservation {
    #[serde(with = "label")]
    pub source: String,
    #[serde(with = "label")]
    pub series: String,
    /// `None` = dato de todo el mercado, no de un instrumento. El campo es obligatorio
    /// (aunque sea `null`), igual que en el contrato.
    #[serde(deserialize_with = "required_option")]
    pub instrument_id: Option<InstrumentId>,
    /// Momento al que corresponde el dato, en UTC.
    #[serde(with = "utc_z")]
    pub observed_for: DateTime<Utc>,
    /// Desde cuándo estuvo disponible. Nada lo usa antes (CLAUDE.md §5b).
    #[serde(with = "utc_z")]
    pub published_at: DateTime<Utc>,
    #[serde(with = "crate::decimal_str::signed")]
    pub value: Decimal,
}

/// Con `deserialize_with`, serde exige el campo: un `instrument_id` ausente no pasa por `None`.
fn required_option<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<InstrumentId>, D::Error> {
    Option::<InstrumentId>::deserialize(d)
}

/// Nombres de fuente y serie: `^[a-z][a-z0-9_]{0,31}$`, igual que el contrato.
pub(crate) mod label {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn is_valid(s: &str) -> bool {
        let mut chars = s.chars();
        s.len() <= 32
            && chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    pub fn serialize<S: Serializer>(v: &str, s: S) -> Result<S::Ok, S::Error> {
        if !is_valid(v) {
            return Err(serde::ser::Error::custom(format!("nombre inválido: {v:?}")));
        }
        s.serialize_str(v)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        let v = String::deserialize(d)?;
        if is_valid(&v) {
            Ok(v)
        } else {
            Err(D::Error::custom(format!("nombre inválido: {v:?}")))
        }
    }
}

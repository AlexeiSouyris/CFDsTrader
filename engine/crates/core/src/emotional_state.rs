//! `contracts/emotional_state.v1` (spec §9.1, #220): los estados emocionales activos de un
//! instrumento en un momento, con los valores de su regla. Informativo: no es un pronóstico ni
//! suma al puntaje.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::InstrumentId;
use crate::candle::utc_z;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmotionSource {
    /// De las velas (Rust).
    Price,
    /// Del índice de sentimiento (Python).
    Sentiment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EmotionBasis {
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "sentiment_index")]
    SentimentIndex,
}

impl EmotionBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            EmotionBasis::M15 => "15m",
            EmotionBasis::H1 => "1h",
            EmotionBasis::SentimentIndex => "sentiment_index",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmotionKind {
    Fomo,
    SellingClimax,
    Rejection,
    ExtremeFear,
    ExtremeEuphoria,
    Complacency,
}

/// Un estado activo con los valores de su regla (texto, decimales con punto).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveEmotion {
    pub state: EmotionKind,
    pub values: BTreeMap<String, String>,
}

/// `emotional_state.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmotionalState {
    pub instrument_id: InstrumentId,
    pub source: EmotionSource,
    pub basis: EmotionBasis,
    /// Desde cuándo se conoce: el cierre de la vela evaluada o el `published_at` del índice.
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
    /// Activos, en un orden fijo; vacío si no hay ninguno.
    pub states: Vec<ActiveEmotion>,
}

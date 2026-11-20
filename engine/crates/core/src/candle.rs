//! Velas OHLC. Contrato: `contracts/candle.v1.schema.json`.
//!
//! Regla de oro (CLAUDE.md §5b): una vela solo se usa cuando está **cerrada**,
//! es decir, a partir de `start + timeframe`. [`Candle::is_closed_at`] es la única
//! forma correcta de preguntarlo.

use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::instrument::InstrumentId;

/// Duración de una vela. Solo los timeframes de la especificación (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Timeframe {
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "1h")]
    H1,
}

impl Timeframe {
    pub fn duration(self) -> TimeDelta {
        match self {
            Timeframe::M15 => TimeDelta::minutes(15),
            Timeframe::H1 => TimeDelta::hours(1),
        }
    }
}

/// Una vela OHLC con precios decimales exactos y hora en UTC.
///
/// Este tipo no garantiza que los precios sean coherentes (por ejemplo `high >= low`):
/// eso lo comprueba la validación de datos (Issue #5) antes de que la vela entre al sistema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candle {
    pub instrument_id: InstrumentId,
    pub timeframe: Timeframe,
    /// Inicio del período, en UTC.
    #[serde(with = "utc_z")]
    pub start: DateTime<Utc>,
    #[serde(with = "crate::decimal_str")]
    pub open: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub high: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub low: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub close: Decimal,
    /// Volumen del período, si la fuente lo informa.
    #[serde(with = "crate::decimal_str::option")]
    pub volume: Option<Decimal>,
}

impl Candle {
    /// Momento en que la vela cierra (exclusivo): `start + timeframe`.
    pub fn end(&self) -> DateTime<Utc> {
        self.start + self.timeframe.duration()
    }

    /// `true` si en el instante `now` la vela ya cerró y se puede usar para decidir.
    pub fn is_closed_at(&self, now: DateTime<Utc>) -> bool {
        now >= self.end()
    }
}

/// Timestamps del contrato: ISO 8601 en UTC **terminados en `Z`**.
/// Se rechaza cualquier otra zona aunque represente el mismo instante,
/// para que Rust acepte exactamente lo mismo que el JSON Schema.
mod utc_z {
    use chrono::{DateTime, SecondsFormat, Utc};
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(t: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&t.to_rfc3339_opts(SecondsFormat::AutoSi, true))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let s = String::deserialize(d)?;
        if !s.ends_with('Z') {
            return Err(D::Error::custom(format!("timestamp sin 'Z' (UTC): {s:?}")));
        }
        s.parse::<DateTime<Utc>>().map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    fn candle(start: &str, timeframe: Timeframe) -> Candle {
        Candle {
            instrument_id: "US100".parse().unwrap(),
            timeframe,
            start: start.parse().unwrap(),
            open: dec!(26756.6),
            high: dec!(26760.8),
            low: dec!(26732.3),
            close: dec!(26747.8),
            volume: None,
        }
    }

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn a_15m_candle_closes_exactly_15_minutes_after_its_start() {
        let c = candle("2026-11-19T15:30:00Z", Timeframe::M15);
        assert_eq!(c.end(), at("2026-11-19T15:45:00Z"));
    }

    #[test]
    fn candle_is_not_usable_until_it_closes() {
        // Usar la vela de las 15:30 antes de las 15:45 sería mirar el futuro.
        let c = candle("2026-11-19T15:30:00Z", Timeframe::M15);
        assert!(!c.is_closed_at(at("2026-11-19T15:30:00Z")));
        assert!(!c.is_closed_at(at("2026-11-19T15:44:59.999Z")));
        assert!(c.is_closed_at(at("2026-11-19T15:45:00Z")));
    }

    #[test]
    fn a_1h_candle_is_only_usable_after_the_full_hour() {
        let c = candle("2026-11-19T15:00:00Z", Timeframe::H1);
        assert!(!c.is_closed_at(at("2026-11-19T15:45:00Z")));
        assert!(c.is_closed_at(at("2026-11-19T16:00:00Z")));
    }
}

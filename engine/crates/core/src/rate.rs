//! Tipos de referencia diarios de la financiación nocturna (#79).
//! Contrato: `contracts/reference_rate.v1.schema.json`.
//!
//! Los escribe Python (`cfds_intel.rates`, €STR del BCE); el motor los **lee** para el costo de
//! financiación de cada noche. Un valor solo existe desde `published_at` (CLAUDE.md §5b).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceRate {
    /// Mismo nombre que `[funding_benchmarks]` del catálogo (hoy `ESTR`) o que la base de los
    /// futuros (`EFFR`, `SOFR`, #467).
    #[serde(with = "series_name")]
    pub series: String,
    /// Día hábil al que corresponde el valor.
    pub ref_date: NaiveDate,
    /// Desde cuándo estuvo disponible. Nada lo usa antes.
    #[serde(with = "utc_z")]
    pub published_at: DateTime<Utc>,
    /// Tasa anual en %; puede ser negativa.
    #[serde(with = "crate::decimal_str::signed")]
    pub rate_pct: Decimal,
}

/// Nombres de serie: `^[A-Z][A-Z0-9_]{0,31}$`, igual que el contrato.
pub mod series_name {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn is_valid(s: &str) -> bool {
        let mut chars = s.chars();
        s.len() <= 32
            && chars.next().is_some_and(|c| c.is_ascii_uppercase())
            && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    }

    pub fn serialize<S: Serializer>(v: &str, s: S) -> Result<S::Ok, S::Error> {
        if !is_valid(v) {
            return Err(serde::ser::Error::custom(format!("serie inválida: {v:?}")));
        }
        s.serialize_str(v)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        let v = String::deserialize(d)?;
        if is_valid(&v) {
            Ok(v)
        } else {
            Err(D::Error::custom(format!("serie inválida: {v:?}")))
        }
    }
}

/// Un tipo de cambio diario de referencia (ADR 0013 §4). Contrato:
/// `contracts/fx_rate.v1.schema.json`. Lo escribe Python (`cfds_intel.rates.fx`, EUR/USD del
/// BCE); el motor lo lee para convertir montos a la moneda de la cuenta. Solo existe desde
/// `published_at` (CLAUDE.md §5b).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FxRate {
    /// `BASE` + `COTIZADA` (ISO 4217), por ejemplo `EURUSD`.
    #[serde(with = "pair_name")]
    pub pair: String,
    /// Día al que corresponde el valor.
    pub ref_date: NaiveDate,
    /// Desde cuándo estuvo disponible. Nada lo usa antes.
    #[serde(with = "utc_z")]
    pub published_at: DateTime<Utc>,
    /// Unidades de la cotizada por 1 de la base.
    #[serde(with = "crate::decimal_str")]
    pub rate: Decimal,
}

/// Pares: `^[A-Z]{6}$`, igual que el contrato.
pub mod pair_name {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn is_valid(s: &str) -> bool {
        s.len() == 6 && s.bytes().all(|b| b.is_ascii_uppercase())
    }

    pub fn serialize<S: Serializer>(v: &str, s: S) -> Result<S::Ok, S::Error> {
        if !is_valid(v) {
            return Err(serde::ser::Error::custom(format!("par inválido: {v:?}")));
        }
        s.serialize_str(v)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
        let v = String::deserialize(d)?;
        if is_valid(&v) {
            Ok(v)
        } else {
            Err(D::Error::custom(format!("par inválido: {v:?}")))
        }
    }
}

//! Pronóstico y dato real del calendario (ADR 0018). Contratos:
//! `contracts/calendar_estimate.v1.schema.json` y `contracts/release_actual.v1.schema.json`.
//!
//! Los escribe Python (tablas `calendar_estimates` y `release_actuals`); la sorpresa la calcula
//! `cfds_market_data::surprise`. Acá solo está la forma del dato.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;

/// Unidad de un valor del calendario o de una serie oficial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatUnit {
    Percent,
    Thousands,
    Millions,
    Billions,
    Trillions,
    Level,
}

/// `first`: leído antes de la publicación siguiente de la serie. `revised`: después.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vintage {
    First,
    Revised,
}

/// Una instantánea del `forecast` y el `previous` de un evento (`calendar_estimate.v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "EstimateRaw")]
pub struct CalendarEstimate {
    #[serde(with = "crate::sentiment::label")]
    pub source: String,
    pub event: String,
    pub country: String,
    #[serde(with = "utc_z")]
    pub scheduled_at: DateTime<Utc>,
    pub forecast_raw: Option<String>,
    pub previous_raw: Option<String>,
    #[serde(with = "crate::decimal_str::signed_option")]
    pub forecast_value: Option<Decimal>,
    #[serde(with = "crate::decimal_str::signed_option")]
    pub previous_value: Option<Decimal>,
    pub unit: Option<StatUnit>,
    /// Primera lectura con este par de valores: antes no existía para el sistema.
    #[serde(with = "utc_z")]
    pub seen_at: DateTime<Utc>,
}

/// La forma sin validar: el contrato exige todos los campos (aunque sean `null`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EstimateRaw {
    #[serde(with = "crate::sentiment::label")]
    source: String,
    event: String,
    country: String,
    #[serde(with = "utc_z")]
    scheduled_at: DateTime<Utc>,
    #[serde(deserialize_with = "required")]
    forecast_raw: Option<String>,
    #[serde(deserialize_with = "required")]
    previous_raw: Option<String>,
    #[serde(with = "crate::decimal_str::signed_option")]
    forecast_value: Option<Decimal>,
    #[serde(with = "crate::decimal_str::signed_option")]
    previous_value: Option<Decimal>,
    #[serde(deserialize_with = "required")]
    unit: Option<StatUnit>,
    #[serde(with = "utc_z")]
    seen_at: DateTime<Utc>,
}

fn required<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

fn is_country(c: &str) -> bool {
    c.len() == 2 && c.chars().all(|x| x.is_ascii_uppercase())
}

impl TryFrom<EstimateRaw> for CalendarEstimate {
    type Error = String;

    fn try_from(r: EstimateRaw) -> Result<Self, String> {
        let chars = r.event.chars().count();
        if chars == 0 || chars > 200 {
            return Err("event tiene que tener entre 1 y 200 caracteres".into());
        }
        if !is_country(&r.country) {
            return Err(format!(
                "country inválido: {:?} (ISO 3166-1 alfa-2)",
                r.country
            ));
        }
        for raw in [&r.forecast_raw, &r.previous_raw].into_iter().flatten() {
            let n = raw.chars().count();
            if n == 0 || n > 50 {
                return Err("forecast_raw/previous_raw: entre 1 y 50 caracteres o null".into());
            }
        }
        if (r.forecast_value.is_some() && r.forecast_raw.is_none())
            || (r.previous_value.is_some() && r.previous_raw.is_none())
        {
            return Err("un valor numérico necesita su texto".into());
        }
        let any_value = r.forecast_value.is_some() || r.previous_value.is_some();
        if any_value != r.unit.is_some() {
            return Err("unit va si y solo si hay algún valor numérico".into());
        }
        Ok(Self {
            source: r.source,
            event: r.event,
            country: r.country,
            scheduled_at: r.scheduled_at,
            forecast_raw: r.forecast_raw,
            previous_raw: r.previous_raw,
            forecast_value: r.forecast_value,
            previous_value: r.previous_value,
            unit: r.unit,
            seen_at: r.seen_at,
        })
    }
}

/// El dato real de una serie oficial para un mes (`release_actual.v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "ActualRaw")]
pub struct ReleaseActual {
    pub source: String,
    pub series_id: String,
    /// Mes de referencia, `AAAA-MM`.
    pub reference_period: String,
    #[serde(with = "crate::decimal_str::signed")]
    pub value: Decimal,
    pub unit: StatUnit,
    pub vintage: Vintage,
    /// Hora oficial de publicación: desde acá existe para una simulación.
    #[serde(with = "utc_z")]
    pub released_at: DateTime<Utc>,
    /// Cuándo lo leímos: desde acá existe para una decisión en vivo.
    #[serde(with = "utc_z")]
    pub retrieved_at: DateTime<Utc>,
    pub source_url: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActualRaw {
    source: String,
    series_id: String,
    reference_period: String,
    #[serde(with = "crate::decimal_str::signed")]
    value: Decimal,
    unit: StatUnit,
    vintage: Vintage,
    #[serde(with = "utc_z")]
    released_at: DateTime<Utc>,
    #[serde(with = "utc_z")]
    retrieved_at: DateTime<Utc>,
    source_url: String,
}

/// `AAAA-MM` con mes 01..12.
pub fn is_month(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() == 7
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && matches!(p[5..].parse::<u8>(), Ok(1..=12))
        && b[5..].iter().all(u8::is_ascii_digit)
}

/// El mes anterior de `AAAA-MM` (enero → diciembre del año anterior).
pub fn previous_month(p: &str) -> Option<String> {
    if !is_month(p) {
        return None;
    }
    let (y, m): (u32, u32) = (p[..4].parse().ok()?, p[5..].parse().ok()?);
    Some(if m == 1 {
        format!("{:04}-12", y.checked_sub(1)?)
    } else {
        format!("{y:04}-{:02}", m - 1)
    })
}

impl TryFrom<ActualRaw> for ReleaseActual {
    type Error = String;

    fn try_from(r: ActualRaw) -> Result<Self, String> {
        if r.source != "bls" {
            return Err(format!("source desconocida: {:?}", r.source));
        }
        let b = r.series_id.as_bytes();
        let ok_char = |c: &u8| c.is_ascii_uppercase() || c.is_ascii_digit();
        if b.is_empty()
            || b.len() > 40
            || !ok_char(&b[0])
            || !b
                .iter()
                .all(|c| ok_char(c) || matches!(c, b'_' | b'#' | b'-'))
        {
            return Err(format!("series_id inválido: {:?}", r.series_id));
        }
        if !is_month(&r.reference_period) {
            return Err(format!(
                "reference_period inválido: {:?}",
                r.reference_period
            ));
        }
        if !r.source_url.starts_with("https://") || r.source_url.chars().count() > 500 {
            return Err("source_url tiene que ser https y de hasta 500 caracteres".into());
        }
        // Más estricto que el contrato (lo exige también la tabla): no se lee antes de salir.
        if r.retrieved_at < r.released_at {
            return Err("retrieved_at anterior a released_at".into());
        }
        Ok(Self {
            source: r.source,
            series_id: r.series_id,
            reference_period: r.reference_period,
            value: r.value,
            unit: r.unit,
            vintage: r.vintage,
            released_at: r.released_at,
            retrieved_at: r.retrieved_at,
            source_url: r.source_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_and_previous_months() {
        assert!(is_month("2026-10") && !is_month("2026-13") && !is_month("2026-00"));
        assert!(!is_month("2026-1") && !is_month("2026-10-01") && !is_month("20a6-10"));
        assert_eq!(previous_month("2027-01").as_deref(), Some("2026-12"));
        assert_eq!(previous_month("2026-10").as_deref(), Some("2026-09"));
        assert_eq!(previous_month("2026-13"), None);
    }
}

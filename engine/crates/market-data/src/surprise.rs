//! Sorpresa del calendario (ADR 0018 §3, #698): **dato real oficial − pronóstico previo**.
//!
//! Función pura: misma entrada, misma salida. Lee lo que guardó Python (`calendar_estimate.v1`
//! y `release_actual.v1`) y la correspondencia de `config/calendar_surprise.toml` (`[[surprise]]`).
//! La sorpresa no se guarda: se calcula acá, en un solo lugar.
//!
//! Reglas (anti look-ahead):
//! - **Consenso** = la última instantánea del evento con `seen_at` **estrictamente anterior** a
//!   `scheduled_at` (ADR 0018 §1.4). Una vista en o después de la hora no cuenta.
//! - **Dato real** = la fila `first` de la serie cuyo `released_at` es **la misma hora** del
//!   evento. Si Forex Factory y el organismo no coinciden en la hora, no hay sorpresa: no se
//!   adivina cuál es cuál. Un valor `revised` nunca entra.
//! - Para `change` y `pct_change`, el mes anterior es el que se conocía **al leer el primer dato**
//!   (la fila del mes anterior con `retrieved_at` más reciente que no pase de esa lectura): así
//!   coincide con lo que dijo esa publicación, que ya revisa el mes anterior.
//! - `pct_change` no se redondea. El BLS publica la variación con un decimal; la nuestra puede
//!   diferir de la publicada en hasta media décima.
//! - La sorpresa existe desde `released_at` (simulación) o `retrieved_at` (en vivo): las dos
//!   horas van en el resultado y las elige quien la usa.

use std::path::Path;

use cfds_core::{CalendarEstimate, ReleaseActual, StatUnit, Vintage, calendar_surprise};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;

/// Cómo sale el dato del evento a partir de la serie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transform {
    /// El valor del mes.
    Level,
    /// Mes − mes anterior (misma unidad que la serie).
    Change,
    /// (mes / mes anterior − 1) × 100, en `percent`.
    PctChange,
}

/// Un evento de Forex Factory y la serie oficial con la que se compara.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurpriseMapping {
    pub event: String,
    pub country: String,
    pub series_id: String,
    pub transform: Transform,
}

/// `[[surprise]]` de `config/calendar_surprise.toml` (la sección `[bls]` es de Python).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurpriseConfig {
    pub mappings: Vec<SurpriseMapping>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SurpriseConfigError {
    #[error("no se pudo leer {0}: {1}")]
    Io(String, String),
    #[error("config/calendar_surprise.toml inválida: {0}")]
    Invalid(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[allow(dead_code)] // la valida Python (`cfds_intel.calendar.actuals.load_config`)
    bls: toml::Table,
    #[serde(default)]
    surprise: Vec<SurpriseMapping>,
}

impl SurpriseConfig {
    pub fn from_toml(text: &str) -> Result<Self, SurpriseConfigError> {
        let raw: RawConfig =
            toml::from_str(text).map_err(|e| SurpriseConfigError::Invalid(e.to_string()))?;
        let mut seen = std::collections::HashSet::new();
        for m in &raw.surprise {
            if m.event.trim().is_empty() || m.series_id.trim().is_empty() {
                return Err(SurpriseConfigError::Invalid(
                    "event o series_id vacío".into(),
                ));
            }
            if !(m.country.len() == 2 && m.country.chars().all(|c| c.is_ascii_uppercase())) {
                return Err(SurpriseConfigError::Invalid(format!(
                    "country inválido: {:?}",
                    m.country
                )));
            }
            if !seen.insert((m.event.clone(), m.country.clone())) {
                return Err(SurpriseConfigError::Invalid(format!(
                    "evento repetido: {} ({})",
                    m.event, m.country
                )));
            }
        }
        Ok(Self {
            mappings: raw.surprise,
        })
    }

    pub fn load(path: &Path) -> Result<Self, SurpriseConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| SurpriseConfigError::Io(path.display().to_string(), e.to_string()))?;
        Self::from_toml(&text)
    }

    /// La correspondencia de un evento de Forex Factory, si tiene.
    pub fn mapping(&self, event: &str, country: &str) -> Option<&SurpriseMapping> {
        self.mappings
            .iter()
            .find(|m| m.event == event && m.country == country)
    }
}

/// La sorpresa de un evento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surprise {
    pub event: String,
    pub country: String,
    pub scheduled_at: DateTime<Utc>,
    pub series_id: String,
    pub reference_period: String,
    pub unit: StatUnit,
    /// El dato real, ya transformado (nivel, cambio o variación %).
    pub actual: Decimal,
    /// El `forecast` de la instantánea vigente antes de la hora.
    pub consensus: Decimal,
    /// `actual − consensus`.
    pub surprise: Decimal,
    pub consensus_seen_at: DateTime<Utc>,
    pub released_at: DateTime<Utc>,
    pub retrieved_at: DateTime<Utc>,
}

/// Por qué un evento no tiene sorpresa. Nunca se rellena con un valor inventado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoSurprise {
    /// No hay instantánea vista antes de la hora del evento.
    NoConsensus,
    /// La instantánea vigente no tiene un `forecast` numérico.
    NoForecastValue,
    /// No hay un primer dato publicado a la hora del evento.
    NoActual,
    /// Hay más de un primer dato a esa hora (no debería pasar: la tabla lo impide).
    AmbiguousActual,
    /// Falta el mes anterior conocido al leer el primer dato (para `change`/`pct_change`).
    NoPrevious,
    /// El mes anterior vale 0: no hay variación porcentual.
    ZeroPrevious,
    /// La unidad del pronóstico no es la del dato real transformado.
    UnitMismatch {
        forecast: StatUnit,
        actual: StatUnit,
    },
    /// El mes anterior tiene otra unidad que el mes.
    PreviousUnitMismatch,
}

/// El consenso de un evento: la última instantánea **del mismo evento** con
/// `seen_at < scheduled_at`.
pub fn consensus_before<'a>(
    estimates: &'a [CalendarEstimate],
    event: &str,
    country: &str,
    scheduled_at: DateTime<Utc>,
) -> Option<&'a CalendarEstimate> {
    estimates
        .iter()
        .filter(|e| {
            e.event == event
                && e.country == country
                && e.scheduled_at == scheduled_at
                && e.seen_at < scheduled_at
        })
        .max_by_key(|e| e.seen_at)
}

/// El dato real del evento, transformado, con la fila del primer dato.
fn actual_of<'a>(
    m: &SurpriseMapping,
    scheduled_at: DateTime<Utc>,
    actuals: &'a [ReleaseActual],
) -> Result<(Decimal, StatUnit, &'a ReleaseActual), NoSurprise> {
    let mut firsts = actuals.iter().filter(|a| {
        a.series_id == m.series_id && a.vintage == Vintage::First && a.released_at == scheduled_at
    });
    let first = firsts.next().ok_or(NoSurprise::NoActual)?;
    if firsts.next().is_some() {
        return Err(NoSurprise::AmbiguousActual);
    }
    if m.transform == Transform::Level {
        return Ok((first.value, first.unit, first));
    }
    let prev_period =
        calendar_surprise::previous_month(&first.reference_period).ok_or(NoSurprise::NoPrevious)?;
    let prev = actuals
        .iter()
        .filter(|a| {
            a.series_id == m.series_id
                && a.reference_period == prev_period
                && a.retrieved_at <= first.retrieved_at
        })
        .max_by_key(|a| a.retrieved_at)
        .ok_or(NoSurprise::NoPrevious)?;
    if prev.unit != first.unit {
        return Err(NoSurprise::PreviousUnitMismatch);
    }
    match m.transform {
        Transform::Change => Ok((first.value - prev.value, first.unit, first)),
        Transform::PctChange => {
            if prev.value.is_zero() {
                return Err(NoSurprise::ZeroPrevious);
            }
            let pct = (first.value / prev.value - Decimal::ONE) * Decimal::ONE_HUNDRED;
            Ok((pct.normalize(), StatUnit::Percent, first))
        }
        Transform::Level => unreachable!("resuelto arriba"),
    }
}

/// La sorpresa de un evento de Forex Factory con su correspondencia.
pub fn surprise(
    m: &SurpriseMapping,
    scheduled_at: DateTime<Utc>,
    estimates: &[CalendarEstimate],
    actuals: &[ReleaseActual],
) -> Result<Surprise, NoSurprise> {
    let est = consensus_before(estimates, &m.event, &m.country, scheduled_at)
        .ok_or(NoSurprise::NoConsensus)?;
    let (consensus, forecast_unit) = match (est.forecast_value, est.unit) {
        (Some(v), Some(u)) => (v, u),
        _ => return Err(NoSurprise::NoForecastValue),
    };
    let (actual, unit, first) = actual_of(m, scheduled_at, actuals)?;
    if forecast_unit != unit {
        return Err(NoSurprise::UnitMismatch {
            forecast: forecast_unit,
            actual: unit,
        });
    }
    Ok(Surprise {
        event: m.event.clone(),
        country: m.country.clone(),
        scheduled_at,
        series_id: m.series_id.clone(),
        reference_period: first.reference_period.clone(),
        unit,
        actual,
        consensus,
        surprise: actual - consensus,
        consensus_seen_at: est.seen_at,
        released_at: first.released_at,
        retrieved_at: first.retrieved_at,
    })
}

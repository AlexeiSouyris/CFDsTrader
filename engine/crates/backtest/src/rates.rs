//! Tipo de referencia de la financiación en cada momento (#79).
//!
//! - [`FundingRates::Constant`]: un valor fijo, el de `[funding_benchmarks]` del catálogo.
//! - [`FundingRates::Daily`]: la serie diaria guardada (`reference_rate.v1`, hoy el €STR).
//!
//! Regla sin look-ahead (CLAUDE.md §5b): en el momento `t` vale el valor **del día más
//! reciente ya publicado** a `t`. Una corrección (otra fila del mismo día, publicada después)
//! reemplaza a la anterior desde que se publica; la corrección tardía de un día viejo no pisa
//! el valor de un día más nuevo.

use cfds_core::ReferenceRate;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FundingRates {
    Constant(Decimal),
    Daily(DailyRates),
}

impl FundingRates {
    /// Tasa anual en % vigente en `t`, o `None` si todavía no se había publicado ninguna.
    pub fn at(&self, t: DateTime<Utc>) -> Option<Decimal> {
        match self {
            Self::Constant(v) => Some(*v),
            Self::Daily(d) => d.at(t),
        }
    }
}

/// Serie diaria lista para consultar por momento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyRates {
    series: String,
    /// Horas de publicación, en orden.
    published: Vec<DateTime<Utc>>,
    /// Valor vigente después de cada publicación (el del día más reciente publicado hasta ahí).
    in_force: Vec<(NaiveDate, Decimal)>,
}

impl DailyRates {
    /// `None` si `rates` está vacío o mezcla series.
    pub fn new(mut rates: Vec<ReferenceRate>) -> Option<Self> {
        let series = rates.first()?.series.clone();
        if rates.iter().any(|r| r.series != series) {
            return None;
        }
        rates.sort_by_key(|r| (r.published_at, r.ref_date));
        let mut published = Vec::with_capacity(rates.len());
        let mut in_force: Vec<(NaiveDate, Decimal)> = Vec::with_capacity(rates.len());
        for r in rates {
            // Un día igual o más nuevo que el vigente lo reemplaza (el igual es una corrección).
            let current = match in_force.last() {
                Some(&(day, value)) if day > r.ref_date => (day, value),
                _ => (r.ref_date, r.rate_pct),
            };
            published.push(r.published_at);
            in_force.push(current);
        }
        Some(Self {
            series,
            published,
            in_force,
        })
    }

    pub fn series(&self) -> &str {
        &self.series
    }

    /// Tasa vigente en `t` y el día al que corresponde.
    pub fn at_with_day(&self, t: DateTime<Utc>) -> Option<(NaiveDate, Decimal)> {
        let n = self.published.partition_point(|p| *p <= t);
        n.checked_sub(1).map(|i| self.in_force[i])
    }

    pub fn at(&self, t: DateTime<Utc>) -> Option<Decimal> {
        self.at_with_day(t).map(|(_, v)| v)
    }
}

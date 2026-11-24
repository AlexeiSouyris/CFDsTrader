//! Tipo de referencia de la financiación en cada momento (#79).
//!
//! - [`FundingRates::Constant`]: un valor fijo, el de `[funding_benchmarks]` del catálogo.
//! - [`FundingRates::Daily`]: la serie diaria guardada (`reference_rate.v1`, hoy el €STR).
//! - [`FxRates`]: tipo de cambio diario de referencia (`fx_rate.v1`, EUR/USD del BCE, ADR 0013).
//!
//! Regla sin look-ahead (CLAUDE.md §5b): en el momento `t` vale el valor **del día más
//! reciente ya publicado** a `t`. Una corrección (otra fila del mismo día, publicada después)
//! reemplaza a la anterior desde que se publica; la corrección tardía de un día viejo no pisa
//! el valor de un día más nuevo.

use cfds_core::{FxRate, ReferenceRate};
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
    pub fn new(rates: Vec<ReferenceRate>) -> Option<Self> {
        let series = rates.first()?.series.clone();
        if rates.iter().any(|r| r.series != series) {
            return None;
        }
        let (published, in_force) = in_force_timeline(
            rates
                .into_iter()
                .map(|r| (r.published_at, r.ref_date, r.rate_pct)),
        );
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

/// Horas de publicación en orden y el valor vigente después de cada una: el del día más
/// reciente publicado hasta ahí. Un día igual o más nuevo que el vigente lo reemplaza (el igual
/// es una corrección); uno más viejo publicado tarde no pisa al más nuevo.
type Timeline = (Vec<DateTime<Utc>>, Vec<(NaiveDate, Decimal)>);

fn in_force_timeline(rows: impl Iterator<Item = (DateTime<Utc>, NaiveDate, Decimal)>) -> Timeline {
    let mut rows: Vec<_> = rows.collect();
    rows.sort_by_key(|&(published, day, _)| (published, day));
    let mut published = Vec::with_capacity(rows.len());
    let mut in_force: Vec<(NaiveDate, Decimal)> = Vec::with_capacity(rows.len());
    for (at, day, value) in rows {
        let current = match in_force.last() {
            Some(&(d, v)) if d > day => (d, v),
            _ => (day, value),
        };
        published.push(at);
        in_force.push(current);
    }
    (published, in_force)
}

/// Tipo de cambio diario de referencia de un par (ADR 0013 §4), listo para consultar por
/// momento, con la misma regla sin look-ahead que [`DailyRates`]: en `t` vale el del día más
/// reciente ya publicado. Sin ninguno publicado, `None`: nunca se inventa un tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FxRates {
    pair: String,
    published: Vec<DateTime<Utc>>,
    in_force: Vec<(NaiveDate, Decimal)>,
}

impl FxRates {
    /// `None` si `rates` está vacío, mezcla pares o trae un tipo no positivo.
    pub fn new(rates: Vec<FxRate>) -> Option<Self> {
        let pair = rates.first()?.pair.clone();
        if rates
            .iter()
            .any(|r| r.pair != pair || r.rate <= Decimal::ZERO)
        {
            return None;
        }
        let (published, in_force) = in_force_timeline(
            rates
                .into_iter()
                .map(|r| (r.published_at, r.ref_date, r.rate)),
        );
        Some(Self {
            pair,
            published,
            in_force,
        })
    }

    pub fn pair(&self) -> &str {
        &self.pair
    }

    /// Tipo vigente en `t` (unidades de la cotizada por 1 de la base) y su día.
    pub fn at_with_day(&self, t: DateTime<Utc>) -> Option<(NaiveDate, Decimal)> {
        let n = self.published.partition_point(|p| *p <= t);
        n.checked_sub(1).map(|i| self.in_force[i])
    }

    pub fn at(&self, t: DateTime<Utc>) -> Option<Decimal> {
        self.at_with_day(t).map(|(_, v)| v)
    }
}

/// Unidades de la moneda del instrumento por 1 de la moneda de la cuenta, en cada momento
/// (ADR 0013 §4). Convierte P/L, margen, financiación y riesgo de la moneda del instrumento a
/// la de la cuenta: `monto_cuenta = monto_instrumento / tipo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountFx {
    /// Instrumento y cuenta en la misma moneda: 1.
    Same,
    /// Serie diaria del par `CUENTA+INSTRUMENTO` (por ejemplo `EURUSD`): el tipo vigente en cada
    /// momento, sin look-ahead.
    Daily(FxRates),
    /// Monedas distintas y todavía sin serie: no hay tipo en ningún momento (nunca se inventa).
    Missing { pair: String },
}

impl AccountFx {
    /// Tipo vigente en `t`, o `None` si no hay ninguno publicado (o falta la serie).
    pub fn at(&self, t: DateTime<Utc>) -> Option<Decimal> {
        match self {
            Self::Same => Some(Decimal::ONE),
            Self::Daily(r) => r.at(t),
            Self::Missing { .. } => None,
        }
    }
}

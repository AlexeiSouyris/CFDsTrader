//! Base de los futuros en la financiación (#467, ADR 0015).
//!
//! Dentro de un contrato, el precio del futuro converge al contado: su deriva ya descuenta la
//! tasa menos los dividendos (`r − q`). El CFD de IG al contado paga la financiación completa
//! (referencia ± tasa administrativa) y recibe el ajuste por dividendos. Con las velas del futuro,
//! sumar la financiación completa cuenta `r` dos veces: a los largos se les cobra de más y a los
//! cortos se les paga de menos. Por eso, en cada noche se resta lo que la base ya descontó:
//!
//! ```text
//! descuento = Σ_noches (r − q)(noche) × precio × tamaño / 100 / divisor     (+ largo, − corto)
//! financiación con futuros = financiación del CFD − descuento
//! ```
//!
//! Con `BasisMode::Rate` (por omisión), `q` = 0: IG ajusta los dividendos en el CFD, así que los
//! que trae la base son los mismos que recibe el CFD. Con velas de IG no se aplica nada.
//!
//! Comprar y mantener cruza los vencimientos con el continuo sin ajustar: en cada cambio de
//! contrato el precio salta por la base del contrato nuevo. Ese salto no es un movimiento del
//! mercado ([`roll_jumps`]): se descuenta, y la deriva queda como en las operaciones.

use cfds_core::{Candle, ReferenceRate};
use cfds_market_data::RateSegment;
use cfds_strategy::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{DailyRates, FundingRates, InstrumentCosts};

/// Lo que la base del futuro ya descuenta, en cada momento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuturesBasis {
    /// Tasa de la base (`r`), anual en %.
    pub rate: FundingRates,
    /// Dividendos (`q`), anual en %; `None` con `BasisMode::Rate`.
    pub dividends: Option<FundingRates>,
    /// Inicios de la primera vela de cada contrato nuevo (los de `mark_rollovers`), en orden.
    pub rolls: Vec<DateTime<Utc>>,
}

impl FuturesBasis {
    /// `r − q` vigente en `t`, o `None` si todavía no había ningún valor publicado.
    pub fn at(&self, t: DateTime<Utc>) -> Option<Decimal> {
        let r = self.rate.at(t)?;
        match &self.dividends {
            None => Some(r),
            Some(q) => Some(r - q.at(t)?),
        }
    }
}

/// Descuento de la base sobre `cutoffs` (los cortes de financiación de la operación), en la
/// moneda del instrumento: positivo en un largo (paga de menos), negativo en un corto (recibe de
/// menos). Mismo precio, tamaño y divisor que [`crate::funding_over`]. Si en algún corte no había
/// ningún valor publicado, devuelve ese corte como error: nunca se inventa una tasa.
pub fn basis_carry(
    side: Side,
    close: Decimal,
    size: Decimal,
    cutoffs: &[DateTime<Utc>],
    basis: &FuturesBasis,
    costs: &InstrumentCosts,
) -> Result<Decimal, DateTime<Utc>> {
    let mut rate_nights = Decimal::ZERO;
    for &at in cutoffs {
        rate_nights += basis.at(at).ok_or(at)?;
    }
    let carry = close * size * rate_nights / Decimal::ONE_HUNDRED / costs.funding_day_count;
    Ok(match side {
        Side::Long => carry,
        Side::Short => -carry,
    })
}

/// Suma de los saltos de precio en los cambios de contrato dentro de `candles` (en orden):
/// apertura de la primera vela del contrato nuevo − cierre de la última del viejo. Solo cuenta
/// los cambios con velas de los dos lados.
pub fn roll_jumps(candles: &[Candle], rolls: &[DateTime<Utc>]) -> Decimal {
    let mut total = Decimal::ZERO;
    for &r in rolls {
        let next = candles.partition_point(|c| c.start < r);
        if next == 0 || next >= candles.len() {
            continue;
        }
        total += candles[next].open - candles[next - 1].close;
    }
    total
}

/// Serie diaria empalmada: de cada tramo, los valores con `ref_date` desde su `from` hasta el
/// `from` del siguiente. `rows` tiene los valores guardados de cada serie, en el orden de
/// `segments`. Queda rotulada `label`. `None` si no queda ningún valor.
pub fn splice_rates(
    segments: &[RateSegment],
    rows: Vec<Vec<ReferenceRate>>,
    label: &str,
) -> Option<DailyRates> {
    let mut out = Vec::new();
    for (i, (seg, rows)) in segments.iter().zip(rows).enumerate() {
        let until = segments.get(i + 1).and_then(|s| s.from);
        out.extend(rows.into_iter().filter_map(|mut r| {
            let after = seg.from.is_none_or(|f| r.ref_date >= f);
            let before = until.is_none_or(|u| r.ref_date < u);
            (after && before && r.series == seg.series).then(|| {
                r.series = label.to_string();
                r
            })
        }));
    }
    DailyRates::new(out)
}

//! Costos de una operación (spec §11). Nunca en cero; ningún valor del broker se inventa: los
//! que faltan (spread típico, tick, tipo de referencia) entran como parámetro.
//!
//! ```text
//! ejecución:   precio = medio ± (spread / 2 + slippage), siempre en contra
//!              slippage = 0,1 × spread + 1 tick
//! comisión:    índices 0 · acciones EE. UU. max(mín, por_acción × acciones)
//!              · acciones UE max(mín, % × nocional)          (por ejecución)
//! financiación: noches × cierre × tamaño × tasa / base_días
//!              largo: tasa = referencia + adm. · corto: tasa = adm. − referencia (puede cobrar)
//! noches:      cortes de las 23:00 (Madrid) cruzados entre la apertura y el cierre
//! ```
//! Tamaño en moneda del instrumento por punto; en acciones equivale a la cantidad de acciones.

use cfds_strategy::Side;
use chrono::{DateTime, Days, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;

use cfds_core::AssetClass;
use cfds_market_data::{InstrumentSpec, PendingField};

use crate::CostConfig;
use crate::rates::FundingRates;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commission {
    /// CFD de índices: el costo es el spread.
    None,
    /// Acciones de EE. UU.: por acción, con mínimo por ejecución.
    PerShare { per_share: Decimal, min: Decimal },
    /// Acciones europeas: % del nocional, con mínimo por ejecución.
    Percent { pct: Decimal, min: Decimal },
}

/// Costos de un instrumento, en puntos y en su moneda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstrumentCosts {
    /// Spread típico en puntos (en acciones, 0: IG cobra comisión).
    pub spread: Decimal,
    /// Movimiento mínimo de precio.
    pub tick: Decimal,
    pub commission: Commission,
    /// Tasa administrativa anual de la financiación, en %.
    pub funding_admin_fee_pct: Decimal,
    /// Base de días de la financiación (360 o 365).
    pub funding_day_count: Decimal,
}

impl InstrumentCosts {
    /// Costos de `spec` tomados **solo** del catálogo (`config/instruments.toml`, con fuente).
    /// Si a este instrumento le falta cualquier costo, devuelve los pendientes y no arma nada:
    /// ningún backtest corre con un costo inventado o en cero (spec §11, CLAUDE.md §5b).
    ///
    /// - Spread: el típico del catálogo (ya es el mayor entre el medido y el mínimo publicado).
    /// - Comisión: índices, ninguna. Acciones: todavía no están en el catálogo, quedan pendientes.
    pub fn from_spec(spec: &InstrumentSpec) -> Result<Self, Vec<PendingField>> {
        let mut missing = spec.missing_costs();
        let commission = match spec.instrument.asset_class {
            AssetClass::Index => Some(Commission::None),
            _ => {
                missing.push(PendingField {
                    item: spec.instrument.id.as_str().to_owned(),
                    field: "commission",
                    reason: "las acciones todavía no están en el catálogo de instrumentos".into(),
                });
                None
            }
        };
        let t = &spec.terms;
        match (
            commission,
            t.spread_typical_points.value(),
            t.tick.value(),
            t.funding_admin_fee_pct.value(),
            t.funding_day_count.value(),
        ) {
            (Some(commission), Some(&spread), Some(&tick), Some(&fee), Some(&days))
                if missing.is_empty() =>
            {
                Ok(Self {
                    spread,
                    tick,
                    commission,
                    funding_admin_fee_pct: fee,
                    funding_day_count: days,
                })
            }
            _ => Err(missing),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Execution {
    Entry,
    Exit,
}

/// Precio al que se ejecuta una orden sobre el precio medio `mid`, pagando medio spread y el
/// slippage.
pub fn fill_price(
    side: Side,
    execution: Execution,
    mid: Decimal,
    costs: &InstrumentCosts,
    cfg: &CostConfig,
) -> Decimal {
    let slippage = cfg.slippage_spread_fraction * costs.spread + cfg.slippage_ticks * costs.tick;
    let adverse = costs.spread / Decimal::TWO + slippage;
    // Comprar: más caro. Vender: más barato.
    let buying = matches!(
        (side, execution),
        (Side::Long, Execution::Entry) | (Side::Short, Execution::Exit)
    );
    if buying { mid + adverse } else { mid - adverse }
}

/// Comisión de una ejecución de `size` a `price`.
pub fn commission(costs: &InstrumentCosts, size: Decimal, price: Decimal) -> Decimal {
    match costs.commission {
        Commission::None => Decimal::ZERO,
        Commission::PerShare { per_share, min } => (per_share * size).max(min),
        Commission::Percent { pct, min } => (pct / Decimal::ONE_HUNDRED * size * price).max(min),
    }
}

/// Financiación de `nights` noches: positivo es un costo, negativo un cobro. `benchmark_pct`
/// es el tipo de referencia anual en % (por ejemplo, el €STR de ese día).
pub fn funding(
    side: Side,
    close: Decimal,
    size: Decimal,
    nights: u32,
    benchmark_pct: Decimal,
    costs: &InstrumentCosts,
) -> Decimal {
    let rate = match side {
        Side::Long => benchmark_pct + costs.funding_admin_fee_pct,
        Side::Short => costs.funding_admin_fee_pct - benchmark_pct,
    };
    Decimal::from(nights) * close * size * rate / Decimal::ONE_HUNDRED / costs.funding_day_count
}

/// Cortes de financiación (la hora `cutoff` local de `tz` de cada día) que hay en
/// `(open, close]`, en orden. Si una posición sigue abierta en el corte, paga esa noche.
pub fn funding_cutoffs(
    open: DateTime<Utc>,
    close: DateTime<Utc>,
    cutoff: NaiveTime,
    tz: Tz,
) -> Vec<DateTime<Utc>> {
    let mut day = open.with_timezone(&tz).date_naive();
    let last = close.with_timezone(&tz).date_naive();
    let mut out = Vec::new();
    while day <= last {
        // 23:00 nunca cae en el salto del cambio de hora; si una zona lo hiciera, ese día
        // no tiene corte y no se cobra (nunca se cobra dos veces).
        if let Some(at) = tz.from_local_datetime(&day.and_time(cutoff)).earliest() {
            let at = at.with_timezone(&Utc);
            if at > open && at <= close {
                out.push(at);
            }
        }
        day = match day.checked_add_days(Days::new(1)) {
            Some(d) => d,
            None => break,
        };
    }
    out
}

/// Cantidad de cortes de financiación en `(open, close]` (ver [`funding_cutoffs`]).
pub fn funding_nights(open: DateTime<Utc>, close: DateTime<Utc>, cutoff: NaiveTime, tz: Tz) -> u32 {
    u32::try_from(funding_cutoffs(open, close, cutoff, tz).len()).unwrap_or(u32::MAX)
}

/// Financiación noche por noche (#79): cada corte paga con la tasa de referencia **vigente en
/// ese corte** (ver [`FundingRates::at`]). Si en algún corte todavía no había ninguna tasa
/// publicada, devuelve ese corte como error: nunca se inventa una tasa.
pub fn funding_over(
    side: Side,
    close: Decimal,
    size: Decimal,
    cutoffs: &[DateTime<Utc>],
    rates: &FundingRates,
    costs: &InstrumentCosts,
) -> Result<Decimal, DateTime<Utc>> {
    // Se suman las tasas de cada noche y se divide una sola vez: con una tasa constante da
    // exactamente lo mismo que `funding` con `nights` (sin redondeos intermedios).
    let mut rate_nights = Decimal::ZERO;
    for &at in cutoffs {
        let benchmark = rates.at(at).ok_or(at)?;
        rate_nights += match side {
            Side::Long => benchmark + costs.funding_admin_fee_pct,
            Side::Short => costs.funding_admin_fee_pct - benchmark,
        };
    }
    Ok(close * size * rate_nights / Decimal::ONE_HUNDRED / costs.funding_day_count)
}

//! Sweep de liquidez externa (spec §5.4, gatillo).
//!
//! ```text
//! P = último pivote bajo de 1h confirmado al cierre de la vela t (de 15m)
//! largo: L[t] < P y C[t] > P      (pinchó por debajo y volvió a cerrar adentro)
//! corto: H[t] > P' y C[t] < P'    (P' = último pivote alto de 1h confirmado)
//! ```
//! Un pivote de 1h está confirmado cuando **cerró** su vela de confirmación
//! (`h1[confirmed_at].end() ≤ c.end()`): antes no se sabe que es pivote.

use cfds_core::Candle;
use cfds_patterns::{Pivot, PivotKind};
use rust_decimal::Decimal;

use crate::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sweep {
    pub side: Side,
    /// El pivote de 1h barrido (referencia del stop, spec §8: la mecha del sweep).
    pub level: Decimal,
}

fn last_known(c: &Candle, h1: &[Candle], pivots: &[Pivot], kind: PivotKind) -> Option<Decimal> {
    pivots
        .iter()
        .filter(|p| p.kind == kind)
        .filter(|p| h1.get(p.confirmed_at).is_some_and(|k| k.end() <= c.end()))
        .max_by_key(|p| p.index)
        .map(|p| p.price)
}

/// Sweeps que hace la vela de 15m `c` contra los pivotes de 1h (`h1_pivots`, calculados sobre `h1`).
pub fn sweeps_at(c: &Candle, h1: &[Candle], h1_pivots: &[Pivot]) -> Vec<Sweep> {
    let mut out = Vec::new();
    if let Some(p) = last_known(c, h1, h1_pivots, PivotKind::Low)
        && c.low < p
        && c.close > p
    {
        out.push(Sweep {
            side: Side::Long,
            level: p,
        });
    }
    if let Some(p) = last_known(c, h1, h1_pivots, PivotKind::High)
        && c.high > p
        && c.close < p
    {
        out.push(Sweep {
            side: Side::Short,
            level: p,
        });
    }
    out
}

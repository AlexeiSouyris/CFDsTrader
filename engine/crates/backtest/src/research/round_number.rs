//! Familia M1: cascada al cruzar un número redondo (Osler, 2003 y 2005; informe de #492), con la
//! definición del pre-registro de la ronda 3 §4 (`docs/analysis/2026-11-ronda3-preregistro.md`).
//!
//! ```text
//! niveles R = k × paso + desplazamiento (redondos: desplazamiento 0; control: paso / 2)
//! cruce en t (cerrada, contigua a t − 1, sin vencimiento entre las dos):
//!   alcista  C[t−1] < R ≤ C[t]   → largo        bajista  C[t−1] > R ≥ C[t] → corto
//! decisión al cierre de t (si la preparación la permite), entrada en la apertura de t + 1
//! stop = referencia ∓ stop_atr × ATR de 15m en t; salida al cierre de la última vela que termina
//!   `horizon` o menos después del cierre de t
//! ```

use cfds_core::Candle;
use cfds_strategy::Side;
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;

use super::{Plan, decision_allowed};

/// ¿Hay un nivel `k × step + offset` en `(lo, hi]`? (cruce alcista: `C[t−1] < R ≤ C[t]`)
fn level_up(lo: Decimal, hi: Decimal, step: Decimal, offset: Decimal) -> bool {
    ((hi - offset) / step).floor() * step + offset > lo
}

/// ¿Hay un nivel `k × step + offset` en `[lo, hi)`? (cruce bajista: `C[t−1] > R ≥ C[t]`)
fn level_down(lo: Decimal, hi: Decimal, step: Decimal, offset: Decimal) -> bool {
    ((hi - offset) / step).ceil() * step + offset - step >= lo
}

/// Niveles `k × step + offset` (redondos: `offset = 0`; control: `offset = step / 2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Levels {
    pub step: Decimal,
    pub offset: Decimal,
}

/// Operaciones de la familia M1 sobre `m15` con los niveles `levels`. `entries` es la marca
/// de la preparación, `atr` el ATR de 15m alineado con `m15` y `rolls` los inicios de cada
/// contrato nuevo. Función pura: cada decisión usa solo las velas `t − 1` y `t`, ya cerradas.
pub fn round_number_plans(
    m15: &[Candle],
    entries: &[bool],
    atr: &[Option<Decimal>],
    rolls: &[DateTime<Utc>],
    levels: Levels,
    stop_atr: Decimal,
    horizon: TimeDelta,
) -> Vec<Plan> {
    let Levels { step, offset } = levels;
    let mut plans = Vec::new();
    if step <= Decimal::ZERO {
        return plans;
    }
    for t in 1..m15.len() {
        let (prev, cur) = (&m15[t - 1], &m15[t]);
        if prev.end() != cur.start
            || rolls.iter().any(|&r| prev.start < r && r <= cur.start)
            || !decision_allowed(m15, entries, t)
        {
            continue;
        }
        let side = if level_up(prev.close, cur.close, step, offset) {
            Side::Long
        } else if level_down(cur.close, prev.close, step, offset) {
            Side::Short
        } else {
            continue;
        };
        let Some(Some(a)) = atr.get(t).copied() else {
            continue;
        };
        if a <= Decimal::ZERO {
            continue;
        }
        let limit = cur.end() + horizon;
        let last = m15.partition_point(|c| c.end() <= limit);
        let Some(exit_index) = last.checked_sub(1).filter(|&e| e > t + 1) else {
            continue;
        };
        let reference = cur.close;
        plans.push(Plan {
            event: t,
            signal_index: t,
            side,
            reference,
            stop: match side {
                Side::Long => reference - stop_atr * a,
                Side::Short => reference + stop_atr * a,
            },
            risk_atr: a,
            exit_index,
            target_r: None,
        });
    }
    plans
}

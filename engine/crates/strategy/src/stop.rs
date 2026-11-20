//! Stop estructural y R (spec §8).
//!
//! ```text
//! largo:  estructura = el mínimo de las estructuras de los gatillos
//!         stop = estructura − 0,5 × ATR;   R = entrada − stop
//! corto:  estructura = el máximo;  stop = estructura + 0,5 × ATR;   R = stop − entrada
//! ```

use rust_decimal::Decimal;

use crate::{Side, StopConfig, Trigger};

/// `None` si no hay gatillos o el ATR no es positivo (calentamiento o datos rotos).
pub fn structural_stop(
    side: Side,
    triggers: &[Trigger],
    atr: Decimal,
    cfg: &StopConfig,
) -> Option<Decimal> {
    if atr <= Decimal::ZERO {
        return None;
    }
    let structures = triggers.iter().map(|t| t.structure);
    let buffer = cfg.atr_buffer * atr;
    match side {
        Side::Long => structures.min().map(|s| s - buffer),
        Side::Short => structures.max().map(|s| s + buffer),
    }
}

/// R: la distancia de la entrada al stop. `None` si el stop está del lado equivocado
/// (o en la entrada), porque esa operación no tiene un riesgo definido.
pub fn risk_per_unit(side: Side, entry: Decimal, stop: Decimal) -> Option<Decimal> {
    let r = match side {
        Side::Long => entry - stop,
        Side::Short => stop - entry,
    };
    (r > Decimal::ZERO).then_some(r)
}

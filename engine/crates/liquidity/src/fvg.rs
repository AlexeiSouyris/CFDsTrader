//! Fair value gaps (spec §5.4, contexto).
//!
//! ```text
//! FVG alcista en la vela i:  L[i] > H[i−2]   zona [H[i−2], L[i]]
//!   se llena (deja de valer) cuando una vela posterior cierra por debajo de H[i−2]
//!   toque en t:  L[t] ≤ techo de la zona y C[t] ≥ piso   (entra a la zona sin llenarla)
//! FVG bajista: el espejo  (H[i] < L[i−2], zona [H[i], L[i−2]], se llena al cerrar arriba)
//! ```
//! Solo cuentan los FVG creados antes de `t` y dentro de las últimas `lookback` velas.

use cfds_core::Candle;
use cfds_indicators::Hlc;
use rust_decimal::Decimal;

use crate::{LiquidityConfig, Side};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fvg {
    pub side: Side,
    pub created_at: usize,
    pub bottom: Decimal,
    pub top: Decimal,
}

/// `true` si la vela `c` llena el FVG `f` (§5.4: cierra más allá del borde lejano). Es la única
/// definición de llenado: la usan el contexto de §5.4 y las estadísticas de §5.4.1.
pub fn is_filled_by<T: Hlc>(f: &Fvg, c: &T) -> bool {
    match f.side {
        Side::Long => c.close() < f.bottom,
        Side::Short => c.close() > f.top,
    }
}

/// FVG que crea la vela `i` junto con la `i − 2`, si lo hay (§5.4). Sirve para velas de
/// cualquier timeframe, también las diarias.
pub fn fvg_created_at<T: Hlc>(candles: &[T], i: usize) -> Option<Fvg> {
    let (a, c) = (candles.get(i.checked_sub(2)?)?, candles.get(i)?);
    if c.low() > a.high() {
        Some(Fvg {
            side: Side::Long,
            created_at: i,
            bottom: a.high(),
            top: c.low(),
        })
    } else if c.high() < a.low() {
        Some(Fvg {
            side: Side::Short,
            created_at: i,
            bottom: c.high(),
            top: a.low(),
        })
    } else {
        None
    }
}

/// FVG creados hasta la vela `t` (inclusive) que siguen sin llenarse al cierre de `t`.
pub fn fvgs_up_to<T: Hlc>(candles: &[T], t: usize) -> Vec<Fvg> {
    let end = t.min(candles.len().saturating_sub(1));
    (2..=end)
        .filter_map(|i| fvg_created_at(candles, i))
        .filter(|f| {
            !candles[f.created_at + 1..=end]
                .iter()
                .any(|later| is_filled_by(f, later))
        })
        .collect()
}

/// FVG vigentes que la vela `t` toca (creados antes de `t`, en las últimas `lookback` velas).
pub fn fvg_touches_at(candles: &[Candle], t: usize, cfg: &LiquidityConfig) -> Vec<Fvg> {
    let Some(c) = candles.get(t) else {
        return Vec::new();
    };
    let from = (t + 1).saturating_sub(cfg.fvg_lookback_bars);
    // Lo mismo que filtrar `fvgs_up_to(candles, t)`, pero solo con los FVG creados dentro de la
    // ventana: no recorre toda la historia en cada vela (#390).
    (from.max(2)..t)
        .filter_map(|i| fvg_created_at(candles, i))
        .filter(|f| {
            !candles[f.created_at + 1..=t]
                .iter()
                .any(|later| is_filled_by(f, later))
        })
        .filter(|f| match f.side {
            Side::Long => c.low <= f.top && c.close >= f.bottom,
            Side::Short => c.high >= f.bottom && c.close <= f.top,
        })
        .collect()
}

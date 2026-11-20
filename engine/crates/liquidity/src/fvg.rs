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
use rust_decimal::Decimal;

use crate::{LiquidityConfig, Side};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fvg {
    pub side: Side,
    pub created_at: usize,
    pub bottom: Decimal,
    pub top: Decimal,
}

fn filled(f: &Fvg, c: &Candle) -> bool {
    match f.side {
        Side::Long => c.close < f.bottom,
        Side::Short => c.close > f.top,
    }
}

/// FVG creados hasta la vela `t` (inclusive) que siguen sin llenarse al cierre de `t`.
pub fn fvgs_up_to(candles: &[Candle], t: usize) -> Vec<Fvg> {
    let end = t.min(candles.len().saturating_sub(1));
    let mut out = Vec::new();
    for i in 2..=end {
        let (a, c) = (&candles[i - 2], &candles[i]);
        let created = if c.low > a.high {
            Some(Fvg {
                side: Side::Long,
                created_at: i,
                bottom: a.high,
                top: c.low,
            })
        } else if c.high < a.low {
            Some(Fvg {
                side: Side::Short,
                created_at: i,
                bottom: c.high,
                top: a.low,
            })
        } else {
            None
        };
        if let Some(f) = created
            && !candles[i + 1..=end].iter().any(|later| filled(&f, later))
        {
            out.push(f);
        }
    }
    out
}

/// FVG vigentes que la vela `t` toca (creados antes de `t`, en las últimas `lookback` velas).
pub fn fvg_touches_at(candles: &[Candle], t: usize, cfg: &LiquidityConfig) -> Vec<Fvg> {
    let Some(c) = candles.get(t) else {
        return Vec::new();
    };
    let from = (t + 1).saturating_sub(cfg.fvg_lookback_bars);
    fvgs_up_to(candles, t)
        .into_iter()
        .filter(|f| f.created_at < t && f.created_at >= from)
        .filter(|f| match f.side {
            Side::Long => c.low <= f.top && c.close >= f.bottom,
            Side::Short => c.high >= f.bottom && c.close <= f.top,
        })
        .collect()
}

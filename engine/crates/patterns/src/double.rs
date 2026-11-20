//! Doble suelo y doble techo (spec §5.2), evaluados en la vela cerrada `t`.
//!
//! ```text
//! DOBLE SUELO (largo):
//!   S1, S2 = pivotes bajos confirmados en t (S2 posterior a S1)
//!   min ≤ velas entre S1 y S2 ≤ max                (10 y 60)
//!   |S1 − S2| / S1 ≤ diferencia máxima             (0,3 %)
//!   neckline = máximo más alto entre S1 y S2
//!   neckline − min(S1, S2) ≥ k · ATR[t]            (k = 1)
//!   gatillo: C[t] > neckline y C[t−1] ≤ neckline
//!            y t ≤ confirmación de S2 + 20 velas
//! DOBLE TECHO (corto): el espejo.
//! ```
//! Se usan **solo pivotes confirmados en `t`** (`confirmed_at ≤ t`), aunque la lista que se
//! pase tenga pivotes posteriores. Si hay varios pares válidos, gana el de S2 más reciente.

use cfds_core::Candle;
use rust_decimal::Decimal;

use crate::config::DoubleConfig;
use crate::pivots::{Pivot, PivotKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleKind {
    /// Doble suelo: señal de compra.
    Bottom,
    /// Doble techo: señal de venta.
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoublePattern {
    pub kind: DoubleKind,
    pub first: Pivot,
    pub second: Pivot,
    pub neckline: Decimal,
}

impl DoublePattern {
    /// Extremo del patrón (el suelo más bajo o el techo más alto): referencia del stop (spec §8).
    pub fn extreme(&self) -> Decimal {
        match self.kind {
            DoubleKind::Bottom => self.first.price.min(self.second.price),
            DoubleKind::Top => self.first.price.max(self.second.price),
        }
    }
}

/// Doble suelo o doble techo cuyo gatillo ocurre en la vela `t`, si lo hay.
pub fn double_at(
    candles: &[Candle],
    t: usize,
    pivots: &[Pivot],
    atr: &[Option<Decimal>],
    cfg: &DoubleConfig,
) -> Vec<DoublePattern> {
    let (Some(c), Some(prev), Some(Some(atr_t))) = (
        candles.get(t),
        t.checked_sub(1).and_then(|i| candles.get(i)),
        atr.get(t),
    ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for kind in [DoubleKind::Bottom, DoubleKind::Top] {
        let pivot_kind = match kind {
            DoubleKind::Bottom => PivotKind::Low,
            DoubleKind::Top => PivotKind::High,
        };
        // Pivotes de ese tipo, ya confirmados en t, en orden.
        let known: Vec<&Pivot> = pivots
            .iter()
            .filter(|p| p.kind == pivot_kind && p.confirmed_at <= t)
            .collect();
        // S2 más reciente primero.
        let found = known.iter().enumerate().rev().find_map(|(j, s2)| {
            if t > s2.confirmed_at + cfg.max_bars_after_confirmation {
                return None;
            }
            known[..j].iter().rev().find_map(|s1| {
                let gap = s2.index - s1.index;
                if gap < cfg.min_bars_between || gap > cfg.max_bars_between {
                    return None;
                }
                if (s1.price - s2.price).abs() > cfg.max_difference * s1.price {
                    return None;
                }
                let between = &candles[s1.index + 1..s2.index];
                let neckline = match kind {
                    DoubleKind::Bottom => between.iter().map(|b| b.high).max()?,
                    DoubleKind::Top => between.iter().map(|b| b.low).min()?,
                };
                let height = match kind {
                    DoubleKind::Bottom => neckline - s1.price.min(s2.price),
                    DoubleKind::Top => s1.price.max(s2.price) - neckline,
                };
                let crossed = match kind {
                    DoubleKind::Bottom => c.close > neckline && prev.close <= neckline,
                    DoubleKind::Top => c.close < neckline && prev.close >= neckline,
                };
                (height >= cfg.min_height_atr * *atr_t && crossed).then_some(DoublePattern {
                    kind,
                    first: **s1,
                    second: **s2,
                    neckline,
                })
            })
        });
        out.extend(found);
    }
    out
}

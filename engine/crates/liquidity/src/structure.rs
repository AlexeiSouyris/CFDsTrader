//! Rupturas de estructura (spec §5.4, contexto).
//!
//! ```text
//! ruptura alcista en i: C[i] > P y C[i−1] ≤ P, con P = último pivote alto de 15m
//!                       confirmado en i
//!   BOS   si la tendencia de 1h en i es ALCISTA   (continuación)
//!   CHoCH si la tendencia de 1h en i es BAJISTA   (posible giro)
//!   con tendencia NEUTRAL no es ninguna de las dos
//! ruptura bajista: el espejo con pivotes bajos.
//! Cada pivote se rompe una sola vez: si el precio vuelve y lo cruza de nuevo, no es otra ruptura.
//! ```
//! `trend` está alineada con las velas de 15m: en cada vela, la tendencia de la última vela
//! de 1h **cerrada** (la arma quien llama, con `cfds_indicators::trend`).

use cfds_core::Candle;
use cfds_indicators::Trend;
use cfds_patterns::{Pivot, PivotKind};
use rust_decimal::Decimal;

use crate::{LiquidityConfig, Side};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakKind {
    Bos,
    Choch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructureBreak {
    pub index: usize,
    pub side: Side,
    pub kind: BreakKind,
    /// Precio del pivote roto.
    pub level: Decimal,
    /// Vela del pivote roto.
    pub pivot_index: usize,
}

fn last_known(pivots: &[Pivot], kind: PivotKind, i: usize) -> Option<&Pivot> {
    pivots
        .iter()
        .filter(|p| p.kind == kind && p.confirmed_at <= i)
        .max_by_key(|p| p.index)
}

/// Todas las rupturas de estructura de la serie.
pub fn structure_breaks(
    candles: &[Candle],
    pivots: &[Pivot],
    trend: &[Option<Trend>],
) -> Vec<StructureBreak> {
    let mut out = Vec::new();
    let (mut broken_high, mut broken_low) = (None, None);
    for i in 1..candles.len() {
        let (prev, c) = (candles[i - 1].close, candles[i].close);
        let t = trend.get(i).copied().flatten();
        if let Some(p) = last_known(pivots, PivotKind::High, i)
            && broken_high != Some(p.index)
            && c > p.price
            && prev <= p.price
        {
            broken_high = Some(p.index);
            let kind = match t {
                Some(Trend::Up) => Some(BreakKind::Bos),
                Some(Trend::Down) => Some(BreakKind::Choch),
                _ => None,
            };
            out.extend(kind.map(|kind| StructureBreak {
                index: i,
                side: Side::Long,
                kind,
                level: p.price,
                pivot_index: p.index,
            }));
        }
        if let Some(p) = last_known(pivots, PivotKind::Low, i)
            && broken_low != Some(p.index)
            && c < p.price
            && prev >= p.price
        {
            broken_low = Some(p.index);
            let kind = match t {
                Some(Trend::Down) => Some(BreakKind::Bos),
                Some(Trend::Up) => Some(BreakKind::Choch),
                _ => None,
            };
            out.extend(kind.map(|kind| StructureBreak {
                index: i,
                side: Side::Short,
                kind,
                level: p.price,
                pivot_index: p.index,
            }));
        }
    }
    out
}

/// `true` si hubo un CHoCH de ese lado en las últimas `choch_recent_bars` velas hasta `t`.
pub fn recent_choch(
    breaks: &[StructureBreak],
    t: usize,
    side: Side,
    cfg: &LiquidityConfig,
) -> bool {
    let from = (t + 1).saturating_sub(cfg.choch_recent_bars);
    breaks
        .iter()
        .any(|b| b.kind == BreakKind::Choch && b.side == side && b.index >= from && b.index <= t)
}

//! Order blocks (spec §5.4, contexto).
//!
//! ```text
//! Para cada ruptura alcista en b (BOS o CHoCH) del pivote alto de la vela p:
//!   impulso k = última vela alcista en (p, b] con cuerpo ≥ m · ATR[k]       (m = 1,5)
//!   OB        = última vela bajista antes de k; zona [L, H] de esa vela
//!   vigente desde b (cuando se conoce la ruptura) hasta que una vela cierre por debajo de L
//!   toque en t > b:  L[t] ≤ H_OB y C[t] ≥ L_OB
//! OB bajista: el espejo.
//! ```
//! Decisión del agente: la spec dice "el movimiento que produce un BOS"; acá cuenta cualquier
//! ruptura (BOS o CHoCH), porque en los dos casos el precio rompió estructura con un impulso.

use cfds_core::Candle;
use rust_decimal::Decimal;

use crate::structure::StructureBreak;
use crate::{LiquidityConfig, Side};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderBlock {
    pub side: Side,
    pub candle_index: usize,
    pub low: Decimal,
    pub high: Decimal,
    /// Vela de la ruptura: el OB existe para el sistema recién desde su cierre.
    pub known_at: usize,
}

/// Order blocks de todas las rupturas. `atr` alineado con `candles`.
pub fn order_blocks(
    candles: &[Candle],
    breaks: &[StructureBreak],
    atr: &[Option<Decimal>],
    cfg: &LiquidityConfig,
) -> Vec<OrderBlock> {
    let body = |c: &Candle| (c.close - c.open).abs();
    breaks
        .iter()
        .filter_map(|b| {
            let bullish = |c: &Candle| c.close > c.open;
            let impulse_side = |c: &Candle| match b.side {
                Side::Long => bullish(c),
                Side::Short => c.close < c.open,
            };
            let k = (b.pivot_index + 1..=b.index).rev().find(|&k| {
                let c = &candles[k];
                impulse_side(c)
                    && atr
                        .get(k)
                        .copied()
                        .flatten()
                        .is_some_and(|a| body(c) >= cfg.min_impulse_body_atr * a)
            })?;
            let ob = (0..k).rev().find(|&j| {
                let c = &candles[j];
                match b.side {
                    Side::Long => c.close < c.open,
                    Side::Short => bullish(c),
                }
            })?;
            Some(OrderBlock {
                side: b.side,
                candle_index: ob,
                low: candles[ob].low,
                high: candles[ob].high,
                known_at: b.index,
            })
        })
        .collect()
}

/// Order blocks vigentes que la vela `t` toca.
pub fn order_block_touches_at(candles: &[Candle], t: usize, obs: &[OrderBlock]) -> Vec<OrderBlock> {
    let Some(c) = candles.get(t) else {
        return Vec::new();
    };
    obs.iter()
        .filter(|ob| ob.known_at < t)
        .filter(|ob| {
            // Invalidado si alguna vela posterior a la ruptura (incluida t) cerró del otro lado.
            !candles[ob.known_at + 1..=t].iter().any(|x| match ob.side {
                Side::Long => x.close < ob.low,
                Side::Short => x.close > ob.high,
            })
        })
        .filter(|ob| match ob.side {
            Side::Long => c.low <= ob.high && c.close >= ob.low,
            Side::Short => c.high >= ob.low && c.close <= ob.high,
        })
        .copied()
        .collect()
}

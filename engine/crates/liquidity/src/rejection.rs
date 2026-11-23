//! Rechazo, estado emocional de la spec §9.1 (#220). **No es un detector nuevo:** reúne los que
//! ya existen, evaluados en la vela cerrada `t` (15m o 1h).
//!
//! ```text
//! rechazo largo = martillo (§5.1) cuya mecha inferior toca
//!                   un soporte (§5.3: L ≤ nivel·(1 + tol) y C > nivel),
//!                   un FVG alcista o un order block alcista (§5.4: L ≤ techo y C ≥ piso)
//!               o un sweep largo (§5.4: pinchó el último pivote bajo del timeframe de
//!                 referencia y cerró de vuelta adentro)
//! rechazo corto = el espejo (estrella fugaz, resistencia, FVG y OB bajistas, sweep corto)
//! ```
//! - Para velas de 15m, el sweep es contra los pivotes de 1h, como en §5.4. Para velas de 1h,
//!   contra los pivotes de 1h de la misma serie.
//! - Sin repintado: todo lo que se usa está confirmado o cerrado al cierre de `t`.
//! - Informativo: no suma al puntaje hasta validarse en la fase 5 y votarse.

use cfds_core::Candle;
use cfds_patterns::{
    CandlePattern, LevelKind, PatternsConfig, Pivot, candle_patterns_at, levels_at, rejections_at,
};
use rust_decimal::Decimal;

use crate::{LiquidityConfig, OrderBlock, Side, fvg_touches_at, order_block_touches_at, sweeps_at};

/// Por qué hubo rechazo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RejectionVia {
    Level,
    Fvg,
    OrderBlock,
    Sweep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rejection {
    pub side: Side,
    pub via: RejectionVia,
}

/// Lo que ya calcularon los detectores para la serie de `candles` (15m o 1h).
#[derive(Debug, Clone, Copy)]
pub struct RejectionInputs<'a> {
    pub candles: &'a [Candle],
    /// ATR de `candles` (el de los patrones de velas), alineado por índice.
    pub atr: &'a [Option<Decimal>],
    /// Pivotes confirmados de `candles` (soportes y resistencias).
    pub pivots: &'a [Pivot],
    pub order_blocks: &'a [OrderBlock],
    /// Serie de 1h y sus pivotes confirmados, para el sweep.
    pub h1: &'a [Candle],
    pub h1_pivots: &'a [Pivot],
}

/// Rechazos de la vela `t`, sin repetidos y en un orden fijo (largos primero).
pub fn rejection_state_at(
    inp: &RejectionInputs<'_>,
    t: usize,
    patterns: &PatternsConfig,
    liq: &LiquidityConfig,
) -> Vec<Rejection> {
    let Some(c) = inp.candles.get(t) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for pat in candle_patterns_at(inp.candles, t, inp.atr, &patterns.candles) {
        let (side, level) = match pat {
            CandlePattern::Hammer => (Side::Long, LevelKind::Support),
            CandlePattern::ShootingStar => (Side::Short, LevelKind::Resistance),
            _ => continue,
        };
        let levels = levels_at(t, inp.pivots, &patterns.levels);
        if rejections_at(inp.candles, t, &levels, &patterns.levels)
            .iter()
            .any(|l| l.kind == level)
        {
            out.push(Rejection {
                side,
                via: RejectionVia::Level,
            });
        }
        if fvg_touches_at(inp.candles, t, liq)
            .iter()
            .any(|f| f.side == side)
        {
            out.push(Rejection {
                side,
                via: RejectionVia::Fvg,
            });
        }
        if order_block_touches_at(inp.candles, t, inp.order_blocks)
            .iter()
            .any(|o| o.side == side)
        {
            out.push(Rejection {
                side,
                via: RejectionVia::OrderBlock,
            });
        }
    }
    for s in sweeps_at(c, inp.h1, inp.h1_pivots) {
        out.push(Rejection {
            side: s.side,
            via: RejectionVia::Sweep,
        });
    }
    out.sort_by_key(|r| (r.side != Side::Long, r.via));
    out.dedup();
    out
}

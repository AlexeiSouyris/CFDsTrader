//! Programa 5 (#649, `docs/analysis/2026-11-programa-5-fvg-preregistro.md`): reacción al primer
//! toque de un FVG. Este módulo **no define** el FVG, el toque ni el llenado: recibe los eventos
//! que ya calculó el motor (`cfds_liquidity::stats::fvg_events`, spec §5.4.1) y solo elige cuáles
//! entran al programa (filtros F1 a F3 de §3) y etiqueta las operaciones de la estrategia (§5.2).
//!
//! ```text
//! evento del motor (creado en i, primer toque en t = i + touch.bars)
//!   F3: t empieza en [desde, hasta) y t + h existe y empieza antes de `hasta`
//!   F1: velas i − 2 … t + h del mismo contrato
//!   F2: el toque no llena en la misma vela (fill ≠ touch)
//!   → evento real (F4 y F5: sin tamaño mínimo, espera desde 1 vela, los del motor)
//! operación con decisión al cierre de t y lado L es "F" si hay un evento real de la misma
//! dirección con toque en t (se conoce al cierre de t: no mira el futuro)
//! ```

use std::collections::BTreeSet;

use cfds_core::{Candle, Direction, FvgEvent};
use cfds_strategy::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// Un evento real (§3) con sus índices en la serie con la que se calculó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealEvent {
    pub side: Side,
    /// Vela que crea el FVG (`i`).
    pub created: usize,
    /// Vela del primer toque (`t`).
    pub touch: usize,
    pub bottom: Decimal,
    pub top: Decimal,
    /// ATR de la vela de creación (el del motor).
    pub atr: Decimal,
    /// Tamaño en ATR (el del motor).
    pub size_atr: Decimal,
    /// Reacciones del motor en ATR (`[fvg_stats] reaction_bars`), en el mismo orden.
    pub reactions: Vec<(u32, Decimal)>,
}

impl RealEvent {
    /// Espera `a(e) = t − i`.
    pub fn wait(&self) -> usize {
        self.touch - self.created
    }

    /// Reacción del motor a `k` velas, si el motor la calculó.
    pub fn reaction(&self, k: u32) -> Option<Decimal> {
        self.reactions
            .iter()
            .find(|(b, _)| *b == k)
            .map(|(_, r)| *r)
    }

    /// Distancia de llegada `d(e)` en ATR[i] (§4.1): desde el cierre de `t − 1` hasta el borde
    /// cercano de la zona. Nunca negativa salvo un error de datos.
    pub fn arrival_atr(&self, bars: &[Candle]) -> Option<Decimal> {
        let prev = bars.get(self.touch.checked_sub(1)?)?.close;
        let d = match self.side {
            Side::Long => prev - self.top,
            Side::Short => self.bottom - prev,
        };
        Some(d / self.atr)
    }
}

/// Cuántos eventos quedan en cada paso de los filtros (§7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EventCounts {
    /// Primeros toques dentro del período, con sus `h` velas (F3).
    pub in_period: usize,
    /// Además, sin vencimiento de por medio (F1).
    pub same_contract: usize,
    /// Además, el toque no llena (F2): los eventos reales.
    pub real: usize,
}

fn side(d: Direction) -> Side {
    match d {
        Direction::Bullish => Side::Long,
        Direction::Bearish => Side::Short,
    }
}

/// Eventos reales de `events` (los de `fvg_events` sobre `bars`, un instrumento y timeframe),
/// con la vela del toque en `[from, to)` y `horizon` velas de reacción que empiezan antes de `to`.
/// `contracts[j]` es el contrato de `bars[j]`. Orden: hora del toque, luego vela de creación y
/// dirección (alcista primero).
pub fn real_events(
    bars: &[Candle],
    contracts: &[u64],
    events: &[FvgEvent],
    (from, to): (DateTime<Utc>, DateTime<Utc>),
    horizon: usize,
) -> (Vec<RealEvent>, EventCounts) {
    let mut out = Vec::new();
    let mut n = EventCounts::default();
    if contracts.len() != bars.len() {
        return (out, n);
    }
    for e in events {
        let Some(touch) = &e.touch else { continue };
        let i = bars.partition_point(|c| c.end() < e.created_at);
        if bars.get(i).is_none_or(|c| c.end() != e.created_at) {
            continue;
        }
        let t = i + touch.bars.get() as usize;
        let Some(last) = bars.get(t + horizon) else {
            continue;
        };
        if bars[t].start < from || last.start >= to {
            continue;
        }
        n.in_period += 1;
        let Some(first) = i.checked_sub(2) else {
            continue;
        };
        let c = contracts[first];
        if contracts[first..=t + horizon].iter().any(|x| *x != c) {
            continue;
        }
        n.same_contract += 1;
        if e.fill.is_some_and(|f| f.bars == touch.bars) {
            continue;
        }
        n.real += 1;
        out.push(RealEvent {
            side: side(e.direction),
            created: i,
            touch: t,
            bottom: e.bottom,
            top: e.top,
            atr: e.atr,
            size_atr: e.size_atr,
            reactions: e
                .reactions
                .iter()
                .filter_map(|r| Some((r.bars.get(), r.return_atr?)))
                .collect(),
        });
    }
    out.sort_by_key(|x| (x.touch, x.created, x.side == Side::Short));
    (out, n)
}

/// Cierres de las velas de toque de los eventos reales, con su dirección (la etiqueta de §5.2).
pub fn touch_closes(bars: &[Candle], events: &[RealEvent]) -> BTreeSet<(DateTime<Utc>, bool)> {
    events
        .iter()
        .map(|e| (bars[e.touch].end(), e.side == Side::Long))
        .collect()
}

/// `true` si una operación de lado `side` decidida al cierre `signal_at` es "F" (§5.2).
pub fn is_fvg_trade(
    touches: &BTreeSet<(DateTime<Utc>, bool)>,
    signal_at: DateTime<Utc>,
    side: Side,
) -> bool {
    touches.contains(&(signal_at, side == Side::Long))
}

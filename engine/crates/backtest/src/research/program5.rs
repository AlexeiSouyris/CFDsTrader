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
//! placebo de e (§4.1 y §13.3.3): vela s del mismo mes UTC y franja que t, con
//!   P = C[s−1] − d(e)·ATR[s−1], B = P − w(e)·ATR[s−1] (alcista; bajista: espejo)
//!   L[s] ≤ P, C[s] ≥ B, L > P en s − a + 1 … s − 1, sin evento real de la misma dirección en s,
//!   s − a − 2 … s + h del mismo contrato, ATR[s−1] > 0, s + h antes de `hasta`
//! ```

use std::collections::{BTreeMap, BTreeSet};

use cfds_core::{Candle, Direction, FvgEvent};
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, Utc};
use rust_decimal::Decimal;

use super::program4::SplitMix64;

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

/// `true` si la vela está entera dentro de la sesión regular de su día (la franja "en contado",
/// §13.3.2). La misma regla que comprar y mantener.
pub fn in_cash_session(session: &Session, c: &Candle) -> bool {
    super::buy_and_hold::in_session(session, c)
}

/// Mes UTC del inicio de una vela.
pub fn month_of(c: &Candle) -> (i32, u32) {
    (c.start.year(), c.start.month())
}

/// Mínimos de los bajos y máximos de los altos por tramos (tabla dispersa), para saber en O(1)
/// si ninguna vela de un tramo llegó a un nivel.
struct Extremes {
    lows: Vec<Vec<Decimal>>,
    highs: Vec<Vec<Decimal>>,
}

impl Extremes {
    fn new(bars: &[Candle], max_len: usize) -> Self {
        let mut lows = vec![bars.iter().map(|c| c.low).collect::<Vec<_>>()];
        let mut highs = vec![bars.iter().map(|c| c.high).collect::<Vec<_>>()];
        let mut w = 1;
        while 2 * w <= max_len.max(1) {
            let (pl, ph) = (&lows[lows.len() - 1], &highs[highs.len() - 1]);
            let n = pl.len().saturating_sub(w);
            let nl: Vec<Decimal> = (0..n).map(|i| pl[i].min(pl[i + w])).collect();
            let nh: Vec<Decimal> = (0..n).map(|i| ph[i].max(ph[i + w])).collect();
            lows.push(nl);
            highs.push(nh);
            w *= 2;
        }
        Self { lows, highs }
    }

    fn level(len: usize) -> usize {
        (usize::BITS - 1 - len.leading_zeros()) as usize
    }

    /// Mínimo de los bajos de `[a, b]` (inclusive, `a ≤ b`, largo ≤ `max_len`).
    fn min_low(&self, a: usize, b: usize) -> Decimal {
        let k = Self::level(b - a + 1);
        self.lows[k][a].min(self.lows[k][b + 1 - (1 << k)])
    }

    fn max_high(&self, a: usize, b: usize) -> Decimal {
        let k = Self::level(b - a + 1);
        self.highs[k][a].max(self.highs[k][b + 1 - (1 << k)])
    }
}

/// Lo que necesita el sorteo del placebo, alineado con `bars`.
pub struct PlaceboInputs<'a> {
    pub bars: &'a [Candle],
    pub contracts: &'a [u64],
    /// ATR de Wilder (el mismo `wilder_atr` del motor) de cada vela.
    pub atr: &'a [Option<Decimal>],
    /// Franja de cada vela (`in_cash_session`).
    pub cash: &'a [bool],
    /// Inicio del fuera de muestra: `s + horizon` tiene que empezar antes.
    pub to: DateTime<Utc>,
    pub horizon: usize,
}

/// Velas candidatas a placebo de cada evento (en el orden de `events`): las de `e` son
/// `candidates[offsets[e]..offsets[e + 1]]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Placebos {
    pub offsets: Vec<usize>,
    pub candidates: Vec<u32>,
}

impl Placebos {
    pub fn of(&self, e: usize) -> &[u32] {
        &self.candidates[self.offsets[e]..self.offsets[e + 1]]
    }
}

/// Primer índice de cada tramo de contrato y último, por vela.
fn contract_runs(contracts: &[u64]) -> (Vec<usize>, Vec<usize>) {
    let n = contracts.len();
    let mut start = vec![0; n];
    let mut end = vec![0; n];
    for j in 1..n {
        start[j] = if contracts[j] == contracts[j - 1] {
            start[j - 1]
        } else {
            j
        };
    }
    for j in (0..n).rev() {
        end[j] = if j + 1 < n && contracts[j] == contracts[j + 1] {
            end[j + 1]
        } else {
            j
        };
    }
    (start, end)
}

/// Candidatas a placebo de cada evento real (§4.1 y §13.3.3). Función pura: misma entrada,
/// misma salida.
pub fn placebo_candidates(inp: &PlaceboInputs, events: &[RealEvent]) -> Placebos {
    let bars = inp.bars;
    let n = bars.len();
    let mut out = Placebos {
        offsets: vec![0],
        candidates: Vec::new(),
    };
    if inp.contracts.len() != n || inp.atr.len() != n || inp.cash.len() != n {
        out.offsets.resize(events.len() + 1, 0);
        return out;
    }
    let max_wait = events.iter().map(RealEvent::wait).max().unwrap_or(1);
    let ext = Extremes::new(bars, max_wait);
    let (run_start, run_end) = contract_runs(inp.contracts);
    // Velas que son el toque de un evento real, por dirección (condición 3, lectura literal).
    let mut real = vec![[false; 2]; n];
    for e in events {
        real[e.touch][usize::from(e.side == Side::Short)] = true;
    }
    // Velas que pueden ser placebo de algún evento, por mes y franja.
    let mut buckets: BTreeMap<((i32, u32), bool), Vec<usize>> = BTreeMap::new();
    for s in 1..n.saturating_sub(inp.horizon) {
        if bars[s + inp.horizon].start >= inp.to || run_end[s] < s + inp.horizon {
            continue;
        }
        if inp.atr[s - 1].is_none_or(|a| a <= Decimal::ZERO) {
            continue;
        }
        buckets
            .entry((month_of(&bars[s]), inp.cash[s]))
            .or_default()
            .push(s);
    }
    for e in events {
        let t = e.touch;
        let a = e.wait();
        let d = e.arrival_atr(bars).unwrap_or(Decimal::ZERO);
        let w = e.size_atr;
        let dir = usize::from(e.side == Side::Short);
        if let Some(list) = buckets.get(&(month_of(&bars[t]), inp.cash[t])) {
            for &s in list {
                let Some(first) = s.checked_sub(a + 2) else {
                    continue;
                };
                if run_start[s] > first || real[s][dir] {
                    continue;
                }
                let Some(atr) = inp.atr[s - 1] else { continue };
                let prev = bars[s - 1].close;
                let c = &bars[s];
                let ok = match e.side {
                    Side::Long => {
                        let p = prev - d * atr;
                        let b = p - w * atr;
                        c.low <= p && c.close >= b && (a < 2 || ext.min_low(s + 1 - a, s - 1) > p)
                    }
                    Side::Short => {
                        let p = prev + d * atr;
                        let b = p + w * atr;
                        c.high >= p && c.close <= b && (a < 2 || ext.max_high(s + 1 - a, s - 1) < p)
                    }
                };
                if ok {
                    out.candidates.push(u32::try_from(s).unwrap_or(u32::MAX));
                }
            }
        }
        out.offsets.push(out.candidates.len());
    }
    out
}

/// Sortea `k` placebos con reposición para cada evento con candidatas, en orden (§13.3.4). Los
/// eventos sin candidatas no consumen números. Devuelve las velas sorteadas de cada evento.
pub fn draw_placebos(rng: &mut SplitMix64, placebos: &Placebos, k: usize) -> Vec<Vec<u32>> {
    (0..placebos.offsets.len().saturating_sub(1))
        .map(|e| {
            let c = placebos.of(e);
            if c.is_empty() {
                Vec::new()
            } else {
                (0..k).map(|_| c[rng.below(c.len())]).collect()
            }
        })
        .collect()
}

/// Reacción a `k` velas desde la vela `s`, a favor de `side`, en ATR de `s − 1` (la del placebo,
/// §4.1).
pub fn placebo_reaction(
    bars: &[Candle],
    atr: &[Option<Decimal>],
    s: usize,
    k: usize,
    side: Side,
) -> Option<Decimal> {
    let a = atr.get(s.checked_sub(1)?).copied().flatten()?;
    if a <= Decimal::ZERO {
        return None;
    }
    let r = (bars.get(s + k)?.close - bars.get(s)?.close) / a;
    Some(match side {
        Side::Long => r,
        Side::Short => -r,
    })
}

/// Sorteo sin reposición de `m` de `n` índices (Fisher-Yates parcial, §13.3.12).
pub fn sample_without_replacement(rng: &mut SplitMix64, n: usize, m: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..n).collect();
    let m = m.min(n);
    for k in 0..m {
        let j = k + rng.below(n - k);
        idx.swap(k, j);
    }
    idx.truncate(m);
    idx
}

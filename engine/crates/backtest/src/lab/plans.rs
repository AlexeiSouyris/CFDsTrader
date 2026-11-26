//! `plans_from_spec`: una spec válida → los `Plan` que ya ejecuta `research::run_plans`.
//!
//! ```text
//! señal en t (velas del timeframe de la spec, solo cerradas):
//!   cada condición de entrada da un lado o nada en t; entra si TODAS dan el mismo lado ("y")
//!   ma_cross: la media rápida cruza la lenta al cierre de t (cfds_indicators::crosses)
//!   opening_range_breakout: la ruptura del día (research::orb_plans, solo 15m)
//!   dirección: solo largos, solo cortos o los dos
//!   filtro de horas: el cierre de t (la decisión) en [desde, hasta) UTC
//! decisión: la vela de 15m que cierra junto con t, con la decisión permitida (marcas de entrada
//!   y vela siguiente sin hueco); entrada en la apertura de la siguiente (t + 1)
//! stop: cierre de t ∓ k × ATR(período) de Wilder del timeframe de la señal (el mismo ATR mide el
//!   riesgo); sin ATR (calentamiento) no hay plan
//! salida planificada (la primera): cierre de la vela t + N del timeframe de la señal y/o la
//!   última vela de 15m de la sesión de contado de la entrada (si la entrada no cae dentro de
//!   esa sesión, no hay plan); objetivo en R si la spec lo tiene
//! ```
//! Función pura: los planes de una vela solo usan velas hasta su salida planificada, que se
//! conoce de antemano (cantidad de velas o calendario). Cortar la serie no cambia los planes
//! anteriores (test de propiedad).

use cfds_core::{Candle, Timeframe};
use cfds_indicators::{CrossKind, crosses, ema, sma, wilder_atr};
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use super::spec::Direction;
use super::validation::{Average, EntryCondition, EntryFilter, StopRule, ValidSpec};
use crate::research::{OrbConfig, Plan, decision_allowed, orb_plans};

/// Las velas y marcas de la serie, como las arma la preparación de las rondas.
#[derive(Debug, Clone, Copy)]
pub struct LabSeries<'a> {
    /// Velas de 15m: decisión, ejecución, stop y salidas.
    pub m15: &'a [Candle],
    /// Velas de 1h cerradas (solo se usan con timeframe 1h).
    pub h1: &'a [Candle],
    pub session: &'a Session,
    /// Marcas de entrada de la preparación, alineadas con `m15`.
    pub entries: &'a [bool],
}

/// Índice de la vela de `candles` que termina exactamente en `end`.
fn index_ending(candles: &[Candle], end: DateTime<Utc>) -> Option<usize> {
    let i = candles.partition_point(|c| c.end() < end);
    (candles.get(i)?.end() == end).then_some(i)
}

/// Lado de cada condición en cada vela de la señal.
fn signals(
    cond: &EntryCondition,
    candles: &[Candle],
    s: &LabSeries,
    atr: &[Option<Decimal>],
) -> Vec<Option<Side>> {
    let mut out = vec![None; candles.len()];
    match *cond {
        EntryCondition::MaCross {
            average,
            fast,
            slow,
        } => {
            let ma = |p| match average {
                Average::Sma => sma(candles, p),
                Average::Ema => ema(candles, p),
            };
            for c in crosses(&ma(fast), &ma(slow)) {
                out[c.index] = Some(match c.kind {
                    CrossKind::Golden => Side::Long,
                    CrossKind::Death => Side::Short,
                });
            }
        }
        EntryCondition::OpeningRangeBreakout {
            range_candles,
            last_breakout_close,
        } => {
            let cfg = OrbConfig {
                range_candles,
                last_breakout_close,
            };
            for p in orb_plans(candles, s.session, s.entries, atr, &cfg) {
                out[p.signal_index] = Some(p.side);
            }
        }
    }
    out
}

/// Salida planificada de una decisión en la vela de 15m `d` (señal en la vela `t` del timeframe):
/// la primera entre la vela `t + N` y el cierre de la sesión. Una salida que cae después de la
/// última vela todavía no se conoce: si es la única, no hay plan (así cortar la serie no cambia
/// los planes anteriores); un hueco justo en la vela de salida tampoco da plan.
fn planned_exit(
    spec: &ValidSpec,
    candles: &[Candle],
    s: &LabSeries,
    t: usize,
    d: usize,
) -> Option<usize> {
    const LATER: usize = usize::MAX;
    let last_end = s.m15.last()?.end();
    let at = |end: DateTime<Utc>| match index_ending(s.m15, end) {
        Some(i) => Some(i),
        None if end > last_end => Some(LATER),
        None => None,
    };
    let by_time = match spec.exit.max_candles {
        Some(n) => Some(match candles.get(t + n) {
            Some(c) => at(c.end())?,
            None => LATER,
        }),
        None => None,
    };
    let by_session = if spec.exit.session_close {
        let entry = s.m15.get(d + 1)?;
        let (open, close) = s.session.hours(s.session.trading_day(entry.start))??;
        if entry.start < open || entry.start >= close {
            return None;
        }
        Some(at(close)?)
    } else {
        None
    };
    let exit = by_time.into_iter().chain(by_session).min()?;
    (exit > d && exit != LATER).then_some(exit)
}

/// Los planes de `spec` sobre la serie (ver el módulo). Cada plan es su propio evento.
pub fn plans_from_spec(spec: &ValidSpec, s: &LabSeries) -> Vec<Plan> {
    let candles = match spec.timeframe {
        Timeframe::M15 => s.m15,
        Timeframe::H1 => s.h1,
    };
    let StopRule::Atr { period, k } = spec.stop;
    let atr = wilder_atr(candles, period);
    let per_condition: Vec<Vec<Option<Side>>> = spec
        .entry
        .iter()
        .map(|c| signals(c, candles, s, &atr))
        .collect();
    let mut out = Vec::new();
    for (t, c) in candles.iter().enumerate() {
        let mut sides = per_condition.iter().map(|sig| sig[t]);
        let Some(Some(side)) = sides.next() else {
            continue;
        };
        if !sides.all(|x| x == Some(side)) {
            continue;
        }
        let allowed = match spec.spec.direction {
            Direction::Long => side == Side::Long,
            Direction::Short => side == Side::Short,
            Direction::Both => true,
        };
        let at = c.end().time();
        let hours_ok = spec.filters.iter().all(|f| match *f {
            EntryFilter::EntryHoursUtc { from, to } if from <= to => from <= at && at < to,
            EntryFilter::EntryHoursUtc { from, to } => at >= from || at < to,
        });
        if !allowed || !hours_ok {
            continue;
        }
        let Some(d) = index_ending(s.m15, c.end()) else {
            continue;
        };
        let Some(risk_atr) = atr[t].filter(|a| *a > Decimal::ZERO) else {
            continue;
        };
        if !decision_allowed(s.m15, s.entries, d) {
            continue;
        }
        let Some(exit_index) = planned_exit(spec, candles, s, t, d) else {
            continue;
        };
        let reference = c.close;
        let distance = k * risk_atr;
        out.push(Plan {
            event: out.len(),
            signal_index: d,
            side,
            reference,
            stop: match side {
                Side::Long => reference - distance,
                Side::Short => reference + distance,
            },
            risk_atr,
            exit_index,
            target_r: spec.exit.target_r,
            target_cap: None,
        });
    }
    out
}

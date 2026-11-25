//! Programa 4 (#561): estrategia A de Gianluca Bruni en el CFD US100, con las definiciones
//! congeladas en `docs/analysis/2026-11-programa-4-preregistro.md` (§2 a §5, §13).
//!
//! ```text
//! vela válida      cuerpo > mecha más larga; alcista si cierre > apertura, bajista si <
//! zona             empieza en una vela (techo = máximo, piso = mínimo); cada vela siguiente:
//!                    ruptura alcista si la zona tiene ≥ 3 válidas bajistas y el cierre > techo
//!                    ruptura bajista si la zona tiene ≥ 3 válidas alcistas y el cierre < piso
//!                    con ruptura, una zona nueva empieza en esa vela; sin ruptura, se estira
//! estructura       actual = dirección de la última ruptura; periférica = la de la anterior
//! vencimiento      reinicia zonas, rupturas y ATR (tramo de contrato nuevo)
//! señal en t       t−1 y t consecutivas, del mismo color, válidas, cuerpo ≥ umbral
//!                    (H4.1: puntos; H4.2: k × ATR de t−2), estructura actual y periférica de
//!                    ese lado, decisión permitida, ejecución en t+1 dentro de la sesión y antes
//!                    del cierre − 30 min, día sin roll ni dato fuerte
//! operación        entrada en la apertura de t+1, stop en el extremo opuesto de t−1,
//!                    objetivo 1,5 R, salida a la apertura de la última vela de la sesión
//! ```
//! Funciones puras sobre velas cerradas: lo calculado para una vela no cambia con velas nuevas.

use std::collections::{BTreeMap, BTreeSet};

use cfds_core::Candle;
use cfds_indicators::wilder_atr;
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::{Plan, Program4Config, StudyTrade, decision_allowed, simulate_leg};
use crate::{Execution, Setup, fill_price};

/// Cuerpo de la vela: |cierre − apertura|.
pub fn body(c: &Candle) -> Decimal {
    (c.close - c.open).abs()
}

/// Dirección de la vela: alcista (largo), bajista (corto) o ninguna si abre y cierra igual.
pub fn direction(c: &Candle) -> Option<Side> {
    if c.close > c.open {
        Some(Side::Long)
    } else if c.close < c.open {
        Some(Side::Short)
    } else {
        None
    }
}

/// Vela válida: cuerpo estrictamente mayor que la mecha más larga.
pub fn is_valid(c: &Candle) -> bool {
    let upper = c.high - c.open.max(c.close);
    let lower = c.open.min(c.close) - c.low;
    body(c) > upper.max(lower)
}

/// Inicio del tramo de contrato de cada vela: `true` en la primera vela y en la primera vela
/// en o después de cada vencimiento de `rolls`.
fn segment_starts(m15: &[Candle], rolls: &[DateTime<Utc>]) -> Vec<bool> {
    let mut out = vec![false; m15.len()];
    if let Some(first) = out.first_mut() {
        *first = true;
    }
    for &r in rolls {
        let k = m15.partition_point(|c| c.start < r);
        if let Some(x) = out.get_mut(k) {
            *x = true;
        }
    }
    out
}

/// ATR de Wilder de `period` calculado por tramo de contrato: se reinicia en cada vencimiento,
/// con su calentamiento (§2.5).
pub fn segment_atr(m15: &[Candle], rolls: &[DateTime<Utc>], period: usize) -> Vec<Option<Decimal>> {
    let starts = segment_starts(m15, rolls);
    let mut out = Vec::with_capacity(m15.len());
    let mut from = 0;
    for i in 1..=m15.len() {
        if i == m15.len() || starts[i] {
            out.extend(wilder_atr(&m15[from..i], period));
            from = i;
        }
    }
    out
}

/// Una ruptura de zona (§3): en la vela `index`, con el techo y el piso de la zona rota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breakout {
    pub index: usize,
    pub side: Side,
    pub high: Decimal,
    pub low: Decimal,
}

/// Estructura al cierre de una vela: dirección de la última ruptura (actual) y de la anterior
/// (periférica) desde el último reinicio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Structure {
    pub current: Option<Side>,
    pub peripheral: Option<Side>,
}

impl Structure {
    /// Actual y periférica existen y son de `side`.
    pub fn aligned(&self, side: Side) -> bool {
        self.current == Some(side) && self.peripheral == Some(side)
    }
}

struct Zone {
    high: Decimal,
    low: Decimal,
    bull: usize,
    bear: usize,
}

impl Zone {
    fn start(c: &Candle) -> Self {
        let mut z = Self {
            high: c.high,
            low: c.low,
            bull: 0,
            bear: 0,
        };
        z.count(c);
        z
    }

    fn count(&mut self, c: &Candle) {
        if is_valid(c) {
            match direction(c) {
                Some(Side::Long) => self.bull += 1,
                Some(Side::Short) => self.bear += 1,
                None => {}
            }
        }
    }
}

/// Rupturas de zona y estructura al cierre de cada vela de `m15` (§3), reiniciadas en cada
/// vencimiento de `rolls`. `min_valid` = velas válidas del color contrario que necesita la zona.
pub fn structure(
    m15: &[Candle],
    rolls: &[DateTime<Utc>],
    min_valid: usize,
) -> (Vec<Breakout>, Vec<Structure>) {
    let starts = segment_starts(m15, rolls);
    let mut breakouts = Vec::new();
    let mut states = Vec::with_capacity(m15.len());
    let mut zone: Option<Zone> = None;
    let mut state = Structure::default();
    for (i, c) in m15.iter().enumerate() {
        if starts[i] {
            zone = None;
            state = Structure::default();
        }
        zone = Some(match zone {
            None => Zone::start(c),
            Some(mut z) => {
                let side = if z.bear >= min_valid && c.close > z.high {
                    Some(Side::Long)
                } else if z.bull >= min_valid && c.close < z.low {
                    Some(Side::Short)
                } else {
                    None
                };
                match side {
                    Some(side) => {
                        breakouts.push(Breakout {
                            index: i,
                            side,
                            high: z.high,
                            low: z.low,
                        });
                        state = Structure {
                            current: Some(side),
                            peripheral: state.current,
                        };
                        Zone::start(c)
                    }
                    None => {
                        z.high = z.high.max(c.high);
                        z.low = z.low.min(c.low);
                        z.count(c);
                        z
                    }
                }
            }
        });
        states.push(state);
    }
    (breakouts, states)
}

/// Día de roll de cada vencimiento (§13.1): la primera sesión de contado que abre en o después
/// del primer instante del contrato nuevo.
pub fn roll_days(session: &Session, rolls: &[DateTime<Utc>]) -> BTreeSet<NaiveDate> {
    let mut out = BTreeSet::new();
    for &r in rolls {
        let mut d = session.trading_day(r);
        for _ in 0..14 {
            if let Some(Some((open, _))) = session.hours(d)
                && open >= r
            {
                out.insert(d);
                break;
            }
            match d.succ_opt() {
                Some(n) => d = n,
                None => break,
            }
        }
    }
    out
}

/// Días con un comunicado fuerte (decisión 11): la hora programada cae dentro de la sesión de
/// contado de ese día.
pub fn release_days(session: &Session, scheduled: &[DateTime<Utc>]) -> BTreeSet<NaiveDate> {
    scheduled
        .iter()
        .filter_map(|&t| {
            let d = session.trading_day(t);
            let (open, close) = session.hours(d)??;
            (open <= t && t < close).then_some(d)
        })
        .collect()
}

/// Umbral del cuerpo de las velas del impulso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyThreshold {
    /// H4.1: puntos fijos.
    Points(Decimal),
    /// H4.2: k × ATR de la vela anterior a la primera del impulso.
    Atr(Decimal),
}

/// Si la vela `j` puede ser la de ejecución (§13.2): empieza dentro de la sesión de contado de
/// su día, antes del cierre menos `last_entry_minutes_before_close`, y su día no está
/// excluido. Devuelve la vela de la salida planificada (la última que termina en o antes del
/// cierre menos `exit_minutes_before_close`), que tiene que ser `j` o posterior.
pub fn execution_exit(
    m15: &[Candle],
    session: &Session,
    excluded: &BTreeSet<NaiveDate>,
    cfg: &Program4Config,
    j: usize,
) -> Option<usize> {
    let c = m15.get(j)?;
    let day = session.trading_day(c.start);
    if excluded.contains(&day) {
        return None;
    }
    let (open, close) = session.hours(day)??;
    let last_entry = close - TimeDelta::minutes(i64::from(cfg.last_entry_minutes_before_close));
    if c.start < open || c.start >= last_entry {
        return None;
    }
    let exit_at = close - TimeDelta::minutes(i64::from(cfg.exit_minutes_before_close));
    let exit_index = m15.partition_point(|x| x.end() <= exit_at).checked_sub(1)?;
    (exit_index >= j).then_some(exit_index)
}

/// Señales de la estrategia A (H4.1 o H4.2 según `threshold`). `entries` son las marcas de
/// entrada de la preparación, `atr` el de [`segment_atr`], `states` el de [`structure`] y
/// `excluded` los días sin señales (roll y datos fuertes), todo alineado con `m15`.
#[allow(clippy::too_many_arguments)]
pub fn impulse_plans(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    atr: &[Option<Decimal>],
    states: &[Structure],
    excluded: &BTreeSet<NaiveDate>,
    threshold: BodyThreshold,
    cfg: &Program4Config,
) -> Vec<Plan> {
    let mut out = Vec::new();
    for t in 1..m15.len() {
        if let Some(p) = impulse_at(
            m15, session, entries, atr, states, excluded, threshold, cfg, t,
        ) {
            out.push(p);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn impulse_at(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    atr: &[Option<Decimal>],
    states: &[Structure],
    excluded: &BTreeSet<NaiveDate>,
    threshold: BodyThreshold,
    cfg: &Program4Config,
    t: usize,
) -> Option<Plan> {
    if !decision_allowed(m15, entries, t) {
        return None;
    }
    let (a, b) = (m15.get(t.checked_sub(1)?)?, m15.get(t)?);
    if a.end() != b.start {
        return None;
    }
    let side = direction(a)?;
    if direction(b) != Some(side) || !is_valid(a) || !is_valid(b) {
        return None;
    }
    let min_body = match threshold {
        BodyThreshold::Points(p) => p,
        BodyThreshold::Atr(k) => k * atr.get(t.checked_sub(2)?).copied().flatten()?,
    };
    if body(a) < min_body || body(b) < min_body {
        return None;
    }
    if !states.get(t)?.aligned(side) {
        return None;
    }
    let exit_index = execution_exit(m15, session, excluded, cfg, t + 1)?;
    let stop = match side {
        Side::Long => a.low,
        Side::Short => a.high,
    };
    let reference = b.close;
    let ok = match side {
        Side::Long => reference > stop,
        Side::Short => reference < stop,
    };
    if !ok {
        return None;
    }
    Some(Plan {
        event: t,
        signal_index: t,
        side,
        reference,
        stop,
        risk_atr: atr.get(t).copied().flatten()?,
        exit_index,
        target_r: Some(cfg.target_r),
    })
}

/// Muestra de la calibración de k (§5.1, §13.2): `(cuerpo, cuerpo / ATR de la vela anterior)`
/// de cada vela de la sesión de contado que empieza en `[from, to)`. La vela anterior tiene que
/// ser contigua y tener ATR (del mismo tramo de contrato).
pub fn body_atr_sample(
    m15: &[Candle],
    session: &Session,
    atr: &[Option<Decimal>],
    (from, to): (DateTime<Utc>, DateTime<Utc>),
) -> Vec<(Decimal, Decimal)> {
    let mut out = Vec::new();
    for i in 1..m15.len() {
        let c = &m15[i];
        if c.start < from || c.start >= to || m15[i - 1].end() != c.start {
            continue;
        }
        let Some(Some((open, close))) = session.hours(session.trading_day(c.start)) else {
            continue;
        };
        if c.start < open || c.end() > close {
            continue;
        }
        let Some(a) = atr
            .get(i - 1)
            .copied()
            .flatten()
            .filter(|a| *a > Decimal::ZERO)
        else {
            continue;
        };
        out.push((body(c), body(c) / a));
    }
    out
}

/// k "equivalente" (§5.1): q = proporción de la muestra con cuerpo ≥ `points`; k = cuantil
/// (1 − q) de los cocientes (inversa de la distribución empírica: el valor ordenado número
/// ⌈(1 − q)·n⌉, mínimo 1), redondeado al múltiplo de `step` más cercano. `None` sin muestra.
pub fn equivalent_k(
    sample: &[(Decimal, Decimal)],
    points: Decimal,
    step: Decimal,
) -> Option<(Decimal, Decimal)> {
    if sample.is_empty() || step <= Decimal::ZERO {
        return None;
    }
    let n = Decimal::from(sample.len());
    let q = Decimal::from(sample.iter().filter(|(b, _)| *b >= points).count()) / n;
    let mut ratios: Vec<Decimal> = sample.iter().map(|(_, r)| *r).collect();
    ratios.sort();
    let rank = ((Decimal::ONE - q) * n).ceil().max(Decimal::ONE);
    let idx = rank.to_usize()? - 1;
    let raw = *ratios.get(idx.min(ratios.len() - 1))?;
    let k = (raw / step)
        .round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
        * step;
    Some((q, k))
}

/// Generador SplitMix64 (Steele, Lea y Flood, 2014): determinístico y sin dependencias.
#[derive(Debug, Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Índice uniforme en `[0, n)`: `(x · n) >> 64`.
    pub fn below(&mut self, n: usize) -> usize {
        let x = u128::from(self.next_u64());
        usize::try_from((x * n as u128) >> 64).unwrap_or(0)
    }

    /// `true` con probabilidad `num / den`: `(x >> 11) / 2⁵³ < num / den`.
    pub fn chance(&mut self, num: usize, den: usize) -> bool {
        let x = u128::from(self.next_u64() >> 11);
        x * (den as u128) < (num as u128) << 53
    }
}

/// R neto de una ejecución: `(salida − entrada) × lado / distancia`, con los precios ejecutados.
fn net_r(side: Side, entry_price: Decimal, exit_price: Decimal, distance: Decimal) -> Decimal {
    let s = match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    };
    (exit_price - entry_price) * s / distance
}

/// R neto medio y retorno total con `risk` por operación (∏(1 + risk·R) − 1).
pub fn r_summary(rs: &[Decimal], risk: Decimal) -> (Decimal, Decimal) {
    if rs.is_empty() {
        return (Decimal::ZERO, Decimal::ZERO);
    }
    let mean = rs.iter().sum::<Decimal>() / Decimal::from(rs.len());
    let total = rs
        .iter()
        .fold(Decimal::ONE, |acc, r| acc * (Decimal::ONE + risk * r))
        - Decimal::ONE;
    (mean, total)
}

/// R neto de una operación ejecutada, medido como en el test de suerte (sin financiación).
pub fn trade_net_r(t: &StudyTrade) -> Option<Decimal> {
    let d = (t.entry_mid - t.stop).abs();
    (d > Decimal::ZERO).then(|| net_r(t.side, t.entry_price, t.exit_price, d))
}

/// Resultado del test de suerte: la estrategia y cada simulación, `(R neto medio, retorno total)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuckTest {
    pub strategy: (Decimal, Decimal),
    pub simulations: Vec<(Decimal, Decimal)>,
}

impl LuckTest {
    /// Porcentaje de simulaciones con un valor menor que el de la estrategia (R medio, total).
    pub fn percentiles(&self) -> (Decimal, Decimal) {
        let n = Decimal::from(self.simulations.len().max(1));
        let below = |f: &dyn Fn(&(Decimal, Decimal)) -> Decimal, v: Decimal| {
            Decimal::ONE_HUNDRED
                * Decimal::from(self.simulations.iter().filter(|s| f(s) < v).count())
                / n
        };
        (
            below(&|s| s.0, self.strategy.0),
            below(&|s| s.1, self.strategy.1),
        )
    }
}

/// Test de suerte (§7.2, §13.2): `simulations` corridas al azar con la misma cantidad de
/// operaciones por sesión que `trades`, sus distancias de stop en orden, la proporción de largos
/// de la estrategia, el mismo objetivo en R, la misma gestión y los mismos costos. `risk` es el
/// riesgo por operación del retorno total. Solo reporte.
#[allow(clippy::too_many_arguments)]
pub fn luck_test(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    excluded: &BTreeSet<NaiveDate>,
    force_close: &[bool],
    cfg: &Program4Config,
    trades: &[StudyTrade],
    setup: &Setup,
    risk: Decimal,
) -> LuckTest {
    let real: Vec<Decimal> = trades.iter().filter_map(trade_net_r).collect();
    let strategy = r_summary(&real, risk);
    let longs = trades.iter().filter(|t| t.side == Side::Long).count();
    // Por sesión: las distancias de las operaciones reales, en orden.
    let mut per_day: BTreeMap<NaiveDate, Vec<Decimal>> = BTreeMap::new();
    for t in trades {
        let d = (t.entry_mid - t.stop).abs();
        if d > Decimal::ZERO {
            per_day
                .entry(session.trading_day(t.entry_time))
                .or_default()
                .push(d);
        }
    }
    // Velas de ejecución permitidas por sesión, con su salida planificada.
    let mut candidates: BTreeMap<NaiveDate, Vec<(usize, usize)>> = BTreeMap::new();
    for j in 1..m15.len() {
        if !decision_allowed(m15, entries, j - 1) {
            continue;
        }
        if let Some(exit) = execution_exit(m15, session, excluded, cfg, j) {
            let day = session.trading_day(m15[j].start);
            if per_day.contains_key(&day) {
                candidates.entry(day).or_default().push((j, exit));
            }
        }
    }
    let mut rng = SplitMix64::new(cfg.luck_seed);
    let mut simulations = Vec::with_capacity(cfg.luck_simulations);
    for _ in 0..cfg.luck_simulations {
        let mut rs = Vec::new();
        for (day, distances) in &per_day {
            let Some(pool) = candidates.get(day) else {
                continue;
            };
            // n velas distintas (Fisher–Yates parcial), en orden.
            let mut idx: Vec<usize> = (0..pool.len()).collect();
            let n = distances.len().min(pool.len());
            for k in 0..n {
                let r = k + rng.below(pool.len() - k);
                idx.swap(k, r);
            }
            let mut picked: Vec<(usize, usize)> = idx[..n].iter().map(|&k| pool[k]).collect();
            picked.sort_unstable();
            let mut busy_until: Option<usize> = None;
            for (k, (entry_index, exit_index)) in picked.into_iter().enumerate() {
                let side = if rng.chance(longs, trades.len()) {
                    Side::Long
                } else {
                    Side::Short
                };
                if busy_until.is_some_and(|b| entry_index <= b) {
                    continue;
                }
                let distance = distances[k];
                let entry_mid = m15[entry_index].open;
                let (stop, target) = match side {
                    Side::Long => (entry_mid - distance, entry_mid + cfg.target_r * distance),
                    Side::Short => (entry_mid + distance, entry_mid - cfg.target_r * distance),
                };
                let Some((out_index, exit_mid, _)) = simulate_leg(
                    side,
                    entry_index,
                    stop,
                    Some(target),
                    exit_index,
                    m15,
                    force_close,
                ) else {
                    continue;
                };
                let c = &setup.backtest.costs;
                let entry_price = fill_price(side, Execution::Entry, entry_mid, &setup.costs, c);
                let exit_price = fill_price(side, Execution::Exit, exit_mid, &setup.costs, c);
                rs.push(net_r(side, entry_price, exit_price, distance));
                busy_until = Some(out_index);
            }
        }
        simulations.push(r_summary(&rs, risk));
    }
    LuckTest {
        strategy,
        simulations,
    }
}

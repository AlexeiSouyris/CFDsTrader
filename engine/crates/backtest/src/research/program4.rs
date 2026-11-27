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

use std::collections::BTreeSet;

use cfds_core::Candle;
use cfds_indicators::wilder_atr;
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::luck::{LuckConfig, LuckSeries};
use super::{Plan, Program4Config, StudyTrade, decision_allowed};
use crate::Setup;

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
pub fn segment_starts(m15: &[Candle], rolls: &[DateTime<Utc>]) -> Vec<bool> {
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
        target_cap: None,
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

pub use super::luck::{LuckTest, r_summary, trade_net_r};

/// Test de suerte (§7.2, §13.2): `simulations` corridas al azar con la misma cantidad de
/// operaciones por sesión que `trades`, sus distancias de stop en orden, la proporción de largos
/// de la estrategia, el mismo objetivo en R, la misma gestión y los mismos costos. `risk` es el
/// riesgo por operación del retorno total. Solo reporte. Es el test general de
/// [`super::luck::luck_test`] con la salida planificada del programa ([`execution_exit`]).
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
    super::luck::luck_test(
        &LuckSeries {
            m15,
            session,
            entries,
            force_close,
        },
        |j| execution_exit(m15, session, excluded, cfg, j),
        &LuckConfig {
            simulations: cfg.luck_simulations,
            seed: cfg.luck_seed,
            target_r: Some(cfg.target_r),
        },
        trades,
        setup,
        risk,
    )
}

/// Con barras de 1 minuto `(máximo, mínimo)` en orden, ¿qué tocó primero una vela que tocó el
/// stop y el objetivo (§2.4, descriptivo)? `Some(true)` = el objetivo, `Some(false)` = el stop;
/// `None` si las dos cosas pasan en la misma barra o no se ve ninguna.
pub fn target_first(
    side: Side,
    stop: Decimal,
    target: Decimal,
    bars: &[(Decimal, Decimal)],
) -> Option<bool> {
    for &(high, low) in bars {
        let (hit_stop, hit_target) = match side {
            Side::Long => (low <= stop, high >= target),
            Side::Short => (high >= stop, low <= target),
        };
        match (hit_stop, hit_target) {
            (true, true) => return None,
            (true, false) => return Some(false),
            (false, true) => return Some(true),
            (false, false) => {}
        }
    }
    None
}

/// Autocorrelación de resultados (§7.1, descriptivo): `(aciertos, operaciones)` en total, después
/// de una ganadora y después de una perdedora.
pub fn outcome_after(wins: &[bool]) -> [(usize, usize); 3] {
    let mut out = [(0, 0); 3];
    for (i, &w) in wins.iter().enumerate() {
        out[0].0 += usize::from(w);
        out[0].1 += 1;
        if let Some(&prev) = i.checked_sub(1).and_then(|p| wins.get(p)) {
            let k = if prev { 1 } else { 2 };
            out[k].0 += usize::from(w);
            out[k].1 += 1;
        }
    }
    out
}

/// En ventanas móviles de `window` operaciones: `(ventanas con la tasa tras una pérdida menor que
/// la general, ventanas con alguna operación tras una pérdida)`.
pub fn rolling_after_loss_below(wins: &[bool], window: usize) -> (usize, usize) {
    let mut below = 0;
    let mut total = 0;
    if window == 0 {
        return (0, 0);
    }
    for w in wins.windows(window) {
        let [all, _, after_loss] = outcome_after(w);
        if after_loss.1 == 0 {
            continue;
        }
        total += 1;
        // a/n < g/m  ⇔  a·m < g·n (enteros).
        if after_loss.0 * all.1 < all.0 * after_loss.1 {
            below += 1;
        }
    }
    (below, total)
}

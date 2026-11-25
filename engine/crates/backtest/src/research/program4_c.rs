//! Programa 4 (#561), H4.3: estrategia C ("de campeonato") de Gianluca Bruni en el CFD US100,
//! con las definiciones congeladas en `docs/analysis/2026-11-programa-4-preregistro.md` (§3, §6
//! y §13). Usa la estructura, el ATR, los días excluidos y la ventana de ejecución de
//! [`super::program4`].
//!
//! ```text
//! sesión del perfil  día del CME: de las 18:00 ET del día hábil anterior a las 17:00 ET de d
//! perfil             barras de 1 min; volumen repartido en partes iguales en [mínimo, máximo];
//!                    casillero w = tick × máx(1, redondeo(0,0002 × apertura / tick));
//!                    POC = casillero con más volumen; área de valor del 70 % desde el POC
//! señal larga en t   POC y VAL en desarrollo de d > los de d − 1; estructura actual alcista;
//!                    vuelta al área (mínimo ≤ VAH(d − 1)) antes de t; mínimo de la estructura
//!                    sin perforar; cierre ≥ máximo de t − 1 + m y ≥ VAH(d − 1) + m; volumen de t
//!                    < el de t − 1 o < la media de 20; primera de la secuencia: cierre de t − 1
//!                    < VAH + m; día sin dato fuerte; máximo de la estructura > cierre
//! operación          entrada en la apertura de t + 1, stop en el mínimo de t, objetivo el menor
//!                    entre 2 R y el máximo de la estructura; reentradas hasta que el precio
//!                    toca ese máximo. Cortos: espejo exacto.
//! ```
//! Funciones puras sobre velas y barras cerradas: lo calculado para una vela no cambia con velas
//! nuevas.

use std::collections::{BTreeMap, BTreeSet};

use cfds_core::{Candle, FuturesBar};
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc, Weekday};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use super::program4::{Breakout, Structure, direction, execution_exit, is_valid, segment_starts};
use super::{ChampionshipConfig, Plan, Program4Config, decision_allowed};

// ── perfil de volumen ──────────────────────────────────────────────────────────────────────

/// POC, borde de arriba (VAH) y de abajo (VAL) del área de valor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    pub poc: Decimal,
    pub vah: Decimal,
    pub val: Decimal,
}

fn is_weekday(d: NaiveDate) -> bool {
    !matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

fn next_weekday(d: NaiveDate) -> Option<NaiveDate> {
    let mut n = d.succ_opt()?;
    while !is_weekday(n) {
        n = n.succ_opt()?;
    }
    Some(n)
}

/// Día hábil (lunes a viernes) anterior.
pub fn previous_weekday(d: NaiveDate) -> Option<NaiveDate> {
    let mut p = d.pred_opt()?;
    while !is_weekday(p) {
        p = p.pred_opt()?;
    }
    Some(p)
}

/// Sesión del perfil (día del CME) a la que pertenece un instante: desde la hora de inicio, la
/// del día hábil siguiente; antes de la hora de fin, la del mismo día si es hábil; entre medio
/// (la pausa diaria), ninguna.
pub fn profile_session(t: DateTime<Utc>, cfg: &ChampionshipConfig) -> Option<NaiveDate> {
    let local = t.with_timezone(&cfg.timezone);
    let (date, time) = (local.date_naive(), local.time());
    if time >= cfg.profile_session_start {
        next_weekday(date)
    } else if time < cfg.profile_session_end {
        is_weekday(date).then_some(date)
    } else {
        None
    }
}

/// Minutos que tiene una sesión completa (de la hora de inicio a la de fin del día siguiente).
pub fn session_minutes(cfg: &ChampionshipConfig) -> i64 {
    let day = Duration::hours(24);
    (day - (cfg.profile_session_start - cfg.profile_session_end)).num_minutes()
}

/// Ancho del casillero de una sesión que abre en `open`.
pub fn bin_width(open: Decimal, cfg: &ChampionshipConfig) -> Decimal {
    let ticks = (cfg.profile_bin_fraction * open / cfg.tick)
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .max(Decimal::ONE);
    cfg.tick * ticks
}

/// Perfil de volumen que se arma barra a barra (casilleros en múltiplos de `w`).
#[derive(Debug, Clone)]
pub struct ProfileBuilder {
    w: Decimal,
    bins: BTreeMap<i64, Decimal>,
    high: Option<Decimal>,
    low: Option<Decimal>,
}

fn bin_of(price: Decimal, w: Decimal) -> i64 {
    (price / w).floor().to_i64().unwrap_or(0)
}

impl ProfileBuilder {
    pub fn new(w: Decimal) -> Self {
        Self {
            w,
            bins: BTreeMap::new(),
            high: None,
            low: None,
        }
    }

    /// Reparte el volumen de una barra en partes iguales a lo largo de [mínimo, máximo].
    pub fn add(&mut self, high: Decimal, low: Decimal, volume: Decimal) {
        self.high = Some(self.high.map_or(high, |h| h.max(high)));
        self.low = Some(self.low.map_or(low, |l| l.min(low)));
        if volume <= Decimal::ZERO {
            return;
        }
        if high <= low {
            *self.bins.entry(bin_of(low, self.w)).or_default() += volume;
            return;
        }
        let range = high - low;
        for k in bin_of(low, self.w)..=bin_of(high, self.w) {
            let bottom = Decimal::from(k) * self.w;
            let overlap = high.min(bottom + self.w) - low.max(bottom);
            if overlap > Decimal::ZERO {
                *self.bins.entry(k).or_default() += volume * overlap / range;
            }
        }
    }

    /// POC y área de valor (`va` del volumen), o `None` sin volumen.
    pub fn profile(&self, va: Decimal) -> Option<Profile> {
        let total: Decimal = self.bins.values().copied().sum();
        if total <= Decimal::ZERO {
            return None;
        }
        let (lo_k, hi_k) = (*self.bins.keys().next()?, *self.bins.keys().next_back()?);
        let vol = |k: i64| self.bins.get(&k).copied().unwrap_or_default();
        let mid = (self.high? + self.low?) / Decimal::TWO;
        let center = |k: i64| Decimal::from(k) * self.w + self.w / Decimal::TWO;
        let max = self.bins.values().copied().max()?;
        // Empate: el más cercano al punto medio del rango; después, el más bajo.
        let poc = self
            .bins
            .iter()
            .filter(|(_, v)| **v == max)
            .map(|(k, _)| *k)
            .min_by_key(|k| ((center(*k) - mid).abs(), *k))?;
        let (mut up, mut down) = (poc, poc);
        let mut acc = vol(poc);
        while acc < va * total {
            let (has_up, has_down) = (up < hi_k, down > lo_k);
            match (has_up, has_down) {
                (false, false) => break,
                (true, false) => {
                    up += 1;
                    acc += vol(up);
                }
                (false, true) => {
                    down -= 1;
                    acc += vol(down);
                }
                (true, true) => {
                    let (a, b) = (vol(up + 1), vol(down - 1));
                    if a >= b {
                        up += 1;
                        acc += a;
                    }
                    if b >= a {
                        down -= 1;
                        acc += b;
                    }
                }
            }
        }
        Some(Profile {
            poc: Decimal::from(poc) * self.w,
            vah: Decimal::from(up + 1) * self.w,
            val: Decimal::from(down) * self.w,
        })
    }
}

/// Lo que se sabe del perfil al cierre de una vela de 15 m: el cerrado de la sesión anterior y
/// el que se desarrolla en la sesión de la vela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileContext {
    pub session: NaiveDate,
    pub previous: Profile,
    pub developing: Profile,
}

#[derive(Debug, Clone)]
struct SessionState {
    date: NaiveDate,
    first_start: DateTime<Utc>,
    bars: usize,
    contract: Option<u64>,
    mixed: bool,
    builder: ProfileBuilder,
}

#[derive(Debug, Clone, Copy)]
struct ClosedSession {
    date: NaiveDate,
    first_start: DateTime<Utc>,
    bars: usize,
    contract: Option<u64>,
    mixed: bool,
    profile: Option<Profile>,
}

/// Arma los perfiles barra a barra, en orden (para no tener toda la historia de 1 minuto en
/// memoria): [`ProfileTracker::push`] con cada barra y [`ProfileTracker::context`] al cierre de
/// cada vela de 15 m, después de pasarle todas las barras que empiezan antes de ese cierre.
#[derive(Debug, Clone)]
pub struct ProfileTracker {
    cfg: ChampionshipConfig,
    rolls: Vec<DateTime<Utc>>,
    previous: Option<ClosedSession>,
    current: Option<SessionState>,
    /// Sesiones cerradas: `(fecha, barras, perfil)`, para el descriptivo.
    pub closed: Vec<(NaiveDate, usize, Option<Profile>)>,
}

impl ProfileTracker {
    pub fn new(cfg: &ChampionshipConfig, rolls: &[DateTime<Utc>]) -> Self {
        Self {
            cfg: cfg.clone(),
            rolls: rolls.to_vec(),
            previous: None,
            current: None,
            closed: Vec::new(),
        }
    }

    fn close_current(&mut self) {
        if let Some(s) = self.current.take() {
            let profile = s.builder.profile(self.cfg.value_area);
            self.closed.push((s.date, s.bars, profile));
            self.previous = Some(ClosedSession {
                date: s.date,
                first_start: s.first_start,
                bars: s.bars,
                contract: s.contract,
                mixed: s.mixed,
                profile,
            });
        }
    }

    /// Suma una barra de 1 minuto (en orden de inicio). Las de la pausa diaria no cuentan.
    pub fn push(&mut self, bar: &FuturesBar) {
        let Some(date) = profile_session(bar.start, &self.cfg) else {
            return;
        };
        if self.current.as_ref().is_some_and(|s| s.date != date) {
            self.close_current();
        }
        let cfg = &self.cfg;
        let s = self.current.get_or_insert_with(|| SessionState {
            date,
            first_start: bar.start,
            bars: 0,
            contract: Some(bar.contract_id),
            mixed: false,
            builder: ProfileBuilder::new(bin_width(bar.open, cfg)),
        });
        s.bars += 1;
        if s.contract != Some(bar.contract_id) {
            s.mixed = true;
        }
        s.builder.add(bar.high, bar.low, Decimal::from(bar.volume));
    }

    /// Contexto al cierre de `candle` (ya pasaron todas las barras que empiezan antes de su
    /// cierre). `None` si la vela no es de la sesión en curso, si la sesión anterior no es el día
    /// hábil anterior, si le faltan más barras de las permitidas, si alguna de las dos mezcla
    /// contratos o son de contratos distintos, o si hubo un vencimiento desde el inicio de la
    /// sesión anterior (§2.5: el perfil se reinicia).
    pub fn context(&self, candle: &Candle) -> Option<ProfileContext> {
        let cur = self.current.as_ref()?;
        if profile_session(candle.start, &self.cfg)? != cur.date {
            return None;
        }
        let prev = self.previous.as_ref()?;
        if Some(prev.date) != previous_weekday(cur.date) {
            return None;
        }
        let min_bars =
            (self.cfg.profile_min_bars_fraction * Decimal::from(session_minutes(&self.cfg))).ceil();
        if Decimal::from(prev.bars) < min_bars
            || prev.mixed
            || cur.mixed
            || prev.contract != cur.contract
        {
            return None;
        }
        let end = candle.end();
        if self.rolls.iter().any(|&r| prev.first_start < r && r <= end) {
            return None;
        }
        Some(ProfileContext {
            session: cur.date,
            previous: prev.profile?,
            developing: cur.builder.profile(self.cfg.value_area)?,
        })
    }
}

/// Contextos alineados con `m15` a partir de todas las barras de 1 minuto en orden (para tests y
/// series cortas; el ejemplo usa [`ProfileTracker`] por tramos).
pub fn profile_contexts(
    m15: &[Candle],
    bars: &[FuturesBar],
    rolls: &[DateTime<Utc>],
    cfg: &ChampionshipConfig,
) -> Vec<Option<ProfileContext>> {
    let mut tracker = ProfileTracker::new(cfg, rolls);
    let mut next = 0;
    m15.iter()
        .map(|c| {
            while let Some(b) = bars.get(next).filter(|b| b.start < c.end()) {
                tracker.push(b);
                next += 1;
            }
            tracker.context(c)
        })
        .collect()
}

// ── días con datos fuertes ──────────────────────────────────────────────────────────────────

/// Días con un dato fuerte (decisión 11, lectura de este informe): el día de la sesión de
/// contado en cuya fecha (hora local de la bolsa) está programado el dato, antes de su cierre.
/// Para el FOMC (14:00 ET) es lo mismo que [`super::program4::release_days`]; para el CPI y el
/// empleo (08:30 ET, antes de la apertura) es el día del dato.
pub fn release_dates(session: &Session, scheduled: &[DateTime<Utc>]) -> BTreeSet<NaiveDate> {
    scheduled
        .iter()
        .filter_map(|&t| {
            let d = session.trading_day(t);
            let (_, close) = session.hours(d)??;
            (t < close).then_some(d)
        })
        .collect()
}

// ── señales ────────────────────────────────────────────────────────────────────────────────

/// Paso de la secuencia de un lado (§6.2 y §6.3). La vuelta al área de valor que arma una
/// secuencia es la primera vela de la sesión **posterior a la última ruptura** de ese lado que
/// toca el área: el extremo de la estructura se mide entre la ruptura y la vela anterior a esa
/// vuelta, así que la vuelta tiene que venir después (lectura declarada en el informe).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Esperando una vuelta al área de valor.
    Idle,
    /// Hubo una vuelta (en la vela `touch`); falta la primera operación.
    Armed { touch: usize },
    /// Ya hubo una primera operación: reentradas hasta que el precio toque el extremo de la
    /// estructura.
    Sequence { touch: usize },
}

/// Cuántas velas quedan después de cada condición (§7.1, descriptivo), sumando largos y cortos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Funnel {
    /// Decisión permitida, ejecución dentro del horario y perfiles de las dos sesiones.
    pub base: usize,
    pub structure: usize,
    pub displacement: usize,
    pub value_area_return: usize,
    pub structure_intact: usize,
    pub close_above_previous: usize,
    pub close_beyond_value_area: usize,
    pub volume: usize,
    pub first_or_reentry: usize,
    pub strong_data: usize,
    pub room: usize,
}

/// Qué es cada plan (descriptivo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanInfo {
    pub signal_index: usize,
    pub first: bool,
    /// Extremo de la estructura (tope del objetivo).
    pub extreme: Decimal,
}

/// Lo que necesitan las señales de H4.3, alineado con `m15`.
pub struct ChampionshipInputs<'a> {
    pub m15: &'a [Candle],
    pub session: &'a Session,
    pub entries: &'a [bool],
    pub atr: &'a [Option<Decimal>],
    pub breakouts: &'a [Breakout],
    pub states: &'a [Structure],
    pub rolls: &'a [DateTime<Utc>],
    pub contexts: &'a [Option<ProfileContext>],
    pub excluded: &'a BTreeSet<NaiveDate>,
}

fn sign(side: Side) -> Decimal {
    match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    }
}

/// `a` está estrictamente "más allá" de `b` en la dirección `side` (más arriba si es largo).
fn beyond(side: Side, a: Decimal, b: Decimal) -> bool {
    sign(side) * (a - b) > Decimal::ZERO
}

/// Valor "a favor" de la vela para el lado (máximo si es largo, mínimo si es corto), y el opuesto.
fn favorable(side: Side, c: &Candle) -> Decimal {
    match side {
        Side::Long => c.high,
        Side::Short => c.low,
    }
}

fn adverse(side: Side, c: &Candle) -> Decimal {
    match side {
        Side::Long => c.low,
        Side::Short => c.high,
    }
}

/// Estado de un lado, recorrido vela a vela.
struct SideState {
    side: Side,
    phase: Phase,
    /// Última ruptura de este lado y el extremo de la estructura (mínimo en largos) con su
    /// validez: `None` si no hay vela válida contraria antes, o si ya se perforó.
    breakout: Option<usize>,
    guard: Option<Decimal>,
}

/// Señales de H4.3 (largos y cortos) con lo que es cada una y el embudo de condiciones.
pub fn championship_plans(
    i: &ChampionshipInputs<'_>,
    cfg4: &Program4Config,
    cfg: &ChampionshipConfig,
) -> (Vec<Plan>, Vec<PlanInfo>, Funnel) {
    let m15 = i.m15;
    let starts = segment_starts(m15, i.rolls);
    // Inicio del tramo de contrato de cada vela.
    let mut seg_start = vec![0usize; m15.len()];
    let mut s0 = 0;
    for (k, st) in starts.iter().enumerate() {
        if *st {
            s0 = k;
        }
        seg_start[k] = s0;
    }
    let mut plans = Vec::new();
    let mut infos = Vec::new();
    let mut funnel = Funnel::default();
    let mut sides = [Side::Long, Side::Short].map(|side| SideState {
        side,
        phase: Phase::Idle,
        breakout: None,
        guard: None,
    });
    let mut next_breakout = 0usize;
    let mut last_session: Option<NaiveDate> = None;
    let no_days = BTreeSet::new();
    for t in 0..m15.len() {
        let c = &m15[t];
        if starts[t] {
            for s in sides.iter_mut() {
                s.breakout = None;
                s.guard = None;
            }
        }
        // Rupturas hasta t inclusive (la estructura en t incluye una ruptura en t).
        while let Some(b) = i.breakouts.get(next_breakout).filter(|b| b.index <= t) {
            for s in sides.iter_mut() {
                if s.side == b.side {
                    s.breakout = Some(b.index);
                    s.guard = structure_guard(m15, &seg_start, s.side, b.index);
                    // La vuelta al área de valor cuenta solo después de la ruptura (el extremo
                    // de la estructura se mide entre las dos).
                    if matches!(s.phase, Phase::Armed { touch } if touch <= b.index) {
                        s.phase = Phase::Idle;
                    }
                }
            }
            next_breakout += 1;
        }
        // Perforación del extremo que protege la estructura (desde la ruptura hasta t).
        for s in sides.iter_mut() {
            if s.guard
                .is_some_and(|g| beyond(s.side, g, adverse(s.side, c)))
            {
                s.guard = None;
            }
        }
        let ctx = i.contexts.get(t).copied().flatten();
        let session = ctx.map(|x| x.session);
        if session != last_session || ctx.is_none() {
            for s in sides.iter_mut() {
                s.phase = Phase::Idle;
            }
        }
        last_session = session;
        let Some(ctx) = ctx else {
            continue;
        };
        for s in sides.iter_mut() {
            let side = s.side;
            // 1. La secuencia termina cuando el precio toca el extremo de la estructura.
            if let Phase::Sequence { touch } = s.phase
                && s.breakout
                    .and_then(|b| extreme(m15, side, b, touch))
                    .is_none_or(|e| !beyond(side, e, favorable(side, c)))
            {
                s.phase = Phase::Idle;
            }
            // 2. Señal en t.
            if let Some((touch, first, cap)) =
                evaluate(i, cfg4, cfg, s, &seg_start, &ctx, t, &no_days, &mut funnel)
                && let Some(risk_atr) = i.atr.get(t).copied().flatten()
                && let Some(exit_index) = execution_exit(m15, i.session, i.excluded, cfg4, t + 1)
            {
                plans.push(Plan {
                    event: t,
                    signal_index: t,
                    side,
                    reference: c.close,
                    stop: adverse(side, c),
                    risk_atr,
                    exit_index,
                    target_r: Some(cfg.target_r),
                    target_cap: Some(cap),
                });
                infos.push(PlanInfo {
                    signal_index: t,
                    first,
                    extreme: cap,
                });
                s.phase = if beyond(side, cap, favorable(side, c)) {
                    Phase::Sequence { touch }
                } else {
                    Phase::Idle
                };
            }
            // 3. Vuelta al área de valor en t (cuenta desde t + 1).
            let back = match side {
                Side::Long => c.low <= ctx.previous.vah,
                Side::Short => c.high >= ctx.previous.val,
            };
            if s.phase == Phase::Idle && back && s.breakout.is_some_and(|b| t > b) {
                s.phase = Phase::Armed { touch: t };
            }
        }
    }
    (plans, infos, funnel)
}

/// Extremo de la estructura de una secuencia (§6.2): el mayor máximo (largos) o el menor mínimo
/// (cortos) entre la ruptura `b` y la vela anterior a la vuelta al área de valor `touch`. `None`
/// si la vuelta no es posterior a la ruptura.
fn extreme(m15: &[Candle], side: Side, b: usize, touch: usize) -> Option<Decimal> {
    if touch <= b {
        return None;
    }
    let sg = sign(side);
    m15.get(b..touch)?
        .iter()
        .map(|x| sg * favorable(side, x))
        .max()
        .map(|v| sg * v)
}

/// Extremo que protege la estructura (§3): el mínimo de la vela bajista válida más cercana antes
/// de la ruptura alcista `b`, en el mismo tramo de contrato (espejo en cortos).
fn structure_guard(m15: &[Candle], seg_start: &[usize], side: Side, b: usize) -> Option<Decimal> {
    let from = *seg_start.get(b)?;
    let opposite = match side {
        Side::Long => Side::Short,
        Side::Short => Side::Long,
    };
    (from..b)
        .rev()
        .find(|&k| is_valid(&m15[k]) && direction(&m15[k]) == Some(opposite))
        .map(|k| adverse(side, &m15[k]))
}

/// Evalúa la señal de un lado en `t`. Devuelve la vela de la vuelta al área de valor de la
/// secuencia, si es la primera operación y el extremo de la estructura. Cuenta el embudo.
#[allow(clippy::too_many_arguments)]
fn evaluate(
    i: &ChampionshipInputs<'_>,
    cfg4: &Program4Config,
    cfg: &ChampionshipConfig,
    s: &SideState,
    seg_start: &[usize],
    ctx: &ProfileContext,
    t: usize,
    no_days: &BTreeSet<NaiveDate>,
    funnel: &mut Funnel,
) -> Option<(usize, bool, Decimal)> {
    let m15 = i.m15;
    let side = s.side;
    let c = &m15[t];
    if t == 0 || !decision_allowed(m15, i.entries, t) {
        return None;
    }
    execution_exit(m15, i.session, no_days, cfg4, t + 1)?;
    funnel.base += 1;
    // 2. Estructura actual de este lado.
    if i.states.get(t)?.current != Some(side) {
        return None;
    }
    funnel.structure += 1;
    // 1. Desplazamiento del perfil.
    let (p, d) = (ctx.previous, ctx.developing);
    let shifted = match side {
        Side::Long => d.poc > p.poc && d.val > p.val,
        Side::Short => d.poc < p.poc && d.vah < p.vah,
    };
    if !shifted {
        return None;
    }
    funnel.displacement += 1;
    // 3. Vuelta al área de valor antes de t (o secuencia en curso).
    let (touch, first) = match s.phase {
        Phase::Idle => return None,
        Phase::Armed { touch } => (touch, true),
        Phase::Sequence { touch } => (touch, false),
    };
    if touch >= t {
        return None;
    }
    funnel.value_area_return += 1;
    // 4. Extremo de la estructura sin perforar.
    s.guard?;
    s.breakout?;
    funnel.structure_intact += 1;
    // 5. Disparador. Velas t − 1 y t consecutivas.
    let prev = &m15[t - 1];
    if prev.end() != c.start {
        return None;
    }
    let m = cfg.trigger_margin;
    let edge = match side {
        Side::Long => p.vah,
        Side::Short => p.val,
    };
    let sg = sign(side);
    // a. cierre más allá del extremo de la vela anterior + m.
    if sg * (c.close - favorable(side, prev)) < m {
        return None;
    }
    funnel.close_above_previous += 1;
    // b. cierre más allá del borde del área de valor de d − 1 + m.
    if sg * (c.close - edge) < m {
        return None;
    }
    funnel.close_beyond_value_area += 1;
    // c. volumen eficiente: menor que el de t − 1 o que la media de las 20 anteriores, con las
    // 21 velas consecutivas y del mismo tramo de contrato.
    let n = cfg.volume_sma;
    let from = t.checked_sub(n)?;
    if seg_start[t] > from || m15[from..=t].windows(2).any(|w| w[0].end() != w[1].start) {
        return None;
    }
    let vol = |k: usize| m15[k].volume;
    let v = vol(t)?;
    let window: Option<Vec<Decimal>> = (from..t).map(vol).collect();
    let window = window?;
    let sma = window.iter().copied().sum::<Decimal>() / Decimal::from(n);
    if !(v < vol(t - 1)? || v < sma) {
        return None;
    }
    funnel.volume += 1;
    // 6. Primera de la secuencia: t es la vela que cruza el borde.
    if first && sg * (prev.close - edge) >= m {
        return None;
    }
    funnel.first_or_reentry += 1;
    // 7. Sin datos fuertes ese día (y sin roll): la vela de ejecución no cae en un día excluido.
    execution_exit(m15, i.session, i.excluded, cfg4, t + 1)?;
    funnel.strong_data += 1;
    // 8. Hay recorrido: el extremo de la estructura queda más allá del cierre de t (en la
    // ejecución, además, más allá de la apertura de t + 1: `Plan::open_beyond_cap`).
    let cap = extreme(m15, side, s.breakout?, touch)?;
    if !beyond(side, cap, c.close) {
        return None;
    }
    // El cierre tiene que quedar del lado bueno del stop (el extremo opuesto de t).
    if !beyond(side, c.close, adverse(side, c)) {
        return None;
    }
    funnel.room += 1;
    Some((touch, first, cap))
}

// ── régimen (descriptivo) ──────────────────────────────────────────────────────────────────

/// Percentil del ATR en `t` (§6.4): 100 × (valores de ATR entre `t − window + 1` y `t` que son
/// ≤ ATR de `t`) / `window`, con la ventana entera en el mismo tramo de contrato y con ATR.
pub fn atr_percentile(
    atr: &[Option<Decimal>],
    m15: &[Candle],
    rolls: &[DateTime<Utc>],
    window: usize,
    t: usize,
) -> Option<Decimal> {
    let from = (t + 1).checked_sub(window)?;
    let starts = segment_starts(m15, rolls);
    if starts[from + 1..=t].iter().any(|s| *s) {
        return None;
    }
    let now = atr.get(t).copied().flatten()?;
    let mut below = 0usize;
    for a in &atr[from..=t] {
        if (*a)? <= now {
            below += 1;
        }
    }
    Some(Decimal::ONE_HUNDRED * Decimal::from(below) / Decimal::from(window))
}

/// Tramo del régimen: 0 = bajo (≤ primer borde), … , `bands.len()` = extremo.
pub fn regime_band(percentile: Decimal, bands: &[Decimal]) -> usize {
    bands.iter().take_while(|b| percentile > **b).count()
}

/// Hora UTC de una fecha y hora local (para tests y descriptivos).
pub fn local_to_utc(
    cfg: &ChampionshipConfig,
    day: NaiveDate,
    t: chrono::NaiveTime,
) -> Option<DateTime<Utc>> {
    cfg.timezone
        .from_local_datetime(&day.and_time(t))
        .single()
        .map(|d| d.with_timezone(&Utc))
}

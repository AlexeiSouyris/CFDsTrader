//! Vela **en formación** a partir de los precios por streaming de IBKR (#558). **Solo visual**: la
//! dibuja la UI y nada más la usa. Ningún indicador, patrón, señal ni estrategia la consume (siguen
//! con velas cerradas, CLAUDE.md §5b); un test revisa que esos crates no dependan de este.
//!
//! Todo lo de este archivo es puro (misma entrada, misma salida, sin reloj ni red):
//! - [`apply_tick`]: un precio con su hora → la vela en curso de un timeframe (abre, actualiza o la
//!   cierra porque el precio ya es del período siguiente).
//! - [`is_due`]: la vela terminó su período aunque no lleguen más precios.
//! - [`QuoteState`]: junta los eventos crudos de IBKR (tipo de dato, último precio, su hora) y arma
//!   las velas de todos los timeframes de un instrumento.
//!
//! Los períodos se alinean en UTC (`inicio = piso(hora / tamaño)`), igual que las velas históricas
//! de IBKR de 1m, 15m y 1h con horario extendido (`regular_hours_only = false`). La vela en
//! formación **no es** un `cfds_core::Candle`: no hay conversión entre los dos tipos.

use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;

/// Timeframes de la vela en formación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LiveTimeframe {
    M1,
    M5,
    M15,
    H1,
}

impl LiveTimeframe {
    pub const ALL: [LiveTimeframe; 4] = [
        LiveTimeframe::M1,
        LiveTimeframe::M5,
        LiveTimeframe::M15,
        LiveTimeframe::H1,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            LiveTimeframe::M1 => "1m",
            LiveTimeframe::M5 => "5m",
            LiveTimeframe::M15 => "15m",
            LiveTimeframe::H1 => "1h",
        }
    }

    pub fn duration(self) -> TimeDelta {
        match self {
            LiveTimeframe::M1 => TimeDelta::minutes(1),
            LiveTimeframe::M5 => TimeDelta::minutes(5),
            LiveTimeframe::M15 => TimeDelta::minutes(15),
            LiveTimeframe::H1 => TimeDelta::hours(1),
        }
    }

    fn index(self) -> usize {
        match self {
            LiveTimeframe::M1 => 0,
            LiveTimeframe::M5 => 1,
            LiveTimeframe::M15 => 2,
            LiveTimeframe::H1 => 3,
        }
    }
}

impl std::str::FromStr for LiveTimeframe {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        LiveTimeframe::ALL
            .into_iter()
            .find(|t| t.as_str() == s)
            .ok_or_else(|| format!("timeframe desconocido: {s:?} (1m, 5m, 15m o 1h)"))
    }
}

/// Qué datos manda IBKR, tal como lo informa (`marketDataType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiveDataType {
    /// En vivo (hace falta la suscripción de datos).
    Live,
    /// Demorados (sin suscripción): unos 10 minutos en CME, medido en #541.
    Delayed,
    /// Último valor en vivo con el mercado cerrado.
    Frozen,
    /// Último valor demorado con el mercado cerrado.
    DelayedFrozen,
}

impl LiveDataType {
    pub fn as_str(self) -> &'static str {
        match self {
            LiveDataType::Live => "live",
            LiveDataType::Delayed => "delayed",
            LiveDataType::Frozen => "frozen",
            LiveDataType::DelayedFrozen => "delayed_frozen",
        }
    }

    /// El código de IBKR (`reqMarketDataType`): 1 en vivo, 2 congelado, 3 demorado, 4 demorado
    /// congelado.
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(LiveDataType::Live),
            2 => Some(LiveDataType::Frozen),
            3 => Some(LiveDataType::Delayed),
            4 => Some(LiveDataType::DelayedFrozen),
            _ => None,
        }
    }

    pub fn code(self) -> i32 {
        match self {
            LiveDataType::Live => 1,
            LiveDataType::Frozen => 2,
            LiveDataType::Delayed => 3,
            LiveDataType::DelayedFrozen => 4,
        }
    }

    fn is_delayed(self) -> bool {
        matches!(self, LiveDataType::Delayed | LiveDataType::DelayedFrozen)
    }
}

/// Un precio operado, con la hora (UTC) en que se operó y el contrato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tick {
    pub at: DateTime<Utc>,
    pub price: Decimal,
    pub contract: String,
}

/// La vela en curso de un timeframe. Solo para mostrar: no es un `cfds_core::Candle`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveBar {
    pub contract: String,
    pub timeframe: LiveTimeframe,
    pub start: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    /// Hora del precio más nuevo que entró (el cierre es el de ese precio).
    pub last_at: DateTime<Utc>,
    /// Cuántos precios entraron.
    pub ticks: u32,
    /// La vela empezó a armarse a mitad de su período (primera después de suscribirse o de un
    /// cambio de contrato): la apertura, el máximo y el mínimo pueden no ser los del período entero.
    pub partial: bool,
}

impl LiveBar {
    pub fn end(&self) -> DateTime<Utc> {
        self.start + self.timeframe.duration()
    }

    fn open_with(tick: &Tick, timeframe: LiveTimeframe, partial: bool) -> Self {
        Self {
            partial,
            contract: tick.contract.clone(),
            timeframe,
            start: period_start(tick.at, timeframe),
            open: tick.price,
            high: tick.price,
            low: tick.price,
            close: tick.price,
            last_at: tick.at,
            ticks: 1,
        }
    }
}

/// Inicio del período de `timeframe` que contiene `t`, alineado en UTC.
pub fn period_start(t: DateTime<Utc>, timeframe: LiveTimeframe) -> DateTime<Utc> {
    let len = timeframe.duration().num_seconds();
    let secs = t.timestamp().div_euclid(len) * len;
    DateTime::<Utc>::from_timestamp(secs, 0).unwrap_or(t)
}

/// Qué pasó con la vela en curso al entrar un precio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// No había vela: abre una.
    Opened(LiveBar),
    /// El precio es del mismo período: actualiza máximo, mínimo y cierre.
    Updated(LiveBar),
    /// El precio es de un período posterior (puede haber períodos vacíos en el medio, un gap):
    /// la anterior queda cerrada tal como estaba y abre otra.
    Rolled { closed: LiveBar, current: LiveBar },
    /// Llegó otro contrato (cambio de vencimiento): la vela del anterior se descarta sin cerrarla
    /// (no es una vela completa de ninguna serie) y abre otra con el nuevo.
    ContractChanged { dropped: LiveBar, current: LiveBar },
    /// El precio es de un período anterior al de la vela en curso: se ignora (esa vela ya se
    /// cerró y una vela cerrada no cambia).
    Late,
}

/// Aplica un precio a la vela en curso de `timeframe` (`None` si no hay). Función pura.
///
/// `seen_after`: desde cuándo se vieron **todos** los operados (los posteriores a esa hora; `None`
/// si no se sabe). La vela que abre sin una anterior queda marcada `partial` si su período empezó
/// antes de eso (le pueden faltar precios del principio).
///
/// Un precio del mismo período pero más viejo que el último que entró (llegó tarde) mueve el
/// máximo o el mínimo, pero no el cierre.
pub fn apply_tick(
    current: Option<&LiveBar>,
    tick: &Tick,
    timeframe: LiveTimeframe,
    seen_after: Option<DateTime<Utc>>,
) -> Step {
    let Some(bar) = current else {
        let start = period_start(tick.at, timeframe);
        let partial = seen_after.is_none_or(|t0| start <= t0);
        return Step::Opened(LiveBar::open_with(tick, timeframe, partial));
    };
    if bar.contract != tick.contract {
        return Step::ContractChanged {
            dropped: bar.clone(),
            current: LiveBar::open_with(tick, timeframe, true),
        };
    }
    if tick.at < bar.start {
        return Step::Late;
    }
    if tick.at >= bar.end() {
        return Step::Rolled {
            closed: bar.clone(),
            // Se vieron los precios del período anterior: esta arranca con el primero del suyo.
            current: LiveBar::open_with(tick, timeframe, false),
        };
    }
    let mut next = bar.clone();
    next.high = next.high.max(tick.price);
    next.low = next.low.min(tick.price);
    next.ticks = next.ticks.saturating_add(1);
    if tick.at >= bar.last_at {
        next.close = tick.price;
        next.last_at = tick.at;
    }
    Step::Updated(next)
}

/// La vela terminó su período a la hora de los datos `data_now` (sin esperar otro precio).
pub fn is_due(bar: &LiveBar, data_now: DateTime<Utc>) -> bool {
    data_now >= bar.end()
}

/// Un evento crudo de IBKR, ya en decimales y UTC (lo arma [`crate::client`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuoteEvent {
    /// El contrato al que se suscribió (por ejemplo `MNQZ6` o `QQQ`).
    Contract(String),
    /// El tipo de dato que manda IBKR para esta suscripción.
    DataType(LiveDataType),
    /// Último precio operado (`LAST` o `DELAYED_LAST`), sin hora propia.
    Last {
        price: Decimal,
        received: DateTime<Utc>,
    },
    /// Hora del último precio operado (`LAST_TIMESTAMP` o `DELAYED_LAST_TIMESTAMP`, en segundos).
    LastTime {
        at: DateTime<Utc>,
        received: DateTime<Utc>,
    },
}

/// Una actualización para mostrar: la vela de un timeframe, en formación o recién cerrada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarUpdate {
    pub bar: LiveBar,
    /// `true`: sigue en formación. `false`: terminó su período y no cambia más.
    pub forming: bool,
}

/// Estado de los precios de **un** instrumento y sus velas en formación de todos los timeframes.
///
/// La hora de cada precio sale de la hora del último operado que manda IBKR (con segundos): el
/// precio espera su hora hasta `pending_wait`; si no llega, se usa la hora de llegada menos el
/// atraso medido. El atraso es `llegada − hora del operado` del último par (0 si es negativo).
///
/// Al suscribirse, IBKR manda primero el **último operado conocido**, que puede ser de hace horas
/// (con el mercado cerrado): ese primer par no arma velas ni mide atraso (medido en #558: QQQ fuera
/// de hora daba 2 h 22 min de "atraso"). Solo cuentan los operados nuevos que llegan después.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteState {
    pub contract: Option<String>,
    pub data_type: Option<LiveDataType>,
    pub delay: Option<TimeDelta>,
    timeframes: Vec<LiveTimeframe>,
    bars: [Option<LiveBar>; 4],
    pending: Option<(Decimal, DateTime<Utc>)>,
    pending_wait: TimeDelta,
    /// Hora del último operado visto (el primero es el que ya había al suscribirse).
    last_trade_at: Option<DateTime<Utc>>,
    /// Hora del operado que ya había al suscribirse: todos los posteriores se ven.
    seen_after: Option<DateTime<Utc>>,
    /// Fin de la última vela cerrada de cada timeframe: un precio anterior llegó tarde y no
    /// reabre ese período (una vela cerrada no cambia).
    closed_until: [Option<DateTime<Utc>>; 4],
}

impl QuoteState {
    pub fn new(timeframes: &[LiveTimeframe], pending_wait: TimeDelta) -> Self {
        let mut tfs: Vec<LiveTimeframe> = timeframes.to_vec();
        tfs.sort();
        tfs.dedup();
        Self {
            contract: None,
            data_type: None,
            delay: None,
            timeframes: tfs,
            bars: [None, None, None, None],
            pending: None,
            pending_wait,
            last_trade_at: None,
            seen_after: None,
            closed_until: [None; 4],
        }
    }

    pub fn timeframes(&self) -> &[LiveTimeframe] {
        &self.timeframes
    }

    /// La vela en curso de `timeframe`, si hay.
    pub fn bar(&self, timeframe: LiveTimeframe) -> Option<&LiveBar> {
        self.bars[timeframe.index()].as_ref()
    }

    /// Hora de los datos: `now` menos el atraso (sin atraso medido, el de un dato en vivo es 0 y
    /// el de uno demorado es desconocido → `None`).
    pub fn data_now(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match (self.delay, self.data_type) {
            (Some(d), _) => Some(now - d),
            (None, Some(LiveDataType::Live | LiveDataType::Frozen)) => Some(now),
            _ => None,
        }
    }

    /// Hora estimada de un precio que llegó a `received`, o `None` si no se puede saber.
    fn estimated_at(&self, received: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.data_now(received)
    }

    /// Un evento de IBKR → las velas que cambiaron.
    pub fn on_event(&mut self, ev: QuoteEvent) -> Vec<BarUpdate> {
        match ev {
            QuoteEvent::Contract(c) => {
                self.contract = Some(c);
                self.pending = None;
                Vec::new()
            }
            QuoteEvent::DataType(t) => {
                if self
                    .data_type
                    .is_some_and(|old| old.is_delayed() != t.is_delayed())
                {
                    // De demorado a en vivo (o al revés) el atraso medido ya no vale.
                    self.delay = None;
                }
                self.data_type = Some(t);
                Vec::new()
            }
            QuoteEvent::Last { price, received } => {
                if price <= Decimal::ZERO {
                    // IBKR manda -1 o 0 cuando no hay dato.
                    return Vec::new();
                }
                let mut out = self.flush_pending(None);
                self.pending = Some((price, received));
                out.extend(self.on_clock(received));
                out
            }
            QuoteEvent::LastTime { at, received } => {
                if self.last_trade_at.is_none() {
                    // El último operado que ya había al suscribirse: no es un precio nuevo.
                    self.last_trade_at = Some(at);
                    self.seen_after = Some(at);
                    self.pending = None;
                    return Vec::new();
                }
                self.last_trade_at = self.last_trade_at.max(Some(at));
                self.delay = Some((received - at).max(TimeDelta::zero()));
                self.flush_pending(Some(at))
            }
        }
    }

    /// Manda a las velas el precio que esperaba su hora: con `at` si llegó, si no con la estimada.
    fn flush_pending(&mut self, at: Option<DateTime<Utc>>) -> Vec<BarUpdate> {
        let Some((price, received)) = self.pending.take() else {
            return Vec::new();
        };
        if self.last_trade_at.is_none() {
            // Todavía no llegó la hora del último operado al suscribirse: este precio es ese.
            return Vec::new();
        }
        let Some(contract) = self.contract.clone() else {
            return Vec::new();
        };
        let Some(at) = at.or_else(|| self.estimated_at(received)) else {
            return Vec::new();
        };
        self.apply(&Tick {
            at,
            price,
            contract,
        })
    }

    fn apply(&mut self, tick: &Tick) -> Vec<BarUpdate> {
        let mut out = Vec::new();
        for tf in self.timeframes.clone() {
            let i = tf.index();
            if self.closed_until[i].is_some_and(|end| tick.at < end) {
                continue;
            }
            let slot = &mut self.bars[i];
            match apply_tick(slot.as_ref(), tick, tf, self.seen_after) {
                Step::Opened(b) | Step::Updated(b) => {
                    out.push(BarUpdate {
                        bar: b.clone(),
                        forming: true,
                    });
                    *slot = Some(b);
                }
                Step::Rolled { closed, current } => {
                    self.closed_until[i] = Some(closed.end());
                    out.push(BarUpdate {
                        bar: closed,
                        forming: false,
                    });
                    out.push(BarUpdate {
                        bar: current.clone(),
                        forming: true,
                    });
                    *slot = Some(current);
                }
                Step::ContractChanged { current, .. } => {
                    out.push(BarUpdate {
                        bar: current.clone(),
                        forming: true,
                    });
                    *slot = Some(current);
                }
                Step::Late => {}
            }
        }
        out
    }

    /// Paso del reloj: suelta el precio que esperó su hora demasiado y cierra las velas cuyo
    /// período terminó (a la hora de los datos, más `pending_wait` de margen), aunque no lleguen
    /// más precios.
    pub fn on_clock(&mut self, now: DateTime<Utc>) -> Vec<BarUpdate> {
        let mut out = Vec::new();
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, received)| now - *received >= self.pending_wait)
        {
            out.extend(self.flush_pending(None));
        }
        let Some(data_now) = self.data_now(now) else {
            return out;
        };
        for tf in self.timeframes.clone() {
            let slot = &mut self.bars[tf.index()];
            // Con un margen de `pending_wait`: un precio del final del período puede llegar un
            // poco más tarde que el atraso medido (medido: 600-604 s en demorados).
            if slot
                .as_ref()
                .is_some_and(|b| is_due(b, data_now - self.pending_wait))
                && let Some(b) = slot.take()
            {
                self.closed_until[tf.index()] = Some(b.end());
                out.push(BarUpdate {
                    bar: b,
                    forming: false,
                });
            }
        }
        out
    }

    /// Se cortó la suscripción: las velas en curso se descartan (no se cierran: les faltan datos).
    pub fn reset(&mut self) {
        self.bars = [None, None, None, None];
        self.pending = None;
        self.last_trade_at = None;
        self.seen_after = None;
        self.closed_until = [None; 4];
        self.contract = None;
        self.data_type = None;
        self.delay = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::dec;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 25, h, m, s).unwrap()
    }

    fn tick(t: DateTime<Utc>, p: Decimal) -> Tick {
        Tick {
            at: t,
            price: p,
            contract: "MNQZ6".into(),
        }
    }

    #[test]
    fn periods_align_on_utc() {
        let t = at(14, 37, 42);
        assert_eq!(period_start(t, LiveTimeframe::M1), at(14, 37, 0));
        assert_eq!(period_start(t, LiveTimeframe::M5), at(14, 35, 0));
        assert_eq!(period_start(t, LiveTimeframe::M15), at(14, 30, 0));
        assert_eq!(period_start(t, LiveTimeframe::H1), at(14, 0, 0));
        assert_eq!(period_start(at(14, 0, 0), LiveTimeframe::H1), at(14, 0, 0));
    }

    #[test]
    fn builds_and_rolls_a_bar() {
        let tf = LiveTimeframe::M1;
        let Step::Opened(b) = apply_tick(None, &tick(at(10, 0, 5), dec!(100)), tf, None) else {
            panic!()
        };
        let Step::Updated(b) = apply_tick(Some(&b), &tick(at(10, 0, 20), dec!(103)), tf, None)
        else {
            panic!()
        };
        let Step::Updated(b) = apply_tick(Some(&b), &tick(at(10, 0, 40), dec!(98)), tf, None)
        else {
            panic!()
        };
        assert_eq!(
            (b.open, b.high, b.low, b.close),
            (dec!(100), dec!(103), dec!(98), dec!(98))
        );
        assert_eq!(b.ticks, 3);
        let Step::Rolled { closed, current } =
            apply_tick(Some(&b), &tick(at(10, 1, 0), dec!(99)), tf, None)
        else {
            panic!()
        };
        assert_eq!(closed, b);
        assert_eq!(current.start, at(10, 1, 0));
        assert_eq!(current.open, dec!(99));
    }

    #[test]
    fn gap_closes_and_skips_empty_periods() {
        let tf = LiveTimeframe::M1;
        let Step::Opened(b) = apply_tick(None, &tick(at(10, 0, 5), dec!(100)), tf, None) else {
            panic!()
        };
        // Sin precios de 10:01 a 10:06: no se inventan velas vacías.
        let Step::Rolled { closed, current } =
            apply_tick(Some(&b), &tick(at(10, 7, 3), dec!(110)), tf, None)
        else {
            panic!()
        };
        assert_eq!(closed.start, at(10, 0, 0));
        assert_eq!(current.start, at(10, 7, 0));
        assert_eq!(current.open, dec!(110));
    }

    #[test]
    fn late_tick_never_changes_a_closed_period_nor_the_close() {
        let tf = LiveTimeframe::M1;
        let Step::Opened(b) = apply_tick(None, &tick(at(10, 1, 30), dec!(100)), tf, None) else {
            panic!()
        };
        assert_eq!(
            apply_tick(Some(&b), &tick(at(10, 0, 59), dec!(500)), tf, None),
            Step::Late
        );
        // Del mismo período pero más viejo que el último: mueve el máximo, no el cierre.
        let Step::Updated(b2) = apply_tick(Some(&b), &tick(at(10, 1, 10), dec!(105)), tf, None)
        else {
            panic!()
        };
        assert_eq!(b2.high, dec!(105));
        assert_eq!(b2.close, dec!(100));
        assert_eq!(b2.last_at, at(10, 1, 30));
    }

    #[test]
    fn contract_change_drops_the_partial_bar() {
        let tf = LiveTimeframe::M15;
        let Step::Opened(b) = apply_tick(None, &tick(at(10, 0, 5), dec!(100)), tf, None) else {
            panic!()
        };
        let next = Tick {
            contract: "MNQH7".into(),
            ..tick(at(10, 3, 0), dec!(140))
        };
        let Step::ContractChanged { dropped, current } = apply_tick(Some(&b), &next, tf, None)
        else {
            panic!()
        };
        assert_eq!(dropped, b);
        assert_eq!(current.contract, "MNQH7");
        assert_eq!(current.open, dec!(140));
        assert_eq!(current.start, at(10, 0, 0));
    }

    /// Lo que manda IBKR al suscribirse: el último operado que ya había (aunque sea viejo).
    fn subscribed(s: &mut QuoteState, data_type: LiveDataType, last_at: DateTime<Utc>) {
        s.on_event(QuoteEvent::Contract("MNQZ6".into()));
        s.on_event(QuoteEvent::DataType(data_type));
        s.on_event(QuoteEvent::Last {
            price: dec!(90),
            received: at(10, 0, 0),
        });
        let up = s.on_event(QuoteEvent::LastTime {
            at: last_at,
            received: at(10, 0, 0),
        });
        assert!(up.is_empty(), "el último operado previo no arma velas");
        assert_eq!(s.delay, None, "ni mide atraso");
    }

    #[test]
    fn stale_last_trade_at_subscription_is_not_a_bar() {
        // QQQ fuera de hora (medido): el último operado era de 2 h 22 min antes.
        let mut s = QuoteState::new(&[LiveTimeframe::M1], TimeDelta::seconds(1));
        subscribed(&mut s, LiveDataType::Delayed, at(7, 38, 0));
        assert!(s.on_clock(at(10, 0, 5)).is_empty());
        assert!(s.bar(LiveTimeframe::M1).is_none());
    }

    #[test]
    fn first_bar_is_partial_and_the_next_ones_are_not() {
        let mut s = QuoteState::new(&[LiveTimeframe::M1], TimeDelta::seconds(1));
        // Al suscribirse, el último operado era de 10:00:10: lo de antes en ese minuto no se vio.
        subscribed(&mut s, LiveDataType::Live, at(10, 0, 10));
        let mut push = |p: Decimal, t: DateTime<Utc>| {
            let mut up = s.on_event(QuoteEvent::Last {
                price: p,
                received: t,
            });
            up.extend(s.on_event(QuoteEvent::LastTime { at: t, received: t }));
            up
        };
        let up = push(dec!(100), at(10, 0, 30));
        assert!(up[0].bar.partial);
        let up = push(dec!(101), at(10, 1, 2));
        assert!(up[0].bar.partial && !up[0].forming);
        assert!(!up[1].bar.partial && up[1].forming);
        // Con el último operado previo de horas antes, la primera vela no es parcial.
        let mut s = QuoteState::new(&[LiveTimeframe::M15], TimeDelta::seconds(1));
        subscribed(&mut s, LiveDataType::Delayed, at(7, 0, 0));
        s.on_event(QuoteEvent::Last {
            price: dec!(100),
            received: at(10, 10, 0),
        });
        let up = s.on_event(QuoteEvent::LastTime {
            at: at(10, 0, 0),
            received: at(10, 10, 0),
        });
        assert!(!up[0].bar.partial);
    }

    #[test]
    fn end_of_period_without_ticks_closes_by_clock() {
        let mut s = QuoteState::new(&[LiveTimeframe::M1], TimeDelta::seconds(1));
        subscribed(&mut s, LiveDataType::Live, at(9, 59, 58));
        s.on_event(QuoteEvent::Last {
            price: dec!(100),
            received: at(10, 0, 10),
        });
        let up = s.on_event(QuoteEvent::LastTime {
            at: at(10, 0, 10),
            received: at(10, 0, 10),
        });
        assert_eq!(up.len(), 1);
        assert!(up[0].forming);
        assert!(
            s.on_clock(at(10, 1, 0)).is_empty(),
            "margen de pending_wait"
        );
        let up = s.on_clock(at(10, 1, 1));
        assert_eq!(up.len(), 1);
        assert!(!up[0].forming);
        assert_eq!(up[0].bar.close, dec!(100));
        assert!(s.bar(LiveTimeframe::M1).is_none());
    }

    #[test]
    fn delayed_data_uses_trade_time_and_delay() {
        let mut s = QuoteState::new(&[LiveTimeframe::M1], TimeDelta::seconds(1));
        subscribed(&mut s, LiveDataType::Delayed, at(9, 50, 0));
        // Sin atraso medido todavía, un precio demorado sin su hora no tiene hora: no entra.
        s.on_event(QuoteEvent::Last {
            price: dec!(100),
            received: at(10, 10, 0),
        });
        assert!(s.on_clock(at(10, 10, 2)).is_empty());
        // Precio y su hora (10 minutos antes).
        s.on_event(QuoteEvent::Last {
            price: dec!(101),
            received: at(10, 10, 5),
        });
        let up = s.on_event(QuoteEvent::LastTime {
            at: at(10, 0, 5),
            received: at(10, 10, 5),
        });
        assert_eq!(up[0].bar.start, at(10, 0, 0));
        assert_eq!(s.delay, Some(TimeDelta::minutes(10)));
        // Un precio sin su hora: llegada menos el atraso.
        s.on_event(QuoteEvent::Last {
            price: dec!(102),
            received: at(10, 10, 20),
        });
        let up = s.on_clock(at(10, 10, 22));
        assert_eq!(up[0].bar.close, dec!(102));
        assert_eq!(up[0].bar.last_at, at(10, 0, 20));
        // El cierre por reloj es a la hora de los datos: 10:01 de datos = 10:11 de reloj.
        assert!(s.on_clock(at(10, 11, 0)).is_empty());
        let up = s.on_clock(at(10, 11, 1));
        assert!(!up[0].forming);
    }

    #[test]
    fn ignores_non_positive_prices_and_needs_a_contract() {
        let mut s = QuoteState::new(&[LiveTimeframe::M1], TimeDelta::seconds(1));
        s.on_event(QuoteEvent::DataType(LiveDataType::Live));
        s.on_event(QuoteEvent::LastTime {
            at: at(9, 59, 0),
            received: at(9, 59, 0),
        });
        s.on_event(QuoteEvent::Last {
            price: dec!(-1),
            received: at(10, 0, 0),
        });
        assert!(s.on_clock(at(10, 0, 5)).is_empty());
        s.on_event(QuoteEvent::Last {
            price: dec!(100),
            received: at(10, 0, 6),
        });
        // Sin contrato todavía: no hay vela.
        assert!(s.on_clock(at(10, 0, 8)).is_empty());
        assert!(s.bar(LiveTimeframe::M1).is_none());
    }

    #[test]
    fn switching_between_delayed_and_live_forgets_the_delay() {
        let mut s = QuoteState::new(&[LiveTimeframe::M1], TimeDelta::seconds(1));
        subscribed(&mut s, LiveDataType::Delayed, at(9, 50, 0));
        s.on_event(QuoteEvent::LastTime {
            at: at(10, 0, 0),
            received: at(10, 10, 0),
        });
        assert!(s.delay.is_some());
        s.on_event(QuoteEvent::DataType(LiveDataType::Live));
        assert_eq!(s.delay, None);
        assert_eq!(s.data_now(at(10, 0, 0)), Some(at(10, 0, 0)));
    }

    #[test]
    fn timeframe_names_round_trip() {
        for tf in LiveTimeframe::ALL {
            assert_eq!(tf.as_str().parse::<LiveTimeframe>(), Ok(tf));
        }
        assert!("1d".parse::<LiveTimeframe>().is_err());
        for code in 1..=4 {
            assert_eq!(
                LiveDataType::from_code(code).map(LiveDataType::code),
                Some(code)
            );
        }
        assert_eq!(LiveDataType::from_code(0), None);
    }
}

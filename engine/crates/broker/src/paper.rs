//! Broker simulado (paper) contra las velas guardadas (fase 7, #278).
//!
//! ```text
//! orden (order.v1, sin tamaño, con stop)
//!   → ¿ya vista? devuelve el mismo informe (idempotente por order_id)
//!   → vela de la señal = última cerrada en created_at; vela del fill = la primera que empieza
//!     en created_at o después (spec §6: entra en la apertura de t+1)
//!   → cfds_risk::check (stop, distancia en ATR, riesgo, apalancamiento, margen, pérdida diaria,
//!     drawdown, kill switch, posiciones, mínimo del broker): **el único que da el tamaño**
//!   → fill a la apertura con medio spread + slippage, y comisión (cfds-backtest)
//!   → si la vela del fill todavía no llegó (una orden manual de ahora, #471): **pendiente**.
//!     No entra al registro: espera esa vela y recién ahí se decide igual que en la
//!     reconstrucción (`settle_pending`), que ya tiene la vela. Se guarda como pendiente
//!     (#479): al arrancar se retoma sin evaluarla otra vez (`restore_pending`)
//! cada posición se valúa con cfds_backtest::simulate (stop, TP1, trailing, tiempo, gaps y el
//! cierre de fin de semana de la spec §8, con las mismas marcas que el backtest: #334)
//! y paga la financiación nocturna (swap) de cada corte en que siguió abierta, con la tasa de
//! referencia vigente en ese corte (cfds_backtest::funding_cutoffs y funding_over, #79)
//! ```
//! Funciones del tiempo: `positions(now)` y `account(now)` usan solo velas cerradas en `now`.
//! Sin ejecuciones parciales (la spec §12 las prevé solo para el registro real).
//! Si en algún corte no hay tasa de referencia publicada, no se inventa: ese corte no se cobra y
//! se activa el kill switch por error de datos (spec §12).
//!
//! Moneda (ADR 0013 §4): los montos de un contrato en otra moneda que la cuenta (el oro, en USD)
//! pasan a EUR con el tipo **vigente en cada momento** (`MarketSetup::account_to_instrument`,
//! el mismo `AccountFx` del backtest): la comisión de entrada al tipo de la apertura, cada salida
//! al de su vela, lo no realizado y el margen al de `now`, y el riesgo de una orden al de la vela
//! de la señal. Sin tipo publicado a la hora de la señal, la orden se rechaza
//! (`missing_fx_rate`); si falta para valuar algo ya abierto, ese monto no se cuenta y se activa
//! el kill switch por error de datos, como con la financiación.

use std::collections::{BTreeMap, BTreeSet};

use std::cell::Cell;

use cfds_backtest::{
    AccountFx, CostConfig, Execution, ExitConfig, ExitReason, Fill, FundingRates, InstrumentCosts,
    Trade, commission, fill_price, funding_cutoffs, funding_over, simulate,
};
use cfds_core::{
    AccountCurrency, Candle, ClientId, InstrumentId, KillSwitch, Order, OrderRejection,
    OrderReport, OrderSide, OrderStatus, PaperAccount, PaperMode, Position, TicketVerdict,
};
use cfds_indicators::{AtrSeed, wilder_atr, wilder_atr_after};
use cfds_market_data::{ExchangeCalendar, Session, trading_window};
use cfds_risk::{
    Account, InstrumentLimits, OpenPosition, Rejection, Request, RiskConfig, Sizing, check,
    mark_peak,
};
use cfds_strategy::Side;
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuración del broker simulado. Todo sale de `config/` (broker, risk, backtest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerConfig {
    pub initial_equity: Decimal,
    pub atr_period: usize,
    pub risk: RiskConfig,
    pub costs: CostConfig,
    pub exits: ExitConfig,
    /// Zona del "día" de la pérdida diaria (spec §10: Madrid).
    pub day_tz: Tz,
    /// Cada cuánto se reconcilia el broker con el registro guardado (`[reconcile]`).
    pub reconcile_every: chrono::TimeDelta,
    /// Sesión de cada instrumento (`config/sessions.toml` y feriados), para el cierre de fin de
    /// semana de la spec §8 (#334). Un instrumento sin sesión acá no se cierra el viernes: en
    /// producción se cargan todos con [`BrokerConfig::with_calendar`].
    pub sessions: BTreeMap<InstrumentId, Session>,
}

impl BrokerConfig {
    /// `config/broker.toml` más los límites, costos y salidas ya leídos.
    pub fn from_toml(
        text: &str,
        risk: RiskConfig,
        costs: CostConfig,
        exits: ExitConfig,
        day_tz: Tz,
    ) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            paper: RawPaper,
            reconcile: RawReconcile,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawReconcile {
            every_minutes: u32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawPaper {
            initial_equity: String,
            atr_period: usize,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let initial_equity: Decimal = raw
            .paper
            .initial_equity
            .parse()
            .map_err(|e| format!("paper.initial_equity: {e}"))?;
        if initial_equity <= Decimal::ZERO || raw.paper.atr_period == 0 {
            return Err("paper: initial_equity y atr_period tienen que ser > 0".into());
        }
        if raw.reconcile.every_minutes == 0 {
            return Err("reconcile.every_minutes tiene que ser > 0".into());
        }
        Ok(Self {
            initial_equity,
            atr_period: raw.paper.atr_period,
            risk,
            costs,
            exits,
            day_tz,
            reconcile_every: chrono::TimeDelta::minutes(i64::from(raw.reconcile.every_minutes)),
            sessions: BTreeMap::new(),
        })
    }

    /// La misma config con la sesión de cada instrumento de `instruments` según `calendar`, para
    /// el cierre de fin de semana (#334). Un instrumento sin bolsa en el calendario es un error.
    pub fn with_calendar<'a>(
        mut self,
        calendar: &ExchangeCalendar,
        instruments: impl IntoIterator<Item = &'a InstrumentId>,
    ) -> Result<Self, String> {
        for id in instruments {
            let session = calendar
                .session_for(id)
                .ok_or_else(|| format!("{id}: sin bolsa en el calendario de sesiones"))?;
            self.sessions.insert(id.clone(), session.clone());
        }
        Ok(self)
    }
}

/// Marcas de cierre de fin de semana de `candles` (spec §8), las mismas que usa el backtest
/// (`trading_window::is_week_close_candle`). Sin sesión, o fuera de la cobertura del calendario,
/// no hay cierre forzado. Antes de `from` no hace falta calcularlas.
fn week_close_flags(session: Option<&Session>, candles: &[Candle], from: usize) -> Vec<bool> {
    let Some(session) = session else {
        return Vec::new();
    };
    candles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            i >= from && trading_window::is_week_close_candle(session, c).unwrap_or(false)
        })
        .collect()
}

/// Lo que el broker necesita de un instrumento: límites y costos del catálogo, tipo de cambio y
/// las velas guardadas (en orden).
#[derive(Debug, Clone)]
pub struct MarketSetup {
    pub limits: InstrumentLimits,
    pub costs: InstrumentCosts,
    /// Moneda del instrumento por 1 unidad de la moneda de la cuenta, en cada momento
    /// (`AccountFx::Same` si son iguales; la serie `fx_rates` si no, ADR 0013).
    pub account_to_instrument: AccountFx,
    pub candles: Vec<Candle>,
    /// Financiación nocturna: corte del catálogo y tasa de referencia (€STR guardado o constante).
    pub funding: FundingSetup,
}

/// Lo que hace falta para la financiación nocturna (el mismo cálculo que el backtest).
#[derive(Debug, Clone)]
pub struct FundingSetup {
    /// Hora del corte (`funding_cutoff` del catálogo), en `tz`.
    pub cutoff: NaiveTime,
    pub tz: Tz,
    pub rates: FundingRates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OpenTrade {
    /// Posición de su orden en el registro (para devolver las operaciones en ese orden).
    pub(crate) seq: usize,
    position_id: ClientId,
    order_id: ClientId,
    instrument: InstrumentId,
    side: OrderSide,
    size: Decimal,
    /// Apertura de la vela del fill (precio medio): base de las salidas.
    entry_mid: Decimal,
    /// Precio pagado (con medio spread y slippage).
    entry_fill: Decimal,
    /// Índice de la vela del fill en las velas del broker (que pueden empezar en un punto de
    /// control, #463).
    pub(crate) entry_index: usize,
    entry_commission: Decimal,
    stop: Decimal,
    tp1: Option<Decimal>,
    margin: Decimal,
    pub(crate) opened_at: DateTime<Utc>,
}

impl OpenTrade {
    pub(crate) fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
}

/// Operaciones ya cerradas que selló un punto de control (#463): su resultado no cambia más, así
/// que no se vuelven a valuar ni hacen falta sus velas.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Sealed {
    /// Lo realizado por cada una, en EUR (comisiones, salidas y financiación), con la posición
    /// de su orden en el registro: el capital se suma en ese mismo orden, así da exactamente lo
    /// mismo que valuándolas (la suma de decimales redondea según el orden).
    pub(crate) realized: Vec<(usize, Decimal)>,
    /// Momento de su último movimiento: antes de esto no se puede valuar sin ellas.
    pub(crate) until: Option<DateTime<Utc>>,
    /// Sus instrumentos: valuarlas en `t` miraba el tipo de cambio de `t` (aunque el monto sea
    /// 0), y sin tipo activaba el kill switch. Se sigue mirando igual.
    pub(crate) instruments: BTreeSet<InstrumentId>,
    /// Su resultado final (`trade_outcomes`), con la posición de su orden en el registro.
    pub(crate) outcomes: Vec<(usize, PaperTrade)>,
}

/// Una salida de una operación del broker simulado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperExit {
    /// Fin de la vela de la salida.
    pub time: DateTime<Utc>,
    pub mid: Decimal,
    pub price: Decimal,
    pub fraction: Decimal,
    /// Seguía abierta: se cierra al último cierre (como `EndOfData` en el backtest).
    pub end_of_data: bool,
    /// Motivo de la salida; `None` si `end_of_data` (#333).
    pub reason: Option<ExitReason>,
}

/// Una operación del broker simulado, valuada (ver [`PaperBroker::trade_outcomes`]). Montos en la
/// moneda del instrumento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperTrade {
    pub order_id: ClientId,
    pub instrument: InstrumentId,
    pub side: OrderSide,
    pub opened_at: DateTime<Utc>,
    pub entry_mid: Decimal,
    pub entry_fill: Decimal,
    pub size: Decimal,
    pub stop: Decimal,
    pub exits: Vec<PaperExit>,
    pub commission: Decimal,
    pub funding: Decimal,
    pub gross_pnl: Decimal,
    pub net_pnl: Decimal,
}

/// Lo que pasó en el broker, para los logs estructurados y las alertas (fase 9, #297). Quien
/// usa el broker los saca con [`PaperBroker::take_events`]; el broker no escribe logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BrokerEvent {
    /// Llegó una orden nueva (las repetidas por `order_id` no cuentan).
    OrderReceived {
        order_id: ClientId,
        instrument_id: InstrumentId,
        side: OrderSide,
        created_at: DateTime<Utc>,
    },
    /// Una orden repetida: se devolvió el informe guardado.
    OrderDuplicate {
        order_id: ClientId,
    },
    /// El riesgo la aceptaría, pero la vela del fill (la primera que empieza en `created_at` o
    /// después) todavía no llegó: queda pendiente hasta esa vela (#471).
    OrderPending {
        order_id: ClientId,
        created_at: DateTime<Utc>,
    },
    /// La decisión del riesgo (o el motivo del broker) sobre una orden enviada.
    RiskDecision {
        order_id: ClientId,
        accepted: bool,
        rejection: Option<OrderRejection>,
        size: Option<Decimal>,
        as_of: DateTime<Utc>,
    },
    OrderReport {
        report: OrderReport,
    },
    /// El kill switch se activó (no se desactiva desde el broker).
    KillSwitch {
        reason: String,
        at: DateTime<Utc>,
    },
}

/// Por qué una orden guardada como pendiente no se puede retomar (#479).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestorePending {
    /// Ya está en el registro: se decidió.
    AlreadyDecided,
    /// Ya está pendiente.
    AlreadyPending,
    /// Su instrumento no se opera en este arranque (no está en los mercados del broker).
    UnknownInstrument,
}

/// El broker simulado. Solo modo paper.
#[derive(Debug, Clone)]
pub struct PaperBroker {
    cfg: BrokerConfig,
    markets: BTreeMap<InstrumentId, MarketSetup>,
    reports: BTreeMap<ClientId, OrderReport>,
    /// Las órdenes procesadas, en el orden en que llegaron, con su informe: lo que se guarda
    /// para reconstruir el broker al arrancar (ver `crate::reconcile`).
    journal: Vec<(Order, OrderReport)>,
    /// Órdenes que el riesgo aceptaría pero cuya vela del fill todavía no llegó (#471), en
    /// orden de llegada. No están en el registro: se deciden en `settle_pending`.
    pending: Vec<Order>,
    /// Si está, las velas se recortan a esto antes de la **primera** orden que entra al
    /// registro (#438), en el momento en que entra: con órdenes pendientes (#471) la primera en
    /// llegar puede no ser la primera del registro.
    first_order_window: Option<TimeDelta>,
    events: Vec<BrokerEvent>,
    pub(crate) trades: Vec<OpenTrade>,
    pub(crate) kill_switch: Option<(String, DateTime<Utc>)>,
    /// Máximo del capital para el drawdown (reducción gradual y kill switch), con el capital
    /// valuado a mercado al cierre de **cada** vela cerrada hasta `peak_through`, como el
    /// backtest de cartera (#691, `cfds_risk::mark_peak`). Lo pone al día `assess` (cada orden)
    /// hasta la vela de la señal, nunca una lectura: el valor depende solo del registro y de las
    /// velas, así la reconstrucción desde el registro llega al mismo valor (#464).
    pub(crate) peak_equity: Decimal,
    /// Hasta qué cierre de vela está incluido el capital en `peak_equity` (`None`: ninguno).
    pub(crate) peak_through: Option<DateTime<Utc>>,
    /// Estado del ATR de Wilder antes de la primera vela de cada instrumento cuyas velas
    /// empiezan en un punto de control (#463): el ATR sigue igual que con todas las velas.
    pub(crate) atr_seeds: BTreeMap<InstrumentId, AtrSeed>,
    /// Lo que selló el último punto de control (#463).
    pub(crate) sealed: Sealed,
    /// Una valuación anterior al último movimiento de lo sellado (no se puede repetir igual sin
    /// las velas): se activa el kill switch, nunca se adivina.
    before_checkpoint: Cell<Option<DateTime<Utc>>>,
    /// Primer corte sin tasa de referencia que encontró una valuación (error de datos).
    missing_rate: Cell<Option<DateTime<Utc>>>,
    /// Primer momento sin tipo de cambio que encontró una valuación (error de datos).
    missing_fx: Cell<Option<DateTime<Utc>>>,
}

fn strategy_side(s: OrderSide) -> Side {
    match s {
        OrderSide::Long => Side::Long,
        OrderSide::Short => Side::Short,
    }
}

fn sign(s: OrderSide) -> Decimal {
    match s {
        OrderSide::Long => Decimal::ONE,
        OrderSide::Short => Decimal::NEGATIVE_ONE,
    }
}

/// Motivo del riesgo → motivo del contrato.
pub fn rejection_code(r: &Rejection) -> OrderRejection {
    match r {
        Rejection::InvalidInput(_) => OrderRejection::InvalidInput,
        Rejection::KillSwitchActive => OrderRejection::KillSwitchActive,
        Rejection::DrawdownLimit => OrderRejection::DrawdownLimit,
        Rejection::DailyLossLimit => OrderRejection::DailyLossLimit,
        Rejection::MaxOpenPositions => OrderRejection::MaxOpenPositions,
        Rejection::InstrumentAlreadyOpen => OrderRejection::InstrumentAlreadyOpen,
        Rejection::StopTooClose => OrderRejection::StopTooClose,
        Rejection::StopTooFar => OrderRejection::StopTooFar,
        Rejection::InsufficientMargin => OrderRejection::InsufficientMargin,
        Rejection::BelowMinDealSize => OrderRejection::BelowMinDealSize,
        Rejection::MinDealSizeUnknown => OrderRejection::MinDealSizeUnknown,
        Rejection::GroupRiskLimit => OrderRejection::GroupRiskLimit,
    }
}

/// Lo que se sabe de una orden al cierre de la vela de la señal.
struct Assessment {
    as_of: DateTime<Utc>,
    reference: Option<Decimal>,
    fill_index: Option<usize>,
    result: Result<Sizing, OrderRejection>,
}

/// Valuación de una posición en un momento.
struct Valued {
    realized_eur: Decimal,
    /// Realizado por fecha (fin de la vela de cada salida), en EUR.
    realized_events: Vec<(DateTime<Utc>, Decimal)>,
    open_fraction: Decimal,
    last: Option<Decimal>,
    unrealized: Decimal,
    as_of: Option<DateTime<Utc>>,
}

impl PaperBroker {
    pub fn new(cfg: BrokerConfig, markets: BTreeMap<InstrumentId, MarketSetup>) -> Self {
        let peak = cfg.initial_equity;
        Self {
            cfg,
            markets,
            reports: BTreeMap::new(),
            journal: Vec::new(),
            pending: Vec::new(),
            first_order_window: None,
            events: Vec::new(),
            trades: Vec::new(),
            kill_switch: None,
            peak_equity: peak,
            peak_through: None,
            atr_seeds: BTreeMap::new(),
            sealed: Sealed::default(),
            before_checkpoint: Cell::new(None),
            missing_rate: Cell::new(None),
            missing_fx: Cell::new(None),
        }
    }

    /// Agrega una vela nueva (cerrada) de un instrumento, en orden.
    pub fn push_candle(&mut self, candle: Candle) -> Result<(), &'static str> {
        let m = self
            .markets
            .get_mut(&candle.instrument_id)
            .ok_or("instrumento desconocido")?;
        if m.candles.last().is_some_and(|c| c.start >= candle.start) {
            return Err("vela fuera de orden o repetida");
        }
        m.candles.push(candle);
        Ok(())
    }

    /// Activa el kill switch (límite superado, error de datos, desconexión o reconciliación).
    /// No se desactiva desde acá: reactivar el sistema pide la votación de la spec §1.2.
    pub fn activate_kill_switch(&mut self, reason: impl Into<String>, at: DateTime<Utc>) {
        if self.kill_switch.is_none() {
            let reason = reason.into();
            self.events.push(BrokerEvent::KillSwitch {
                reason: reason.clone(),
                at,
            });
            self.kill_switch = Some((reason, at));
        }
    }

    /// Los eventos desde la última vez (y los vacía).
    pub fn take_events(&mut self) -> Vec<BrokerEvent> {
        std::mem::take(&mut self.events)
    }

    /// Fija el comienzo de las velas antes de la **primera** orden (#438): quita las velas que
    /// empiezan antes de `from`. Quien reconstruye el broker carga las velas desde ese mismo
    /// punto, así el ATR de Wilder (que depende de la primera vela) sale igual en la sesión y en
    /// la reconstrucción. Con órdenes ya procesadas no cambia nada y devuelve error: cambiar
    /// las velas por debajo de un informe es repintado.
    pub fn start_candles_at(&mut self, from: DateTime<Utc>) -> Result<(), &'static str> {
        if !self.journal.is_empty() {
            return Err("el broker ya procesó órdenes: sus velas no se recortan");
        }
        for m in self.markets.values_mut() {
            m.candles.retain(|c| c.start >= from);
        }
        Ok(())
    }

    /// Agrega velas **anteriores** a las que tiene un instrumento, mientras el registro está
    /// vacío (#481): quien carga las velas desde `now` menos la ventana y recibe una primera
    /// orden con `created_at` anterior a `now` completa así las que le faltan desde esa orden
    /// menos la ventana, como las cargará la reconstrucción (#438). Con órdenes ya procesadas,
    /// con el instrumento ya cortado por un punto de control, o con velas desordenadas o que no
    /// terminan antes de la primera que tiene, no cambia nada y devuelve error.
    pub fn prepend_candles(
        &mut self,
        instrument: &InstrumentId,
        candles: Vec<Candle>,
    ) -> Result<(), &'static str> {
        if !self.journal.is_empty() {
            return Err("el broker ya procesó órdenes: sus velas no se cambian");
        }
        if self.atr_seeds.contains_key(instrument) {
            return Err("el instrumento ya arranca en un punto de control");
        }
        let m = self
            .markets
            .get_mut(instrument)
            .ok_or("instrumento desconocido")?;
        if candles.iter().any(|c| &c.instrument_id != instrument)
            || candles.windows(2).any(|w| w[0].start >= w[1].start)
        {
            return Err("velas de otro instrumento o fuera de orden");
        }
        if let (Some(last), Some(first)) = (candles.last(), m.candles.first())
            && last.start >= first.start
        {
            return Err("las velas agregadas no terminan antes de las que ya están");
        }
        m.candles.splice(0..0, candles);
        Ok(())
    }

    /// Las velas van a empezar `window` antes de la primera orden que entre al registro
    /// ([`Self::start_candles_at`] en ese momento), como las carga la reconstrucción (#438).
    /// Con órdenes ya en el registro no cambia nada.
    pub fn anchor_candles_before_first_order(&mut self, window: TimeDelta) {
        self.first_order_window = Some(window);
    }

    /// Reemplaza la serie de tasas de referencia de la financiación de un instrumento por una
    /// leída de nuevo (#439), con las publicaciones que llegaron desde el arranque. Cada corte
    /// usa la tasa publicada hasta ese corte (`DailyRates`, sin mirar el futuro), así que las
    /// publicaciones nuevas no cambian los cortes ya pasados: solo dejan de usar una tasa vieja
    /// en los cortes nuevos. Un instrumento desconocido es un error.
    pub fn update_funding_rates(
        &mut self,
        instrument: &InstrumentId,
        rates: FundingRates,
    ) -> Result<(), &'static str> {
        let m = self
            .markets
            .get_mut(instrument)
            .ok_or("instrumento desconocido")?;
        m.funding.rates = rates;
        Ok(())
    }

    /// Reemplaza la serie del tipo de cambio cuenta → instrumento por una leída de nuevo (#439,
    /// ADR 0013), con las publicaciones que llegaron desde el arranque. Cada momento usa el tipo
    /// publicado hasta ese momento (`FxRates`, sin mirar el futuro): lo ya valuado no cambia.
    pub fn update_account_fx(
        &mut self,
        instrument: &InstrumentId,
        fx: AccountFx,
    ) -> Result<(), &'static str> {
        let m = self
            .markets
            .get_mut(instrument)
            .ok_or("instrumento desconocido")?;
        m.account_to_instrument = fx;
        Ok(())
    }

    /// Los mercados con sus velas (para reconstruir aparte, por ejemplo al reconciliar).
    pub fn markets(&self) -> &BTreeMap<InstrumentId, MarketSetup> {
        &self.markets
    }

    pub(crate) fn markets_mut(&mut self) -> &mut BTreeMap<InstrumentId, MarketSetup> {
        &mut self.markets
    }

    /// Solo para arrancar desde un punto de control (#463): las órdenes que ya cubre, con sus
    /// informes (para la idempotencia y la reconciliación).
    pub(crate) fn set_journal(&mut self, journal: &[(Order, OrderReport)]) {
        self.reports = journal
            .iter()
            .map(|(o, r)| (o.order_id.clone(), r.clone()))
            .collect();
        self.journal = journal.to_vec();
    }

    /// El ATR de Wilder de `candles` (velas del broker de `instrument`, desde la primera): si
    /// empiezan en un punto de control, sigue desde su estado (#463).
    pub(crate) fn atr(
        &self,
        instrument: &InstrumentId,
        candles: &[Candle],
    ) -> Vec<Option<Decimal>> {
        match self.atr_seeds.get(instrument) {
            Some(seed) => wilder_atr_after(*seed, candles, self.cfg.atr_period),
            None => wilder_atr(candles, self.cfg.atr_period),
        }
    }

    /// Lo sellado por un punto de control, mirado en `t` (#463): su resultado es constante
    /// desde su último movimiento. Mira el tipo de cambio de `t` de sus instrumentos como lo
    /// hacía su valuación (`with_last`: solo si el instrumento ya tiene una vela cerrada en `t`).
    fn touch_sealed(&self, t: DateTime<Utc>, with_last: bool) {
        if self.sealed.until.is_some_and(|until| t < until)
            && self.before_checkpoint.get().is_none_or(|prev| t < prev)
        {
            self.before_checkpoint.set(Some(t));
        }
        for id in &self.sealed.instruments {
            let closed = self
                .markets
                .get(id)
                .is_some_and(|m| m.candles.first().is_some_and(|c| c.end() <= t));
            if !with_last || closed {
                self.to_eur(id, Decimal::ZERO, t);
            }
        }
    }

    /// Las marcas de error de datos pendientes (para que un punto de control no selle nada
    /// valuado con datos faltantes).
    pub(crate) fn take_data_flags(&self) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
        (self.missing_rate.take(), self.missing_fx.take())
    }

    pub(crate) fn restore_data_flags(&self, flags: (Option<DateTime<Utc>>, Option<DateTime<Utc>>)) {
        self.missing_rate.set(flags.0);
        self.missing_fx.set(flags.1);
    }

    pub fn config(&self) -> &BrokerConfig {
        &self.cfg
    }

    pub fn kill_switch_active(&self) -> bool {
        self.kill_switch.is_some()
    }

    /// El kill switch activo: motivo y desde cuándo.
    pub fn kill_switch(&self) -> Option<(&str, DateTime<Utc>)> {
        self.kill_switch.as_ref().map(|(r, at)| (r.as_str(), *at))
    }

    /// Solo para reconstruir el broker desde el registro (`crate::reconcile`): una
    /// liberación **ya guardada** (después de la votación de la spec §1.2, #441). El broker en
    /// marcha nunca se libera solo.
    pub(crate) fn replay_kill_switch_release(&mut self) {
        self.kill_switch = None;
    }

    /// Si una valuación encontró un corte sin tasa de referencia o un momento sin tipo de
    /// cambio, activa el kill switch.
    fn flag_missing_rate(&mut self) {
        if let Some(at) = self.missing_rate.take() {
            self.activate_kill_switch(
                format!(
                    "Falta la tasa de referencia de la financiación del corte {at} (spec §12)."
                ),
                at,
            );
        }
        if let Some(at) = self.missing_fx.take() {
            self.activate_kill_switch(
                format!("Falta el tipo de cambio de {at} (ADR 0013, spec §12)."),
                at,
            );
        }
        if let Some(at) = self.before_checkpoint.take() {
            self.activate_kill_switch(
                format!(
                    "Valuación en {at}, antes de lo que selló el punto de control del broker \
                     (#463): no se puede repetir igual."
                ),
                at,
            );
        }
    }

    /// Tipo de cambio de `instrument` vigente en `at`, o `None` (sin inventar).
    fn fx_at(&self, instrument: &InstrumentId, at: DateTime<Utc>) -> Option<Decimal> {
        self.markets.get(instrument)?.account_to_instrument.at(at)
    }

    /// `amount` (moneda del instrumento) en EUR al tipo vigente en `at`. Si no hay tipo, 0 y
    /// se anota el error de datos (kill switch en el próximo `flag_missing_rate`).
    fn to_eur(&self, instrument: &InstrumentId, amount: Decimal, at: DateTime<Utc>) -> Decimal {
        match self.fx_at(instrument, at) {
            Some(fx) => amount / fx,
            None => {
                if self.missing_fx.get().is_none_or(|prev| at < prev) {
                    self.missing_fx.set(Some(at));
                }
                Decimal::ZERO
            }
        }
    }

    /// Financiación de `size` abierto en `(from, to]`, en moneda del instrumento (positivo es
    /// un costo). Si falta la tasa de algún corte, cobra solo los anteriores y lo anota.
    fn funding(&self, t: &OpenTrade, size: Decimal, to: DateTime<Utc>) -> Decimal {
        let m = &self.markets[&t.instrument];
        let f = &m.funding;
        let cutoffs = funding_cutoffs(t.opened_at, to, f.cutoff, f.tz);
        let side = strategy_side(t.side);
        match funding_over(side, t.entry_mid, size, &cutoffs, &f.rates, &m.costs) {
            Ok(v) => v,
            Err(at) => {
                if self.missing_rate.get().is_none_or(|prev| at < prev) {
                    self.missing_rate.set(Some(at));
                }
                let known: Vec<_> = cutoffs.into_iter().take_while(|c| *c < at).collect();
                funding_over(side, t.entry_mid, size, &known, &f.rates, &m.costs)
                    .unwrap_or(Decimal::ZERO)
            }
        }
    }

    /// Cada operación abierta hasta `now`, valuada como la valúa el broker (las mismas salidas
    /// de `valuate`), para compararla con el backtest (fase 8, #315). Lo que sigue abierto se
    /// cierra al último cierre ya cerrado, como `EndOfData` en el backtest. Montos en la moneda
    /// del instrumento.
    pub fn trade_outcomes(&self, now: DateTime<Utc>) -> Vec<PaperTrade> {
        // Las selladas por un punto de control (#463) ya terminaron: su resultado guardado.
        let mut out: Vec<(usize, PaperTrade)> = self
            .sealed
            .outcomes
            .iter()
            .filter(|(_, t)| t.opened_at <= now)
            .cloned()
            .collect();
        out.extend(
            self.trades
                .iter()
                .filter(|t| t.opened_at <= now)
                .map(|t| (t.seq, self.outcome(t, now))),
        );
        out.sort_by_key(|(seq, _)| *seq);
        out.into_iter().map(|(_, t)| t).collect()
    }

    /// La operación valuada en `now` (para sellarla en un punto de control, #463).
    pub(crate) fn outcome_of(&self, t: &OpenTrade, now: DateTime<Utc>) -> PaperTrade {
        self.outcome(t, now)
    }

    fn outcome(&self, t: &OpenTrade, now: DateTime<Utc>) -> PaperTrade {
        let m = &self.markets[&t.instrument];
        let side = strategy_side(t.side);
        // Las velas de un instrumento entran en orden (`push_candle`): las cerradas en `now`
        // son un prefijo.
        let closed = &m.candles[..m.candles.partition_point(|c| c.end() <= now)];
        let exit_px = |mid| fill_price(side, Execution::Exit, mid, &m.costs, &self.cfg.costs);
        let (fills, open) = self.exits_of(t, closed);
        let mut exits: Vec<PaperExit> = fills
            .iter()
            .map(|f| PaperExit {
                time: closed[f.index].end(),
                mid: f.price,
                price: exit_px(f.price),
                fraction: f.fraction,
                end_of_data: false,
                reason: Some(f.reason),
            })
            .collect();
        if open > Decimal::ZERO
            && let Some(last) = closed.last()
        {
            exits.push(PaperExit {
                time: last.end(),
                mid: last.close,
                price: exit_px(last.close),
                fraction: open,
                end_of_data: true,
                reason: None,
            });
        }
        let (mut commission_total, mut funding, mut gross) =
            (t.entry_commission, Decimal::ZERO, Decimal::ZERO);
        for e in &exits {
            let part = t.size * e.fraction;
            commission_total += commission(&m.costs, part, e.price);
            funding += self.funding(t, part, e.time);
            gross += part * (e.price - t.entry_fill) * sign(t.side);
        }
        PaperTrade {
            order_id: t.order_id.clone(),
            instrument: t.instrument.clone(),
            side: t.side,
            opened_at: t.opened_at,
            entry_mid: t.entry_mid,
            entry_fill: t.entry_fill,
            size: t.size,
            stop: t.stop,
            exits,
            commission: commission_total,
            funding,
            gross_pnl: gross,
            net_pnl: gross - commission_total - funding,
        }
    }

    /// Las órdenes procesadas, en orden de llegada, con su informe.
    pub fn journal(&self) -> &[(Order, OrderReport)] {
        &self.journal
    }

    /// Las órdenes pendientes de su vela del fill (#471), en orden de llegada.
    pub fn pending(&self) -> &[Order] {
        &self.pending
    }

    /// Decide las órdenes pendientes cuya vela del fill ya llegó, en orden de llegada, y
    /// devuelve lo que entró al registro (para guardarlo). Cada una se decide con
    /// [`Self::decide`] sobre el estado de ese momento, igual que la reconstrucción la decide
    /// en ese mismo punto del registro (#471).
    pub fn settle_pending(&mut self) -> Vec<(Order, OrderReport)> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.pending.len() {
            if self.awaits_fill(&self.pending[i]) {
                i += 1;
                continue;
            }
            let order = self.pending.remove(i);
            let report = self.decide(&order);
            out.push((order, report));
        }
        out
    }

    /// Vuelve a dejar pendiente una orden guardada como pendiente (#479), al arrancar y después
    /// de reconstruir el broker desde el registro, en el orden en que llegó. No se evalúa otra
    /// vez: la sesión que la recibió ya la evaluó al llegar y no la vuelve a mirar hasta su vela
    /// del fill; ahí la decide [`Self::settle_pending`] con el estado de ese momento, igual que
    /// si no hubiera habido reinicio.
    pub fn restore_pending(&mut self, order: Order) -> Result<(), RestorePending> {
        if self.reports.contains_key(&order.order_id) {
            return Err(RestorePending::AlreadyDecided);
        }
        if self.pending.iter().any(|o| o.order_id == order.order_id) {
            return Err(RestorePending::AlreadyPending);
        }
        if !self.markets.contains_key(&order.instrument_id) {
            return Err(RestorePending::UnknownInstrument);
        }
        self.pending.push(order);
        Ok(())
    }

    /// Quita una orden pendiente (la que no se pudo guardar como pendiente, #479): así no queda
    /// una pendiente que un reinicio perdería, y se puede reenviar con el mismo `order_id`.
    pub fn forget_pending(&mut self, order_id: &ClientId) -> Option<Order> {
        let i = self.pending.iter().position(|o| &o.order_id == order_id)?;
        Some(self.pending.remove(i))
    }

    /// `true` si el instrumento existe y todavía no hay vela que empiece en `created_at` o
    /// después (la del fill).
    fn awaits_fill(&self, order: &Order) -> bool {
        self.markets
            .get(&order.instrument_id)
            .is_some_and(|m| m.candles.last().is_none_or(|c| c.start < order.created_at))
    }

    fn pending_report(order: &Order) -> OrderReport {
        OrderReport {
            order_id: order.order_id.clone(),
            status: OrderStatus::Pending,
            rejection: None,
            position_id: None,
            size: None,
            fill_price: None,
            commission: None,
            at: order.created_at,
        }
    }

    /// Decide la orden (riesgo y fill) y la anota en el registro. La vela del fill, si el riesgo
    /// la acepta, tiene que estar (si no, `no_price`).
    fn decide(&mut self, order: &Order) -> OrderReport {
        if let (true, Some(w)) = (self.journal.is_empty(), self.first_order_window) {
            // Registro vacío: no puede fallar.
            let _ = self.start_candles_at(order.created_at - w);
        }
        let a = self.assess(order);
        self.events.push(BrokerEvent::RiskDecision {
            order_id: order.order_id.clone(),
            accepted: a.result.is_ok(),
            rejection: a.result.as_ref().err().copied(),
            size: a.result.as_ref().ok().map(|s| s.size),
            as_of: a.as_of,
        });
        let report = match (a.result, a.fill_index) {
            (Ok(s), Some(i)) => self.open_trade(order, &s, i),
            (result, _) => OrderReport {
                order_id: order.order_id.clone(),
                status: OrderStatus::Rejected,
                rejection: Some(result.err().unwrap_or(OrderRejection::NoPrice)),
                position_id: None,
                size: None,
                fill_price: None,
                commission: None,
                at: order.created_at,
            },
        };
        self.reports.insert(order.order_id.clone(), report.clone());
        self.journal.push((order.clone(), report.clone()));
        self.events.push(BrokerEvent::OrderReport {
            report: report.clone(),
        });
        report
    }

    /// Abre la operación en la apertura de la vela `i` (la del fill).
    fn open_trade(&mut self, order: &Order, s: &Sizing, i: usize) -> OrderReport {
        let m = &self.markets[&order.instrument_id];
        let bar = &m.candles[i];
        let side = strategy_side(order.side);
        let px = fill_price(side, Execution::Entry, bar.open, &m.costs, &self.cfg.costs);
        let fee = commission(&m.costs, s.size, px);
        let position_id: ClientId = format!("pos:{}", order.order_id)
            .chars()
            .take(200)
            .collect::<String>()
            .parse()
            .expect("prefijo y order_id válidos");
        let opened_at = bar.start;
        let entry_mid = bar.open;
        self.trades.push(OpenTrade {
            seq: self.journal.len(),
            position_id: position_id.clone(),
            order_id: order.order_id.clone(),
            instrument: order.instrument_id.clone(),
            side: order.side,
            size: s.size,
            entry_mid,
            entry_fill: px,
            entry_index: i,
            entry_commission: fee,
            stop: order.stop,
            tp1: order.tp1,
            margin: s.margin,
            opened_at,
        });
        OrderReport {
            order_id: order.order_id.clone(),
            status: OrderStatus::Filled,
            rejection: None,
            position_id: Some(position_id),
            size: Some(s.size),
            fill_price: Some(px),
            commission: Some(fee),
            at: opened_at,
        }
    }

    /// Informe ya guardado de una orden, si existe.
    pub fn report(&self, order_id: &ClientId) -> Option<&OrderReport> {
        self.reports.get(order_id)
    }

    /// Las salidas de una operación con las velas cerradas, y la fracción que sigue abierta.
    /// Igual que el backtest (`cfds_backtest::execute`, #319): TP1, break-even y trailing se
    /// calculan desde el **precio ejecutado** de la entrada, y si la apertura de la entrada ya
    /// saltó el stop (gap), sale entera a esa apertura (spec §12).
    fn exits_of(&self, t: &OpenTrade, closed: &[Candle]) -> (Vec<Fill>, Decimal) {
        if closed.len() <= t.entry_index {
            return (Vec::new(), Decimal::ONE);
        }
        let atr = self.atr(&t.instrument, closed);
        let trade = Trade {
            side: strategy_side(t.side),
            entry_index: t.entry_index,
            entry: t.entry_fill,
            stop: t.stop,
        };
        // Cierre de fin de semana (spec §8, #334): las mismas marcas que el backtest.
        let force_close =
            week_close_flags(self.cfg.sessions.get(&t.instrument), closed, t.entry_index);
        match simulate(&trade, closed, &atr, &force_close, &self.cfg.exits) {
            Some(out) => (out.fills, out.open_fraction),
            None => (
                vec![Fill {
                    index: t.entry_index,
                    price: t.entry_mid,
                    fraction: Decimal::ONE,
                    reason: ExitReason::Stop,
                }],
                Decimal::ZERO,
            ),
        }
    }

    pub(crate) fn valuate_open_fraction_and_events(
        &self,
        t: &OpenTrade,
        now: DateTime<Utc>,
    ) -> (Decimal, Decimal, Vec<(DateTime<Utc>, Decimal)>) {
        let v = self.valuate(t, now);
        (v.open_fraction, v.realized_eur, v.realized_events)
    }

    fn valuate(&self, t: &OpenTrade, now: DateTime<Utc>) -> Valued {
        let m = &self.markets[&t.instrument];
        let eur = |amount, at| self.to_eur(&t.instrument, amount, at);
        // Las velas de un instrumento entran en orden (`push_candle`): las cerradas en `now`
        // son un prefijo.
        let closed = &m.candles[..m.candles.partition_point(|c| c.end() <= now)];
        let entry_fee = eur(-t.entry_commission, t.opened_at);
        let mut v = Valued {
            realized_eur: entry_fee,
            realized_events: vec![(t.opened_at, entry_fee)],
            open_fraction: Decimal::ONE,
            last: closed.last().map(|c| c.close),
            unrealized: Decimal::ZERO,
            as_of: closed.last().map(Candle::end),
        };
        let (fills, open) = self.exits_of(t, closed);
        for f in &fills {
            let size = t.size * f.fraction;
            let px = fill_price(
                strategy_side(t.side),
                Execution::Exit,
                f.price,
                &m.costs,
                &self.cfg.costs,
            );
            let exit_at = closed[f.index].end();
            let pnl = size * (px - t.entry_fill) * sign(t.side)
                - commission(&m.costs, size, px)
                - self.funding(t, size, exit_at);
            let pnl = eur(pnl, exit_at);
            v.realized_eur += pnl;
            v.realized_events.push((exit_at, pnl));
        }
        v.open_fraction = open;
        if let Some(last) = v.last {
            let open = v.open_fraction * t.size;
            // El no realizado ya descuenta la financiación de las noches que lleva abierta.
            let swap = if open > Decimal::ZERO {
                self.funding(t, open, now)
            } else {
                Decimal::ZERO
            };
            v.unrealized = eur(open * (last - t.entry_fill) * sign(t.side) - swap, now);
        }
        v
    }

    /// Capital en `now` (realizado + no realizado), y lo realizado antes de `day_start`.
    fn equity_at(&self, now: DateTime<Utc>) -> (Decimal, Decimal, Decimal) {
        self.touch_sealed(now, false);
        let mut equity = self.cfg.initial_equity;
        let mut unrealized = Decimal::ZERO;
        let mut margin_used = Decimal::ZERO;
        let mut sealed = self.sealed.realized.iter().peekable();
        for t in self.trades.iter().filter(|t| t.opened_at <= now) {
            while let Some((_, r)) = sealed.next_if(|(seq, _)| *seq < t.seq) {
                equity += *r;
            }
            let v = self.valuate(t, now);
            equity += v.realized_eur + v.unrealized;
            unrealized += v.unrealized;
            margin_used += self.to_eur(&t.instrument, v.open_fraction * t.margin, now);
        }
        for (_, r) in sealed {
            equity += *r;
        }
        (equity, unrealized, margin_used)
    }

    /// El capital en `now` como `equity_at`, sumado en el mismo orden (da exactamente lo mismo).
    /// `settled`: el valor de las operaciones ya cerradas del todo, que no cambia después (sin
    /// repintado); evita volver a valuarlas en cada vela de [`Self::peak_until`].
    fn equity_marked(&self, now: DateTime<Utc>, settled: &mut BTreeMap<usize, Decimal>) -> Decimal {
        self.touch_sealed(now, false);
        let mut equity = self.cfg.initial_equity;
        let mut sealed = self.sealed.realized.iter().peekable();
        for t in self.trades.iter().filter(|t| t.opened_at <= now) {
            while let Some((_, r)) = sealed.next_if(|(seq, _)| *seq < t.seq) {
                equity += *r;
            }
            equity += match settled.get(&t.seq) {
                Some(v) => *v,
                None => {
                    let v = self.valuate(t, now);
                    let value = v.realized_eur + v.unrealized;
                    if v.open_fraction.is_zero() {
                        settled.insert(t.seq, value);
                    }
                    value
                }
            };
        }
        for (_, r) in sealed {
            equity += *r;
        }
        equity
    }

    /// El máximo del capital con el capital valuado a mercado al cierre de cada vela cerrada
    /// (de cualquier instrumento) posterior a `peak_through` y hasta `until`, en orden, como el
    /// backtest de cartera en cada vela (#691). Solo velas que ya cerraron en `until`: nada del
    /// futuro. Devuelve el máximo y hasta dónde quedó incluido; no cambia el broker.
    pub(crate) fn peak_until(&self, until: DateTime<Utc>) -> (Decimal, Option<DateTime<Utc>>) {
        let through = self.peak_through;
        if through.is_some_and(|p| until <= p) {
            return (self.peak_equity, through);
        }
        let mut peak = self.peak_equity;
        // Antes de la primera operación el capital no se mueve (lo sellado ya está en el máximo).
        if let Some(first) = self.trades.iter().map(|t| t.opened_at).min() {
            let mut times: Vec<DateTime<Utc>> = self
                .markets
                .values()
                .flat_map(|m| m.candles.iter().map(Candle::end))
                .filter(|t| *t >= first && *t <= until && through.is_none_or(|p| *t > p))
                .collect();
            times.sort_unstable();
            times.dedup();
            let mut settled = BTreeMap::new();
            for t in times {
                peak = mark_peak(peak, self.equity_marked(t, &mut settled));
            }
        }
        (peak, Some(until))
    }

    pub(crate) fn day_start(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        let local = now.with_timezone(&self.cfg.day_tz).date_naive();
        local
            .and_hms_opt(0, 0, 0)
            .and_then(|d| d.and_local_timezone(self.cfg.day_tz).earliest())
            .map_or(now, |d| d.with_timezone(&Utc))
    }

    fn open_instruments(&self, now: DateTime<Utc>) -> Vec<InstrumentId> {
        self.touch_sealed(now, true);
        self.trades
            .iter()
            // También las que esperan su fill (abren en la apertura siguiente).
            .filter(|t| t.opened_at > now || self.valuate(t, now).open_fraction > Decimal::ZERO)
            .map(|t| t.instrument.clone())
            .collect()
    }

    /// Las posiciones abiertas para el riesgo. Lo que arriesga cada una (en EUR, hasta su stop
    /// original, con la fracción que sigue abierta: cota conservadora) solo lo usa el tope por
    /// grupo correlacionado (#670). Sin grupos en `config/risk.toml` va 0 y no se pide ningún tipo
    /// de cambio: el comportamiento de siempre. Con grupos, sin tipo no se inventa uno.
    fn open_positions(&self, now: DateTime<Utc>) -> Result<Vec<OpenPosition>, OrderRejection> {
        self.touch_sealed(now, true);
        let need_risk = !self.cfg.risk.groups.is_empty();
        let mut out = Vec::new();
        for t in &self.trades {
            let pending = t.opened_at > now;
            let fraction = if pending {
                Decimal::ONE
            } else {
                self.valuate(t, now).open_fraction
            };
            if !pending && fraction <= Decimal::ZERO {
                continue;
            }
            let risk = if need_risk {
                let adverse = match t.side {
                    OrderSide::Long => t.entry_fill - t.stop,
                    OrderSide::Short => t.stop - t.entry_fill,
                }
                .max(Decimal::ZERO);
                let fx = self
                    .fx_at(&t.instrument, now)
                    .or_else(|| self.fx_at(&t.instrument, t.opened_at))
                    .ok_or(OrderRejection::MissingFxRate)?;
                t.size * fraction * adverse / fx
            } else {
                Decimal::ZERO
            };
            out.push(OpenPosition {
                instrument: t.instrument.clone(),
                risk,
            });
        }
        Ok(out)
    }

    fn assess(&mut self, order: &Order) -> Assessment {
        let now = order.created_at;
        let Some(m) = self.markets.get(&order.instrument_id) else {
            return Assessment {
                as_of: now,
                reference: None,
                fill_index: None,
                result: Err(OrderRejection::UnknownInstrument),
            };
        };
        let closed = m.candles.iter().take_while(|c| c.end() <= now).count();
        let fill_index = m.candles.iter().position(|c| c.start >= now);
        let Some(signal) = closed.checked_sub(1).map(|i| &m.candles[i]) else {
            return Assessment {
                as_of: now,
                reference: None,
                fill_index,
                result: Err(OrderRejection::NoPrice),
            };
        };
        let reference = signal.close;
        let as_of = signal.end();
        let fail = |r| Assessment {
            as_of,
            reference: Some(reference),
            fill_index,
            result: Err(r),
        };
        let wrong_side = match order.side {
            OrderSide::Long => order.stop >= reference,
            OrderSide::Short => order.stop <= reference,
        };
        if wrong_side {
            return fail(OrderRejection::StopWrongSide);
        }
        let atr = self
            .atr(&order.instrument_id, &m.candles[..closed])
            .last()
            .copied()
            .flatten();
        // Sin ATR (faltan velas cerradas recientes): motivo propio, no un dato inválido (#361).
        let Some(atr) = atr else {
            return fail(OrderRejection::NoRecentCandles);
        };
        // El riesgo, al tipo vigente a la hora de la señal (ADR 0013). Sin tipo, no se opera.
        let Some(fx) = m.account_to_instrument.at(as_of) else {
            return fail(OrderRejection::MissingFxRate);
        };
        let req = Request {
            instrument: order.instrument_id.clone(),
            entry: reference,
            stop: order.stop,
            atr,
            account_to_instrument: fx,
        };
        let limits = m.limits;
        let (equity, _, margin_used) = self.equity_at(as_of);
        // El máximo, al día hasta la vela de la señal: valuado en cada vela cerrada (#691).
        let (peak, through) = self.peak_until(as_of);
        self.peak_equity = mark_peak(peak, equity);
        self.peak_through = through;
        let day_start = self.day_start(as_of);
        let (day_start_equity, _, _) = self.equity_at(day_start);
        // Un corte sin tasa de referencia es un error de datos: kill switch antes de decidir.
        self.flag_missing_rate();
        let open_positions = match self.open_positions(as_of) {
            Ok(p) => p,
            Err(r) => return fail(r),
        };
        let account = Account {
            equity,
            peak_equity: self.peak_equity,
            day_start_equity,
            free_margin: equity - margin_used,
            kill_switch: self.kill_switch.is_some(),
            open_positions,
            // Volatilidad objetivo apagada en la config (#671): se conecta cuando se apruebe.
            realized_vol_pct: None,
        };
        match check(&req, &account, &limits, &self.cfg.risk) {
            Ok(sizing) => Assessment {
                as_of,
                reference: Some(reference),
                fill_index,
                result: Ok(sizing),
            },
            Err(r) => {
                if r == Rejection::DrawdownLimit {
                    // El riesgo pide activar el kill switch (spec §10).
                    self.activate_kill_switch("Drawdown máximo alcanzado (spec §10).", as_of);
                }
                fail(rejection_code(&r))
            }
        }
    }
}

impl crate::Broker for PaperBroker {
    fn preview(&self, order: &Order, now: DateTime<Utc>) -> TicketVerdict {
        // La vista previa no puede cambiar el estado (ni el kill switch): trabaja sobre una copia.
        let mut probe = self.clone();
        let order = Order {
            created_at: now,
            ..order.clone()
        };
        let a = probe.assess(&order);
        // Aceptada implica tipo vigente en `as_of` (si no, `assess` la rechazó).
        let fx = self
            .fx_at(&order.instrument_id, a.as_of)
            .unwrap_or(Decimal::ONE);
        let (equity, _, _) = self.equity_at(a.as_of);
        match a.result {
            Ok(s) => {
                let risk = s.risk / fx;
                TicketVerdict {
                    order_id: order.order_id.clone(),
                    accepted: true,
                    rejection: None,
                    reference: a.reference,
                    size: Some(s.size),
                    risk: Some(risk.round_dp(2)),
                    risk_pct: Some((risk / equity * Decimal::ONE_HUNDRED).round_dp(2)),
                    leverage: Some(s.leverage.round_dp(2)),
                    margin: Some(s.margin.round_dp(2)),
                    leverage_capped: s.leverage_capped,
                    as_of: a.as_of,
                }
            }
            Err(r) => TicketVerdict {
                order_id: order.order_id.clone(),
                accepted: false,
                rejection: Some(r),
                reference: a.reference,
                size: None,
                risk: None,
                risk_pct: None,
                leverage: None,
                margin: None,
                leverage_capped: false,
                as_of: a.as_of,
            },
        }
    }

    fn submit(&mut self, order: &Order) -> OrderReport {
        if let Some(r) = self.reports.get(&order.order_id) {
            let r = r.clone();
            self.events.push(BrokerEvent::OrderDuplicate {
                order_id: order.order_id.clone(),
            });
            return r;
        }
        if self.pending.iter().any(|o| o.order_id == order.order_id) {
            self.events.push(BrokerEvent::OrderDuplicate {
                order_id: order.order_id.clone(),
            });
            return Self::pending_report(order);
        }
        self.events.push(BrokerEvent::OrderReceived {
            order_id: order.order_id.clone(),
            instrument_id: order.instrument_id.clone(),
            side: order.side,
            created_at: order.created_at,
        });
        // Sin la vela del fill todavía (#471): si el riesgo la acepta (sobre una copia, sin
        // efectos), queda pendiente fuera del registro y se decide cuando llegue esa vela, igual
        // que la reconstrucción. Si la rechaza, el rechazo no depende de la vela del fill: se
        // decide y se registra ya, con el mismo resultado que en la reconstrucción.
        if self.awaits_fill(order) && self.clone().assess(order).result.is_ok() {
            self.pending.push(order.clone());
            self.events.push(BrokerEvent::OrderPending {
                order_id: order.order_id.clone(),
                created_at: order.created_at,
            });
            return Self::pending_report(order);
        }
        self.decide(order)
    }

    fn positions(&mut self, now: DateTime<Utc>) -> Vec<Position> {
        self.touch_sealed(now, true);
        let mut out = Vec::new();
        for t in self.trades.iter().filter(|t| t.opened_at <= now) {
            let v = self.valuate(t, now);
            let (Some(last), Some(as_of)) = (v.last, v.as_of) else {
                continue;
            };
            if v.open_fraction <= Decimal::ZERO {
                continue;
            }
            // Lo no realizado vuelve a la moneda del instrumento con el mismo tipo de `now`.
            let Some(fx) = self.fx_at(&t.instrument, now) else {
                continue;
            };
            let size = t.size * v.open_fraction;
            let r = (t.entry_fill - t.stop).abs();
            let pl_instr = v.unrealized * fx;
            out.push(Position {
                position_id: t.position_id.clone(),
                order_id: t.order_id.clone(),
                instrument_id: t.instrument.clone(),
                side: t.side,
                size,
                entry: t.entry_fill,
                stop: t.stop,
                tp1: t.tp1,
                opened_at: t.opened_at,
                last,
                unrealized_pl: pl_instr.round_dp(2),
                unrealized_r: if r.is_zero() {
                    Decimal::ZERO
                } else {
                    (pl_instr / size / r).round_dp(2)
                },
                margin: (t.margin * v.open_fraction).round_dp(2),
                as_of,
            });
        }
        self.flag_missing_rate();
        out
    }

    fn activate_kill_switch(&mut self, reason: &str, at: DateTime<Utc>) {
        PaperBroker::activate_kill_switch(self, reason, at);
    }

    fn account(&mut self, now: DateTime<Utc>) -> PaperAccount {
        // Es una lectura (la UI la pide cuando quiere): no mueve el máximo del capital. Solo lo
        // pone al día la evaluación de una orden, que la reconstrucción repite igual (#464, #691).
        let (equity, unrealized, margin_used) = self.equity_at(now);
        let day_start = self.day_start(now);
        let realized_today: Decimal = self
            .trades
            .iter()
            .filter(|t| t.opened_at <= now)
            .flat_map(|t| self.valuate(t, now).realized_events)
            .filter(|(at, _)| *at >= day_start && *at <= now)
            .map(|(_, pnl)| pnl)
            .sum();
        let (day_start_equity, _, _) = self.equity_at(day_start);
        let max_loss = day_start_equity * self.cfg.risk.max_daily_loss_pct / Decimal::ONE_HUNDRED;
        let lost = (day_start_equity - equity).max(Decimal::ZERO);
        let used_pct = if max_loss > Decimal::ZERO {
            (lost / max_loss * Decimal::ONE_HUNDRED).round_dp(1)
        } else {
            Decimal::ZERO
        };
        let notional: Decimal = self
            .positions(now)
            .iter()
            .map(|p| self.to_eur(&p.instrument_id, p.size * p.last, now))
            .sum();
        let open = self.open_instruments(now).len();
        self.flag_missing_rate();
        PaperAccount {
            mode: PaperMode::Paper,
            currency: AccountCurrency::Eur,
            as_of: now,
            equity: equity.round_dp(2),
            unrealized_pl: unrealized.round_dp(2),
            realized_pl_today: realized_today.round_dp(2),
            margin_used: margin_used.round_dp(2),
            margin_free: (equity - margin_used).round_dp(2),
            leverage: if equity > Decimal::ZERO {
                (notional / equity).round_dp(2)
            } else {
                Decimal::ZERO
            },
            daily_loss_used_pct: used_pct,
            open_positions: u32::try_from(open).unwrap_or(u32::MAX),
            max_positions: u32::try_from(self.cfg.risk.max_open_positions).unwrap_or(u32::MAX),
            kill_switch: match &self.kill_switch {
                None => KillSwitch::Inactive,
                Some((reason, since)) => KillSwitch::Active {
                    reason: reason.clone(),
                    since: *since,
                },
            },
        }
    }
}

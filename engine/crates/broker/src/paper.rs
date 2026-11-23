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
//! cada posición se valúa con cfds_backtest::simulate (stop, TP1, trailing, tiempo, gaps)
//! y paga la financiación nocturna (swap) de cada corte en que siguió abierta, con la tasa de
//! referencia vigente en ese corte (cfds_backtest::funding_cutoffs y funding_over, #79)
//! ```
//! Funciones del tiempo: `positions(now)` y `account(now)` usan solo velas cerradas en `now`.
//! Sin ejecuciones parciales (la spec §12 las prevé solo para el registro real).
//! Si en algún corte no hay tasa de referencia publicada, no se inventa: ese corte no se cobra y
//! se activa el kill switch por error de datos (spec §12).

use std::collections::BTreeMap;

use std::cell::Cell;

use cfds_backtest::{
    CostConfig, Execution, ExitConfig, FundingRates, InstrumentCosts, Trade, commission,
    fill_price, funding_cutoffs, funding_over, simulate,
};
use cfds_core::{
    AccountCurrency, Candle, ClientId, InstrumentId, KillSwitch, Order, OrderRejection,
    OrderReport, OrderSide, OrderStatus, PaperAccount, PaperMode, Position, TicketVerdict,
};
use cfds_indicators::wilder_atr;
use cfds_risk::{Account, InstrumentLimits, Rejection, Request, RiskConfig, Sizing, check};
use cfds_strategy::Side;
use chrono::{DateTime, NaiveTime, Utc};
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
        })
    }
}

/// Lo que el broker necesita de un instrumento: límites y costos del catálogo, tipo de cambio y
/// las velas guardadas (en orden).
#[derive(Debug, Clone)]
pub struct MarketSetup {
    pub limits: InstrumentLimits,
    pub costs: InstrumentCosts,
    /// Moneda del instrumento por 1 unidad de la moneda de la cuenta (1 si son iguales).
    pub account_to_instrument: Decimal,
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

#[derive(Debug, Clone)]
struct OpenTrade {
    position_id: ClientId,
    order_id: ClientId,
    instrument: InstrumentId,
    side: OrderSide,
    size: Decimal,
    /// Apertura de la vela del fill (precio medio): base de las salidas.
    entry_mid: Decimal,
    /// Precio pagado (con medio spread y slippage).
    entry_fill: Decimal,
    entry_index: usize,
    entry_commission: Decimal,
    stop: Decimal,
    tp1: Option<Decimal>,
    margin: Decimal,
    opened_at: DateTime<Utc>,
}

/// Una salida de una operación del broker simulado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperExit {
    /// Fin de la vela de la salida.
    pub time: DateTime<Utc>,
    pub mid: Decimal,
    pub price: Decimal,
    pub fraction: Decimal,
    /// Seguía abierta: se cierra al último cierre (como `EndOfData` en el backtest).
    pub end_of_data: bool,
}

/// Una operación del broker simulado, valuada (ver [`PaperBroker::trade_outcomes`]). Montos en la
/// moneda del instrumento.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// El broker simulado. Solo modo paper.
#[derive(Debug, Clone)]
pub struct PaperBroker {
    cfg: BrokerConfig,
    markets: BTreeMap<InstrumentId, MarketSetup>,
    reports: BTreeMap<ClientId, OrderReport>,
    /// Las órdenes procesadas, en el orden en que llegaron, con su informe: lo que se guarda
    /// para reconstruir el broker al arrancar (ver `crate::reconcile`).
    journal: Vec<(Order, OrderReport)>,
    events: Vec<BrokerEvent>,
    trades: Vec<OpenTrade>,
    kill_switch: Option<(String, DateTime<Utc>)>,
    peak_equity: Decimal,
    /// Primer corte sin tasa de referencia que encontró una valuación (error de datos).
    missing_rate: Cell<Option<DateTime<Utc>>>,
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
            events: Vec::new(),
            trades: Vec::new(),
            kill_switch: None,
            peak_equity: peak,
            missing_rate: Cell::new(None),
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

    /// Los mercados con sus velas (para reconstruir aparte, por ejemplo al reconciliar).
    pub fn markets(&self) -> &BTreeMap<InstrumentId, MarketSetup> {
        &self.markets
    }

    pub fn config(&self) -> &BrokerConfig {
        &self.cfg
    }

    pub fn kill_switch_active(&self) -> bool {
        self.kill_switch.is_some()
    }

    /// Si una valuación encontró un corte sin tasa de referencia, activa el kill switch.
    fn flag_missing_rate(&mut self) {
        if let Some(at) = self.missing_rate.take() {
            self.activate_kill_switch(
                format!(
                    "Falta la tasa de referencia de la financiación del corte {at} (spec §12)."
                ),
                at,
            );
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
        self.trades
            .iter()
            .filter(|t| t.opened_at <= now)
            .map(|t| self.outcome(t, now))
            .collect()
    }

    fn outcome(&self, t: &OpenTrade, now: DateTime<Utc>) -> PaperTrade {
        let m = &self.markets[&t.instrument];
        let side = strategy_side(t.side);
        let closed: Vec<Candle> = m
            .candles
            .iter()
            .filter(|c| c.end() <= now)
            .cloned()
            .collect();
        let exit_px = |mid| fill_price(side, Execution::Exit, mid, &m.costs, &self.cfg.costs);
        let mut exits = Vec::new();
        let mut open = Decimal::ONE;
        if closed.len() > t.entry_index {
            let atr = wilder_atr(&closed, self.cfg.atr_period);
            let trade = Trade {
                side,
                entry_index: t.entry_index,
                entry: t.entry_mid,
                stop: t.stop,
            };
            if let Some(out) = simulate(&trade, &closed, &atr, &[], &self.cfg.exits) {
                for f in &out.fills {
                    exits.push(PaperExit {
                        time: closed[f.index].end(),
                        mid: f.price,
                        price: exit_px(f.price),
                        fraction: f.fraction,
                        end_of_data: false,
                    });
                }
                open = out.open_fraction;
            }
        }
        if open > Decimal::ZERO
            && let Some(last) = closed.last()
        {
            exits.push(PaperExit {
                time: last.end(),
                mid: last.close,
                price: exit_px(last.close),
                fraction: open,
                end_of_data: true,
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

    /// Informe ya guardado de una orden, si existe.
    pub fn report(&self, order_id: &ClientId) -> Option<&OrderReport> {
        self.reports.get(order_id)
    }

    fn valuate(&self, t: &OpenTrade, now: DateTime<Utc>) -> Valued {
        let m = &self.markets[&t.instrument];
        let fx = m.account_to_instrument;
        let closed: Vec<Candle> = m
            .candles
            .iter()
            .filter(|c| c.end() <= now)
            .cloned()
            .collect();
        let mut v = Valued {
            realized_eur: -t.entry_commission / fx,
            realized_events: vec![(t.opened_at, -t.entry_commission / fx)],
            open_fraction: Decimal::ONE,
            last: closed.last().map(|c| c.close),
            unrealized: Decimal::ZERO,
            as_of: closed.last().map(Candle::end),
        };
        if closed.len() > t.entry_index {
            let atr = wilder_atr(&closed, self.cfg.atr_period);
            let trade = Trade {
                side: strategy_side(t.side),
                entry_index: t.entry_index,
                entry: t.entry_mid,
                stop: t.stop,
            };
            if let Some(out) = simulate(&trade, &closed, &atr, &[], &self.cfg.exits) {
                for f in &out.fills {
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
                    v.realized_eur += pnl / fx;
                    v.realized_events.push((exit_at, pnl / fx));
                }
                v.open_fraction = out.open_fraction;
            }
        }
        if let Some(last) = v.last {
            let open = v.open_fraction * t.size;
            // El no realizado ya descuenta la financiación de las noches que lleva abierta.
            let swap = if open > Decimal::ZERO {
                self.funding(t, open, now)
            } else {
                Decimal::ZERO
            };
            v.unrealized = (open * (last - t.entry_fill) * sign(t.side) - swap) / fx;
        }
        v
    }

    /// Capital en `now` (realizado + no realizado), y lo realizado antes de `day_start`.
    fn equity_at(&self, now: DateTime<Utc>) -> (Decimal, Decimal, Decimal) {
        let mut equity = self.cfg.initial_equity;
        let mut unrealized = Decimal::ZERO;
        let mut margin_used = Decimal::ZERO;
        for t in self.trades.iter().filter(|t| t.opened_at <= now) {
            let v = self.valuate(t, now);
            equity += v.realized_eur + v.unrealized;
            unrealized += v.unrealized;
            margin_used +=
                v.open_fraction * t.margin / self.markets[&t.instrument].account_to_instrument;
        }
        (equity, unrealized, margin_used)
    }

    fn day_start(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        let local = now.with_timezone(&self.cfg.day_tz).date_naive();
        local
            .and_hms_opt(0, 0, 0)
            .and_then(|d| d.and_local_timezone(self.cfg.day_tz).earliest())
            .map_or(now, |d| d.with_timezone(&Utc))
    }

    fn open_instruments(&self, now: DateTime<Utc>) -> Vec<InstrumentId> {
        self.trades
            .iter()
            // También las que esperan su fill (abren en la apertura siguiente).
            .filter(|t| t.opened_at > now || self.valuate(t, now).open_fraction > Decimal::ZERO)
            .map(|t| t.instrument.clone())
            .collect()
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
        let atr = wilder_atr(&m.candles[..closed], self.cfg.atr_period)
            .last()
            .copied()
            .flatten();
        let Some(atr) = atr else {
            return fail(OrderRejection::InvalidInput);
        };
        let req = Request {
            instrument: order.instrument_id.clone(),
            entry: reference,
            stop: order.stop,
            atr,
            account_to_instrument: m.account_to_instrument,
        };
        let limits = m.limits;
        let (equity, _, margin_used) = self.equity_at(as_of);
        self.peak_equity = self.peak_equity.max(equity);
        let day_start = self.day_start(as_of);
        let (day_start_equity, _, _) = self.equity_at(day_start);
        // Un corte sin tasa de referencia es un error de datos: kill switch antes de decidir.
        self.flag_missing_rate();
        let account = Account {
            equity,
            peak_equity: self.peak_equity,
            day_start_equity,
            free_margin: equity - margin_used,
            kill_switch: self.kill_switch.is_some(),
            open_positions: self.open_instruments(as_of),
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
        let fx = self
            .markets
            .get(&order.instrument_id)
            .map_or(Decimal::ONE, |m| m.account_to_instrument);
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
        self.events.push(BrokerEvent::OrderReceived {
            order_id: order.order_id.clone(),
            instrument_id: order.instrument_id.clone(),
            side: order.side,
            created_at: order.created_at,
        });
        let a = self.assess(order);
        self.events.push(BrokerEvent::RiskDecision {
            order_id: order.order_id.clone(),
            accepted: a.result.is_ok(),
            rejection: a.result.as_ref().err().copied(),
            size: a.result.as_ref().ok().map(|s| s.size),
            as_of: a.as_of,
        });
        let report = match (a.result, a.fill_index) {
            (Ok(s), Some(i)) => {
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
                self.trades.push(OpenTrade {
                    position_id: position_id.clone(),
                    order_id: order.order_id.clone(),
                    instrument: order.instrument_id.clone(),
                    side: order.side,
                    size: s.size,
                    entry_mid: bar.open,
                    entry_fill: px,
                    entry_index: i,
                    entry_commission: fee,
                    stop: order.stop,
                    tp1: order.tp1,
                    margin: s.margin,
                    opened_at: bar.start,
                });
                OrderReport {
                    order_id: order.order_id.clone(),
                    status: OrderStatus::Filled,
                    rejection: None,
                    position_id: Some(position_id),
                    size: Some(s.size),
                    fill_price: Some(px),
                    commission: Some(fee),
                    at: bar.start,
                }
            }
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

    fn positions(&mut self, now: DateTime<Utc>) -> Vec<Position> {
        let mut out = Vec::new();
        for t in self.trades.iter().filter(|t| t.opened_at <= now) {
            let v = self.valuate(t, now);
            let (Some(last), Some(as_of)) = (v.last, v.as_of) else {
                continue;
            };
            if v.open_fraction <= Decimal::ZERO {
                continue;
            }
            let fx = self.markets[&t.instrument].account_to_instrument;
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
        let (equity, unrealized, margin_used) = self.equity_at(now);
        self.peak_equity = self.peak_equity.max(equity);
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
            .map(|p| p.size * p.last / self.markets[&p.instrument_id].account_to_instrument)
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

//! Recorrido completo de un instrumento (Fase 3): señal → riesgo → entrada en `t+1` → salidas →
//! costos → registro. Una sola posición a la vez (límite de 1 por instrumento, spec §10).
//!
//! ```text
//! para cada vela cerrada t (con t+1 disponible):
//!   si cambió el día en Madrid → capital del día = capital actual
//!   señal en t (largo y corto); si las dos son candidatas, gana el puntaje más alto;
//!     con empate, ninguna (no hay criterio objetivo)
//!   riesgo con la cuenta en t (sin posición abierta: margen libre = capital)
//!     drawdown → se activa el kill switch y no se opera más en este recorrido
//!   entrada a la apertura de t+1 (spread + slippage) → salidas → costos → P/L
//!   la próxima decisión es al cierre de la vela de salida
//! ```
//! Supuestos del agente (documentados en el PR):
//! - La financiación usa el precio medio de entrada como "precio de cierre" de cada noche, y la
//!   hora de cada salida es el **fin** de su vela (si un corte cae dentro, se cobra).
//! - Si los datos terminan con la posición abierta, se cierra al último cierre (`EndOfData`).

use cfds_core::{Candle, InstrumentId};
use cfds_indicators::wilder_atr;
use cfds_risk::{Account, InstrumentLimits, Rejection, Request, RiskConfig, check};
use cfds_strategy::{Analysis, Configs, ScoreError, Side, Signal, TriggerKind};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;

use crate::{
    BacktestConfig, Execution, ExitReason, InstrumentCosts, Trade, commission, fill_price, funding,
    funding_nights, simulate,
};

/// Todo lo fijo de un recorrido.
#[derive(Debug, Clone)]
pub struct Setup {
    pub instrument: InstrumentId,
    pub strategy: Configs,
    pub risk: RiskConfig,
    pub limits: InstrumentLimits,
    pub costs: InstrumentCosts,
    pub backtest: BacktestConfig,
    /// Capital inicial, en la moneda de la cuenta.
    pub initial_equity: Decimal,
    /// Moneda del instrumento por unidad de la moneda de la cuenta (constante en el recorrido).
    pub account_to_instrument: Decimal,
    /// Tipo de referencia anual de la financiación, en % (constante en el recorrido).
    pub benchmark_pct: Decimal,
    pub funding_cutoff: NaiveTime,
    pub funding_tz: Tz,
    /// Zona del "día" de la pérdida diaria (spec §10: desde las 00:00 de Madrid).
    pub day_tz: Tz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    Exit(ExitReason),
    /// Los datos terminaron con la posición abierta: se cierra al último cierre.
    EndOfData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitRecord {
    pub index: usize,
    /// Fin de la vela de salida.
    pub time: DateTime<Utc>,
    /// Precio medio de la salida (antes de spread y slippage).
    pub mid: Decimal,
    /// Precio ejecutado.
    pub price: Decimal,
    pub fraction: Decimal,
    pub reason: CloseReason,
}

/// Registro de una operación (ROADMAP Fase 3). Montos en la moneda del instrumento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeRecord {
    pub side: Side,
    pub signal_index: usize,
    pub score: i32,
    pub triggers: Vec<TriggerKind>,
    pub entry_index: usize,
    /// Apertura de la vela de entrada.
    pub entry_time: DateTime<Utc>,
    pub entry_mid: Decimal,
    pub entry_price: Decimal,
    pub stop: Decimal,
    pub size: Decimal,
    pub leverage: Decimal,
    pub margin: Decimal,
    pub exits: Vec<ExitRecord>,
    pub commission: Decimal,
    pub funding: Decimal,
    /// P/L a precios medios: lo que se habría ganado sin spread ni slippage.
    pub mid_pnl: Decimal,
    /// P/L a precios ejecutados (ya descuenta spread y slippage).
    pub gross_pnl: Decimal,
    /// Bruto − comisión − financiación.
    pub net_pnl: Decimal,
}

impl TradeRecord {
    /// Todos los costos: spread + slippage + comisión + financiación.
    pub fn total_costs(&self) -> Decimal {
        self.mid_pnl - self.net_pnl
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedSignal {
    pub index: usize,
    pub side: Side,
    pub reason: Rejection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub trades: Vec<TradeRecord>,
    pub rejected: Vec<RejectedSignal>,
    /// Vela en la que se activó el kill switch, si pasó.
    pub kill_switch_at: Option<usize>,
    /// Capital final, en la moneda de la cuenta.
    pub final_equity: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    Score(ScoreError),
    /// Las velas no tienen la vela `t+1` para entrar.
    NoEntryCandle,
}

fn sign(side: Side) -> Decimal {
    match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    }
}

/// Elige la señal del cierre de `t`: la candidata de mayor puntaje; con empate, ninguna.
pub fn choose(analysis: &Analysis, t: usize) -> Result<Option<Signal>, ScoreError> {
    let mut candidates = Vec::new();
    for side in [Side::Long, Side::Short] {
        candidates.extend(analysis.signal_at(t, side, 0)?);
    }
    Ok(pick(candidates))
}

/// Entre las señales de una vela, la candidata (puntaje ≥ mínimo) de mayor puntaje. Con
/// empate entre direcciones no hay criterio objetivo: ninguna.
pub fn pick(signals: Vec<Signal>) -> Option<Signal> {
    let mut candidates: Vec<Signal> = signals.into_iter().filter(|s| s.score.candidate).collect();
    candidates.sort_by_key(|s| std::cmp::Reverse(s.score.total));
    match candidates.as_slice() {
        [] => None,
        [one] => Some(one.clone()),
        [a, b, ..] if a.score.total > b.score.total => Some(a.clone()),
        _ => None,
    }
}

/// Ejecuta una señal ya aprobada por el riesgo con `size`, `leverage` y `margin`: entra en la
/// apertura de `signal.index + 1` y simula las salidas con todos los costos.
#[allow(clippy::too_many_arguments)]
pub fn execute(
    signal: &Signal,
    size: Decimal,
    leverage: Decimal,
    margin: Decimal,
    candles: &[Candle],
    atr: &[Option<Decimal>],
    force_close: &[bool],
    setup: &Setup,
) -> Result<TradeRecord, RunError> {
    let side = signal.side;
    let entry_index = signal.index + 1;
    let entry_candle = candles.get(entry_index).ok_or(RunError::NoEntryCandle)?;
    let cost_cfg = &setup.backtest.costs;
    let entry_mid = entry_candle.open;
    let entry_price = fill_price(side, Execution::Entry, entry_mid, &setup.costs, cost_cfg);
    let trade = Trade {
        side,
        entry_index,
        entry: entry_price,
        stop: signal.stop,
    };

    let mut exits: Vec<ExitRecord> = Vec::new();
    let exit = |index: usize, mid: Decimal, fraction: Decimal, reason: CloseReason| ExitRecord {
        index,
        time: candles[index].end(),
        mid,
        price: fill_price(side, Execution::Exit, mid, &setup.costs, cost_cfg),
        fraction,
        reason,
    };
    match simulate(&trade, candles, atr, force_close, &setup.backtest.exits) {
        Some(outcome) => {
            exits.extend(
                outcome
                    .fills
                    .iter()
                    .map(|f| exit(f.index, f.price, f.fraction, CloseReason::Exit(f.reason))),
            );
            if outcome.open_fraction > Decimal::ZERO {
                let last = candles.len() - 1;
                exits.push(exit(
                    last,
                    candles[last].close,
                    outcome.open_fraction,
                    CloseReason::EndOfData,
                ));
            }
        }
        // La apertura de t+1 ya saltó el stop (gap): se ejecuta a la apertura (spec §12).
        None => exits.push(exit(
            entry_index,
            entry_mid,
            Decimal::ONE,
            CloseReason::Exit(ExitReason::Stop),
        )),
    }

    let s = sign(side);
    let entry_time = entry_candle.start;
    let mut commission_total = commission(&setup.costs, size, entry_price);
    let (mut funding_total, mut mid_pnl, mut gross_pnl) =
        (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for e in &exits {
        let part = size * e.fraction;
        commission_total += commission(&setup.costs, part, e.price);
        let nights = funding_nights(entry_time, e.time, setup.funding_cutoff, setup.funding_tz);
        funding_total += funding(
            side,
            entry_mid,
            part,
            nights,
            setup.benchmark_pct,
            &setup.costs,
        );
        mid_pnl += part * (e.mid - entry_mid) * s;
        gross_pnl += part * (e.price - entry_price) * s;
    }
    Ok(TradeRecord {
        side,
        signal_index: signal.index,
        score: signal.score.total,
        triggers: signal.triggers.iter().map(|g| g.kind).collect(),
        entry_index,
        entry_time,
        entry_mid,
        entry_price,
        stop: signal.stop,
        size,
        leverage,
        margin,
        exits,
        commission: commission_total,
        funding: funding_total,
        mid_pnl,
        gross_pnl,
        net_pnl: gross_pnl - commission_total - funding_total,
    })
}

/// Recorre la serie de 15m (`force_close` marca las velas de cierre obligatorio: viernes o
/// fin de sesión, del calendario).
pub fn run(
    m15: &[Candle],
    h1: &[Candle],
    force_close: &[bool],
    setup: &Setup,
) -> Result<Report, RunError> {
    let analysis = Analysis::new(m15, h1, setup.strategy);
    let atr = wilder_atr(m15, setup.strategy.patterns.candles.atr_period);
    let fx = setup.account_to_instrument;
    let mut report = Report {
        trades: Vec::new(),
        rejected: Vec::new(),
        kill_switch_at: None,
        final_equity: setup.initial_equity,
    };
    let (mut equity, mut peak) = (setup.initial_equity, setup.initial_equity);
    let mut day: Option<NaiveDate> = None;
    let mut day_start = equity;

    let mut t = 0;
    while t + 1 < m15.len() {
        let today = m15[t].end().with_timezone(&setup.day_tz).date_naive();
        if day != Some(today) {
            day = Some(today);
            day_start = equity;
        }
        if report.kill_switch_at.is_some() {
            break;
        }
        let Some(signal) = choose(&analysis, t).map_err(RunError::Score)? else {
            t += 1;
            continue;
        };
        let Some(atr_t) = atr[t] else {
            t += 1;
            continue;
        };
        let request = Request {
            instrument: setup.instrument.clone(),
            entry: signal.reference,
            stop: signal.stop,
            atr: atr_t,
            account_to_instrument: fx,
        };
        let account = Account {
            equity,
            peak_equity: peak,
            day_start_equity: day_start,
            free_margin: equity,
            kill_switch: false,
            open_positions: vec![],
        };
        let sizing = match check(&request, &account, &setup.limits, &setup.risk) {
            Ok(s) => s,
            Err(reason) => {
                if reason == Rejection::DrawdownLimit {
                    report.kill_switch_at = Some(t);
                }
                report.rejected.push(RejectedSignal {
                    index: t,
                    side: signal.side,
                    reason,
                });
                t += 1;
                continue;
            }
        };
        let record = execute(
            &signal,
            sizing.size,
            sizing.leverage,
            sizing.margin,
            m15,
            &atr,
            force_close,
            setup,
        )?;
        equity += record.net_pnl / fx;
        peak = peak.max(equity);
        let exit_index = record.exits.last().map_or(t + 1, |e| e.index);
        report.trades.push(record);
        t = exit_index.max(t + 1);
    }
    report.final_equity = equity;
    Ok(report)
}

/// Comprar y mantener (benchmark de la Fase 5): compra `size` en la apertura de la primera
/// vela y vende en el cierre de la última, con los mismos costos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyAndHold {
    pub entry_price: Decimal,
    pub exit_price: Decimal,
    pub nights: u32,
    pub commission: Decimal,
    pub funding: Decimal,
    pub gross_pnl: Decimal,
    pub net_pnl: Decimal,
}

pub fn buy_and_hold(candles: &[Candle], size: Decimal, setup: &Setup) -> Option<BuyAndHold> {
    let (first, last) = (candles.first()?, candles.last()?);
    let cfg = &setup.backtest.costs;
    let entry_price = fill_price(Side::Long, Execution::Entry, first.open, &setup.costs, cfg);
    let exit_price = fill_price(Side::Long, Execution::Exit, last.close, &setup.costs, cfg);
    let nights = funding_nights(
        first.start,
        last.end(),
        setup.funding_cutoff,
        setup.funding_tz,
    );
    let commission_total =
        commission(&setup.costs, size, entry_price) + commission(&setup.costs, size, exit_price);
    let funding_total = funding(
        Side::Long,
        first.open,
        size,
        nights,
        setup.benchmark_pct,
        &setup.costs,
    );
    let gross_pnl = size * (exit_price - entry_price);
    Some(BuyAndHold {
        entry_price,
        exit_price,
        nights,
        commission: commission_total,
        funding: funding_total,
        gross_pnl,
        net_pnl: gross_pnl - commission_total - funding_total,
    })
}

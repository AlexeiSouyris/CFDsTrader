//! Ejecución de las operaciones de estudio en una cuenta por instrumento (pre-registro §2.2).
//!
//! ```text
//! para cada Plan decidido en [from, to), en orden, sin posición abierta:
//!   día nuevo (zona del día) → capital del día; kill switch → se reactiva a los N días hábiles
//!   riesgo (cfds_risk::check) con el capital realizado → tamaño o rechazo
//!   entrada en la apertura de t+1 → stop (gap: a la apertura) / cierre forzado / salida planificada
//!   costos: spread + slippage en cada ejecución, comisión, financiación − base del futuro (#470)
//!   si salta el stop, el resto del evento no se abre
//! ```
//! Mismas funciones de costos que el backtest de la estrategia en uso (`fill_price`,
//! `commission`, `funding_over`, `basis_carry`): no hay otra implementación de los costos.

use cfds_core::Candle;
use cfds_risk::{Account, Rejection, Request, check};
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, Days, NaiveDate, Utc, Weekday};
use rust_decimal::Decimal;

use super::Plan;
use crate::{
    Execution, RunError, Setup, basis_carry, commission, fill_price, funding_cutoffs, funding_over,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudyExit {
    /// El mínimo (largo) o el máximo (corto) tocó el stop: sale al stop.
    Stop,
    /// La apertura ya había saltado el stop: sale a la apertura.
    StopGap,
    /// Cierre forzado de la preparación (fin de semana o vencimiento): sale al cierre.
    ForceClose,
    /// La salida de la definición: al cierre de la vela planificada.
    Planned,
    /// El máximo (largo) o el mínimo (corto) tocó el objetivo: sale al objetivo (programa 4).
    Target,
}

/// Una operación de estudio ejecutada. Montos en la moneda del instrumento, salvo
/// `equity_at_entry` (moneda de la cuenta).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyTrade {
    pub event: usize,
    pub side: Side,
    pub signal_index: usize,
    /// Cierre de la vela de la señal (cuando se decidió).
    pub signal_at: DateTime<Utc>,
    pub reference: Decimal,
    pub stop: Decimal,
    /// Objetivo fijado con el precio medio de la entrada, si el plan tiene uno.
    pub target: Option<Decimal>,
    pub entry_index: usize,
    pub entry_time: DateTime<Utc>,
    pub entry_mid: Decimal,
    pub entry_price: Decimal,
    pub size: Decimal,
    /// Lo que se pierde si salta el stop desde la referencia (sin gaps ni costos): 1 R.
    pub risk: Decimal,
    pub exit_index: usize,
    /// Fin de la vela de salida.
    pub exit_time: DateTime<Utc>,
    pub exit_mid: Decimal,
    pub exit_price: Decimal,
    pub exit: StudyExit,
    pub commission: Decimal,
    /// Financiación del CFD menos lo que la base del futuro ya descontó.
    pub funding: Decimal,
    pub mid_pnl: Decimal,
    pub gross_pnl: Decimal,
    pub net_pnl: Decimal,
    /// Capital realizado de la cuenta al decidir.
    pub equity_at_entry: Decimal,
}

impl StudyTrade {
    /// Todos los costos: spread + slippage + comisión + financiación neta.
    pub fn total_costs(&self) -> Decimal {
        self.mid_pnl - self.net_pnl
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyRejected {
    pub signal_index: usize,
    pub at: DateTime<Utc>,
    pub reason: Rejection,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StudyRun {
    pub trades: Vec<StudyTrade>,
    pub rejected: Vec<StudyRejected>,
    /// Planes con objetivo que no se operaron porque la apertura de la entrada ya estaba del
    /// lado del stop (índice de la señal).
    pub through_stop: Vec<usize>,
}

fn sign(side: Side) -> Decimal {
    match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    }
}

/// Recorre una pata desde la vela de entrada hasta `exit_index`: en cada vela, primero el gap
/// por la apertura, después el stop, después el objetivo (si hay), después el cierre forzado y
/// la salida planificada. Si una vela toca el stop y el objetivo, se supone el stop. Devuelve la
/// vela de salida, el precio medio y el motivo.
pub fn simulate_leg(
    side: Side,
    entry_index: usize,
    stop: Decimal,
    target: Option<Decimal>,
    exit_index: usize,
    m15: &[Candle],
    force_close: &[bool],
) -> Option<(usize, Decimal, StudyExit)> {
    for i in entry_index..=exit_index {
        let c = m15.get(i)?;
        let (gapped, touched) = match side {
            Side::Long => (c.open <= stop, c.low <= stop),
            Side::Short => (c.open >= stop, c.high >= stop),
        };
        if gapped {
            return Some((i, c.open, StudyExit::StopGap));
        }
        if touched {
            return Some((i, stop, StudyExit::Stop));
        }
        if let Some(t) = target {
            let reached = match side {
                Side::Long => c.high >= t,
                Side::Short => c.low <= t,
            };
            if reached {
                return Some((i, t, StudyExit::Target));
            }
        }
        if i == exit_index {
            return Some((i, c.close, StudyExit::Planned));
        }
        if force_close.get(i).copied().unwrap_or(false) {
            return Some((i, c.close, StudyExit::ForceClose));
        }
    }
    None
}

/// Ejecuta un plan ya aprobado por el riesgo con `size`, con todos los costos.
pub fn execute_plan(
    plan: &Plan,
    size: Decimal,
    risk: Decimal,
    equity_at_entry: Decimal,
    m15: &[Candle],
    force_close: &[bool],
    setup: &Setup,
) -> Result<StudyTrade, RunError> {
    let side = plan.side;
    let entry_index = plan.signal_index + 1;
    let entry = m15.get(entry_index).ok_or(RunError::NoEntryCandle)?;
    let cfg = &setup.backtest.costs;
    let entry_mid = entry.open;
    let entry_price = fill_price(side, Execution::Entry, entry_mid, &setup.costs, cfg);
    let target = plan.target_from(entry_mid);
    let (exit_index, exit_mid, exit) = simulate_leg(
        side,
        entry_index,
        plan.stop,
        target,
        plan.exit_index,
        m15,
        force_close,
    )
    .ok_or(RunError::NoEntryCandle)?;
    let exit_price = fill_price(side, Execution::Exit, exit_mid, &setup.costs, cfg);
    let exit_time = m15[exit_index].end();
    let commission_total =
        commission(&setup.costs, size, entry_price) + commission(&setup.costs, size, exit_price);
    let cutoffs = funding_cutoffs(
        entry.start,
        exit_time,
        setup.funding_cutoff,
        setup.funding_tz,
    );
    let mut funding = funding_over(
        side,
        entry_mid,
        size,
        &cutoffs,
        &setup.funding_rates,
        &setup.costs,
    )
    .map_err(RunError::MissingFundingRate)?;
    if let Some(basis) = &setup.futures_basis {
        funding -= basis_carry(side, entry_mid, size, &cutoffs, basis, &setup.costs)
            .map_err(RunError::MissingFundingRate)?;
    }
    let s = sign(side);
    let mid_pnl = size * (exit_mid - entry_mid) * s;
    let gross_pnl = size * (exit_price - entry_price) * s;
    Ok(StudyTrade {
        event: plan.event,
        side,
        signal_index: plan.signal_index,
        signal_at: m15[plan.signal_index].end(),
        reference: plan.reference,
        stop: plan.stop,
        target,
        entry_index,
        entry_time: entry.start,
        entry_mid,
        entry_price,
        size,
        risk,
        exit_index,
        exit_time,
        exit_mid,
        exit_price,
        exit,
        commission: commission_total,
        funding,
        mid_pnl,
        gross_pnl,
        net_pnl: gross_pnl - commission_total - funding,
        equity_at_entry,
    })
}

/// Días hábiles (lunes a viernes) en `(from, to]`.
fn weekdays_after(from: NaiveDate, to: NaiveDate) -> u32 {
    let mut n = 0;
    let mut d = from;
    while d < to {
        d = match d.checked_add_days(Days::new(1)) {
            Some(x) => x,
            None => break,
        };
        if !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
            n += 1;
        }
    }
    n
}

/// Ejecuta los planes **decididos** en `[from, to)` en una cuenta nueva de
/// `setup.initial_equity`, una posición a la vez. Cada operación pasa por `cfds_risk::check`.
pub fn run_plans(
    plans: &[Plan],
    m15: &[Candle],
    force_close: &[bool],
    setup: &Setup,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<StudyRun, RunError> {
    let mut out = StudyRun::default();
    let mut equity = setup.initial_equity;
    let mut peak = equity;
    let mut day: Option<NaiveDate> = None;
    let mut day_start = equity;
    let mut killed_on: Option<NaiveDate> = None;
    let mut busy_until = 0usize;
    let mut stopped_event: Option<usize> = None;
    let mut ordered: Vec<&Plan> = plans.iter().collect();
    ordered.sort_by_key(|p| p.signal_index);
    for p in ordered {
        let Some(signal) = m15.get(p.signal_index) else {
            continue;
        };
        let at = signal.end();
        if at < from || at >= to || p.signal_index < busy_until {
            continue;
        }
        if stopped_event == Some(p.event) {
            continue;
        }
        if p.target_r.is_some()
            && m15
                .get(p.signal_index + 1)
                .is_some_and(|e| p.open_beyond_stop(e.open))
        {
            out.through_stop.push(p.signal_index);
            continue;
        }
        let today = at.with_timezone(&setup.day_tz).date_naive();
        if day != Some(today) {
            day = Some(today);
            day_start = equity;
        }
        if let Some(k) = killed_on {
            match setup.backtest.kill_switch_reset_days {
                Some(n) if weekdays_after(k, today) >= n => {
                    killed_on = None;
                    peak = equity;
                }
                _ => continue,
            }
        }
        let fx = setup
            .account_to_instrument
            .at(at)
            .ok_or(RunError::MissingFxRate(at))?;
        let account = Account {
            equity,
            peak_equity: peak,
            day_start_equity: day_start,
            free_margin: equity,
            kill_switch: false,
            open_positions: Vec::new(),
        };
        let request = Request {
            instrument: setup.instrument.clone(),
            entry: p.reference,
            stop: p.stop,
            atr: p.risk_atr,
            account_to_instrument: fx,
        };
        let sizing = match check(&request, &account, &setup.limits, &setup.risk) {
            Ok(s) => s,
            Err(reason) => {
                if reason == Rejection::DrawdownLimit {
                    killed_on = Some(today);
                }
                out.rejected.push(StudyRejected {
                    signal_index: p.signal_index,
                    at,
                    reason,
                });
                continue;
            }
        };
        let trade = execute_plan(p, sizing.size, sizing.risk, equity, m15, force_close, setup)?;
        equity += setup.to_account(trade.net_pnl, trade.exit_time)?;
        peak = peak.max(equity);
        busy_until = trade.exit_index;
        if matches!(trade.exit, StudyExit::Stop | StudyExit::StopGap) {
            stopped_event = Some(p.event);
        }
        out.trades.push(trade);
    }
    Ok(out)
}

/// Retorno de una operación sobre el capital al decidir, con los costos multiplicados por
/// `stress` (1 = sin estrés): `(P/L medio − stress × costos) / capital`.
pub fn trade_return(t: &StudyTrade, stress: Decimal, setup: &Setup) -> Result<Decimal, RunError> {
    let net = t.mid_pnl - stress * t.total_costs();
    Ok(setup.to_account(net, t.exit_time)? / t.equity_at_entry)
}

/// Retornos mensuales (UTC) de `months` a partir de los retornos de cada operación y la hora en
/// que se cerró: cada mes compone las operaciones que se cerraron en él; sin operaciones, 0.
pub fn monthly_returns(closed: &[(DateTime<Utc>, Decimal)], months: &[(i32, u32)]) -> Vec<Decimal> {
    months
        .iter()
        .map(|&(y, m)| {
            closed
                .iter()
                .filter(|(at, _)| at.year() == y && at.month() == m)
                .fold(Decimal::ONE, |acc, (_, r)| acc * (Decimal::ONE + r))
                - Decimal::ONE
        })
        .collect()
}

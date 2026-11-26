//! Recorrido determinista: días de operaciones (P/L en dinero, en orden) por las reglas de cada
//! etapa. El mismo recorrido lo usa el Monte Carlo con días remuestreados: un solo cálculo.
//!
//! - El saldo se mira **al cierre de cada operación** (no hay capital vela por vela).
//! - Pérdida diaria: `saldo al inicio del día − saldo ≥ límite` → falla (tocar es fallar). El
//!   límite es un % del tamaño de la cuenta o del saldo al inicio del día.
//! - Caída máxima: `saldo ≤ piso` → falla. Piso fijo (`static`) = tamaño − monto; que sigue al
//!   máximo al cierre del día (`trailing_eod`) = máximo de los cierres − monto, trabado en el
//!   tamaño si `lock_at_initial`. La que sigue al máximo intradía no se simula (se deja afuera).
//! - Si una operación toca las dos, se informa la pérdida diaria.
//! - Al cierre de cada día: pasa si la ganancia ≥ objetivo, ya operó los días mínimos y el mejor
//!   día no pasa la regla de consistencia; si no, y llegó a los días máximos, se acabó el tiempo.
//! - En la fondeada, el primer cobro llega con ganancia > 0 y ≥ el mínimo, con los mismos
//!   chequeos de días y consistencia; cobro = ganancia × reparto.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use crate::rules::{DrawdownMode, Funded, LossBase, Phase, Rules};

/// Un día de la firma: su fecha (si se conoce) y el P/L de cada operación cerrada, en orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayRef<'a> {
    pub date: Option<NaiveDate>,
    pub pnl: &'a [Decimal],
}

/// El día de la firma de una hora UTC: la fecha local menos la hora de corte (un corte a las
/// 17:00 hace que las 17:00 del lunes ya sean el día que empieza el lunes a las 17:00).
pub fn firm_day(rules: &Rules, t: DateTime<Utc>) -> NaiveDate {
    let local = t.with_timezone(&rules.timezone).naive_local();
    (local - (rules.cutoff - chrono::NaiveTime::MIN)).date()
}

/// Agrupa operaciones `(cierre, P/L)` en días de la firma, en orden de cierre (estable).
pub fn group_days(
    rules: &Rules,
    trades: &[(DateTime<Utc>, Decimal)],
) -> Vec<(NaiveDate, Vec<Decimal>)> {
    let mut sorted: Vec<&(DateTime<Utc>, Decimal)> = trades.iter().collect();
    sorted.sort_by_key(|(t, _)| *t);
    let mut days: Vec<(NaiveDate, Vec<Decimal>)> = Vec::new();
    for (t, pnl) in sorted {
        let d = firm_day(rules, *t);
        match days.last_mut() {
            Some((last, xs)) if *last == d => xs.push(*pnl),
            _ => days.push((d, vec![*pnl])),
        }
    }
    days
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailRule {
    DailyLoss,
    MaxDrawdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// Pasó la fase (o llegó al primer cobro, en la fondeada).
    Passed,
    Failed(FailRule),
    /// Llegó a los días máximos de la firma sin pasar.
    Timeout,
    /// Se acabaron los días (la lista o el horizonte de la simulación).
    Unfinished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageResult {
    pub end: End,
    pub trading_days: u32,
    pub end_day: Option<NaiveDate>,
    /// Saldo − tamaño al terminar.
    pub profit: Decimal,
    pub best_day: Option<Decimal>,
    pub min_balance: Decimal,
    /// Cobro en la fondeada (ganancia × reparto).
    pub payout: Option<Decimal>,
}

/// Qué etapa se corre.
#[derive(Debug, Clone, Copy)]
pub enum Stage<'a> {
    Evaluation(&'a Phase),
    Funded(&'a Funded),
}

fn consistent(best_day: Option<Decimal>, profit: Decimal, max_share: Option<Decimal>) -> bool {
    match (max_share, best_day) {
        (Some(share), Some(best)) if best > Decimal::ZERO => best <= share * profit,
        _ => true,
    }
}

/// Corre una etapa desde el tamaño de la cuenta, consumiendo días de `days` hasta que termina.
pub fn run_stage<'a>(
    rules: &Rules,
    stage: Stage<'_>,
    days: &mut impl Iterator<Item = DayRef<'a>>,
) -> StageResult {
    let size = rules.account_size;
    let (min_days, max_days, consistency) = match stage {
        Stage::Evaluation(p) => (p.min_days, p.max_days, p.consistency),
        Stage::Funded(f) => (f.min_days, None, f.consistency),
    };
    let dd = rules
        .max_drawdown
        .filter(|d| d.mode != DrawdownMode::TrailingIntraday);
    let mut balance = size;
    let mut peak = size;
    let mut out = StageResult {
        end: End::Unfinished,
        trading_days: 0,
        end_day: None,
        profit: Decimal::ZERO,
        best_day: None,
        min_balance: size,
        payout: None,
    };
    let finish = |mut out: StageResult, end: End, day: Option<NaiveDate>, balance: Decimal| {
        out.end = end;
        out.end_day = day;
        out.profit = balance - size;
        out
    };
    for day in days.by_ref() {
        let day_start = balance;
        let daily_limit = rules.daily_loss.map(|d| {
            d.limit
                * match d.base {
                    LossBase::AccountSize => size,
                    LossBase::DayStart => day_start,
                }
        });
        let floor = dd.map(|d| {
            let f = match d.mode {
                DrawdownMode::Static => size - d.amount,
                _ => peak - d.amount,
            };
            if d.lock_at_initial { f.min(size) } else { f }
        });
        for pnl in day.pnl {
            balance += *pnl;
            out.min_balance = out.min_balance.min(balance);
            if daily_limit.is_some_and(|lim| day_start - balance >= lim) {
                out.trading_days += 1;
                return finish(out, End::Failed(FailRule::DailyLoss), day.date, balance);
            }
            if floor.is_some_and(|f| balance <= f) {
                out.trading_days += 1;
                return finish(out, End::Failed(FailRule::MaxDrawdown), day.date, balance);
            }
        }
        if day.pnl.is_empty() {
            continue;
        }
        out.trading_days += 1;
        let day_pnl = balance - day_start;
        out.best_day = Some(out.best_day.map_or(day_pnl, |b| b.max(day_pnl)));
        peak = peak.max(balance);
        let profit = balance - size;
        let reached = match stage {
            Stage::Evaluation(p) => profit >= p.target,
            Stage::Funded(f) => profit > Decimal::ZERO && profit >= f.min_profit,
        };
        if reached && out.trading_days >= min_days && consistent(out.best_day, profit, consistency)
        {
            let mut done = finish(out, End::Passed, day.date, balance);
            if let Stage::Funded(f) = stage {
                done.payout = Some(profit * f.split);
            }
            return done;
        }
        if max_days.is_some_and(|m| out.trading_days >= m) {
            return finish(out, End::Timeout, day.date, balance);
        }
    }
    finish(out, End::Unfinished, None, balance)
}

/// Todas las etapas en orden (cada una arranca de nuevo en el tamaño de la cuenta), hasta la
/// primera que no pasa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walk {
    pub phases: Vec<StageResult>,
    pub funded: Option<StageResult>,
}

impl Walk {
    pub fn passed_evaluation(&self, rules: &Rules) -> bool {
        self.phases.len() == rules.phases.len() && self.phases.iter().all(|p| p.end == End::Passed)
    }

    pub fn paid(&self) -> bool {
        self.funded.is_some_and(|f| f.end == End::Passed)
    }

    /// La etapa que terminó el recorrido (la última corrida).
    pub fn last(&self) -> Option<&StageResult> {
        self.funded.as_ref().or(self.phases.last())
    }
}

pub fn walk<'a>(rules: &Rules, days: &mut impl Iterator<Item = DayRef<'a>>) -> Walk {
    let mut phases = Vec::with_capacity(rules.phases.len());
    for p in &rules.phases {
        let r = run_stage(rules, Stage::Evaluation(p), days);
        phases.push(r);
        if r.end != End::Passed {
            return Walk {
                phases,
                funded: None,
            };
        }
    }
    let funded = rules
        .funded
        .as_ref()
        .map(|f| run_stage(rules, Stage::Funded(f), days));
    Walk { phases, funded }
}

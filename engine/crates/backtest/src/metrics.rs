//! Métricas de evaluación (ROADMAP Fase 5). Funciones puras, todo con `Decimal`.
//!
//! ```text
//! retorno total   = capital final / inicial − 1
//! drawdown máx.   = máx. de (pico − valor) / pico sobre la curva diaria (empieza en el inicial)
//! retornos diarios r_d = E_d / E_(d−1) − 1   (el primero contra el capital inicial)
//! Sharpe          = media(r − rf) / desvío muestral(r) × √días_por_año
//! Sortino         = media(r − rf) / √(media(min(r − rf, 0)²)) × √días_por_año
//! win rate        = operaciones con neto > 0 / operaciones
//! profit factor   = suma de netos positivos / |suma de netos negativos|
//! exposición      = tiempo con ≥ 1 posición abierta / tiempo total
//! ```
//! Cuando una métrica no está definida (sin operaciones, sin pérdidas, sin volatilidad, menos
//! de dos días), vale `None`: nunca se inventa un número.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::{Decimal, MathematicalOps};

use crate::MetricsConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metrics {
    /// Fracción (0,1 = +10 %).
    pub total_return: Decimal,
    /// Fracción positiva (0,2 = cayó 20 % desde un máximo).
    pub max_drawdown: Decimal,
    pub sharpe: Option<Decimal>,
    pub sortino: Option<Decimal>,
    pub trades: usize,
    pub win_rate: Option<Decimal>,
    pub profit_factor: Option<Decimal>,
    pub exposure: Option<Decimal>,
}

/// Retornos diarios de la curva; el primer día se compara con el capital inicial.
pub fn daily_returns(initial: Decimal, curve: &[(NaiveDate, Decimal)]) -> Vec<Decimal> {
    let mut prev = initial;
    curve
        .iter()
        .map(|&(_, e)| {
            let r = e / prev - Decimal::ONE;
            prev = e;
            r
        })
        .collect()
}

pub fn max_drawdown(initial: Decimal, curve: &[(NaiveDate, Decimal)]) -> Decimal {
    let mut peak = initial;
    let mut worst = Decimal::ZERO;
    for &(_, e) in curve {
        peak = peak.max(e);
        worst = worst.max((peak - e) / peak);
    }
    worst
}

fn daily_risk_free(cfg: &MetricsConfig) -> Decimal {
    cfg.risk_free_annual_pct / Decimal::ONE_HUNDRED / Decimal::from(cfg.trading_days_per_year)
}

fn annualize(cfg: &MetricsConfig) -> Option<Decimal> {
    Decimal::from(cfg.trading_days_per_year).sqrt()
}

fn mean(xs: &[Decimal]) -> Option<Decimal> {
    (!xs.is_empty()).then(|| xs.iter().sum::<Decimal>() / Decimal::from(xs.len()))
}

pub fn sharpe(returns: &[Decimal], cfg: &MetricsConfig) -> Option<Decimal> {
    if returns.len() < 2 {
        return None;
    }
    let rf = daily_risk_free(cfg);
    let excess: Vec<Decimal> = returns.iter().map(|r| r - rf).collect();
    let m = mean(&excess)?;
    let raw_mean = mean(returns)?;
    let var = returns
        .iter()
        .map(|r| (r - raw_mean) * (r - raw_mean))
        .sum::<Decimal>()
        / Decimal::from(returns.len() - 1);
    let sd = var.sqrt()?;
    if sd.is_zero() {
        return None;
    }
    Some(m / sd * annualize(cfg)?)
}

pub fn sortino(returns: &[Decimal], cfg: &MetricsConfig) -> Option<Decimal> {
    if returns.len() < 2 {
        return None;
    }
    let rf = daily_risk_free(cfg);
    let excess: Vec<Decimal> = returns.iter().map(|r| r - rf).collect();
    let m = mean(&excess)?;
    let downside: Vec<Decimal> = excess
        .iter()
        .map(|&x| {
            let d = x.min(Decimal::ZERO);
            d * d
        })
        .collect();
    let dd = mean(&downside)?.sqrt()?;
    if dd.is_zero() {
        return None;
    }
    Some(m / dd * annualize(cfg)?)
}

pub fn win_rate(net: &[Decimal]) -> Option<Decimal> {
    let wins = net.iter().filter(|n| **n > Decimal::ZERO).count();
    (!net.is_empty()).then(|| Decimal::from(wins) / Decimal::from(net.len()))
}

pub fn profit_factor(net: &[Decimal]) -> Option<Decimal> {
    let gains: Decimal = net.iter().filter(|n| **n > Decimal::ZERO).sum();
    let losses: Decimal = net.iter().filter(|n| **n < Decimal::ZERO).sum();
    (!losses.is_zero()).then(|| gains / losses.abs())
}

/// Fracción de `[start, end)` cubierta por al menos uno de los intervalos `[desde, hasta)`.
pub fn exposure(
    intervals: &[(DateTime<Utc>, DateTime<Utc>)],
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Option<Decimal> {
    if end <= start {
        return None;
    }
    let mut spans: Vec<(DateTime<Utc>, DateTime<Utc>)> = intervals
        .iter()
        .map(|&(a, b)| (a.max(start), b.min(end)))
        .filter(|(a, b)| a < b)
        .collect();
    spans.sort();
    let mut covered = 0i64;
    let mut cursor = start;
    for (a, b) in spans {
        let from = a.max(cursor);
        if b > from {
            covered += (b - from).num_seconds();
            cursor = b;
        }
    }
    Some(Decimal::from(covered) / Decimal::from((end - start).num_seconds()))
}

/// Todas las métricas de un recorrido. `net` son los netos de cada operación (en la moneda de
/// la cuenta), `intervals` el tiempo que estuvo abierta cada una, `[start, end)` el período.
pub fn evaluate(
    initial: Decimal,
    final_equity: Decimal,
    curve: &[(NaiveDate, Decimal)],
    net: &[Decimal],
    intervals: &[(DateTime<Utc>, DateTime<Utc>)],
    period: (DateTime<Utc>, DateTime<Utc>),
    cfg: &MetricsConfig,
) -> Metrics {
    let returns = daily_returns(initial, curve);
    Metrics {
        total_return: final_equity / initial - Decimal::ONE,
        max_drawdown: max_drawdown(initial, curve),
        sharpe: sharpe(&returns, cfg),
        sortino: sortino(&returns, cfg),
        trades: net.len(),
        win_rate: win_rate(net),
        profit_factor: profit_factor(net),
        exposure: exposure(intervals, period.0, period.1),
    }
}

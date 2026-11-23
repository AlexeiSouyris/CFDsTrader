//! Evaluación de la Fase 5 (ROADMAP, #298): walk-forward, comprar y mantener por ventana y el
//! tramo fuera de muestra aparte.
//!
//! ```text
//! tramos = cfds_market_data::split::plan (config/data_split.toml): ventanas de entrenamiento y
//!   prueba, y el fuera de muestra al final
//! evaluar un tramo [inicio, fin):
//!   velas desde inicio − warmup (solo calientan: ahí no se abre nada) hasta las que cierran en fin
//!   recorrido de siempre (run_with_news) con la cuenta desde el capital inicial
//!   métricas (metrics::evaluate) con la curva desde el día de inicio
//!   comprar y mantener (runner::buy_and_hold, con costos y financiación) con las velas del tramo
//! walk-forward: cada ventana con los parámetros de config, entrenamiento y prueba por separado
//! fuera de muestra: una sola evaluación, con los parámetros de config
//! ```
//! Guardia: toda evaluación de investigación (entrenamiento y prueba) pasa por
//! [`research_period`], que da error si el tramo llega al fuera de muestra. El fuera de muestra
//! solo se evalúa con [`evaluate_holdout`], que no recibe ningún parámetro para variar.
//!
//! Nada cambia los parámetros de la estrategia: todo se evalúa con los valores de config.

use cfds_core::{Candle, NewsInterpretation};
use cfds_market_data::{DataSplit, Period};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{BuyAndHold, Metrics, RunError, Setup, buy_and_hold, run_with_news};

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    Config(String),
    /// Un tramo de investigación llega al fuera de muestra: nada se ajusta ni se mide con él.
    TouchesHoldout {
        period_end: DateTime<Utc>,
        holdout_start: DateTime<Utc>,
    },
    Run(RunError),
}

impl From<RunError> for EvalError {
    fn from(e: RunError) -> Self {
        Self::Run(e)
    }
}

/// `config/evaluation.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationConfig {
    /// Velas anteriores a cada tramo que solo calientan los indicadores.
    pub warmup: TimeDelta,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    walk_forward: RawWf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWf {
    warmup_days: u32,
}

impl EvaluationConfig {
    pub fn from_toml(text: &str) -> Result<Self, EvalError> {
        let raw: Raw = toml::from_str(text).map_err(|e| EvalError::Config(e.to_string()))?;
        if raw.walk_forward.warmup_days == 0 || raw.walk_forward.warmup_days > 365 {
            return Err(EvalError::Config(
                "walk_forward.warmup_days tiene que ir de 1 a 365".into(),
            ));
        }
        Ok(Self {
            warmup: TimeDelta::days(i64::from(raw.walk_forward.warmup_days)),
        })
    }
}

/// Velas y marcas ya preparadas para todo el período (como las arma `backtest_run`).
#[derive(Debug, Clone, Copy)]
pub struct Inputs<'a> {
    pub m15: &'a [Candle],
    pub h1: &'a [Candle],
    /// Vacío = sin restricción de horario (series sintéticas).
    pub entries: &'a [bool],
    pub force_close: &'a [bool],
    pub data_errors: &'a [DateTime<Utc>],
    pub news: &'a [NewsInterpretation],
    pub published_from: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodResult {
    pub period: Period,
    /// Velas de 15m dentro del tramo (sin contar el calentamiento).
    pub candles: usize,
    /// `None` si el tramo no tiene velas (fold vacío): nunca se inventan métricas.
    pub metrics: Option<Metrics>,
    pub final_equity: Decimal,
    /// `None` sin velas o si falta el tipo de referencia de alguna noche.
    pub buy_and_hold: Option<BuyAndHold>,
    /// Neto de comprar y mantener sobre el capital inicial, en la moneda de la cuenta.
    pub buy_and_hold_return: Option<Decimal>,
}

/// Una ventana del walk-forward, con los parámetros de config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldResult {
    pub index: usize,
    pub train: PeriodResult,
    pub test: PeriodResult,
}

/// Error si `period` llega al fuera de muestra.
pub fn research_period(period: Period, split: &DataSplit) -> Result<Period, EvalError> {
    if period.end > split.out_of_sample.start {
        return Err(EvalError::TouchesHoldout {
            period_end: period.end,
            holdout_start: split.out_of_sample.start,
        });
    }
    Ok(period)
}

/// Recorre `period` con `setup` (ver el módulo). Las velas tienen que estar en orden.
pub fn evaluate_period(
    inputs: &Inputs,
    period: Period,
    setup: &Setup,
    warmup: TimeDelta,
    bh_size: Decimal,
) -> Result<PeriodResult, RunError> {
    let m15 = inputs.m15;
    let lo = m15.partition_point(|c| c.start < period.start - warmup);
    let first = m15.partition_point(|c| c.start < period.start);
    let hi = m15.partition_point(|c| c.end() <= period.end);
    let empty = PeriodResult {
        period,
        candles: 0,
        metrics: None,
        final_equity: setup.initial_equity,
        buy_and_hold: None,
        buy_and_hold_return: None,
    };
    if hi <= first {
        return Ok(empty);
    }
    let candles = &m15[lo..hi];
    let entries: Vec<bool> = (lo..hi)
        .map(|i| i >= first && inputs.entries.get(i).copied().unwrap_or(true))
        .collect();
    let force_close: Vec<bool> = if inputs.force_close.is_empty() {
        Vec::new()
    } else {
        inputs.force_close[lo..hi].to_vec()
    };
    let h1_lo = inputs
        .h1
        .partition_point(|c| c.start < period.start - warmup);
    let h1_hi = inputs.h1.partition_point(|c| c.end() <= period.end);
    let h1 = inputs.h1.get(h1_lo..h1_hi).unwrap_or(&[]);
    let report = run_with_news(
        candles,
        h1,
        &entries,
        &force_close,
        inputs.data_errors,
        inputs.news,
        inputs.published_from,
        setup,
    )?;
    let first_day = period.start.with_timezone(&setup.day_tz).date_naive();
    let curve: Vec<_> = report
        .daily_equity
        .iter()
        .copied()
        .filter(|(d, _)| *d >= first_day)
        .collect();
    let net: Vec<Decimal> = report
        .trades
        .iter()
        .map(|t| t.net_pnl / setup.account_to_instrument)
        .collect();
    let intervals: Vec<_> = report
        .trades
        .iter()
        .map(|t| {
            (
                t.entry_time,
                t.exits.last().map_or(t.entry_time, |e| e.time),
            )
        })
        .collect();
    let end = m15[hi - 1].end().min(period.end);
    let metrics = crate::evaluate(
        setup.initial_equity,
        report.final_equity,
        &curve,
        &net,
        &intervals,
        (period.start, end),
        &setup.backtest.metrics,
    );
    let bh = buy_and_hold(&m15[first..hi], bh_size, setup);
    Ok(PeriodResult {
        period,
        candles: hi - first,
        metrics: Some(metrics),
        final_equity: report.final_equity,
        buy_and_hold_return: bh
            .map(|b| b.net_pnl / setup.account_to_instrument / setup.initial_equity),
        buy_and_hold: bh,
    })
}

/// Cada ventana del walk-forward con los parámetros de config: entrenamiento y prueba por
/// separado, cada una desde el capital inicial.
pub fn walk_forward(
    inputs: &Inputs,
    split: &DataSplit,
    setup: &Setup,
    cfg: &EvaluationConfig,
    bh_size: Decimal,
) -> Result<Vec<FoldResult>, EvalError> {
    split
        .windows
        .iter()
        .enumerate()
        .map(|(index, w)| {
            let train = research_period(w.train, split)?;
            let test = research_period(w.validation, split)?;
            Ok(FoldResult {
                index,
                train: evaluate_period(inputs, train, setup, cfg.warmup, bh_size)?,
                test: evaluate_period(inputs, test, setup, cfg.warmup, bh_size)?,
            })
        })
        .collect()
}

/// El tramo fuera de muestra, una sola vez y con los parámetros de config. No recibe nada que
/// variar: se reporta aparte y sin retocar.
pub fn evaluate_holdout(
    inputs: &Inputs,
    split: &DataSplit,
    setup: &Setup,
    cfg: &EvaluationConfig,
    bh_size: Decimal,
) -> Result<PeriodResult, RunError> {
    evaluate_period(inputs, split.out_of_sample, setup, cfg.warmup, bh_size)
}

/// Resumen de varios tramos (por ejemplo, las pruebas del walk-forward).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregate {
    pub periods: usize,
    /// Tramos con velas (los vacíos no cuentan en nada de lo que sigue).
    pub with_data: usize,
    pub trades: usize,
    pub mean_return: Option<Decimal>,
    pub median_return: Option<Decimal>,
    pub positive: usize,
    pub worst_drawdown: Option<Decimal>,
    /// Media de los Sharpe definidos y cuántos lo estaban.
    pub mean_sharpe: Option<Decimal>,
    pub sharpe_defined: usize,
    /// Tramos en que la estrategia superó a comprar y mantener (ambos con dato).
    pub beat_buy_and_hold: usize,
    pub with_buy_and_hold: usize,
}

fn mean(xs: &[Decimal]) -> Option<Decimal> {
    (!xs.is_empty()).then(|| xs.iter().sum::<Decimal>() / Decimal::from(xs.len()))
}

fn median(xs: &[Decimal]) -> Option<Decimal> {
    let mut v = xs.to_vec();
    v.sort();
    let n = v.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(v[n / 2]),
        _ => Some((v[n / 2 - 1] + v[n / 2]) / Decimal::TWO),
    }
}

pub fn aggregate<'a>(periods: impl IntoIterator<Item = &'a PeriodResult>) -> Aggregate {
    let periods: Vec<&PeriodResult> = periods.into_iter().collect();
    let with: Vec<(&PeriodResult, &Metrics)> = periods
        .iter()
        .filter_map(|p| p.metrics.as_ref().map(|m| (*p, m)))
        .collect();
    let returns: Vec<Decimal> = with.iter().map(|(_, m)| m.total_return).collect();
    let sharpes: Vec<Decimal> = with.iter().filter_map(|(_, m)| m.sharpe).collect();
    let both: Vec<(Decimal, Decimal)> = with
        .iter()
        .filter_map(|(p, m)| p.buy_and_hold_return.map(|b| (m.total_return, b)))
        .collect();
    Aggregate {
        periods: periods.len(),
        with_data: with.len(),
        trades: with.iter().map(|(_, m)| m.trades).sum(),
        mean_return: mean(&returns),
        median_return: median(&returns),
        positive: returns.iter().filter(|r| **r > Decimal::ZERO).count(),
        worst_drawdown: with.iter().map(|(_, m)| m.max_drawdown).max(),
        mean_sharpe: mean(&sharpes),
        sharpe_defined: sharpes.len(),
        beat_buy_and_hold: both.iter().filter(|(s, b)| s > b).count(),
        with_buy_and_hold: both.len(),
    }
}

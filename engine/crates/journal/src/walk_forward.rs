//! Reporte de la evaluación de la Fase 5 (`contracts/walk_forward_report.v1`, #298): walk-forward,
//! sensibilidad y su estabilidad, y el fuera de muestra aparte. Se arma con los resultados de
//! `cfds_backtest::evaluation` y `cfds_backtest::sensitivity`, sin recalcular nada.

use cfds_backtest::{
    Aggregate, FoldResult, Metrics, ParamFile, PeriodResult, SensitivityPoint, Stability,
};
use cfds_core::InstrumentId;
use cfds_core::candle::utc_z;
use cfds_core::decimal_str;
use cfds_market_data::SplitConfig;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// `contracts/walk_forward_report.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalkForwardReport {
    pub report_id: String,
    pub instrument_id: InstrumentId,
    pub candle_source: CandleSourceName,
    pub config_sha256: String,
    pub split: WfSplit,
    pub warmup_days: u32,
    #[serde(with = "decimal_str")]
    pub initial_equity: Decimal,
    #[serde(with = "decimal_str")]
    pub buy_and_hold_size: Decimal,
    pub folds: Vec<WfFold>,
    pub test_aggregate: WfAggregate,
    pub sensitivity: Vec<WfSensitivity>,
    pub stability: Vec<WfStability>,
    pub holdout: WfPeriod,
    #[serde(with = "utc_z")]
    pub computed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandleSourceName {
    Ig,
    Futures,
}

impl CandleSourceName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ig => "ig",
            Self::Futures => "futures",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfSplit {
    pub train_months: u32,
    pub validation_months: u32,
    pub step_months: u32,
    pub out_of_sample_months: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfFold {
    pub index: usize,
    pub train: WfPeriod,
    pub test: WfPeriod,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfPeriod {
    #[serde(with = "utc_z")]
    pub from: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub to: DateTime<Utc>,
    pub candles: usize,
    pub metrics: Option<WfMetrics>,
    #[serde(with = "decimal_str::signed")]
    pub final_equity: Decimal,
    pub buy_and_hold: Option<WfBuyAndHold>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfBuyAndHold {
    /// Sobre el capital inicial, en la moneda de la cuenta.
    #[serde(with = "decimal_str::signed")]
    pub r#return: Decimal,
    /// En la moneda del instrumento, como `commission` y `funding`.
    #[serde(with = "decimal_str::signed")]
    pub net_pnl: Decimal,
    #[serde(with = "decimal_str")]
    pub commission: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub funding: Decimal,
    pub nights: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfMetrics {
    pub trades: usize,
    #[serde(with = "decimal_str::signed")]
    pub total_return: Decimal,
    #[serde(with = "decimal_str")]
    pub max_drawdown: Decimal,
    #[serde(with = "decimal_str::signed_option")]
    pub sharpe: Option<Decimal>,
    #[serde(with = "decimal_str::signed_option")]
    pub sortino: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub win_rate: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub profit_factor: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub exposure: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfAggregate {
    pub periods: usize,
    pub with_data: usize,
    pub trades: usize,
    #[serde(with = "decimal_str::signed_option")]
    pub mean_return: Option<Decimal>,
    #[serde(with = "decimal_str::signed_option")]
    pub median_return: Option<Decimal>,
    pub positive: usize,
    #[serde(with = "decimal_str::option")]
    pub worst_drawdown: Option<Decimal>,
    #[serde(with = "decimal_str::signed_option")]
    pub mean_sharpe: Option<Decimal>,
    pub sharpe_defined: usize,
    pub beat_buy_and_hold: usize,
    pub with_buy_and_hold: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfSensitivity {
    pub file: String,
    pub key: String,
    pub value: String,
    pub fold: usize,
    pub metrics: Option<WfMetrics>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfStability {
    pub file: String,
    pub key: String,
    pub by_value: Vec<WfValue>,
    #[serde(with = "decimal_str::option")]
    pub return_spread: Option<Decimal>,
    pub same_sign: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WfValue {
    pub value: String,
    #[serde(with = "decimal_str::signed_option")]
    pub mean_return: Option<Decimal>,
    #[serde(with = "decimal_str::signed_option")]
    pub mean_sharpe: Option<Decimal>,
    pub trades: usize,
}

fn n(d: Decimal) -> Decimal {
    d.normalize()
}

impl From<&Metrics> for WfMetrics {
    fn from(m: &Metrics) -> Self {
        Self {
            trades: m.trades,
            total_return: n(m.total_return),
            max_drawdown: n(m.max_drawdown),
            sharpe: m.sharpe.map(n),
            sortino: m.sortino.map(n),
            win_rate: m.win_rate.map(n),
            profit_factor: m.profit_factor.map(n),
            exposure: m.exposure.map(n),
        }
    }
}

impl From<&PeriodResult> for WfPeriod {
    fn from(p: &PeriodResult) -> Self {
        Self {
            from: p.period.start,
            to: p.period.end,
            candles: p.candles,
            metrics: p.metrics.as_ref().map(WfMetrics::from),
            final_equity: n(p.final_equity),
            buy_and_hold: match (p.buy_and_hold, p.buy_and_hold_return) {
                (Some(b), Some(r)) => Some(WfBuyAndHold {
                    r#return: n(r),
                    net_pnl: n(b.net_pnl),
                    commission: n(b.commission),
                    funding: n(b.funding),
                    nights: b.nights,
                }),
                _ => None,
            },
        }
    }
}

impl From<&Aggregate> for WfAggregate {
    fn from(a: &Aggregate) -> Self {
        Self {
            periods: a.periods,
            with_data: a.with_data,
            trades: a.trades,
            mean_return: a.mean_return.map(n),
            median_return: a.median_return.map(n),
            positive: a.positive,
            worst_drawdown: a.worst_drawdown.map(n),
            mean_sharpe: a.mean_sharpe.map(n),
            sharpe_defined: a.sharpe_defined,
            beat_buy_and_hold: a.beat_buy_and_hold,
            with_buy_and_hold: a.with_buy_and_hold,
        }
    }
}

fn file(f: ParamFile) -> String {
    f.name().to_owned()
}

/// Lo que no sale de los resultados: identidad, fuente y config de la evaluación.
#[derive(Debug, Clone)]
pub struct ReportHeader {
    pub report_id: String,
    pub instrument_id: InstrumentId,
    pub candle_source: CandleSourceName,
    pub config_sha256: String,
    pub split: SplitConfig,
    pub warmup_days: u32,
    pub initial_equity: Decimal,
    pub buy_and_hold_size: Decimal,
    pub computed_at: DateTime<Utc>,
}

impl WalkForwardReport {
    pub fn new(
        h: ReportHeader,
        folds: &[FoldResult],
        test_aggregate: &Aggregate,
        sensitivity: &[SensitivityPoint],
        stability: &[Stability],
        holdout: &PeriodResult,
    ) -> Self {
        Self {
            report_id: h.report_id,
            instrument_id: h.instrument_id,
            candle_source: h.candle_source,
            config_sha256: h.config_sha256,
            split: WfSplit {
                train_months: h.split.train_months,
                validation_months: h.split.validation_months,
                step_months: h.split.step_months,
                out_of_sample_months: h.split.out_of_sample_months,
            },
            warmup_days: h.warmup_days,
            initial_equity: n(h.initial_equity),
            buy_and_hold_size: n(h.buy_and_hold_size),
            folds: folds
                .iter()
                .map(|f| WfFold {
                    index: f.index,
                    train: (&f.train).into(),
                    test: (&f.test).into(),
                })
                .collect(),
            test_aggregate: test_aggregate.into(),
            sensitivity: sensitivity
                .iter()
                .map(|p| WfSensitivity {
                    file: file(p.file),
                    key: p.key.clone(),
                    value: p.value.clone(),
                    fold: p.fold,
                    metrics: p.metrics.as_ref().map(WfMetrics::from),
                })
                .collect(),
            stability: stability
                .iter()
                .map(|s| WfStability {
                    file: file(s.file),
                    key: s.key.clone(),
                    by_value: s
                        .by_value
                        .iter()
                        .map(|(value, r, sh, trades)| WfValue {
                            value: value.clone(),
                            mean_return: r.map(n),
                            mean_sharpe: sh.map(n),
                            trades: *trades,
                        })
                        .collect(),
                    return_spread: s.return_spread.map(n),
                    same_sign: s.same_sign,
                })
                .collect(),
            holdout: holdout.into(),
            computed_at: h.computed_at,
        }
    }

    /// El mismo reporte, sin contar cuándo se calculó.
    pub fn same_report(&self, other: &Self) -> bool {
        Self {
            computed_at: other.computed_at,
            ..self.clone()
        } == *other
    }

    /// Lo que el esquema exige y Rust no garantiza por tipos.
    pub fn valid_id(&self) -> bool {
        (1..=100).contains(&self.report_id.len())
            && self
                .report_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            && self.config_sha256.len() == 64
            && self
                .config_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

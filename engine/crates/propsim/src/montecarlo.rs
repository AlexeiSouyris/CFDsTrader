//! Monte Carlo del simulador de cuentas fondeadas (#803, §6.2 del diseño de la Academia).
//!
//! Cada camino es **un intento** (una cuota): una secuencia de días que pasa por las mismas
//! etapas y el mismo recorrido de [`crate::walk`]. Los días salen de:
//! - la lista real: días de la firma remuestreados en bloques con el bootstrap circular de
//!   `research::montecarlo` (no se escribe otro);
//! - el modelo del simulador de varianza (`research::variance`): `trades_per_day` operaciones
//!   por día, con el win rate del camino (sorteado de la Beta si hay incertidumbre).
//!
//! Hasta `horizon_days` días por intento; si no terminó, queda "sin terminar". Con los conteos:
//! probabilidades con Wilson, días hasta pasar o fallar, cuotas y valor esperado del programa en
//! dinero, y la comparación con operaciones de esperanza 0 (la lista centrada en 0, o el modelo
//! con win rate L / (W + L)), con la misma semilla.
#![allow(
    clippy::float_arithmetic,
    reason = "probabilidades y estadísticos de caminos simulados (como research::montecarlo); \
              el dinero va en Decimal"
)]

use cfds_backtest::lab::sections::{num, summary, text};
use cfds_backtest::research::montecarlo::{bootstrap_n, summarize};
use cfds_backtest::research::program4::SplitMix64;
use cfds_backtest::research::stats;
use cfds_backtest::research::variance::{Payoff, draw_r, path_win_rate};
use cfds_indicators::wilson;
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::rules::Rules;
use crate::walk::{DayRef, End, FailRule, StageResult, Walk, walk};

/// `[monte_carlo]` de `config/propsim.toml` (PENDIENTE del dueño; educativo).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McConfig {
    pub max_paths: usize,
    pub max_horizon_days: usize,
    pub max_attempts: u32,
    pub max_trades_per_day: u32,
    pub max_day_steps: usize,
    pub default_paths: usize,
    pub default_seed: u64,
    pub default_block_len_days: usize,
    pub default_horizon_days: usize,
    pub default_max_attempts: u32,
    pub confidence: Decimal,
}

impl McConfig {
    pub fn validate(&self) -> Result<(), String> {
        let ok = self.max_paths > 0
            && self.max_horizon_days > 0
            && self.max_attempts > 0
            && self.max_trades_per_day > 0
            && self.max_day_steps > 0
            && self.default_paths.saturating_mul(self.default_horizon_days) <= self.max_day_steps
            && (1..=self.max_paths).contains(&self.default_paths)
            && self.default_block_len_days > 0
            && (1..=self.max_horizon_days).contains(&self.default_horizon_days)
            && (1..=self.max_attempts).contains(&self.default_max_attempts)
            && self.confidence > Decimal::ZERO
            && self.confidence < Decimal::ONE;
        if ok {
            Ok(())
        } else {
            Err("propsim.toml: [monte_carlo] tiene un tope o un valor por defecto inválido".into())
        }
    }
}

/// Parámetros ya resueltos (pedido + config).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McParams {
    pub paths: usize,
    pub seed: u64,
    pub block_len_days: usize,
    pub horizon_days: usize,
    pub max_attempts: u32,
}

/// De dónde salen los días de cada camino.
#[derive(Debug, Clone, Copy)]
pub enum DaySource<'a> {
    /// Días reales (P/L en dinero), remuestreados en bloques.
    Days(&'a [Vec<Decimal>]),
    /// El modelo: cada operación gana `win` o pierde `loss` (en dinero, `loss` en positivo).
    Model {
        payoff: Payoff,
        trades_per_day: u32,
        win: Decimal,
        loss: Decimal,
    },
}

/// Lo que se cuenta de cada etapa sobre todos los caminos.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StageTally {
    pub reached: usize,
    pub passed: usize,
    pub failed_daily_loss: usize,
    pub failed_max_drawdown: usize,
    pub timeout: usize,
    pub days_to_pass: Vec<f64>,
    pub days_to_fail: Vec<f64>,
}

impl StageTally {
    fn add(&mut self, s: &StageResult) {
        self.reached += 1;
        let days = f64::from(s.trading_days);
        match s.end {
            End::Passed => {
                self.passed += 1;
                self.days_to_pass.push(days);
            }
            End::Failed(rule) => {
                match rule {
                    FailRule::DailyLoss => self.failed_daily_loss += 1,
                    FailRule::MaxDrawdown => self.failed_max_drawdown += 1,
                }
                self.days_to_fail.push(days);
            }
            End::Timeout => self.timeout += 1,
            End::Unfinished => {}
        }
    }
}

/// Conteos de una corrida de Monte Carlo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tally {
    pub paths: usize,
    pub pass_evaluation: usize,
    pub payout: usize,
    pub failed: usize,
    pub timeout: usize,
    pub unfinished: usize,
    /// Fases de evaluación y, al final, la fondeada (si existe).
    pub stages: Vec<StageTally>,
    /// Cobro de cada camino que cobró.
    pub payouts: Vec<Decimal>,
}

fn tally_walk(t: &mut Tally, rules: &Rules, w: &Walk) {
    t.paths += 1;
    for (k, s) in w.phases.iter().enumerate() {
        t.stages[k].add(s);
    }
    if let Some(f) = &w.funded {
        t.stages[rules.phases.len()].add(f);
        if let Some(p) = f.payout {
            t.payouts.push(p);
        }
    }
    if w.passed_evaluation(rules) {
        t.pass_evaluation += 1;
    }
    let paid = w.paid();
    if paid {
        t.payout += 1;
    }
    let done = paid || (w.passed_evaluation(rules) && rules.funded.is_none());
    if !done {
        match w.last().map(|s| s.end) {
            Some(End::Failed(_)) => t.failed += 1,
            Some(End::Timeout) => t.timeout += 1,
            _ => t.unfinished += 1,
        }
    }
}

/// Corre los caminos. `days` no vacío con la fuente real.
pub fn simulate(rules: &Rules, source: &DaySource<'_>, p: &McParams) -> Tally {
    let stages = rules.phases.len() + usize::from(rules.funded.is_some());
    let mut t = Tally {
        stages: vec![StageTally::default(); stages],
        ..Tally::default()
    };
    let mut rng = SplitMix64::new(p.seed);
    match source {
        DaySource::Days(days) => {
            let idx: Vec<usize> = (0..days.len()).collect();
            for _ in 0..p.paths {
                let path = bootstrap_n(&mut rng, &idx, p.block_len_days, p.horizon_days);
                let mut it = path.iter().map(|&i| DayRef {
                    date: None,
                    pnl: &days[i],
                });
                tally_walk(&mut t, rules, &walk(rules, &mut it));
            }
        }
        DaySource::Model {
            payoff,
            trades_per_day,
            win,
            loss,
        } => {
            for _ in 0..p.paths {
                let wr = path_win_rate(&mut rng, payoff);
                let path: Vec<Vec<Decimal>> = (0..p.horizon_days)
                    .map(|_| {
                        (0..*trades_per_day)
                            .map(|_| {
                                if draw_r(&mut rng, payoff, wr) > 0.0 {
                                    *win
                                } else {
                                    -*loss
                                }
                            })
                            .collect()
                    })
                    .collect();
                let mut it = path.iter().map(|d| DayRef { date: None, pnl: d });
                tally_walk(&mut t, rules, &walk(rules, &mut it));
            }
        }
    }
    t
}

/// La misma fuente con esperanza 0: la lista real centrada en su media por operación, o el
/// modelo con win rate L / (W + L) (sin incertidumbre).
pub fn zero_edge(source: &DaySource<'_>) -> ZeroEdge {
    match source {
        DaySource::Days(days) => {
            let n: usize = days.iter().map(Vec::len).sum();
            let total: Decimal = days.iter().flatten().copied().sum();
            let mean = if n == 0 {
                Decimal::ZERO
            } else {
                total / Decimal::from(n)
            };
            ZeroEdge::Days(
                days.iter()
                    .map(|d| d.iter().map(|x| *x - mean).collect())
                    .collect(),
            )
        }
        DaySource::Model {
            payoff,
            trades_per_day,
            win,
            loss,
        } => ZeroEdge::Model(DaySourceOwned {
            payoff: Payoff {
                win_rate: payoff.avg_loss_r / (payoff.avg_win_r + payoff.avg_loss_r),
                observed_trades: None,
                ..*payoff
            },
            trades_per_day: *trades_per_day,
            win: *win,
            loss: *loss,
        }),
    }
}

/// Fuente sin ventaja, con sus datos propios.
#[derive(Debug, Clone)]
pub enum ZeroEdge {
    Days(Vec<Vec<Decimal>>),
    Model(DaySourceOwned),
}

#[derive(Debug, Clone, Copy)]
pub struct DaySourceOwned {
    pub payoff: Payoff,
    pub trades_per_day: u32,
    pub win: Decimal,
    pub loss: Decimal,
}

impl ZeroEdge {
    pub fn source(&self) -> DaySource<'_> {
        match self {
            Self::Days(d) => DaySource::Days(d),
            Self::Model(m) => DaySource::Model {
                payoff: m.payoff,
                trades_per_day: m.trades_per_day,
                win: m.win,
                loss: m.loss,
            },
        }
    }
}

/// Probabilidad de cobrar (o de pasar, sin fondeada) por intento, como fracción.
fn success(t: &Tally, rules: &Rules) -> usize {
    if rules.funded.is_some() {
        t.payout
    } else {
        t.pass_evaluation
    }
}

fn dec(x: f64) -> Decimal {
    Decimal::from_f64(x).unwrap_or(Decimal::ZERO)
}

/// Cuotas y valor esperado del programa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Economics {
    pub success_within: Decimal,
    pub expected_attempts: Decimal,
    pub expected_fees: Decimal,
    pub expected_payout: Option<Decimal>,
    pub expected_refund: Decimal,
    pub expected_value: Option<Decimal>,
}

/// Intentos independientes con probabilidad `q` de cobrar (o pasar) cada uno, hasta `k`:
/// - P(lograrlo en ≤ k) = 1 − (1 − q)^k
/// - intentos esperados = Σ_{i=1..k} (1 − q)^(i−1) = (1 − (1 − q)^k) / q  (k si q = 0)
/// - cuotas esperadas = intentos × (evaluación + activación × P(pasar la evaluación))
/// - valor esperado = P(lograrlo) × (cobro medio + devolución de la cuota) − cuotas esperadas
pub fn economics(t: &Tally, rules: &Rules, max_attempts: u32) -> Economics {
    let n = t.paths.max(1) as f64;
    let q = success(t, rules) as f64 / n;
    let a = t.pass_evaluation as f64 / n;
    let k = i32::try_from(max_attempts).unwrap_or(i32::MAX);
    let miss_all = (1.0 - q).powi(k);
    let success_within = 1.0 - miss_all;
    let attempts = if q > 0.0 {
        success_within / q
    } else {
        f64::from(max_attempts)
    };
    let fees = rules.fees;
    let per_attempt = fees.evaluation + fees.activation * dec(a);
    let expected_fees = (per_attempt * dec(attempts)).round_dp(2);
    let expected_payout = (!t.payouts.is_empty()).then(|| {
        (t.payouts.iter().copied().sum::<Decimal>() / Decimal::from(t.payouts.len())).round_dp(2)
    });
    let expected_refund = if fees.refund_with_first_payout {
        (fees.evaluation * dec(success_within)).round_dp(2)
    } else {
        Decimal::ZERO
    };
    let expected_value = rules.funded.map(|_| {
        (dec(success_within) * expected_payout.unwrap_or(Decimal::ZERO) + expected_refund
            - expected_fees)
            .round_dp(2)
    });
    Economics {
        success_within: dec(success_within),
        expected_attempts: dec(attempts),
        expected_fees,
        expected_payout,
        expected_refund,
        expected_value,
    }
}

/// `{probability, low, high}` con Wilson.
fn prob(hits: usize, n: usize, z: Decimal) -> (f64, Decimal, Decimal) {
    let as_u32 = |x: usize| u32::try_from(x).unwrap_or(u32::MAX);
    let (lo, hi) = wilson(as_u32(hits), as_u32(n.max(1)), z);
    (hits as f64 / n.max(1) as f64, lo, hi)
}

fn prob_json((p, lo, hi): (f64, Decimal, Decimal)) -> Value {
    json!({ "probability": num(Some(p)), "low": text(lo), "high": text(hi) })
}

fn share(x: usize, n: usize) -> Value {
    num(Some(x as f64 / n.max(1) as f64))
}

fn summary_or_null(xs: &[f64]) -> Value {
    summarize(xs.to_vec()).map_or(Value::Null, |s| summary(&s))
}

/// La sección `monte_carlo` de `prop_sim_result.v1`. `block_len_days` es `None` con el modelo.
pub fn monte_carlo_json(
    rules: &Rules,
    source: &DaySource<'_>,
    p: &McParams,
    block_len_days: Option<usize>,
    confidence: Decimal,
) -> Value {
    let z = dec(stats::normal_inv(
        1.0 - (1.0 - confidence.to_f64().unwrap_or(0.95)) / 2.0,
    ));
    let mine = simulate(rules, source, p);
    let zero_src = zero_edge(source);
    let zero = simulate(rules, &zero_src.source(), p);
    let n = mine.paths;
    let funded = rules.funded.is_some();
    let key = |t: &Tally| prob(success(t, rules), t.paths, z);
    let beats = key(&mine).1 > key(&zero).2;
    let names = rules
        .phases
        .iter()
        .map(|ph| ph.name.clone())
        .chain(funded.then(|| "Cuenta fondeada (primer cobro)".to_string()));
    let phases: Vec<Value> = names
        .zip(&mine.stages)
        .map(|(name, s)| {
            json!({
                "name": name, "reached": share(s.reached, n),
                "passed": prob_json(prob(s.passed, n, z)),
                "failed_daily_loss": share(s.failed_daily_loss, n),
                "failed_max_drawdown": share(s.failed_max_drawdown, n),
                "timeout": share(s.timeout, n),
                "days_to_pass": summary_or_null(&s.days_to_pass),
                "days_to_fail": summary_or_null(&s.days_to_fail),
            })
        })
        .collect();
    let eco = economics(&mine, rules, p.max_attempts);
    let zero_eco = economics(&zero, rules, p.max_attempts);
    let payout = |t: &Tally| {
        if funded {
            prob_json(prob(t.payout, t.paths, z))
        } else {
            Value::Null
        }
    };
    json!({
        "paths": n, "seed": p.seed, "block_len_days": block_len_days,
        "horizon_days": p.horizon_days, "confidence": text(confidence),
        "per_attempt": {
            "pass_evaluation": prob_json(prob(mine.pass_evaluation, n, z)),
            "payout": payout(&mine),
            "failed": prob_json(prob(mine.failed, n, z)),
            "timeout": prob_json(prob(mine.timeout, n, z)),
            "unfinished": prob_json(prob(mine.unfinished, n, z)),
        },
        "phases": phases,
        "economics": {
            "max_attempts": p.max_attempts,
            "success_within_attempts": text(eco.success_within),
            "expected_attempts": text(eco.expected_attempts),
            "expected_fees": text(eco.expected_fees),
            "expected_payout": eco.expected_payout.map(text),
            "expected_refund": text(eco.expected_refund),
            "expected_value": eco.expected_value.map(text),
        },
        "zero_edge": {
            "pass_evaluation": prob_json(prob(zero.pass_evaluation, zero.paths, z)),
            "payout": payout(&zero),
            "expected_value": zero_eco.expected_value.map(text),
        },
        "beats_zero_edge": beats,
    })
}

//! `prop_sim_request.v1` → `prop_sim_result.v1`: arma los días de la lista real (o del informe
//! guardado) con el riesgo por operación en dinero, corre el recorrido determinista y junta los
//! avisos. Las operaciones de un informe guardado (`kind = report`) las busca quien llama (el
//! gateway) y las pasa en `report_trades`.

use cfds_backtest::lab::sections::text;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};

use cfds_backtest::research::variance::Payoff;
use rust_decimal::prelude::ToPrimitive;

use crate::montecarlo::{DaySource, McConfig, McParams, monte_carlo_json};
use crate::rules::{PropsimConfig, RuleError, Rules, RulesDoc};
use crate::walk::{DayRef, End, FailRule, StageResult, Walk, group_days, walk};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeIn {
    #[serde(with = "cfds_core::candle::utc_z")]
    pub closed_at: DateTime<Utc>,
    #[serde(with = "cfds_core::decimal_str::signed")]
    pub r: Decimal,
}

/// De dónde salen las operaciones.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceRequest {
    Trades {
        trades: Vec<TradeIn>,
    },
    Report {
        report_id: String,
    },
    Model {
        #[serde(with = "cfds_core::decimal_str")]
        win_rate: Decimal,
        #[serde(with = "cfds_core::decimal_str")]
        avg_win_r: Decimal,
        #[serde(with = "cfds_core::decimal_str")]
        avg_loss_r: Decimal,
        #[serde(default)]
        observed_trades: Option<u32>,
        trades_per_day: u32,
    },
}

/// Lo opcional del Monte Carlo (lo que falta toma `[monte_carlo]` de `config/propsim.toml`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McRequest {
    pub paths: Option<usize>,
    pub seed: Option<u64>,
    pub block_len_days: Option<usize>,
    pub horizon_days: Option<usize>,
    pub max_attempts: Option<u32>,
}

/// `prop_sim_request.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropSimRequest {
    pub rules: RulesDoc,
    #[serde(with = "cfds_core::decimal_str")]
    pub risk_per_trade_pct: Decimal,
    pub source: SourceRequest,
    #[serde(default)]
    pub monte_carlo: Option<McRequest>,
}

impl PropSimRequest {
    pub fn report_id(&self) -> Option<&str> {
        match &self.source {
            SourceRequest::Report { report_id } => Some(report_id),
            _ => None,
        }
    }
}

/// Por qué no se simula (el gateway responde 400).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropSimError {
    /// Reglas en blanco o inválidas, con su ruta dentro de `rules`.
    Rules(Vec<RuleError>),
    Bad(String),
}

impl PropSimError {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Rules(errs) => json!({ "error": "reglas incompletas o inválidas",
                "rules": errs }),
            Self::Bad(m) => json!({ "error": m }),
        }
    }
}

/// Las reglas validadas, el riesgo en dinero y los días de la lista real (vacío con el modelo).
#[derive(Debug, Clone)]
pub struct Prepared {
    pub rules: Rules,
    pub risk_amount: Decimal,
    pub days: Vec<(NaiveDate, Vec<Decimal>)>,
    pub trades: usize,
}

/// Valida el pedido y arma los días. `report_trades`: `(cierre, R neto)` del informe guardado.
pub fn prepare(
    req: &PropSimRequest,
    cfg: &PropsimConfig,
    report_trades: Option<&[(DateTime<Utc>, Decimal)]>,
) -> Result<Prepared, PropSimError> {
    let rules = req.rules.validate().map_err(PropSimError::Rules)?;
    let lim = &cfg.limits;
    if req.risk_per_trade_pct <= Decimal::ZERO || req.risk_per_trade_pct > lim.max_risk_pct {
        return Err(PropSimError::Bad(format!(
            "risk_per_trade_pct tiene que estar en (0, {}]",
            lim.max_risk_pct
        )));
    }
    let risk_amount = rules.account_size * req.risk_per_trade_pct / Decimal::ONE_HUNDRED;
    let list: Vec<(DateTime<Utc>, Decimal)> = match &req.source {
        SourceRequest::Trades { trades } => trades.iter().map(|t| (t.closed_at, t.r)).collect(),
        SourceRequest::Report { .. } => report_trades
            .ok_or_else(|| PropSimError::Bad("faltan las operaciones del informe".into()))?
            .to_vec(),
        SourceRequest::Model { .. } => Vec::new(),
    };
    if !matches!(req.source, SourceRequest::Model { .. })
        && (list.is_empty() || list.len() > lim.max_trades)
    {
        return Err(PropSimError::Bad(format!(
            "la lista tiene que tener entre 1 y {} operaciones",
            lim.max_trades
        )));
    }
    let money: Vec<(DateTime<Utc>, Decimal)> =
        list.iter().map(|(t, r)| (*t, *r * risk_amount)).collect();
    Ok(Prepared {
        days: group_days(&rules, &money),
        trades: list.len(),
        rules,
        risk_amount,
    })
}

/// El recorrido determinista de la lista real.
pub fn observed(p: &Prepared) -> Walk {
    let mut days = p.days.iter().map(|(d, xs)| DayRef {
        date: Some(*d),
        pnl: xs,
    });
    walk(&p.rules, &mut days)
}

fn stage_json(name: &str, s: Option<&StageResult>, funded: bool) -> Value {
    let Some(s) = s else {
        return json!({ "name": name, "status": "not_reached", "rule": null, "trading_days": 0,
            "end_day": null, "profit": "0", "best_day": null, "min_balance": "0",
            "payout": null });
    };
    let (status, rule) = match s.end {
        End::Passed if funded => ("payout", None),
        End::Passed => ("passed", None),
        End::Failed(FailRule::DailyLoss) => ("failed", Some("daily_loss")),
        End::Failed(FailRule::MaxDrawdown) => ("failed", Some("max_drawdown")),
        End::Timeout => ("timeout", None),
        End::Unfinished => ("unfinished", None),
    };
    json!({
        "name": name, "status": status, "rule": rule, "trading_days": s.trading_days,
        "end_day": s.end_day.map(|d| d.format("%Y-%m-%d").to_string()),
        "profit": text(s.profit), "best_day": s.best_day.map(text),
        "min_balance": text(s.min_balance), "payout": s.payout.map(text),
    })
}

/// `observed` de `prop_sim_result.v1`.
pub fn observed_json(rules: &Rules, w: &Walk) -> Value {
    let phases: Vec<Value> = rules
        .phases
        .iter()
        .enumerate()
        .map(|(k, p)| stage_json(&p.name, w.phases.get(k), false))
        .collect();
    let funded = rules
        .funded
        .as_ref()
        .map(|_| stage_json("Cuenta fondeada (primer cobro)", w.funded.as_ref(), true));
    let status = if w.paid() {
        "payout"
    } else if w.passed_evaluation(rules) && rules.funded.is_none() {
        "passed"
    } else {
        match w.last().map(|s| s.end) {
            Some(End::Failed(_)) => "failed",
            Some(End::Timeout) => "timeout",
            _ => "unfinished",
        }
    };
    json!({ "status": status, "phases": phases, "funded": funded })
}

/// Avisos fijos del resultado.
pub fn notes(rules: &Rules, model: bool) -> Vec<String> {
    let mut n = vec![
        "Simulación educativa con operaciones pasadas o sorteadas: no predice resultados ni es \
         consejo de inversión."
            .to_string(),
        "Las reglas se miran con el saldo al cierre de cada operación: una pérdida flotante \
         dentro de una operación no se ve. Si la firma mide el capital flotante, el resultado \
         real puede ser peor."
            .to_string(),
        "Las firmas cambian sus reglas, sus cuotas y sus pagos: lo que vale es su reglamento \
         vigente. Pagar una evaluación real es dinero real y queda fuera de la app."
            .to_string(),
    ];
    if !rules.unsimulable().is_empty() {
        n.push(
            "La caída que sigue al máximo intradía necesita el capital vela por vela y no se \
             simuló (no se aproxima): pasar es, como mucho, lo que se muestra."
                .to_string(),
        );
    }
    if model {
        n.push(
            "Con el modelo no hay lista real: no hay recorrido determinista, solo caminos \
             sorteados."
                .to_string(),
        );
    }
    n
}

/// El resultado sin Monte Carlo: reglas, riesgo, fuente, recorrido determinista y avisos.
pub fn result_json(req: &PropSimRequest, p: &Prepared, monte_carlo: Value) -> Value {
    let model = matches!(req.source, SourceRequest::Model { .. });
    let unsimulable = p.rules.unsimulable();
    let source = match &req.source {
        SourceRequest::Model { trades_per_day, .. } => json!({ "kind": "model", "trades": null,
            "days": null, "report_id": null, "trades_per_day": trades_per_day }),
        s => json!({
            "kind": match s { SourceRequest::Report { .. } => "report", _ => "trades" },
            "trades": p.trades, "days": p.days.len(), "report_id": req.report_id(),
            "trades_per_day": null,
        }),
    };
    json!({
        "rules": serde_json::to_value(&req.rules).unwrap_or(Value::Null),
        "risk_per_trade_pct": text(req.risk_per_trade_pct),
        "risk_per_trade_amount": text(p.risk_amount),
        "source": source,
        "upper_bound": !unsimulable.is_empty(),
        "unsimulable": unsimulable,
        "observed": if model { Value::Null } else { observed_json(&p.rules, &observed(p)) },
        "monte_carlo": monte_carlo,
        "notes": notes(&p.rules, model),
    })
}

/// Los parámetros del Monte Carlo (pedido + `[monte_carlo]`), dentro de los topes.
pub fn mc_params(req: &PropSimRequest, cfg: &McConfig) -> Result<McParams, PropSimError> {
    let m = req.monte_carlo.unwrap_or_default();
    let p = McParams {
        paths: m.paths.unwrap_or(cfg.default_paths),
        seed: m.seed.unwrap_or(cfg.default_seed),
        block_len_days: m.block_len_days.unwrap_or(cfg.default_block_len_days),
        horizon_days: m.horizon_days.unwrap_or(cfg.default_horizon_days),
        max_attempts: m.max_attempts.unwrap_or(cfg.default_max_attempts),
    };
    let bad = |what: &str, max: usize| {
        Err(PropSimError::Bad(format!(
            "monte_carlo.{what} tiene que estar en [1, {max}]"
        )))
    };
    if p.paths == 0 || p.paths > cfg.max_paths {
        return bad("paths", cfg.max_paths);
    }
    if p.horizon_days == 0 || p.horizon_days > cfg.max_horizon_days {
        return bad("horizon_days", cfg.max_horizon_days);
    }
    if p.max_attempts == 0 || p.max_attempts > cfg.max_attempts {
        return bad("max_attempts", cfg.max_attempts as usize);
    }
    if p.block_len_days == 0 {
        return bad("block_len_days", usize::MAX);
    }
    if p.paths.saturating_mul(p.horizon_days) > cfg.max_day_steps {
        return Err(PropSimError::Bad(format!(
            "monte_carlo: paths × horizon_days tiene que ser ≤ {}",
            cfg.max_day_steps
        )));
    }
    Ok(p)
}

fn ratio(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(f64::NAN)
}

/// Corre el pedido completo: recorrido determinista (con una lista real) y Monte Carlo.
pub fn run(
    req: &PropSimRequest,
    cfg: &PropsimConfig,
    report_trades: Option<&[(DateTime<Utc>, Decimal)]>,
) -> Result<Value, PropSimError> {
    let p = prepare(req, cfg, report_trades)?;
    let mut mc = mc_params(req, &cfg.monte_carlo)?;
    let days: Vec<Vec<Decimal>> = p.days.iter().map(|(_, xs)| xs.clone()).collect();
    let (source, block) = match &req.source {
        SourceRequest::Model {
            win_rate,
            avg_win_r,
            avg_loss_r,
            observed_trades,
            trades_per_day,
        } => {
            let hundred = Decimal::ONE_HUNDRED;
            if *win_rate > Decimal::ONE
                || avg_win_r.is_zero()
                || avg_loss_r.is_zero()
                || *avg_win_r > hundred
                || *avg_loss_r > hundred
            {
                return Err(PropSimError::Bad(
                    "win_rate en [0, 1] y avg_win_r y avg_loss_r en (0, 100]".into(),
                ));
            }
            if *trades_per_day == 0 || *trades_per_day > cfg.monte_carlo.max_trades_per_day {
                return Err(PropSimError::Bad(format!(
                    "trades_per_day tiene que estar en [1, {}]",
                    cfg.monte_carlo.max_trades_per_day
                )));
            }
            let payoff = Payoff {
                win_rate: ratio(*win_rate),
                avg_win_r: ratio(*avg_win_r),
                avg_loss_r: ratio(*avg_loss_r),
                observed_trades: *observed_trades,
            };
            let source = DaySource::Model {
                payoff,
                trades_per_day: *trades_per_day,
                win: *avg_win_r * p.risk_amount,
                loss: *avg_loss_r * p.risk_amount,
            };
            (source, None)
        }
        _ => {
            // Con pocos días, el bloque no puede ser más largo que la serie.
            mc.block_len_days = mc.block_len_days.min(days.len()).max(1);
            (DaySource::Days(&days), Some(mc.block_len_days))
        }
    };
    let mc_json = monte_carlo_json(&p.rules, &source, &mc, block, cfg.monte_carlo.confidence);
    Ok(result_json(req, &p, mc_json))
}

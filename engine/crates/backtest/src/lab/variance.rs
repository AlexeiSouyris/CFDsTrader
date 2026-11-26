//! Contrato del simulador de varianza (#801, `docs/design/2026-11-laboratorio.md` §4.2):
//! `variance_sim_request.v1` → [`research::variance`](crate::research::variance) →
//! `variance_sim_report.v1`, con los topes y valores por defecto de `[variance]` de
//! `config/lab.toml`.
//!
//! Función pura: los R de un informe guardado (`kind = report`) los busca quien llama (el
//! gateway) y los pasa en `report_r`. La UI no calcula nada de esto.
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de caminos simulados (research::variance): no son dinero"
)]

use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde::Deserialize;
use serde_json::{Value, json};

use super::sections::{num, summary, text};
use crate::research::stats;
use crate::research::variance::{Payoff, RSource, VarianceParams, VarianceReport, simulate};
use cfds_indicators::wilson;

/// `[variance]` de `config/lab.toml` (PENDIENTE del dueño: valores recomendados del diseño).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VarianceConfig {
    pub max_paths: usize,
    pub max_trades: usize,
    pub max_r_values: usize,
    pub max_risk_pct: Decimal,
    pub default_paths: usize,
    pub default_seed: u64,
    pub default_ruin_threshold_pct: Decimal,
    pub default_block_len: usize,
    pub streak_max_k: usize,
    pub drawdown_bin: Decimal,
}

impl VarianceConfig {
    pub fn validate(&self) -> Result<(), String> {
        let hundred = Decimal::ONE_HUNDRED;
        let ok = self.max_paths > 0
            && self.max_trades > 0
            && self.max_r_values > 0
            && self.max_risk_pct > Decimal::ZERO
            && self.max_risk_pct <= hundred
            && (1..=self.max_paths).contains(&self.default_paths)
            && self.default_ruin_threshold_pct > Decimal::ZERO
            && self.default_ruin_threshold_pct <= hundred
            && self.default_block_len > 0
            && self.streak_max_k > 0
            && self.drawdown_bin > Decimal::ZERO
            && self.drawdown_bin <= Decimal::ONE;
        if ok {
            Ok(())
        } else {
            Err("lab.toml: [variance] tiene un tope o un valor por defecto inválido".into())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct SignedDecimal(#[serde(with = "cfds_core::decimal_str::signed")] pub Decimal);

/// La fuente pedida (`variance_sim_request.v1#/properties/source`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceRequest {
    Model {
        #[serde(with = "cfds_core::decimal_str")]
        win_rate: Decimal,
        #[serde(with = "cfds_core::decimal_str")]
        avg_win_r: Decimal,
        #[serde(with = "cfds_core::decimal_str")]
        avg_loss_r: Decimal,
        #[serde(default)]
        observed_trades: Option<u32>,
    },
    RValues {
        r: Vec<SignedDecimal>,
        #[serde(default)]
        block_len: Option<usize>,
    },
    Report {
        report_id: String,
        #[serde(default)]
        block_len: Option<usize>,
    },
}

/// `variance_sim_request.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VarianceRequest {
    pub source: SourceRequest,
    #[serde(with = "cfds_core::decimal_str")]
    pub risk_pct: Decimal,
    pub trades: usize,
    #[serde(default)]
    pub paths: Option<usize>,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub ruin_threshold_pct: Option<Decimal>,
}

impl VarianceRequest {
    /// El informe guardado del que hay que traer los R, si la fuente es `report`.
    pub fn report_id(&self) -> Option<&str> {
        match &self.source {
            SourceRequest::Report { report_id, .. } => Some(report_id),
            _ => None,
        }
    }
}

/// Por qué no se corre el pedido (el gateway responde 400 con el mensaje).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarianceError(pub String);

fn bad<T>(msg: impl Into<String>) -> Result<T, VarianceError> {
    Err(VarianceError(msg.into()))
}

fn ratio(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(f64::NAN)
}

/// Corre el pedido. `report_r`: los R netos del informe cuando la fuente es `report` (si falta,
/// error). `project_risk_pct`: el riesgo por operación de `config/risk.toml`, solo para avisar
/// cuando el pedido lo pasa. `confidence`: nivel del intervalo de Wilson (`[report]`).
pub fn run_variance(
    req: &VarianceRequest,
    cfg: &VarianceConfig,
    report_r: Option<&[Decimal]>,
    project_risk_pct: Decimal,
    confidence: Decimal,
) -> Result<Value, VarianceError> {
    let paths = req.paths.unwrap_or(cfg.default_paths);
    let seed = req.seed.unwrap_or(cfg.default_seed);
    let ruin_pct = req
        .ruin_threshold_pct
        .unwrap_or(cfg.default_ruin_threshold_pct);
    if req.risk_pct <= Decimal::ZERO || req.risk_pct > cfg.max_risk_pct {
        return bad(format!(
            "risk_pct tiene que estar en (0, {}]",
            cfg.max_risk_pct
        ));
    }
    if req.trades == 0 || req.trades > cfg.max_trades {
        return bad(format!("trades tiene que estar en [1, {}]", cfg.max_trades));
    }
    if paths == 0 || paths > cfg.max_paths {
        return bad(format!("paths tiene que estar en [1, {}]", cfg.max_paths));
    }
    if ruin_pct <= Decimal::ZERO || ruin_pct > Decimal::ONE_HUNDRED {
        return bad("ruin_threshold_pct tiene que estar en (0, 100]");
    }
    let r_list =
        |r: Vec<Decimal>, block: Option<usize>| -> Result<(Vec<f64>, usize), VarianceError> {
            if r.is_empty() || r.len() > cfg.max_r_values {
                return bad(format!(
                    "la lista de R tiene que tener entre 1 y {} valores",
                    cfg.max_r_values
                ));
            }
            let block = block.unwrap_or(cfg.default_block_len);
            if block == 0 || block > r.len() {
                return bad("block_len tiene que estar entre 1 y la cantidad de R");
            }
            Ok((r.into_iter().map(ratio).collect(), block))
        };
    let (rs, block, echo_source) = match &req.source {
        SourceRequest::Model {
            win_rate,
            avg_win_r,
            avg_loss_r,
            observed_trades,
        } => {
            if *win_rate > Decimal::ONE
                || avg_win_r.is_zero()
                || avg_loss_r.is_zero()
                || *avg_win_r > Decimal::ONE_HUNDRED
                || *avg_loss_r > Decimal::ONE_HUNDRED
            {
                return bad("win_rate en [0, 1] y avg_win_r y avg_loss_r en (0, 100]");
            }
            let echo = json!({ "kind": "model", "win_rate": text(*win_rate),
                "avg_win_r": text(*avg_win_r), "avg_loss_r": text(*avg_loss_r),
                "observed_trades": observed_trades });
            (Vec::new(), 0, echo)
        }
        SourceRequest::RValues { r, block_len } => {
            let (rs, block) = r_list(r.iter().map(|x| x.0).collect(), *block_len)?;
            let echo = json!({ "kind": "r_values", "count": rs.len(), "block_len": block,
                "report_id": null });
            (rs, block, echo)
        }
        SourceRequest::Report {
            report_id,
            block_len,
        } => {
            let Some(r) = report_r else {
                return bad("faltan los R del informe");
            };
            let (rs, block) = r_list(r.to_vec(), *block_len)?;
            let echo = json!({ "kind": "report", "count": rs.len(), "block_len": block,
                "report_id": report_id });
            (rs, block, echo)
        }
    };
    let payoff = match &req.source {
        SourceRequest::Model {
            win_rate,
            avg_win_r,
            avg_loss_r,
            observed_trades,
        } => Some(Payoff {
            win_rate: ratio(*win_rate),
            avg_win_r: ratio(*avg_win_r),
            avg_loss_r: ratio(*avg_loss_r),
            observed_trades: *observed_trades,
        }),
        _ => None,
    };
    let source = match payoff {
        Some(m) => RSource::Model(m),
        None => RSource::Resample {
            r: &rs,
            block_len: block,
        },
    };
    let params = VarianceParams {
        risk_fraction: ratio(req.risk_pct / Decimal::ONE_HUNDRED),
        trades: req.trades,
        paths,
        seed,
        ruin_threshold: ratio(ruin_pct / Decimal::ONE_HUNDRED),
        streak_max_k: cfg.streak_max_k,
        drawdown_bin: ratio(cfg.drawdown_bin),
    };
    let Some(rep) = simulate(&source, &params) else {
        return bad("el pedido no se puede simular (valores no finitos o fuera de rango)");
    };
    let fixed = match payoff {
        Some(m) if m.observed_trades.is_some() => simulate(&RSource::Model(m.fixed()), &params),
        _ => None,
    };
    let z = stats::normal_inv(1.0 - (1.0 - ratio(confidence)) / 2.0);
    let z = Decimal::from_f64(z).unwrap_or(Decimal::TWO);
    let as_u32 = |x: usize| u32::try_from(x).unwrap_or(u32::MAX);
    let (lo, hi) = wilson(as_u32(rep.ruined), as_u32(rep.paths), z);
    let mut notes = vec![
        "Simulación educativa: caminos posibles con estos supuestos, no una predicción. No es \
         consejo de inversión."
            .to_string(),
        "El riesgo es un % fijo del capital del momento (crece y se achica con él); el capital \
         nunca baja de 0."
            .to_string(),
        "El intervalo del riesgo de ruina mide el error de la simulación, no el de los supuestos."
            .to_string(),
    ];
    if payoff.is_some_and(|m| m.observed_trades.is_none()) {
        notes.push(
            "El win rate se toma como cierto. Si salió de pocas operaciones, prendé la \
             incertidumbre: el de verdad puede ser bastante menor."
                .to_string(),
        );
    }
    if payoff.is_none() {
        notes.push(
            "Los R reales se remuestrean: se supone que el futuro se parece a esa muestra, \
             costos incluidos."
                .to_string(),
        );
    }
    if req.risk_pct > project_risk_pct {
        notes.push(format!(
            "El riesgo pedido ({} %) es mayor que el riesgo por operación del proyecto \
             (config/risk.toml, {} %): sirve para ver qué pasa, no para operar así.",
            req.risk_pct.normalize(),
            project_risk_pct.normalize()
        ));
    }
    Ok(report_json(
        &rep,
        echo_source,
        req,
        (paths, seed, ruin_pct),
        (lo, hi),
        fixed.as_ref(),
        notes,
    ))
}

fn report_json(
    rep: &VarianceReport,
    source: Value,
    req: &VarianceRequest,
    (paths, seed, ruin_pct): (usize, u64, Decimal),
    (lo, hi): (Decimal, Decimal),
    fixed: Option<&VarianceReport>,
    notes: Vec<String>,
) -> Value {
    let last = rep.bands.last().copied();
    let band = |b: &crate::research::variance::Band| {
        json!({ "trade": b.trade, "p5": num(Some(b.p5)), "p25": num(Some(b.p25)),
            "p50": num(Some(b.p50)), "p75": num(Some(b.p75)), "p95": num(Some(b.p95)) })
    };
    let fixed = fixed.map_or(Value::Null, |f| {
        json!({
            "risk_of_ruin": num(Some(f.risk_of_ruin())),
            "final_equity_p5": num(f.bands.last().map(|b| b.p5)),
            "final_equity_p50": num(f.bands.last().map(|b| b.p50)),
            "max_drawdown_p50": num(Some(f.max_drawdown.p50)),
        })
    });
    json!({
        "source": source,
        "risk_pct": text(req.risk_pct),
        "trades": req.trades,
        "paths": paths,
        "seed": seed,
        "ruin_threshold_pct": text(ruin_pct),
        "expectancy_r": num(Some(rep.expectancy_r)),
        "bands": rep.bands.iter().map(band).collect::<Vec<_>>(),
        "final_equity": {
            "mean": num(Some(rep.final_mean)),
            "p5": num(last.map(|b| b.p5)), "p25": num(last.map(|b| b.p25)),
            "p50": num(last.map(|b| b.p50)), "p75": num(last.map(|b| b.p75)),
            "p95": num(last.map(|b| b.p95)),
        },
        "max_drawdown": {
            "summary": summary(&rep.max_drawdown),
            "histogram": rep.drawdown_histogram.iter().map(|b| json!({
                "from": num(Some(b.from)), "to": num(Some(b.to)), "share": num(Some(b.share)),
            })).collect::<Vec<_>>(),
        },
        "longest_losing_streak": summary(&rep.longest_losing_streak),
        "losing_streak_at_least": rep.streak_at_least.iter().map(|(k, p)| json!({
            "k": k, "probability": num(Some(*p)),
        })).collect::<Vec<_>>(),
        "risk_of_ruin": {
            "probability": num(Some(rep.risk_of_ruin())), "low": text(lo), "high": text(hi),
        },
        "win_rate_draws": rep.win_rate_draws.as_ref().map_or(Value::Null, summary),
        "fixed_win_rate": fixed,
        "notes": notes,
    })
}

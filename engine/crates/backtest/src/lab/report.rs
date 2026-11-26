//! `lab_report.v1`: el informe de una corrida del laboratorio, armado **solo** con lo que ya
//! existe (`run_plans`, `trade_return`, `monthly_returns`, las secciones de [`super::sections`],
//! el test de suerte y comprar y mantener). Función pura: los datos, la hora y la versión del
//! código los da quien llama.
//!
//! ```text
//! retornos por operación sobre el capital (trade_return, costos × multiplicador) → meses
//!   (monthly_returns) → curva, Sharpe, Sortino, Sharpe deflactado (N = N del proyecto +
//!   variantes de la familia), mitades
//! las mismas operaciones con costos × multiplicador × 1,5 → costos × 1,5
//! Monte Carlo por bloques de los retornos por operación; PBO de las variantes corridas juntas
//! ```
//! En exploratorio nada vota: `votes = false`, "exploratorio, no vota".
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de una muestra de retornos, como research::stats: no son dinero"
)]

use std::collections::BTreeMap;

use cfds_market_data::Period;
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, Timelike, Utc};
use rust_decimal::Decimal;
use serde_json::{Value, json};

use super::sections::{
    ReportConfig, curve, expectancy_block, f, histogram, monte_carlo_block, month, notes, num,
    pbo_section, quantiles, r_of, sharpe_sortino, slices, text, trades_block, utc,
};
use super::spec::{Mode, canonical_json, sha256_hex};
use super::validation::ValidSpec;
use crate::research::buy_and_hold::MonthReturn;
use crate::research::{
    LuckTest, MonteCarloConfig, StudyRun, StudyTrade, monthly_returns, stats, trade_return,
};
use crate::{RunError, Setup};

/// El recorrido de la corrida: el de config con el riesgo por operación de la spec (nunca más
/// que el de `config/risk.toml`: lo valida `validate_spec`, y acá se toma el menor igual).
pub fn run_setup(base: &Setup, spec: &ValidSpec) -> Setup {
    let mut s = base.clone();
    s.risk.risk_per_trade_pct = s.risk.risk_per_trade_pct.min(spec.spec.risk_pct);
    s
}

/// Lo que el informe no calcula: viene de la corrida y de la config.
pub struct ReportInputs<'a> {
    pub spec: &'a ValidSpec,
    pub run: &'a StudyRun,
    pub setup: &'a Setup,
    /// Instrumento con el que se guardan las velas (con futuros, el derivado).
    pub instrument_id: &'a str,
    /// Meses UTC del período, en orden.
    pub months: &'a [(i32, u32)],
    /// Ventanas de validación del walk-forward dentro del período.
    pub windows: &'a [Period],
    pub buy_and_hold: Option<&'a [MonthReturn]>,
    pub luck: Option<&'a LuckTest>,
    /// Retornos mensuales de cada variante de la familia corrida junta (para el PBO).
    pub variants: &'a [Vec<f64>],
    /// N del proyecto (`dsr_trials` vigente) y variantes distintas de la familia.
    pub project_trials: u32,
    pub family_variants: u32,
    pub montecarlo: &'a MonteCarloConfig,
    pub pbo_partitions: usize,
    /// `[rule] cost_stress` (1,5) y `[alpha_budget] edge_cost_factor` (k).
    pub cost_stress: Decimal,
    pub edge_cost_factor: Decimal,
    pub cfg: &'a ReportConfig,
    pub holdout_start: DateTime<Utc>,
    pub config_sha256: &'a str,
    pub code_version: Option<&'a str>,
    pub last_candle: DateTime<Utc>,
    pub computed_at: DateTime<Utc>,
}

/// Curva mensual: retorno, retorno con estrés, capital, caída y comprar y mantener.
/// Curva mensual: retorno, retorno con estrés, capital, caída y comprar y mantener.
fn month_rows(i: &ReportInputs, monthly: &[f64], stressed: &[f64]) -> Vec<Value> {
    let hold: BTreeMap<(i32, u32), Decimal> =
        i.buy_and_hold.unwrap_or(&[]).iter().copied().collect();
    let (points, _, _) = curve(monthly);
    let closed_in = |y: i32, m: u32| {
        let t = &i.run.trades;
        t.iter()
            .filter(|t| (t.exit_time.year(), t.exit_time.month()) == (y, m))
            .count()
    };
    i.months
        .iter()
        .enumerate()
        .map(|(k, &(y, m))| {
            let hold = hold.get(&(y, m)).map_or(Value::Null, |r| text(*r));
            json!({
                "month": month(y, m), "trades": closed_in(y, m),
                "return": num(Some(monthly[k])), "return_stressed": num(Some(stressed[k])),
                "equity": num(Some(points[k].0)), "drawdown": num(Some(points[k].1)),
                "buy_and_hold": hold,
            })
        })
        .collect()
}

/// El informe (ver el módulo). Error solo si falta un tipo de cambio para pasar un P/L a la
/// moneda de la cuenta.
pub fn lab_report(i: &ReportInputs) -> Result<Value, RunError> {
    let (spec, trades) = (i.spec, &i.run.trades);
    let m = spec.spec.cost_multiplier;
    let stress = m * i.cost_stress;
    let net_r = |s: Decimal| -> Vec<f64> {
        let r = |t: &StudyTrade| r_of(t, t.mid_pnl - s * t.total_costs());
        trades.iter().map(r).collect()
    };
    let (net, net_stressed) = (net_r(m), net_r(stress));
    let gross: Vec<f64> = trades.iter().map(|t| r_of(t, t.mid_pnl)).collect();
    let per_trade = |s: Decimal| -> Result<Vec<(DateTime<Utc>, Decimal)>, RunError> {
        let r = |t: &StudyTrade| Ok((t.exit_time, trade_return(t, s, i.setup)?));
        trades.iter().map(r).collect()
    };
    let (closed, closed_stressed) = (per_trade(m)?, per_trade(stress)?);
    let to_f = |xs: Vec<Decimal>| -> Vec<f64> { xs.into_iter().map(f).collect() };
    let monthly = to_f(monthly_returns(&closed, i.months));
    let monthly_s = to_f(monthly_returns(&closed_stressed, i.months));
    let mut notes = notes(trades.len());
    let trials = i.project_trials + i.family_variants;
    let (sharpe, sortino) = sharpe_sortino(&monthly);
    let deflated = stats::deflated_sharpe(&monthly, trials).map(|d| d.dsr);
    let returns: Vec<f64> = closed.iter().map(|(_, r)| f(*r)).collect();
    let windows: Vec<Value> = i
        .windows
        .iter()
        .map(|w| {
            let inside = |(at, _): &&(DateTime<Utc>, Decimal)| *at >= w.start && *at < w.end;
            let rs: Vec<f64> = closed.iter().filter(inside).map(|(_, r)| f(*r)).collect();
            let ret = num(Some(curve(&rs).2));
            json!({ "from": utc(w.start), "to": utc(w.end), "trades": rs.len(), "return": ret })
        })
        .collect();
    let half = i.months.len() / 2;
    let halves = (half > 0).then(|| {
        let (y, mo) = i.months[half];
        let (first, second) = (stats::mean(&monthly[..half]), stats::mean(&monthly[half..]));
        json!({ "split_month": month(y, mo), "first": num(first), "second": num(second) })
    });
    let buy_and_hold = i.buy_and_hold.filter(|h| !h.is_empty()).map(|h| {
        let xs: Vec<f64> = h.iter().map(|(_, r)| f(*r)).collect();
        let (_, worst, total) = curve(&xs);
        json!({
            "monthly_mean": num(stats::mean(&xs)), "sharpe": num(sharpe_sortino(&xs).0),
            "max_drawdown": num(Some(worst)), "total_return": num(Some(total)),
        })
    });
    let luck = i.luck.filter(|_| !trades.is_empty()).map(|l| {
        let (p_mean, p_total) = l.percentiles();
        json!({
            "simulations": l.simulations.len(), "strategy_mean_r": text(l.strategy.0),
            "percentile_mean_r": text(p_mean), "percentile_total": text(p_total),
        })
    });
    let exc: Vec<(f64, f64, bool)> = trades
        .iter()
        .zip(&net)
        .filter_map(|(t, r)| t.excursions_r().map(|(a, b)| (f(a), f(b), *r < 0.0)))
        .collect();
    let cost = stats::mean(
        &trades
            .iter()
            .map(|t| r_of(t, m * t.total_costs()))
            .collect::<Vec<_>>(),
    );
    let k = i.edge_cost_factor;
    let by = |key: &dyn Fn(&StudyTrade) -> String| {
        slices(trades.iter().zip(&net).map(|(t, r)| (key(t), *r)))
    };
    let side = |t: &StudyTrade| {
        if t.side == Side::Long {
            "long"
        } else {
            "short"
        }
        .to_owned()
    };
    let id_doc = json!({
        "spec_sha256": spec.spec_sha256, "config_sha256": i.config_sha256,
        "code_version": i.code_version, "last_candle": utc(i.last_candle),
        "seed": i.montecarlo.seed,
    });
    let exploratory = spec.spec.mode == Mode::Exploratory;
    let pbo = pbo_section(i.variants, i.pbo_partitions, &mut notes);
    Ok(json!({
        "report_id": sha256_hex(&canonical_json(&id_doc)),
        "spec_sha256": spec.spec_sha256, "family_sha256": spec.family_sha256,
        "spec": serde_json::to_value(&spec.spec).unwrap_or(Value::Null),
        "mode": if exploratory { "exploratory" } else { "confirmatory" },
        "votes": false,
        "label": if exploratory { "exploratorio, no vota" } else { "confirmatorio, sin flujo" },
        "instrument_id": i.instrument_id, "candle_source": "futures",
        "period": { "from": utc(spec.spec.period.from), "to": utc(spec.spec.period.to) },
        "holdout_start": utc(i.holdout_start),
        "reproducibility": {
            "config_sha256": i.config_sha256, "code_version": i.code_version,
            "last_candle": utc(i.last_candle), "seed": i.montecarlo.seed,
        },
        "computed_at": utc(i.computed_at),
        "initial_equity": text(i.setup.initial_equity), "cost_multiplier": text(m),
        "trades": trades_block(i.run),
        "expectancy": expectancy_block(&net, &gross, i.cfg.confidence),
        "r_distribution": {
            "step": text(i.cfg.r_step), "bins": histogram(&net, &gross, i.cfg.r_step),
        },
        "months": month_rows(i, &monthly, &monthly_s),
        "sharpe": {
            "monthly_mean": num(stats::mean(&monthly)), "sharpe": num(sharpe),
            "sortino": num(sortino), "deflated": num(deflated),
            "project_trials": i.project_trials, "family_variants": i.family_variants,
            "trials": trials,
        },
        "pbo": pbo,
        "monte_carlo": monte_carlo_block(&returns, i.montecarlo),
        "walk_forward": windows,
        "halves": halves,
        "cost_stress": {
            "stress": text(stress), "mean_r_net": num(stats::mean(&net_stressed)),
            "monthly_mean": num(stats::mean(&monthly_s)),
            "sharpe": num(sharpe_sortino(&monthly_s).0),
        },
        "luck": luck,
        "buy_and_hold": buy_and_hold,
        "excursions": {
            "mae_r": quantiles(exc.iter().map(|e| e.0).collect()),
            "mfe_r": quantiles(exc.iter().map(|e| e.1).collect()),
            "mfe_r_losers": quantiles(exc.iter().filter(|e| e.2).map(|e| e.1).collect()),
        },
        "edge": {
            "cost_per_trade_r": num(cost), "edge_cost_factor": text(k),
            "required_gross_r": num(cost.map(|c| f(k) * c)),
            "measured_gross_r": num(stats::mean(&gross)),
        },
        "slices": {
            "by_year": by(&|t| t.exit_time.year().to_string()),
            "by_hour_utc": by(&|t| format!("{:02}", t.entry_time.hour())),
            "by_side": by(&side),
        },
        "notes": notes,
    }))
}

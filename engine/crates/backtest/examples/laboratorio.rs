//! Laboratorio de estrategias (#730, fase L6 de `docs/design/2026-11-laboratorio.md`): corre una
//! spec (`strategy_spec.v1`) —o varias variantes de una misma familia— desde archivos JSON, con
//! las velas de futuros ya guardadas, y arma el informe honesto (`lab_report.v1`). Exploratorio:
//! **no vota**, no escribe nada en la base (solo lectura) y nunca llama a IG.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example laboratorio -- \
//!     spec.json [variante.json ...] --escenario-financiacion ESTR --ensayos <ensayos.jsonl> \
//!     [--supuestos <archivo.toml>] [--capital 100000] [--desde 2011-01-01T00:00:00Z] \
//!     [--salida <carpeta>] [--validar]
//! ```
//! - Valida cada spec contra el catálogo del motor (`spec_validation.v1`); con `--validar` solo
//!   valida y no carga velas.
//! - Las velas se piden **solo hasta el inicio del fuera de muestra** (como las rondas) y un
//!   período que lo toque se rechaza (guardia `research_period`).
//! - Las variantes de una corrida tienen que ser de la misma familia y el mismo período (PBO).
//! - Cada spec corrida agrega una fila `lab_trial.v1` a `--ensayos` (solo agregar). Las variantes
//!   distintas de la familia suman al N del Sharpe deflactado del laboratorio, nunca al N del
//!   proyecto (§9 punto 1 del diseño, PENDIENTE).
//! - Con `--salida`, cada informe va a `<carpeta>/<report_id>.json`.
#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use std::io::Write as _;

use cfds_backtest::lab::{
    LabConfig, LabSeries, LabTrial, ReportInputs, StrategySpec, ValidSpec, catalog,
    family_variants, lab_report, planned_exit_for, plans_from_spec, read_trials, run_setup,
    validate_spec_json,
};
use cfds_backtest::research::luck::{LuckConfig, LuckSeries, luck_test};
use cfds_backtest::research::{
    ResearchConfig, buy_and_hold_returns, monthly_returns, run_plans, trade_return,
};
use cfds_core::Timeframe;
use cfds_market_data::{CandleSources, Period, SplitConfig, plan};
use cfds_risk::RiskConfig;
use cfds_storage::{CandleFeed, PgStore};
use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// Lo que lee el laboratorio, para el hash de la config del informe.
const CONFIG_FILES: [(&str, &str); 7] = [
    ("backtest.toml", cfg!("backtest.toml")),
    ("candle_sources.toml", cfg!("candle_sources.toml")),
    ("data_split.toml", cfg!("data_split.toml")),
    ("instruments.toml", cfg!("instruments.toml")),
    ("lab.toml", cfg!("lab.toml")),
    ("research.toml", cfg!("research.toml")),
    ("risk.toml", cfg!("risk.toml")),
];

fn take(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
}

/// Meses UTC que empiezan en `[from, to)`.
fn months(from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<(i32, u32)> {
    let mut out = Vec::new();
    let mut d = NaiveDate::from_ymd_opt(from.year(), from.month(), 1);
    while let Some(m) = d.filter(|m| m.and_hms_opt(0, 0, 0).is_some_and(|t| t.and_utc() < to)) {
        out.push((m.year(), m.month()));
        d = m.checked_add_months(Months::new(1));
    }
    out
}

/// N del proyecto: el último `dsr_trials` de `config/research.toml` (nunca baja, así que es el
/// máximo; lo verifica `tools/research/check_dsr_trials.py`).
fn project_trials(r: &ResearchConfig) -> u32 {
    [
        &r.rule,
        &r.rule_round2,
        &r.rule_round3,
        &r.rule_nasdaq,
        &r.rule_program4,
        &r.rule_program5,
    ]
    .iter()
    .map(|x| x.dsr_trials)
    .max()
    .unwrap_or(0)
}

fn code_version() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    let v = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (out.status.success() && v.len() >= 7).then_some(v)
}

fn summary(r: &Value) {
    let s = |p: &str| {
        r.pointer(p)
            .map_or("-".into(), |v| v.to_string().replace('"', ""))
    };
    println!(
        "── informe {} (spec {})",
        &s("/report_id")[..12],
        &s("/spec_sha256")[..12]
    );
    println!("   {} — {} operaciones", s("/label"), s("/trades/count"));
    println!(
        "   R neto medio {} [{}, {}]; R bruto {}; aciertos {} [{}, {}]; profit factor {}",
        s("/expectancy/mean_r_net"),
        s("/expectancy/mean_r_net_ci/low"),
        s("/expectancy/mean_r_net_ci/high"),
        s("/expectancy/mean_r_gross"),
        s("/expectancy/win_rate"),
        s("/expectancy/win_rate_ci/low"),
        s("/expectancy/win_rate_ci/high"),
        s("/expectancy/profit_factor")
    );
    println!(
        "   Sharpe mensual {}; deflactado {} (N = {}: proyecto {} + familia {}); PBO {}",
        s("/sharpe/sharpe"),
        s("/sharpe/deflated"),
        s("/sharpe/trials"),
        s("/sharpe/project_trials"),
        s("/sharpe/family_variants"),
        s("/pbo/pbo")
    );
    println!(
        "   costos × {}: R neto medio {}; Monte Carlo: caída p95 {}, ruina {}; suerte: percentil {} %",
        s("/cost_stress/stress"),
        s("/cost_stress/mean_r_net"),
        s("/monte_carlo/max_drawdown/p95"),
        s("/monte_carlo/risk_of_ruin"),
        s("/luck/percentile_mean_r")
    );
    println!(
        "   borde bruto {} R contra k × C = {} R; comprar y mantener: retorno {}",
        s("/edge/measured_gross_r"),
        s("/edge/required_gross_r"),
        s("/buy_and_hold/total_return")
    );
    for n in r["notes"].as_array().into_iter().flatten() {
        println!("   · {}", n.as_str().unwrap_or_default());
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let only_validate = args.iter().any(|a| a == "--validar");
    args.retain(|a| a != "--validar");
    let equity: Decimal = take(&mut args, "--capital")
        .as_deref()
        .unwrap_or("100000")
        .parse()?;
    let desde: DateTime<Utc> = take(&mut args, "--desde")
        .as_deref()
        .unwrap_or("2011-01-01T00:00:00Z")
        .parse()?;
    let ledger = take(&mut args, "--ensayos").ok_or("falta --ensayos <archivo.jsonl>")?;
    let out_dir = take(&mut args, "--salida");
    let (spec_files, args): (Vec<String>, Vec<String>) =
        args.into_iter().partition(|a| a.ends_with(".json"));
    let lab = LabConfig::from_toml(cfg!("lab.toml"))?;
    let risk = RiskConfig::from_toml(cfg!("risk.toml"))?;
    let research = ResearchConfig::from_toml(cfg!("research.toml"))?;
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let cat = catalog(&lab, &risk, sources.roll.keys().cloned())?;
    if spec_files.is_empty() || spec_files.len() > cat.limits.max_variants_per_run {
        return Err(format!(
            "uso: de 1 a {} archivos de spec (.json)",
            cat.limits.max_variants_per_run
        )
        .into());
    }
    let texts: Vec<String> = spec_files
        .iter()
        .map(std::fs::read_to_string)
        .collect::<Result<_, _>>()?;
    let first: StrategySpec = serde_json::from_str(&texts[0])?;
    let id = first.market.instrument_id.clone();
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let feed = CandleFeed::Futures(&sources.futures_source);
    let last = store
        .last_candle_start_from(feed, &id, Timeframe::M15, Utc::now())
        .await?
        .ok_or("sin velas de futuros")?;
    let split = plan(desde, last, &split_cfg)?;
    let mut specs: Vec<ValidSpec> = Vec::new();
    for (file, text) in spec_files.iter().zip(&texts) {
        let (validation, valid) = validate_spec_json(text, &cat, &split);
        println!("{file}: {}", serde_json::to_string(&validation)?);
        specs.extend(valid);
    }
    if specs.len() != texts.len() {
        return Err("hay specs inválidas (ver spec_validation.v1 arriba)".into());
    }
    let same = |v: &ValidSpec| {
        v.family_sha256 == specs[0].family_sha256 && v.spec.period == specs[0].spec.period
    };
    if !specs.iter().all(same) {
        return Err(
            "las variantes de una corrida tienen que ser de la misma familia y período".into(),
        );
    }
    if only_validate {
        return Ok(());
    }

    let l = research_round::load(&id, &args, desde, equity, &sources, &store, &split_cfg).await?;
    let p = &l.p;
    let session = p.calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let rolls = p
        .setup
        .futures_basis
        .as_ref()
        .map(|b| b.rolls.clone())
        .unwrap_or_default();
    let series = LabSeries {
        m15: &p.m15,
        h1: &p.h1,
        session,
        entries: &p.entries,
    };
    let period = specs[0].spec.period;
    let month_list = months(period.from, period.to);
    let windows: Vec<Period> = split
        .windows
        .iter()
        .map(|w| Period {
            start: w.validation.start.max(period.from),
            end: w.validation.end.min(period.to),
        })
        .filter(|w| w.start < w.end)
        .collect();
    // Corre cada variante: recorrido, retornos mensuales (para el PBO) y comprar y mantener.
    let mut runs = Vec::new();
    for v in &specs {
        let setup = run_setup(&p.setup, v);
        let plans = plans_from_spec(v, &series);
        let run = run_plans(
            &plans,
            &p.m15,
            &p.force_close,
            &setup,
            period.from,
            period.to,
        )
        .map_err(|e| format!("{e:?}"))?;
        let m = v.spec.cost_multiplier;
        let closed = run
            .trades
            .iter()
            .map(|t| Ok((t.exit_time, trade_return(t, m, &setup)?)))
            .collect::<Result<Vec<_>, cfds_backtest::RunError>>()
            .map_err(|e| format!("{e:?}"))?;
        let monthly: Vec<f64> = monthly_returns(&closed, &month_list)
            .iter()
            .map(|d| research_round::f(*d))
            .collect();
        runs.push((setup, run, monthly));
    }
    let variants: Vec<Vec<f64>> = runs.iter().map(|r| r.2.clone()).collect();
    let past = match std::fs::read_to_string(&ledger) {
        Ok(text) => read_trials(&text)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let current: Vec<&str> = specs.iter().map(|v| v.spec_sha256.as_str()).collect();
    let family = family_variants(&past, &specs[0].family_sha256, &current);
    let config_sha = cfds_journal::config_sha256(&CONFIG_FILES);
    let code = code_version();
    let now = Utc::now();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger)?;
    for (v, (setup, run, _)) in specs.iter().zip(&runs) {
        let hold = buy_and_hold_returns(&p.m15, session, &rolls, setup, period.from, period.to)
            .map_err(|e| println!("⚠️  comprar y mantener: {e:?}"))
            .ok();
        let luck = luck_test(
            &LuckSeries {
                m15: &p.m15,
                session,
                entries: &p.entries,
                force_close: &p.force_close,
            },
            |j| {
                j.checked_sub(1)
                    .and_then(|d| planned_exit_for(v, &series, d))
            },
            &LuckConfig {
                simulations: lab.report.luck_simulations,
                seed: research.montecarlo.seed,
                target_r: v.exit.target_r,
            },
            &run.trades,
            setup,
            v.spec.risk_pct / Decimal::ONE_HUNDRED,
        );
        let report = lab_report(&ReportInputs {
            spec: v,
            run,
            setup,
            instrument_id: p.out.as_str(),
            months: &month_list,
            windows: &windows,
            buy_and_hold: hold.as_deref(),
            luck: Some(&luck),
            variants: &variants,
            project_trials: project_trials(&research),
            family_variants: family,
            montecarlo: &research.montecarlo,
            pbo_partitions: research.pbo_partitions,
            cost_stress: research.rule.cost_stress,
            edge_cost_factor: research.alpha_budget.edge_cost_factor,
            cfg: &lab.report,
            holdout_start: split.out_of_sample.start,
            config_sha256: &config_sha,
            code_version: code.as_deref(),
            last_candle: p.m15.last().ok_or("sin velas")?.start,
            computed_at: now,
        })
        .map_err(|e| format!("{e:?}"))?;
        summary(&report);
        let report_id = report["report_id"].as_str().unwrap_or_default().to_owned();
        if let Some(dir) = &out_dir {
            let path = std::path::Path::new(dir).join(format!("{report_id}.json"));
            std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
            println!("   informe en {}", path.display());
        }
        let trial = LabTrial::new(v, &report_id, now);
        writeln!(file, "{}", serde_json::to_string(&trial)?)?;
    }
    println!("ensayos en {ledger}; fuera de muestra: NO se cargó ni se evaluó");
    Ok(())
}

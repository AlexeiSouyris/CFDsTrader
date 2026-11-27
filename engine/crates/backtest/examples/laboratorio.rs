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
#[path = "common/lab_run.rs"]
mod lab_run;
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
    LabTrial, StrategySpec, ValidSpec, check_variants, family_variants, read_trials,
    validate_spec_json,
};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use lab_run::LabEnv;

fn take(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
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
    let env = LabEnv::load()?;
    let cat = &env.catalog;
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
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let split = env
        .split_for(&store, &first.market.instrument_id, desde)
        .await?;
    let mut specs: Vec<ValidSpec> = Vec::new();
    for (file, text) in spec_files.iter().zip(&texts) {
        let (validation, valid) = validate_spec_json(text, cat, &split);
        println!("{file}: {}", serde_json::to_string(&validation)?);
        specs.extend(valid);
    }
    if specs.len() != texts.len() {
        return Err("hay specs inválidas (ver spec_validation.v1 arriba)".into());
    }
    check_variants(&specs, cat)?;
    if only_validate {
        return Ok(());
    }

    let data = lab_run::load(&env, &specs, &args, desde, equity, split, &store).await?;
    // Corre cada variante: recorrido, retornos mensuales (para el PBO) y comprar y mantener.
    let runs = specs
        .iter()
        .map(|v| lab_run::run_variant(&data, v))
        .collect::<Result<Vec<_>, String>>()?;
    let variants: Vec<Vec<f64>> = runs.iter().map(|r| r.monthly.clone()).collect();
    let past = match std::fs::read_to_string(&ledger) {
        Ok(text) => read_trials(&text)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let current: Vec<&str> = specs.iter().map(|v| v.spec_sha256.as_str()).collect();
    let family = family_variants(&past, &specs[0].family_sha256, &current);
    let now = Utc::now();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger)?;
    for (v, vr) in specs.iter().zip(&runs) {
        let report = lab_run::report(&env, &data, v, vr, &variants, family, now)?;
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

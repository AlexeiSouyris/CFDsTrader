//! Ronda 1 de investigación (#472): familias C (ruptura del rango de apertura) y D1 (cambio de
//! mes) en el walk-forward de `config/data_split.toml`, con la regla pre-registrada
//! (`docs/analysis/2026-11-ronda1-preregistro.md`). Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example ronda1 -- \
//!     US500 US100 US30 --escenario-financiacion EUR=<archivo.csv> \
//!     [--supuestos <archivo.toml>] [--capital 100000] [--desde 2011-01-01T00:00:00Z] \
//!     [--salida <archivo.csv>] [--operaciones <archivo.csv>] [--descriptivo]
//! ```
//! - Las velas (futuros) se piden **solo hasta el inicio del fuera de muestra**. Para saber dónde
//!   empieza, se lee solo la hora de la última vela guardada, no sus precios.
//! - Las señales de cada familia se calculan una vez sobre la serie de investigación (funciones
//!   puras, sin mirar el futuro) y se ejecutan en cada ventana de validación las decididas en
//!   ella, con una cuenta nueva por índice.
//! - La cartera es el promedio de los retornos mensuales de los índices. Con `--descriptivo` se
//!   informa sin aplicar la regla (US2000 con costos supuestos).
#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use cfds_backtest::research::{Plan, ResearchConfig, orb_plans, turn_of_month_plans};
use cfds_core::InstrumentId;
use cfds_indicators::wilder_atr;
use cfds_market_data::{CandleSources, SplitConfig};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use research_round::{FamilyResult, describe, load, portfolio, run_family, verdict};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

const FAMILIES: [&str; 2] = ["C (ruptura del rango de apertura)", "D1 (cambio de mes)"];

fn take(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let descriptive = args.iter().any(|a| a == "--descriptivo");
    args.retain(|a| a != "--descriptivo");
    let equity: Decimal = take(&mut args, "--capital")
        .as_deref()
        .unwrap_or("100000")
        .parse()?;
    let from: DateTime<Utc> = take(&mut args, "--desde")
        .as_deref()
        .unwrap_or("2011-01-01T00:00:00Z")
        .parse()?;
    let out_csv = take(&mut args, "--salida");
    let trades_csv = take(&mut args, "--operaciones");
    let research = ResearchConfig::from_toml(cfg!("research.toml"))?;
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;

    // Los flags de estudio se leen una vez por índice (prepare se queda con las series).
    let (_, positional) = study::parse(args.clone())?;
    let instruments: Vec<InstrumentId> = positional
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(|a| a.parse())
        .collect::<Result<_, _>>()?;
    if instruments.is_empty() {
        return Err("uso: <INSTRUMENTO>... --escenario-financiacion <ETIQUETA>=<csv>".into());
    }

    let mut months: Vec<(i32, u32)> = Vec::new();
    // familia → índice → resultado
    let mut results: Vec<Vec<(InstrumentId, FamilyResult)>> = vec![Vec::new(), Vec::new()];
    for id in &instruments {
        let l = load(id, &args, from, equity, &sources, &store, &split_cfg).await?;
        let p = &l.p;
        let session = p.calendar.session_for(id).ok_or("instrumento sin bolsa")?;
        let atr15 = wilder_atr(&p.m15, p.setup.strategy.patterns.candles.atr_period);
        let plans: [Vec<Plan>; 2] = [
            orb_plans(&p.m15, session, &p.entries, &atr15, &research.orb),
            turn_of_month_plans(
                &p.m15,
                session,
                &p.entries,
                &p.force_close,
                &research.turn_of_month,
            ),
        ];
        if months.is_empty() {
            months = l.months.clone();
        } else if months != l.months {
            return Err(
                format!("{id}: los meses de validación no coinciden con los del primero").into(),
            );
        }
        for (fi, family_plans) in plans.iter().enumerate() {
            let r = run_family(family_plans, &l, research.rule.cost_stress)?;
            results[fi].push((id.clone(), r));
        }
    }

    for (fi, per) in results.iter().enumerate() {
        println!();
        println!("════════ Familia {} ════════", FAMILIES[fi]);
        for (id, r) in per {
            describe(id, r, equity);
        }
        verdict(
            &research.rule,
            &months,
            &portfolio(per, months.len(), false),
            &portfolio(per, months.len(), true),
            per,
            &[],
            descriptive,
        );
    }
    if let Some(path) = trades_csv {
        std::fs::write(&path, research_round::trades_csv(&["C", "D1"], &results))?;
        println!("operaciones en {path}");
    }
    if let Some(path) = out_csv {
        std::fs::write(
            &path,
            research_round::returns_csv(&["C", "D1"], &results, &months),
        )?;
        println!("retornos mensuales en {path}");
    }
    println!("fuera de muestra: NO se cargó ni se evaluó (#466 §4.1)");
    Ok(())
}

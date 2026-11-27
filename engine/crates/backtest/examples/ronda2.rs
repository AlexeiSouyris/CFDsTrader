//! Ronda 2 de investigación (#486): familias A (tendencia mensual) y B (momentum intradía) en el
//! walk-forward de `config/data_split.toml`, con la regla pre-registrada
//! (`docs/analysis/2026-11-ronda2-preregistro.md`). Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example ronda2 -- \
//!     US500 US100 US30 --escenario-financiacion EUR=<archivo.csv> \
//!     [--supuestos <archivo.toml>] [--capital 100000] [--desde 2011-01-01T00:00:00Z] \
//!     [--salida <archivo.csv>] [--operaciones <archivo.csv>] [--descriptivo]
//! ```
//! Igual que `ronda1`: velas solo hasta el inicio del fuera de muestra, señales calculadas una
//! vez con funciones puras, ejecución en cada ventana de validación con una cuenta nueva por
//! índice, cartera = promedio de los retornos mensuales. Para A, además, el criterio 5: Sharpe
//! de la cartera ≥ el de comprar y mantener el CFD en los mismos meses, con sus costos.
#![allow(
    clippy::float_arithmetic,
    reason = "solo estadísticos y porcentajes para mostrar; el dinero se calcula en Decimal"
)]

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cfds_backtest::research::{
    Plan, ResearchConfig, buy_and_hold_returns, intraday_momentum_plans, stats, trend_plans,
};
use cfds_core::InstrumentId;
use cfds_indicators::wilder_atr;
use cfds_market_data::{CandleSources, SplitConfig};
use cfds_storage::PgStore;
use cfds_strategy::Side;
use chrono::{DateTime, Utc};
use research_round::{FamilyResult, describe, f, load, mean, portfolio, run_family, verdict};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

const FAMILIES: [&str; 2] = [
    "A (tendencia mensual)",
    "B (momentum intradía: primera media hora → última media hora)",
];
const SHORT: [&str; 2] = ["A", "B"];

fn take(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
}

fn sharpe(xs: &[f64]) -> f64 {
    match (stats::mean(xs), stats::std_dev(xs)) {
        (Some(m), Some(s)) if s > 0.0 => m / s,
        _ => f64::NAN,
    }
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
    let rule = research.rule_round2;
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;

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
    let mut results: Vec<Vec<(InstrumentId, FamilyResult)>> = vec![Vec::new(), Vec::new()];
    // Comprar y mantener por índice (criterio 5 de A).
    let mut hold: Vec<(InstrumentId, Vec<f64>)> = Vec::new();
    for id in &instruments {
        let l = load(id, &args, from, equity, &sources, &store, &split_cfg).await?;
        let p = &l.p;
        let session = p.calendar.session_for(id).ok_or("instrumento sin bolsa")?;
        let rolls = p
            .setup
            .futures_basis
            .as_ref()
            .map(|b| b.rolls.clone())
            .ok_or("sin vencimientos: la ronda 2 corre sobre futuros")?;
        let atr15 = wilder_atr(&p.m15, research.intraday_momentum.atr_period);
        let plans: [Vec<Plan>; 2] = [
            trend_plans(
                &p.m15,
                session,
                &p.entries,
                &p.force_close,
                &rolls,
                &research.trend,
            ),
            intraday_momentum_plans(
                &p.m15,
                session,
                &p.entries,
                &atr15,
                &rolls,
                &research.intraday_momentum,
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
            let r = run_family(family_plans, &l, rule.cost_stress)?;
            results[fi].push((id.clone(), r));
        }
        let mut by_month: BTreeMap<(i32, u32), Decimal> = BTreeMap::new();
        for w in &l.split.windows {
            for (m, r) in buy_and_hold_returns(
                &p.m15,
                session,
                &rolls,
                &p.setup,
                w.validation.start,
                w.validation.end,
            )
            .map_err(|e| format!("{e:?}"))?
            {
                by_month.insert(m, r);
            }
        }
        let series: Vec<f64> = months
            .iter()
            .map(|m| by_month.get(m).copied().map_or(0.0, f))
            .collect();
        let missing = months.iter().filter(|m| !by_month.contains_key(m)).count();
        if missing > 0 {
            println!("⚠️  {id}: comprar y mantener sin valor en {missing} meses (cuentan como 0)");
        }
        hold.push((id.clone(), series));
    }

    let hold_portfolio: Vec<f64> = (0..months.len())
        .map(|k| mean(&hold.iter().map(|(_, s)| s[k]).collect::<Vec<_>>()))
        .collect();
    for (fi, per) in results.iter().enumerate() {
        println!();
        println!("════════ Familia {} ════════", FAMILIES[fi]);
        for (id, r) in per {
            describe(id, r, equity);
            if fi == 0 {
                let mut events: BTreeMap<usize, Side> = BTreeMap::new();
                for t in &r.trades {
                    events.entry(t.event).or_insert(t.side);
                }
                let longs = events.values().filter(|s| **s == Side::Long).count();
                println!(
                    "    meses con operación: {} en largo, {} en corto (de {} meses)",
                    longs,
                    events.len() - longs,
                    months.len()
                );
                if let Some((_, h)) = hold.iter().find(|(x, _)| x == id) {
                    println!(
                        "    comprar y mantener: retorno mensual medio {:+.4} %, Sharpe mensual {:.4}",
                        100.0 * mean(h),
                        sharpe(h)
                    );
                }
            }
        }
        let port = portfolio(per, months.len(), false);
        let extra = if fi == 0 {
            let (a, b) = (sharpe(&port), sharpe(&hold_portfolio));
            println!(
                "  comprar y mantener (cartera): retorno mensual medio {:+.4} %, Sharpe mensual {:.4} \
                 (anual ≈ {:.2})",
                100.0 * mean(&hold_portfolio),
                b,
                b * 12f64.sqrt()
            );
            vec![(
                format!("Sharpe mensual de A {a:.4} ≥ el de comprar y mantener {b:.4}"),
                a >= b,
            )]
        } else {
            Vec::new()
        };
        verdict(
            &rule,
            &months,
            &port,
            &portfolio(per, months.len(), true),
            per,
            &extra,
            descriptive,
        );
    }
    if let Some(path) = trades_csv {
        std::fs::write(&path, research_round::trades_csv(&SHORT, &results))?;
        println!("operaciones en {path}");
    }
    if let Some(path) = out_csv {
        let mut csv = research_round::returns_csv(&SHORT, &results, &months);
        for (id, h) in &hold {
            for (k, (y, m)) in months.iter().enumerate() {
                writeln!(csv, "comprar_y_mantener,{id},{y}-{m:02},{:.8},", h[k])?;
            }
        }
        std::fs::write(&path, csv)?;
        println!("retornos mensuales en {path}");
    }
    println!("fuera de muestra: NO se cargó ni se evaluó (#466 §4.1)");
    Ok(())
}

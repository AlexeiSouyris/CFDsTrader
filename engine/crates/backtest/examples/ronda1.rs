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
#![allow(
    clippy::float_arithmetic,
    reason = "solo estadísticos y porcentajes para mostrar; el dinero se calcula en Decimal"
)]

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/study.rs"]
mod study;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cfds_backtest::research::{
    Plan, ResearchConfig, StudyExit, StudyTrade, monthly_returns, orb_plans, run_plans, stats,
    trade_return, turn_of_month_plans,
};
use cfds_core::{InstrumentId, Timeframe};
use cfds_indicators::wilder_atr;
use cfds_market_data::{CandleSource, CandleSources, SplitConfig, plan};
use cfds_storage::{CandleFeed, PgStore};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

const FAMILIES: [&str; 2] = ["C (ruptura del rango de apertura)", "D1 (cambio de mes)"];

/// Lo de una familia en un índice.
struct Result1 {
    trades: Vec<StudyTrade>,
    rejected: BTreeMap<String, usize>,
    monthly: Vec<f64>,
    monthly_stress: Vec<f64>,
    min_deal: Decimal,
}

fn take(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
}

fn f(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(f64::NAN)
}

fn mean(xs: &[f64]) -> f64 {
    stats::mean(xs).unwrap_or(f64::NAN)
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
    let mut results: Vec<Vec<(InstrumentId, Result1)>> = vec![Vec::new(), Vec::new()];
    for id in &instruments {
        let (study, _) = study::parse(args.clone())?;
        if study.as_ref().is_none_or(|s| s.funding.is_none()) {
            return Err(
                "falta --escenario-financiacion (el tipo de EE. UU. está pendiente)".into(),
            );
        }
        let last = store
            .last_candle_start_from(
                CandleFeed::Futures(&sources.futures_source),
                id,
                Timeframe::M15,
                Utc::now(),
            )
            .await?
            .ok_or("sin velas de futuros")?;
        let oos_start = plan(from, last, &split_cfg)?.out_of_sample.start;
        println!(
            "══ {id}: última vela guardada {last}; fuera de muestra desde {oos_start} (NO se carga)"
        );
        let p = prepare::prepare_with(
            id,
            CandleSource::Futures,
            (from, oos_start),
            equity,
            None,
            true,
            study,
        )
        .await?;
        let (first, last_loaded) = match (p.m15.first(), p.m15.last()) {
            (Some(a), Some(b)) => (a.start, b.start),
            _ => return Err("sin velas".into()),
        };
        if last_loaded >= oos_start {
            return Err("se cargó una vela del fuera de muestra".into());
        }
        let split = plan(first, last, &split_cfg)?;
        if split.out_of_sample.start != oos_start {
            return Err("el fuera de muestra no coincide".into());
        }
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
        let these_months: Vec<(i32, u32)> = {
            let mut m = Vec::new();
            let mut d = split.windows[0].validation.start.date_naive();
            let end = split.out_of_sample.start.date_naive();
            while d < end {
                m.push((d.year(), d.month()));
                d = d
                    .checked_add_months(chrono::Months::new(1))
                    .ok_or("fecha")?;
            }
            m
        };
        println!(
            "{id}: {} ventanas de validación, {} → {} ({} meses)",
            split.windows.len(),
            split.windows[0].validation.start.date_naive(),
            split.out_of_sample.start.date_naive(),
            these_months.len()
        );
        if months.is_empty() {
            months = these_months.clone();
        } else if months != these_months {
            return Err(
                format!("{id}: los meses de validación no coinciden con los del primero").into(),
            );
        }
        let min_deal = p
            .setup
            .limits
            .min_deal_size
            .ok_or("tamaño mínimo pendiente")?;
        for (fi, family_plans) in plans.iter().enumerate() {
            let mut trades = Vec::new();
            let mut rejected: BTreeMap<String, usize> = BTreeMap::new();
            for w in &split.windows {
                let run = run_plans(
                    family_plans,
                    &p.m15,
                    &p.force_close,
                    &p.setup,
                    w.validation.start,
                    w.validation.end,
                )
                .map_err(|e| format!("{e:?}"))?;
                for r in run.rejected {
                    *rejected.entry(format!("{:?}", r.reason)).or_default() += 1;
                }
                trades.extend(run.trades);
            }
            let returns = |stress: Decimal| -> Result<Vec<f64>, String> {
                let closed = trades
                    .iter()
                    .map(|t| Ok((t.exit_time, trade_return(t, stress, &p.setup)?)))
                    .collect::<Result<Vec<_>, cfds_backtest::RunError>>()
                    .map_err(|e| format!("{e:?}"))?;
                Ok(monthly_returns(&closed, &these_months)
                    .into_iter()
                    .map(f)
                    .collect())
            };
            let monthly = returns(Decimal::ONE)?;
            let monthly_stress = returns(research.rule.cost_stress)?;
            results[fi].push((
                id.clone(),
                Result1 {
                    trades,
                    rejected,
                    monthly,
                    monthly_stress,
                    min_deal,
                },
            ));
        }
    }

    let mut csv = String::from("familia,instrumento,mes,retorno,retorno_costos_estresados\n");
    for (fi, per) in results.iter().enumerate() {
        println!();
        println!("════════ Familia {} ════════", FAMILIES[fi]);
        for (id, r) in per {
            describe(id, r, equity);
            for (k, (y, m)) in months.iter().enumerate() {
                writeln!(
                    csv,
                    "{},{id},{y}-{m:02},{:.8},{:.8}",
                    ["C", "D1"][fi],
                    r.monthly[k],
                    r.monthly_stress[k]
                )?;
            }
        }
        let portfolio: Vec<f64> = (0..months.len())
            .map(|k| mean(&per.iter().map(|(_, r)| r.monthly[k]).collect::<Vec<_>>()))
            .collect();
        let portfolio_stress: Vec<f64> = (0..months.len())
            .map(|k| {
                mean(
                    &per.iter()
                        .map(|(_, r)| r.monthly_stress[k])
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        verdict(
            &research,
            &months,
            &portfolio,
            &portfolio_stress,
            per,
            descriptive,
        );
    }
    if let Some(path) = trades_csv {
        let mut t = String::from(
            "familia,instrumento,evento,lado,decision,entrada,referencia,stop,entrada_medio,\
             salida,salida_medio,motivo,tamano,riesgo,pl_medio,costos,financiacion,pl_neto,capital\n",
        );
        for (fi, per) in results.iter().enumerate() {
            for (id, r) in per {
                for x in &r.trades {
                    writeln!(
                        t,
                        "{},{id},{},{:?},{},{},{},{},{},{},{},{:?},{},{},{},{},{},{},{}",
                        ["C", "D1"][fi],
                        x.event,
                        x.side,
                        x.signal_at,
                        x.entry_time,
                        x.reference,
                        x.stop,
                        x.entry_mid,
                        x.exit_time,
                        x.exit_mid,
                        x.exit,
                        x.size.round_dp(4),
                        x.risk.round_dp(2),
                        x.mid_pnl.round_dp(2),
                        x.total_costs().round_dp(2),
                        x.funding.round_dp(2),
                        x.net_pnl.round_dp(2),
                        x.equity_at_entry.round_dp(2)
                    )?;
                }
            }
        }
        std::fs::write(&path, t)?;
        println!("operaciones en {path}");
    }
    if let Some(path) = out_csv {
        std::fs::write(&path, csv)?;
        println!("retornos mensuales en {path}");
    }
    println!("fuera de muestra: NO se cargó ni se evaluó (#466 §4.1)");
    Ok(())
}

fn describe(id: &InstrumentId, r: &Result1, equity: Decimal) {
    let t = &r.trades;
    let r_of = |x: Decimal, risk: Decimal| f(x / risk);
    let mid_r: Vec<f64> = t.iter().map(|x| r_of(x.mid_pnl, x.risk)).collect();
    let net_r: Vec<f64> = t.iter().map(|x| r_of(x.net_pnl, x.risk)).collect();
    let cost_r: Vec<f64> = t.iter().map(|x| r_of(x.total_costs(), x.risk)).collect();
    let longs = t
        .iter()
        .filter(|x| x.side == cfds_strategy::Side::Long)
        .count();
    let mut exits: BTreeMap<String, usize> = BTreeMap::new();
    for x in t {
        let k = match x.exit {
            StudyExit::Stop | StudyExit::StopGap => "stop",
            StudyExit::ForceClose => "cierre forzado",
            StudyExit::Planned => "salida de la definición",
        };
        *exits.entry(k.to_owned()).or_default() += 1;
    }
    // Con 10 000 €: el tamaño escala con el capital al decidir.
    let small = Decimal::from(10_000);
    let too_small = t
        .iter()
        .filter(|x| x.size * small / x.equity_at_entry < r.min_deal)
        .count();
    let wins = net_r.iter().filter(|x| **x > 0.0).count();
    println!(
        "  {id}: {} operaciones ({} largos, {} cortos); salidas {:?}; rechazos del riesgo {:?}",
        t.len(),
        longs,
        t.len() - longs,
        exits,
        r.rejected
    );
    println!(
        "    R medio a precio medio {:+.4}, costo medio {:.4} R, R neto medio {:+.4} \
         (aciertos {:.1} %); con {small} € no se habrían abierto {} ({:.1} %); capital {equity}",
        mean(&mid_r),
        mean(&cost_r),
        mean(&net_r),
        100.0 * wins as f64 / t.len().max(1) as f64,
        too_small,
        100.0 * too_small as f64 / t.len().max(1) as f64
    );
    println!(
        "    retorno mensual medio {:+.4} % (costos × 1,5: {:+.4} %)",
        100.0 * mean(&r.monthly),
        100.0 * mean(&r.monthly_stress)
    );
}

fn verdict(
    research: &ResearchConfig,
    months: &[(i32, u32)],
    portfolio: &[f64],
    stress: &[f64],
    per: &[(InstrumentId, Result1)],
    descriptive: bool,
) {
    let rule = &research.rule;
    let alpha = f(rule.alpha);
    let t = stats::t_test_greater(portfolio);
    let dsr = stats::deflated_sharpe(portfolio, rule.dsr_trials);
    let half = NaiveDate::from_ymd_opt(
        rule.second_half_from.year(),
        rule.second_half_from.month(),
        1,
    )
    .map(|d| (d.year(), d.month()));
    let (first, second): (Vec<f64>, Vec<f64>) = {
        let mut a = Vec::new();
        let mut b = Vec::new();
        for (k, m) in months.iter().enumerate() {
            if Some(*m) < half {
                a.push(portfolio[k])
            } else {
                b.push(portfolio[k])
            }
        }
        (a, b)
    };
    let positive_indices = per.iter().filter(|(_, r)| mean(&r.monthly) > 0.0).count();
    println!(
        "  cartera ({} índices, {} meses): retorno mensual medio {:+.4} %, desvío {:.4} %",
        per.len(),
        portfolio.len(),
        100.0 * mean(portfolio),
        100.0 * stats::std_dev(portfolio).unwrap_or(f64::NAN)
    );
    let c1 = t.is_some_and(|t| t.p_value < alpha);
    if let Some(t) = t {
        println!(
            "  1. prueba t unilateral: t = {:.3}, p = {:.5} (nivel {alpha}) → {}",
            t.t,
            t.p_value,
            if c1 { "PASA" } else { "no pasa" }
        );
    }
    let c2 = dsr.is_some_and(|d| d.dsr >= f(rule.dsr_min));
    if let Some(d) = dsr {
        println!(
            "  2. Sharpe deflactado (N = {}): Sharpe mensual {:.4} (anual ≈ {:.2}), SR0 {:.4}, \
             asimetría {:.3}, curtosis {:.3}, DSR {:.4} (mínimo {}) → {}",
            rule.dsr_trials,
            d.sharpe,
            d.sharpe * 12f64.sqrt(),
            d.sharpe0,
            d.skew,
            d.kurtosis,
            d.dsr,
            rule.dsr_min,
            if c2 { "PASA" } else { "no pasa" }
        );
        for n in [100, 1000] {
            if let Some(x) = stats::deflated_sharpe(portfolio, n) {
                println!(
                    "     (referencia, N = {n}: SR0 {:.4}, DSR {:.4})",
                    x.sharpe0, x.dsr
                );
            }
        }
    }
    let c3 = mean(&first) > 0.0 && mean(&second) > 0.0 && positive_indices * 3 >= per.len() * 2;
    println!(
        "  3. consistencia: primera mitad {:+.4} %, segunda {:+.4} %, índices con media > 0: {} de {} → {}",
        100.0 * mean(&first),
        100.0 * mean(&second),
        positive_indices,
        per.len(),
        if c3 { "PASA" } else { "no pasa" }
    );
    let c4 = mean(stress) > 0.0;
    println!(
        "  4. costos × {}: retorno mensual medio {:+.4} % → {}",
        rule.cost_stress,
        100.0 * mean(stress),
        if c4 { "PASA" } else { "no pasa" }
    );
    if descriptive {
        println!("  (descriptivo: sin veredicto)");
    } else {
        println!(
            "  VEREDICTO con este escenario: {}",
            if c1 && c2 && c3 && c4 {
                "PASA los cuatro criterios"
            } else {
                "NO PASA"
            }
        );
    }
}

//! Ronda 3 de investigación (#493, programa 2): familias V (exposición manejada por volatilidad)
//! y M1 (cascada al cruzar un número redondo) en el walk-forward de `config/data_split.toml`, con
//! la regla pre-registrada (`docs/analysis/2026-11-ronda3-preregistro.md`), más el descriptivo de
//! la deriva nocturna. Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example ronda3 -- \
//!     US500 US100 US30 --escenario-financiacion EUR=<archivo.csv> \
//!     [--supuestos <archivo.toml>] [--capital 100000] [--desde 2011-01-01T00:00:00Z] \
//!     [--salida <archivo.csv>] [--operaciones <archivo.csv>] [--descriptivo]
//! ```
//! Igual que `ronda1` y `ronda2`: velas solo hasta el inicio del fuera de muestra, señales con
//! funciones puras, una cuenta nueva por índice y por ventana de validación, cartera = promedio
//! de los retornos mensuales. M1 corre solo en los índices con paso en `[round_number.steps]`.
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
    Levels, ResearchConfig, RuleConfig, managed_returns, monthly_reviews, review_variances,
    round_number_plans, session_close_indices, stats, vol_constant, vol_weights, window_returns,
};
use cfds_core::InstrumentId;
use cfds_indicators::wilder_atr;
use cfds_market_data::{CandleSources, SplitConfig};
use cfds_storage::PgStore;
use chrono::{DateTime, Datelike, TimeDelta, Utc};
use research_round::{FamilyResult, describe, f, load, mean, portfolio, run_family, verdict};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

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

/// Series mensuales de V y de comprar y mantener en un índice, con y sin estrés.
struct VolSeries {
    v: Vec<f64>,
    v_stress: Vec<f64>,
    hold: Vec<f64>,
    hold_stress: Vec<f64>,
    weights: Vec<f64>,
    at_cap: usize,
    skipped_windows: usize,
}

fn series_of(by_month: &BTreeMap<(i32, u32), Decimal>, months: &[(i32, u32)]) -> (Vec<f64>, usize) {
    let missing = months.iter().filter(|m| !by_month.contains_key(m)).count();
    (
        months
            .iter()
            .map(|m| by_month.get(m).copied().map_or(0.0, f))
            .collect(),
        missing,
    )
}

/// Criterios de V (pre-registro §3.2) sobre la cartera. Devuelve si pasa los cinco.
fn vol_verdict(
    rule: &RuleConfig,
    months: &[(i32, u32)],
    per: &[(InstrumentId, VolSeries)],
    descriptive: bool,
) -> bool {
    let avg = |pick: &dyn Fn(&VolSeries) -> &Vec<f64>| -> Vec<f64> {
        (0..months.len())
            .map(|k| mean(&per.iter().map(|(_, s)| pick(s)[k]).collect::<Vec<_>>()))
            .collect()
    };
    let y = avg(&|s| &s.v);
    let x = avg(&|s| &s.hold);
    let ys = avg(&|s| &s.v_stress);
    let xs = avg(&|s| &s.hold_stress);
    let alpha = f(rule.alpha);
    println!(
        "  cartera ({} índices, {} meses): V {:+.4} % por mes (Sharpe {:.4}, anual ≈ {:.2}); \
         comprar y mantener {:+.4} % (Sharpe {:.4}, anual ≈ {:.2})",
        per.len(),
        months.len(),
        100.0 * mean(&y),
        sharpe(&y),
        sharpe(&y) * 12f64.sqrt(),
        100.0 * mean(&x),
        sharpe(&x),
        sharpe(&x) * 12f64.sqrt()
    );
    let Some(a) = stats::alpha_hc1(&y, &x) else {
        println!("  sin regresión (datos insuficientes)");
        return false;
    };
    let c1 = a.p_value < alpha;
    println!(
        "  1. alfa contra comprar y mantener: α = {:+.4} % por mes (anual ≈ {:+.2} %), β = {:.3}, \
         e.e. HC1 {:.4} %, t = {:.3}, p = {:.5} (nivel {alpha}) → {}",
        100.0 * a.alpha,
        1200.0 * a.alpha,
        a.beta,
        100.0 * a.se_alpha,
        a.t,
        a.p_value,
        if c1 { "PASA" } else { "no pasa" }
    );
    let active: Vec<f64> = y.iter().zip(&x).map(|(yi, xi)| yi - a.beta * xi).collect();
    let dsr = stats::deflated_sharpe(&active, rule.dsr_trials);
    let c2 = dsr.is_some_and(|d| d.dsr >= f(rule.dsr_min));
    if let Some(d) = dsr {
        println!(
            "  2. Sharpe deflactado de la ventaja (y − βx, N = {}): Sharpe mensual {:.4} (anual ≈ \
             {:.2}), SR0 {:.4}, DSR {:.4} (mínimo {}) → {}",
            rule.dsr_trials,
            d.sharpe,
            d.sharpe * 12f64.sqrt(),
            d.sharpe0,
            d.dsr,
            rule.dsr_min,
            if c2 { "PASA" } else { "no pasa" }
        );
        for n in [100, 1000] {
            if let Some(r) = stats::deflated_sharpe(&active, n) {
                println!(
                    "     (referencia, N = {n}: SR0 {:.4}, DSR {:.4})",
                    r.sharpe0, r.dsr
                );
            }
        }
    }
    let half = (rule.second_half_from.year(), rule.second_half_from.month());
    let (mut first, mut second) = (Vec::new(), Vec::new());
    for (k, m) in months.iter().enumerate() {
        if *m < half {
            first.push(active[k]);
        } else {
            second.push(active[k]);
        }
    }
    let positive = per
        .iter()
        .filter(|(_, s)| stats::alpha_hc1(&s.v, &s.hold).is_some_and(|a| a.alpha > 0.0))
        .count();
    let c3 = mean(&first) > 0.0 && mean(&second) > 0.0 && positive * 3 >= per.len() * 2;
    println!(
        "  3. consistencia: ventaja media primera mitad {:+.4} %, segunda {:+.4} %, índices con \
         alfa > 0: {} de {} → {}",
        100.0 * mean(&first),
        100.0 * mean(&second),
        positive,
        per.len(),
        if c3 { "PASA" } else { "no pasa" }
    );
    let stressed = stats::alpha_hc1(&ys, &xs);
    let c4 = stressed.is_some_and(|s| s.alpha > 0.0);
    println!(
        "  4. costos × {}: α = {:+.4} % por mes → {}",
        rule.cost_stress,
        100.0 * stressed.map_or(f64::NAN, |s| s.alpha),
        if c4 { "PASA" } else { "no pasa" }
    );
    let c5 = sharpe(&y) >= sharpe(&x);
    println!(
        "  5. Sharpe mensual de V {:.4} ≥ el de comprar y mantener {:.4} → {}",
        sharpe(&y),
        sharpe(&x),
        if c5 { "PASA" } else { "no pasa" }
    );
    let all = c1 && c2 && c3 && c4 && c5;
    if descriptive {
        println!("  (descriptivo: sin veredicto)");
    } else {
        println!(
            "  VEREDICTO con este escenario: {}",
            if all {
                "PASA los 5 criterios"
            } else {
                "NO PASA"
            }
        );
    }
    all
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
    let rule = research.rule_round3;
    let vcfg = research.vol_managed;
    let rcfg = &research.round_number;
    let ocfg = research.overnight_drift;
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
    let mut vol: Vec<(InstrumentId, VolSeries)> = Vec::new();
    // M1: niveles redondos y control.
    let mut m1: [Vec<(InstrumentId, FamilyResult)>; 2] = [Vec::new(), Vec::new()];
    let mut drift: Vec<(InstrumentId, Vec<f64>)> = Vec::new();
    for id in &instruments {
        let l = load(id, &args, from, equity, &sources, &store, &split_cfg).await?;
        let p = &l.p;
        let session = p.calendar.session_for(id).ok_or("instrumento sin bolsa")?;
        let rolls = p
            .setup
            .futures_basis
            .as_ref()
            .map(|b| b.rolls.clone())
            .ok_or("sin vencimientos: la ronda 3 corre sobre futuros")?;
        if months.is_empty() {
            months = l.months.clone();
        } else if months != l.months {
            return Err(
                format!("{id}: los meses de validación no coinciden con los del primero").into(),
            );
        }

        // ── V ──
        let reviews: Vec<usize> = monthly_reviews(&p.m15, session, &p.entries)
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        let closes = session_close_indices(&p.m15, session);
        let rv = review_variances(&p.m15, &rolls, &reviews, &closes);
        let mut maps: [BTreeMap<(i32, u32), Decimal>; 4] = Default::default();
        let mut weights_used = Vec::new();
        let mut at_cap = 0usize;
        let mut skipped = 0usize;
        for w in &l.split.windows {
            let Some(c) = vol_constant(
                &p.m15,
                &rolls,
                &reviews,
                &rv,
                w.train.start,
                w.train.end,
                vcfg.min_training_pairs,
            ) else {
                skipped += 1;
                continue;
            };
            let weights = vol_weights(&rv, c, vcfg.cap);
            let schedule: Vec<(usize, Decimal)> = reviews
                .iter()
                .zip(&weights)
                .filter_map(|(&i, w)| w.map(|w| (i, w)))
                .collect();
            for &(i, wt) in &schedule {
                let end = p.m15[i].end();
                if end >= w.validation.start && end < w.validation.end {
                    weights_used.push(f(wt));
                    if wt == vcfg.cap {
                        at_cap += 1;
                    }
                }
            }
            let hold: Vec<(usize, Decimal)> = reviews
                .iter()
                .filter(|&&i| p.m15[i].end() <= w.validation.start)
                .map(|&i| (i, Decimal::ONE))
                .collect();
            for (k, (sched, stress)) in [
                (&schedule, Decimal::ONE),
                (&schedule, rule.cost_stress),
                (&hold, Decimal::ONE),
                (&hold, rule.cost_stress),
            ]
            .into_iter()
            .enumerate()
            {
                for (m, r) in managed_returns(
                    &p.m15,
                    session,
                    &rolls,
                    &p.setup,
                    (w.validation.start, w.validation.end),
                    sched,
                    stress,
                )
                .map_err(|e| format!("{e:?}"))?
                {
                    maps[k].insert(m, r);
                }
            }
        }
        let [v_map, vs_map, h_map, hs_map] = maps;
        let (v, miss) = series_of(&v_map, &months);
        if miss > 0 {
            println!("⚠️  {id}: V sin valor en {miss} meses (cuentan como 0)");
        }
        let (hold, miss) = series_of(&h_map, &months);
        if miss > 0 {
            println!("⚠️  {id}: comprar y mantener sin valor en {miss} meses (cuentan como 0)");
        }
        vol.push((
            id.clone(),
            VolSeries {
                v,
                v_stress: series_of(&vs_map, &months).0,
                hold,
                hold_stress: series_of(&hs_map, &months).0,
                weights: weights_used,
                at_cap,
                skipped_windows: skipped,
            },
        ));

        // ── M1 ──
        if let Some(step) = rcfg.step_for(&id.to_string()) {
            let atr15 = wilder_atr(&p.m15, rcfg.atr_period);
            let horizon = TimeDelta::minutes(i64::from(rcfg.horizon_minutes));
            for (k, offset) in [Decimal::ZERO, step / Decimal::TWO].into_iter().enumerate() {
                let plans = round_number_plans(
                    &p.m15,
                    &p.entries,
                    &atr15,
                    &rolls,
                    Levels { step, offset },
                    rcfg.stop_atr,
                    horizon,
                );
                m1[k].push((id.clone(), run_family(&plans, &l, rule.cost_stress)?));
            }
        }

        // ── Deriva nocturna (descriptivo) ──
        let lo = l.split.windows[0].validation.start.date_naive();
        let hi = l.split.out_of_sample.start.date_naive();
        let r: Vec<f64> = window_returns(&p.m15, &rolls, ocfg.timezone, ocfg.from, ocfg.to)
            .into_iter()
            .filter(|(d, _)| *d >= lo && *d < hi)
            .map(|(_, r)| f(r))
            .collect();
        drift.push((id.clone(), r));
    }

    println!();
    println!("════════ Familia V (exposición manejada por volatilidad) ════════");
    for (id, s) in &vol {
        let a = stats::alpha_hc1(&s.v, &s.hold);
        let dw: Vec<f64> = s.weights.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        println!(
            "  {id}: peso medio {:.3} (meses en el tope {} de {}), cambio medio del peso {:.3}; \
             ventanas sin c {}; V {:+.4} % por mes (Sharpe {:.4}, × 1,5: {:+.4} %), comprar y \
             mantener {:+.4} % (Sharpe {:.4}); α {:+.4} %, β {:.3}, t {:.2}",
            mean(&s.weights),
            s.at_cap,
            s.weights.len(),
            mean(&dw),
            s.skipped_windows,
            100.0 * mean(&s.v),
            sharpe(&s.v),
            100.0 * mean(&s.v_stress),
            100.0 * mean(&s.hold),
            sharpe(&s.hold),
            100.0 * a.map_or(f64::NAN, |a| a.alpha),
            a.map_or(f64::NAN, |a| a.beta),
            a.map_or(f64::NAN, |a| a.t),
        );
    }
    vol_verdict(&rule, &months, &vol, descriptive);

    if !m1[0].is_empty() {
        println!();
        println!("════════ Familia M1 (cascada al cruzar un número redondo) ════════");
        for (k, name) in ["niveles redondos", "control: niveles intermedios"]
            .iter()
            .enumerate()
        {
            println!("  ── {name}");
            for (id, r) in &m1[k] {
                describe(id, r, equity);
            }
        }
        let gross = |per: &[(InstrumentId, FamilyResult)]| -> (f64, usize) {
            let xs: Vec<f64> = per
                .iter()
                .flat_map(|(_, r)| r.trades.iter().map(|t| f(t.mid_pnl / t.risk)))
                .collect();
            (mean(&xs), xs.len())
        };
        let (g_round, n_round) = gross(&m1[0]);
        let (g_ctrl, n_ctrl) = gross(&m1[1]);
        let n = months.len();
        verdict(
            &rule,
            &months,
            &portfolio(&m1[0], n, false),
            &portfolio(&m1[0], n, true),
            &m1[0],
            &[(
                format!(
                    "R medio antes de costos de los redondos {g_round:+.4} ({n_round} op.) > el \
                     del control {g_ctrl:+.4} ({n_ctrl} op.)"
                ),
                g_round > g_ctrl,
            )],
            descriptive,
        );
        println!(
            "  (control, solo descriptivo: retorno mensual medio de la cartera {:+.4} %)",
            100.0 * mean(&portfolio(&m1[1], n, false))
        );
    }

    println!();
    println!("════════ Deriva nocturna (descriptivo sin voto) ════════");
    for (id, r) in &drift {
        let t = stats::t_test_greater(r);
        println!(
            "  {id}: {} días, retorno medio {} {}–{} {:+.3} pb (t = {:.2}), anual ≈ {:+.2} %",
            r.len(),
            ocfg.timezone,
            ocfg.from,
            ocfg.to,
            1e4 * mean(r),
            t.map_or(f64::NAN, |t| t.t),
            100.0 * 252.0 * mean(r)
        );
    }

    if let Some(path) = trades_csv {
        std::fs::write(
            &path,
            research_round::trades_csv(&["M1", "M1_control"], &m1),
        )?;
        println!("operaciones en {path}");
    }
    if let Some(path) = out_csv {
        let mut csv = String::from("familia,instrumento,mes,retorno,retorno_costos_estresados\n");
        for (id, s) in &vol {
            for (k, (y, m)) in months.iter().enumerate() {
                writeln!(csv, "V,{id},{y}-{m:02},{:.8},{:.8}", s.v[k], s.v_stress[k])?;
                writeln!(
                    csv,
                    "comprar_y_mantener,{id},{y}-{m:02},{:.8},{:.8}",
                    s.hold[k], s.hold_stress[k]
                )?;
            }
        }
        let body = research_round::returns_csv(&["M1", "M1_control"], &m1, &months);
        csv.push_str(body.split_once('\n').map_or("", |(_, b)| b));
        std::fs::write(&path, csv)?;
        println!("retornos mensuales en {path}");
    }
    println!("fuera de muestra: NO se cargó ni se evaluó (#493, pre-registro §1)");
    Ok(())
}

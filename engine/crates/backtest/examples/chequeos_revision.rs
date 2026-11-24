//! Chequeos **descriptivos** de la revisión externa (#512): sin voto, sin presupuesto de error y
//! sin cambiar ningún veredicto registrado. Reutiliza la carga y las funciones de las rondas 2 y
//! 3 (`cfds_backtest::research`); no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example chequeos_revision -- \
//!     US500 US100 US30 --escenario-financiacion EUR=<archivo.csv> [--capital 100000]
//! ```
//! 1. Deriva nocturna 02–03 ET por mitades y por año.
//! 2. La misma deriva solo después de un cierre de contado con caída fuerte (`[review_checks]`).
//! 3. Cartera 50 % comprar y mantener + 50 % V escalada a la volatilidad de comprar y mantener
//!    (escala estimada solo con el entrenamiento de cada ventana).
//! 5. Momentum de la última media hora (B, ronda 2): R antes de costos por índice y su t.
//!
//! Velas solo hasta el inicio del fuera de muestra, igual que `ronda2` y `ronda3`.
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

use cfds_backtest::research::{
    ResearchConfig, blend_return, drift_after_close, intraday_momentum_plans, managed_returns,
    monthly_reviews, review_variances, session_close_indices, session_close_returns, stats,
    strong_drop_flags, vol_constant, vol_match_scale, vol_weights, window_returns,
};
use cfds_core::InstrumentId;
use cfds_indicators::wilder_atr;
use cfds_market_data::{CandleSources, SplitConfig};
use cfds_storage::PgStore;
use chrono::{DateTime, Datelike, Months, NaiveDate, TimeZone, Utc};
use research_round::{f, load, mean, run_family};
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

/// `n días, media en pb, t` de una serie de retornos diarios.
fn line(r: &[f64]) -> String {
    let t = stats::t_test_greater(r).map_or(f64::NAN, |t| t.t);
    format!(
        "{:>5} días, media {:+.3} pb, t = {:+.2}",
        r.len(),
        1e4 * mean(r),
        t
    )
}

/// Primer inicio de mes UTC en o después de `t`.
fn month_ceil(t: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let m = Utc
        .with_ymd_and_hms(t.year(), t.month(), 1, 0, 0, 0)
        .single()?;
    if m == t {
        Some(m)
    } else {
        m.checked_add_months(Months::new(1))
    }
}

struct Blend {
    hold: Vec<f64>,
    blend: Vec<f64>,
    hold_stress: Vec<f64>,
    blend_stress: Vec<f64>,
    scales: Vec<f64>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let equity: Decimal = take(&mut args, "--capital")
        .as_deref()
        .unwrap_or("100000")
        .parse()?;
    let from: DateTime<Utc> = "2011-01-01T00:00:00Z".parse()?;
    let research = ResearchConfig::from_toml(cfg!("research.toml"))?;
    let rc = research.review_checks;
    let vcfg = research.vol_managed;
    let ocfg = research.overnight_drift;
    let stress = research.rule_round3.cost_stress;
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
    let mut blends: Vec<(InstrumentId, Blend)> = Vec::new();
    for id in &instruments {
        let l = load(id, &args, from, equity, &sources, &store, &split_cfg).await?;
        let p = &l.p;
        let session = p.calendar.session_for(id).ok_or("instrumento sin bolsa")?;
        let rolls = p
            .setup
            .futures_basis
            .as_ref()
            .map(|b| b.rolls.clone())
            .ok_or("sin vencimientos: corre sobre futuros")?;
        if months.is_empty() {
            months = l.months.clone();
        } else if months != l.months {
            return Err(format!("{id}: los meses de validación no coinciden").into());
        }
        let lo = l.split.windows[0].validation.start.date_naive();
        let hi = l.split.out_of_sample.start.date_naive();
        let closes = session_close_indices(&p.m15, session);

        // ── 1. Deriva nocturna por mitades y por año ──
        let drift: Vec<(NaiveDate, Decimal)> =
            window_returns(&p.m15, &rolls, ocfg.timezone, ocfg.from, ocfg.to)
                .into_iter()
                .filter(|(d, _)| *d >= lo && *d < hi)
                .collect();
        let pick = |keep: &dyn Fn(NaiveDate) -> bool| -> Vec<f64> {
            drift
                .iter()
                .filter(|(d, _)| keep(*d))
                .map(|(_, r)| f(*r))
                .collect()
        };
        println!(
            "\n════ 1. Deriva nocturna {}–{} ET, {id} ════",
            ocfg.from, ocfg.to
        );
        println!("  todo {lo} → {hi}: {}", line(&pick(&|_| true)));
        println!(
            "  antes de {}: {}",
            rc.drift_split_from,
            line(&pick(&|d| d < rc.drift_split_from))
        );
        println!(
            "  desde {}:  {}",
            rc.drift_split_from,
            line(&pick(&|d| d >= rc.drift_split_from))
        );
        for y in lo.year()..=hi.year() {
            let r = pick(&|d| d.year() == y);
            if !r.is_empty() {
                println!("    {y}: {}", line(&r));
            }
        }

        // ── 2. Después de un cierre de contado con caída fuerte ──
        let daily = session_close_returns(&p.m15, &rolls, &closes);
        let rets: Vec<Decimal> = daily.iter().map(|(_, r)| *r).collect();
        let flags = strong_drop_flags(&rets, rc.drop_lookback_days, rc.drop_quantile);
        let marked: Vec<(DateTime<Utc>, Option<bool>)> =
            daily.iter().map(|(t, _)| *t).zip(flags).collect();
        let cond = drift_after_close(&drift, ocfg.timezone, ocfg.from, &marked);
        let part = |strong: bool, keep: &dyn Fn(NaiveDate) -> bool| -> Vec<f64> {
            cond.iter()
                .filter(|(d, _, s)| *s == strong && keep(*d))
                .map(|(_, r, _)| f(*r))
                .collect()
        };
        println!(
            "\n════ 2. Deriva después de una caída fuerte (< percentil {} de {} días), {id} ════",
            rc.drop_quantile, rc.drop_lookback_days
        );
        for (name, keep) in [
            ("todo", &(|_: NaiveDate| true) as &dyn Fn(NaiveDate) -> bool),
            ("2013–2020", &|d: NaiveDate| d < rc.drift_split_from),
            ("2021–2025", &|d: NaiveDate| d >= rc.drift_split_from),
        ] {
            println!("  {name}: tras caída fuerte {}", line(&part(true, keep)));
            println!("  {name}: resto            {}", line(&part(false, keep)));
        }

        // ── 3. 50 % comprar y mantener + 50 % V escalada ──
        let reviews: Vec<usize> = monthly_reviews(&p.m15, session, &p.entries)
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        let rv = review_variances(&p.m15, &rolls, &reviews, &closes);
        let mut maps: [BTreeMap<(i32, u32), Decimal>; 4] = Default::default();
        let mut scales = Vec::new();
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
                continue;
            };
            let schedule: Vec<(usize, Decimal)> = reviews
                .iter()
                .zip(vol_weights(&rv, c, vcfg.cap))
                .filter_map(|(&i, w)| w.map(|w| (i, w)))
                .collect();
            let Some(&(first, _)) = schedule.first() else {
                continue;
            };
            // Entrenamiento: desde el primer mes con una decisión de V antes, para V y para
            // comprar y mantener (peso 1 en esa misma decisión, sin rebalanceos).
            let Some(t0) = month_ceil(p.m15[first].end()).map(|t| t.max(w.train.start)) else {
                continue;
            };
            let train = (t0, w.train.end);
            let tv = managed_returns(
                &p.m15,
                session,
                &rolls,
                &p.setup,
                train,
                &schedule,
                Decimal::ONE,
            )
            .map_err(|e| format!("{e:?}"))?;
            let th = managed_returns(
                &p.m15,
                session,
                &rolls,
                &p.setup,
                train,
                &[(first, Decimal::ONE)],
                Decimal::ONE,
            )
            .map_err(|e| format!("{e:?}"))?;
            let tv: Vec<Decimal> = tv.into_iter().map(|(_, r)| r).collect();
            let th: Vec<Decimal> = th.into_iter().map(|(_, r)| r).collect();
            let Some(scale) = vol_match_scale(&th, &tv) else {
                continue;
            };
            scales.push(f(scale));
            let hold: Vec<(usize, Decimal)> = reviews
                .iter()
                .filter(|&&i| p.m15[i].end() <= w.validation.start)
                .map(|&i| (i, Decimal::ONE))
                .collect();
            for s in [Decimal::ONE, stress] {
                let range = (w.validation.start, w.validation.end);
                let v = managed_returns(&p.m15, session, &rolls, &p.setup, range, &schedule, s)
                    .map_err(|e| format!("{e:?}"))?;
                let h = managed_returns(&p.m15, session, &rolls, &p.setup, range, &hold, s)
                    .map_err(|e| format!("{e:?}"))?;
                let k = usize::from(s != Decimal::ONE) * 2;
                for ((m, vr), (_, hr)) in v.into_iter().zip(h) {
                    maps[k].insert(m, hr);
                    maps[k + 1].insert(m, blend_return(hr, vr, rc.blend_hold_weight, scale));
                }
            }
        }
        let series = |m: &BTreeMap<(i32, u32), Decimal>| -> Vec<f64> {
            months
                .iter()
                .map(|k| m.get(k).copied().map_or(0.0, f))
                .collect()
        };
        let missing = months.iter().filter(|m| !maps[1].contains_key(m)).count();
        if missing > 0 {
            println!("⚠️  {id}: cartera mezclada sin valor en {missing} meses (cuentan como 0)");
        }
        blends.push((
            id.clone(),
            Blend {
                hold: series(&maps[0]),
                blend: series(&maps[1]),
                hold_stress: series(&maps[2]),
                blend_stress: series(&maps[3]),
                scales,
            },
        ));

        // ── 5. B antes de costos ──
        let atr15 = wilder_atr(&p.m15, research.intraday_momentum.atr_period);
        let plans = intraday_momentum_plans(
            &p.m15,
            session,
            &p.entries,
            &atr15,
            &rolls,
            &research.intraday_momentum,
        );
        let b = run_family(&plans, &l, research.rule_round2.cost_stress)?;
        let gross: Vec<f64> = b.trades.iter().map(|t| f(t.mid_pnl / t.risk)).collect();
        let net: Vec<f64> = b.trades.iter().map(|t| f(t.net_pnl / t.risk)).collect();
        let cost: Vec<f64> = b
            .trades
            .iter()
            .map(|t| f(t.total_costs() / t.risk))
            .collect();
        let tg = stats::t_test_greater(&gross);
        let tn = stats::t_test_greater(&net);
        let tm = stats::t_test_greater(&b.monthly);
        println!("\n════ 5. B (momentum de la última media hora), {id} ════");
        println!(
            "  {} operaciones: R antes de costos {:+.4} (desvío {:.3}, t = {:+.2}); costo {:.4} R; \
             R neto {:+.4} (desvío {:.3}, t = {:+.2}); retorno mensual {:+.3} % (desvío {:.3} %, \
             t = {:+.2}, {} meses)",
            gross.len(),
            mean(&gross),
            tg.map_or(f64::NAN, |t| t.std_dev),
            tg.map_or(f64::NAN, |t| t.t),
            mean(&cost),
            mean(&net),
            tn.map_or(f64::NAN, |t| t.std_dev),
            tn.map_or(f64::NAN, |t| t.t),
            100.0 * mean(&b.monthly),
            100.0 * tm.map_or(f64::NAN, |t| t.std_dev),
            tm.map_or(f64::NAN, |t| t.t),
            b.monthly.len()
        );
    }

    println!("\n════ 3. 50 % comprar y mantener + 50 % V escalada (descriptivo) ════");
    for (id, b) in &blends {
        println!(
            "  {id}: escala media {:.3} (mín {:.3}, máx {:.3}, {} ventanas); comprar y mantener \
             {:+.4} % (Sharpe {:.4}); mezcla {:+.4} % (Sharpe {:.4}); × {stress}: {:.4} / {:.4}",
            mean(&b.scales),
            b.scales.iter().copied().fold(f64::INFINITY, f64::min),
            b.scales.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            b.scales.len(),
            100.0 * mean(&b.hold),
            sharpe(&b.hold),
            100.0 * mean(&b.blend),
            sharpe(&b.blend),
            sharpe(&b.hold_stress),
            sharpe(&b.blend_stress),
        );
    }
    let avg = |pick: &dyn Fn(&Blend) -> &Vec<f64>| -> Vec<f64> {
        (0..months.len())
            .map(|k| mean(&blends.iter().map(|(_, b)| pick(b)[k]).collect::<Vec<_>>()))
            .collect()
    };
    let (h, m) = (avg(&|b| &b.hold), avg(&|b| &b.blend));
    let (hs, ms) = (avg(&|b| &b.hold_stress), avg(&|b| &b.blend_stress));
    let diff: Vec<f64> = m.iter().zip(&h).map(|(a, b)| a - b).collect();
    println!(
        "  cartera ({} meses): comprar y mantener {:+.4} % por mes, desvío {:.4} %, Sharpe {:.4} \
         (anual ≈ {:.2}); mezcla {:+.4} %, desvío {:.4} %, Sharpe {:.4} (anual ≈ {:.2}); \
         diferencia media {:+.4} % por mes (t = {:+.2}); con costos × {stress}: Sharpe {:.4} \
         frente a {:.4}",
        months.len(),
        100.0 * mean(&h),
        100.0 * stats::std_dev(&h).unwrap_or(f64::NAN),
        sharpe(&h),
        sharpe(&h) * 12f64.sqrt(),
        100.0 * mean(&m),
        100.0 * stats::std_dev(&m).unwrap_or(f64::NAN),
        sharpe(&m),
        sharpe(&m) * 12f64.sqrt(),
        100.0 * mean(&diff),
        stats::t_test_greater(&diff).map_or(f64::NAN, |t| t.t),
        sharpe(&hs),
        sharpe(&ms),
    );
    println!("fuera de muestra: NO se cargó ni se evaluó (#512)");
    Ok(())
}

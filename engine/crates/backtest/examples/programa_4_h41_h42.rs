//! Programa 4 (#561): H4.1 y H4.2, estrategia A de Bruni en el CFD US100, con la regla congelada
//! en `docs/analysis/2026-11-programa-4-preregistro.md` (SHA-256 en el `.sha256` de al lado).
//! Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example programa_4_h41_h42 -- \
//!     US100 --escenario-financiacion EUR=<archivo.csv> [--salida <retornos.csv>] \
//!     [--operaciones <operaciones.csv>]
//! ```
//! Velas solo hasta el inicio del fuera de muestra. Cada ventana de validación con una cuenta
//! nueva (`run_plans`); comprar y mantener con `managed_returns` y una cuenta nueva por ventana,
//! igual que el programa Nasdaq. Primero imprime las señales por año; después los resultados.
#![allow(
    clippy::float_arithmetic,
    reason = "solo estadísticos y porcentajes para mostrar; el dinero se calcula en Decimal"
)]

#[path = "common/alpha_verdict.rs"]
mod alpha_verdict;
#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/program4.rs"]
mod program4;
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use std::collections::BTreeMap;

use alpha_verdict::{Series, verdict};
use cfds_backtest::research::program4::{
    luck_test, outcome_after, rolling_after_loss_below, target_first, trade_net_r,
};
use cfds_backtest::research::{ResearchConfig, StudyExit, managed_returns};
use cfds_core::InstrumentId;
use cfds_market_data::{CandleSources, FuturesFeeds, SplitConfig};
use cfds_storage::PgStore;
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, TimeDelta, Utc};
use research_round::{describe, f, load, mean, returns_csv, run_family, trades_csv};
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

fn pct(a: usize, n: usize) -> String {
    if n == 0 {
        "-".to_owned()
    } else {
        format!("{:.1} % ({a} de {n})", 100.0 * a as f64 / n as f64)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let out_csv = take(&mut args, "--salida");
    let trades_out = take(&mut args, "--operaciones");
    let from: DateTime<Utc> = "2011-01-01T00:00:00Z".parse()?;
    let research = ResearchConfig::from_toml(cfg!("research.toml"))?;
    let (rule, cfg) = (research.rule_program4, research.program4);
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let feeds = FuturesFeeds::from_toml(cfg!("databento.toml"))?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let id: InstrumentId = args
        .first()
        .ok_or("uso: US100 --escenario-financiacion <ETIQUETA>=<csv>")?
        .parse()?;
    let equity = Decimal::from(100_000);
    let l = load(&id, &args, from, equity, &sources, &store, &split_cfg).await?;
    let p = &l.p;
    let session = p.calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let (lo, hi) = (
        l.split.windows[0].validation.start,
        l.split.out_of_sample.start,
    );
    if l.months.len() != 153 {
        return Err(format!("{id}: {} meses de validación, no 153", l.months.len()).into());
    }
    let inputs = program4::inputs(p, session, &cfg, &store, hi).await?;
    let plans = program4::plans(p, session, &inputs, &cfg)?;
    let names = ["H4.1", "H4.2"];

    // 1. Señales por año, antes de calcular retornos (§11).
    println!("══ señales por año (decisión en [{lo}, {hi}))");
    for (name, ps) in names.iter().zip(&plans) {
        let mut by_year: BTreeMap<i32, usize> = BTreeMap::new();
        for x in ps {
            let at = p.m15[x.signal_index].end();
            if at >= lo && at < hi {
                *by_year.entry(at.year()).or_default() += 1;
            }
        }
        println!(
            "  {name}: {} en total; por año {:?}",
            by_year.values().sum::<usize>(),
            by_year
        );
    }

    // 2. Comprar y mantener: cuenta nueva por ventana, costos × 1, × 1,5 y × 2.
    let rolls = &inputs.rolls;
    let stresses = [Decimal::ONE, rule.cost_stress, Decimal::TWO];
    let mut xs: [BTreeMap<(i32, u32), Decimal>; 3] = Default::default();
    let hold = [(0usize, Decimal::ONE)];
    for w in &l.split.windows {
        for (k, stress) in stresses.into_iter().enumerate() {
            for (m, r) in managed_returns(
                &p.m15,
                session,
                rolls,
                &p.setup,
                (w.validation.start, w.validation.end),
                &hold,
                stress,
            )
            .map_err(|e| format!("{e:?}"))?
            {
                xs[k].insert(m, r);
            }
        }
    }
    let x_of = |k: usize| -> Result<Vec<f64>, String> {
        l.months
            .iter()
            .map(|m| xs[k].get(m).copied().map(f).ok_or(format!("falta {m:?}")))
            .collect()
    };
    let (x, x_stress, x_double) = (x_of(0)?, x_of(1)?, x_of(2)?);

    let feed = feeds
        .get(&id)
        .ok_or("sin futuro en config/databento.toml")?;
    let mut results = Vec::new();
    for (name, ps) in names.iter().zip(&plans) {
        let r = run_family(ps, &l, rule.cost_stress)?;
        let double = run_family(ps, &l, Decimal::TWO)?;
        println!();
        println!("════════ {name} en {id} ════════");
        describe(&id, &r, equity);
        let longs = r.trades.iter().filter(|t| t.side == Side::Long).count();
        println!(
            "    no operadas porque la entrada abrió del lado del stop: {}",
            r.through_stop
        );
        let mut per_year: BTreeMap<i32, usize> = BTreeMap::new();
        for t in &r.trades {
            *per_year.entry(t.entry_time.year()).or_default() += 1;
        }
        println!("    operaciones por año: {per_year:?}");
        let mut exits: BTreeMap<String, usize> = BTreeMap::new();
        for t in &r.trades {
            *exits.entry(format!("{:?}", t.exit)).or_default() += 1;
        }
        println!("    salidas por motivo: {exits:?}");
        // Velas que tocaron stop y objetivo: resueltas con barras de 1 minuto (descriptivo).
        let (mut ambiguous, mut to_target, mut unresolved) = (0, 0, 0);
        for t in &r.trades {
            let (StudyExit::Stop, Some(target)) = (t.exit, t.target) else {
                continue;
            };
            let c = &p.m15[t.exit_index];
            let touched = match t.side {
                Side::Long => c.high >= target,
                Side::Short => c.low <= target,
            };
            if !touched {
                continue;
            }
            ambiguous += 1;
            let bars = store
                .futures_bars(
                    &sources.futures_source,
                    &feed.dataset,
                    &feed.symbol,
                    &id,
                    c.start,
                    c.start + TimeDelta::minutes(15),
                )
                .await?;
            let hl: Vec<(Decimal, Decimal)> = bars.iter().map(|b| (b.high, b.low)).collect();
            match target_first(t.side, t.stop, target, &hl) {
                Some(true) => to_target += 1,
                Some(false) => {}
                None => unresolved += 1,
            }
        }
        println!(
            "    velas con stop y objetivo: {ambiguous} (contadas como stop); con barras de 1 \
             minuto, el objetivo fue primero en {to_target} y no se puede saber en {unresolved}"
        );
        let wins: Vec<bool> = r.trades.iter().map(|t| t.net_pnl > Decimal::ZERO).collect();
        let [all, after_win, after_loss] = outcome_after(&wins);
        let (below, windows) = rolling_after_loss_below(&wins, 100);
        println!(
            "    autocorrelación: aciertos {} en total, {} tras una ganadora, {} tras una \
             perdedora; en ventanas de 100 operaciones, tras una perdedora fue menor en {}",
            pct(all.0, all.1),
            pct(after_win.0, after_win.1),
            pct(after_loss.0, after_loss.1),
            pct(below, windows)
        );
        let luck = luck_test(
            &p.m15,
            session,
            &p.entries,
            &inputs.excluded,
            &p.force_close,
            &cfg,
            &r.trades,
            &p.setup,
            Decimal::new(1, 2),
        );
        let (pr, pt) = luck.percentiles();
        let sims_r: Vec<f64> = luck.simulations.iter().map(|s| f(s.0)).collect();
        println!(
            "    test de suerte ({} simulaciones, semilla {}): R neto medio {:+.4} (percentil {}, \
             media de las simulaciones {:+.4}); retorno total con 1 % por operación {:+.2} % \
             (percentil {})",
            luck.simulations.len(),
            cfg.luck_seed,
            f(luck.strategy.0),
            pr.round_dp(1),
            mean(&sims_r),
            100.0 * f(luck.strategy.1),
            pt.round_dp(1)
        );
        let r_net: Vec<f64> = r.trades.iter().filter_map(trade_net_r).map(f).collect();
        println!(
            "    R neto medio (precios ejecutados, sin financiación) {:+.4}; {} largas y {} cortas",
            mean(&r_net),
            longs,
            r.trades.len() - longs
        );
        let s = Series {
            y: r.monthly.clone(),
            y_stress: r.monthly_stress.clone(),
            y_double: double.monthly_stress.clone(),
            x: x.clone(),
            x_stress: x_stress.clone(),
            x_double: x_double.clone(),
        };
        let five = verdict(name, &rule, &l.months, &s, true);
        let enough = r.trades.len() >= cfg.min_trades;
        println!(
            "  6. mínimo de operaciones: {} (mínimo {}) → {}",
            r.trades.len(),
            cfg.min_trades,
            if enough { "PASA" } else { "no pasa" }
        );
        println!(
            "  VEREDICTO de {name} con este escenario: {}",
            if five && enough {
                "PASA los 5 criterios y el mínimo de operaciones"
            } else {
                "NO PASA"
            }
        );
        results.push(vec![(id.clone(), r)]);
    }
    if let Some(path) = out_csv {
        std::fs::write(&path, returns_csv(&names, &results, &l.months))?;
        println!("retornos mensuales → {path}");
    }
    if let Some(path) = trades_out {
        std::fs::write(&path, trades_csv(&names, &results))?;
        println!("operaciones → {path}");
    }
    Ok(())
}

//! Programa 4 (#561): H4.3, estrategia C ("de campeonato") de Bruni en el CFD US100, con la regla
//! congelada en `docs/analysis/2026-11-programa-4-preregistro.md` (SHA-256 en el `.sha256` de al
//! lado). Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example programa_4_h43 -- \
//!     US100 --escenario-financiacion EUR=<archivo.csv> [--salida <retornos.csv>] \
//!     [--operaciones <operaciones.csv>]
//! ```
//! Velas y barras de 1 minuto solo hasta el inicio del fuera de muestra; calendario oficial
//! publicado antes de ese inicio. Cada ventana de validación con una cuenta nueva (`run_plans`,
//! igual que H4.1 y H4.2); comprar y mantener con `managed_returns`. Primero imprime las señales
//! por año; después los resultados.
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

use std::collections::{BTreeMap, BTreeSet};

use alpha_verdict::{Series, verdict};
use cfds_backtest::research::program4::{
    luck_test, outcome_after, rolling_after_loss_below, structure, target_first, trade_net_r,
};
use cfds_backtest::research::program4_c::{
    ChampionshipInputs, ProfileTracker, atr_percentile, championship_plans, regime_band,
    release_dates,
};
use cfds_backtest::research::{ResearchConfig, StudyExit, managed_returns};
use cfds_core::InstrumentId;
use cfds_market_data::{CandleSources, FuturesFeeds, SplitConfig};
use cfds_storage::PgStore;
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, Months, TimeDelta, Utc};
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
    let (rule, cfg4, cfg) = (
        research.rule_program4,
        research.program4,
        research.program4_c,
    );
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
    let inputs = program4::inputs(p, session, &cfg4, &store, hi).await?;
    let rolls = &inputs.rolls;
    let (breakouts, states) = structure(&p.m15, rolls, cfg4.min_valid_in_zone);

    // Días sin señales: roll y datos fuertes (FOMC, CPI y empleo, decisión 11).
    let mut scheduled = Vec::new();
    for (source, series) in &cfg.strong_releases {
        let rows = store.official_releases(source, series, hi).await?;
        if rows.is_empty() {
            return Err(format!("official_releases sin filas de {source}/{series}").into());
        }
        let times: Vec<DateTime<Utc>> = rows
            .iter()
            .filter(|r| r.status == "scheduled" || r.status == "rescheduled")
            .filter_map(|r| r.scheduled_at)
            .collect();
        println!("  {source}/{series}: {} fechas programadas", times.len());
        scheduled.extend(times);
    }
    let strong = release_dates(session, &scheduled);
    let excluded: BTreeSet<_> = inputs.roll_days.union(&strong).copied().collect();
    println!(
        "días sin señales: {} de roll y {} con dato fuerte ({} en total)",
        inputs.roll_days.len(),
        strong.len(),
        excluded.len()
    );

    // Perfiles: barras de 1 minuto del NQ por meses, desde unos días antes de la validación y
    // solo hasta el inicio del fuera de muestra.
    let feed = feeds
        .get(&id)
        .ok_or("sin futuro en config/databento.toml")?;
    let mut tracker = ProfileTracker::new(&cfg, rolls);
    let mut contexts = vec![None; p.m15.len()];
    let bars_from = lo - TimeDelta::days(10);
    let first = p.m15.partition_point(|c| c.start < bars_from);
    let mut chunk_from = bars_from;
    let mut chunk: Vec<cfds_core::FuturesBar> = Vec::new();
    let mut next = 0usize;
    let mut total_bars = 0usize;
    for (t, c) in p.m15.iter().enumerate().skip(first) {
        if c.end() > hi {
            break;
        }
        loop {
            while let Some(b) = chunk.get(next).filter(|b| b.start < c.end()) {
                tracker.push(b);
                next += 1;
            }
            if next < chunk.len() || chunk_from >= hi {
                break;
            }
            let to = (chunk_from + Months::new(1)).min(hi);
            chunk = store
                .futures_bars(
                    &sources.futures_source,
                    &feed.dataset,
                    &feed.symbol,
                    &id,
                    chunk_from,
                    to,
                )
                .await?;
            total_bars += chunk.len();
            next = 0;
            chunk_from = to;
        }
        contexts[t] = tracker.context(c);
    }
    let with_context = contexts.iter().filter(|x| x.is_some()).count();
    let sessions = tracker.closed.len();
    let full = tracker
        .closed
        .iter()
        .filter(|(_, n, _)| {
            Decimal::from(*n)
                >= (cfg.profile_min_bars_fraction
                    * Decimal::from(cfds_backtest::research::program4_c::session_minutes(&cfg)))
                .ceil()
        })
        .count();
    println!(
        "barras de 1 minuto: {total_bars}; sesiones del CME cerradas: {sessions}, con al menos el \
         {} % de sus barras: {full}; velas de 15 m con perfil anterior y en desarrollo: \
         {with_context}",
        cfg.profile_min_bars_fraction * Decimal::ONE_HUNDRED
    );
    let mut full_by_year: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
    for (d, n, _) in &tracker.closed {
        let e = full_by_year.entry(d.year()).or_default();
        e.1 += 1;
        if Decimal::from(*n)
            >= (cfg.profile_min_bars_fraction
                * Decimal::from(cfds_backtest::research::program4_c::session_minutes(&cfg)))
            .ceil()
        {
            e.0 += 1;
        }
    }
    println!("  sesiones con perfil por año (con perfil, total): {full_by_year:?}");

    let ci = ChampionshipInputs {
        m15: &p.m15,
        session,
        entries: &p.entries,
        atr: &inputs.atr,
        breakouts: &breakouts,
        states: &states,
        rolls,
        contexts: &contexts,
        excluded: &excluded,
    };
    let (plans, infos, funnel) = championship_plans(&ci, &cfg4, &cfg);

    // 1. Señales por año, antes de calcular retornos (§11).
    let in_period = |i: usize| {
        let at = p.m15[i].end();
        at >= lo && at < hi
    };
    let mut by_year: BTreeMap<i32, usize> = BTreeMap::new();
    let (mut firsts, mut reentries) = (0, 0);
    for (x, info) in plans.iter().zip(&infos) {
        if in_period(x.signal_index) {
            *by_year
                .entry(p.m15[x.signal_index].end().year())
                .or_default() += 1;
            if info.first {
                firsts += 1;
            } else {
                reentries += 1;
            }
        }
    }
    println!(
        "══ señales de H4.3 (decisión en [{lo}, {hi})): {} en total ({firsts} primeras, \
         {reentries} reentradas); por año {by_year:?}",
        by_year.values().sum::<usize>()
    );
    println!("   embudo (velas × lado, toda la serie): {funnel:?}");

    // 2. Comprar y mantener: cuenta nueva por ventana, costos × 1, × 1,5 y × 2.
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

    let name = "H4.3";
    let r = run_family(&plans, &l, rule.cost_stress)?;
    let double = run_family(&plans, &l, Decimal::TWO)?;
    println!();
    println!("════════ {name} en {id} ════════");
    describe(&id, &r, equity);
    let longs = r.trades.iter().filter(|t| t.side == Side::Long).count();
    println!(
        "    no operadas porque la entrada abrió del lado del stop: {}; porque abrió en el \
         extremo de la estructura (sin recorrido): {}",
        r.through_stop, r.through_cap
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
    // Primeras y reentradas, objetivos recortados, régimen.
    let info_of: BTreeMap<usize, _> = infos.iter().map(|i| (i.signal_index, *i)).collect();
    let net_r = |t: &cfds_backtest::research::StudyTrade| f(t.net_pnl / t.risk);
    let (mut r_first, mut r_re) = (Vec::new(), Vec::new());
    let mut capped = 0usize;
    let mut by_regime: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    for t in &r.trades {
        let Some(info) = info_of.get(&t.signal_index) else {
            continue;
        };
        if info.first {
            r_first.push(net_r(t));
        } else {
            r_re.push(net_r(t));
        }
        let plain = match t.side {
            Side::Long => t.entry_mid + cfg.target_r * (t.entry_mid - t.stop).abs(),
            Side::Short => t.entry_mid - cfg.target_r * (t.entry_mid - t.stop).abs(),
        };
        if t.target.is_some_and(|x| x != plain) {
            capped += 1;
        }
        let band = atr_percentile(
            &inputs.atr,
            &p.m15,
            rolls,
            cfg.regime_window,
            t.signal_index,
        )
        .map_or(usize::MAX, |pc| regime_band(pc, &cfg.regime_bands));
        by_regime.entry(band).or_default().push(net_r(t));
    }
    println!(
        "    primeras: {} (R neto medio {:+.4}); reentradas: {} ({:+.4}); objetivos recortados \
         por el extremo de la estructura: {capped}",
        r_first.len(),
        mean(&r_first),
        r_re.len(),
        mean(&r_re)
    );
    let band_name = ["bajo", "medio", "alto", "extremo"];
    for (b, rs) in &by_regime {
        println!(
            "    régimen {}: {} operaciones, R neto medio {:+.4}",
            band_name.get(*b).copied().unwrap_or("sin percentil"),
            rs.len(),
            mean(rs)
        );
    }
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
        "    velas con stop y objetivo: {ambiguous} (contadas como stop); con barras de 1 minuto, \
         el objetivo fue primero en {to_target} y no se puede saber en {unresolved}"
    );
    let wins: Vec<bool> = r.trades.iter().map(|t| t.net_pnl > Decimal::ZERO).collect();
    let [all, after_win, after_loss] = outcome_after(&wins);
    let (below, windows) = rolling_after_loss_below(&wins, 100);
    println!(
        "    autocorrelación: aciertos {} en total, {} tras una ganadora, {} tras una perdedora; \
         en ventanas de 100 operaciones, tras una perdedora fue menor en {}",
        pct(all.0, all.1),
        pct(after_win.0, after_win.1),
        pct(after_loss.0, after_loss.1),
        pct(below, windows)
    );
    // Test de suerte con el objetivo de H4.3 (2 R), los mismos días excluidos.
    let mut luck_cfg = cfg4.clone();
    luck_cfg.target_r = cfg.target_r;
    let luck = luck_test(
        &p.m15,
        session,
        &p.entries,
        &excluded,
        &p.force_close,
        &luck_cfg,
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
        cfg4.luck_seed,
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
        x,
        x_stress,
        x_double,
    };
    let five = verdict(name, &rule, &l.months, &s, true);
    let enough = r.trades.len() >= cfg4.min_trades;
    println!(
        "  6. mínimo de operaciones: {} (mínimo {}) → {}",
        r.trades.len(),
        cfg4.min_trades,
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
    let results = vec![vec![(id.clone(), r)]];
    if let Some(path) = out_csv {
        std::fs::write(&path, returns_csv(&[name], &results, &l.months))?;
        println!("retornos mensuales → {path}");
    }
    if let Some(path) = trades_out {
        std::fs::write(&path, trades_csv(&[name], &results))?;
        println!("operaciones → {path}");
    }
    Ok(())
}

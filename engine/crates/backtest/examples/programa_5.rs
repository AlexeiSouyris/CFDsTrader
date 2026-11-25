//! Programa 5 (#649, `docs/analysis/2026-11-programa-5-fvg-preregistro.md`, congelado): H5.1
//! (reacción al primer toque de un FVG frente a un placebo emparejado) y H5.2 (operaciones de la
//! estrategia en uso que coinciden con un primer toque a favor frente al resto). Estudio: lee la
//! base en solo lectura, no carga el fuera de muestra y no guarda nada.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example programa_5 -- h51
//! cargo run --release -p cfds-backtest --example programa_5 -- h52 \
//!     --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv
//! ```
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de una muestra (medias, pruebas t, percentiles): no son dinero"
)]

#[path = "common/alpha_verdict.rs"]
mod alpha_verdict;
#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/program5.rs"]
mod program5;
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use cfds_backtest::research::program4::SplitMix64;
use cfds_backtest::research::program5::{
    PlaceboInputs, Placebos, RealEvent, draw_placebos, in_cash_session, is_fvg_trade, month_of,
    placebo_candidates, placebo_reaction, sample_without_replacement, touch_closes,
};
use cfds_backtest::research::{
    Program5Config, ResearchConfig, RuleConfig, managed_returns, monthly_returns, stats,
};
use cfds_backtest::{BacktestConfig, EvaluationConfig, Inputs, TradeRecord, evaluate_period_with};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::{wilder_atr, wilson};
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{CandleSources, Catalog, ExchangeCalendar, Session, SplitConfig};
use cfds_storage::{CandleFeed, PgStore};
use cfds_strategy::{Component, Side};
use chrono::{DateTime, Datelike, Utc};
use research_round::{Loaded, f, mean};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn load_from() -> DateTime<Utc> {
    "2011-01-01T00:00:00Z".parse().unwrap_or_default()
}

async fn read_only_store() -> Res<PgStore> {
    let url = std::env::var("DATABASE_URL")?;
    let sep = if url.contains('?') { '&' } else { '?' };
    Ok(PgStore::connect(&format!(
        "{url}{sep}options=-c%20default_transaction_read_only%3Don"
    ))
    .await?)
}

/// Inicio de la validación y del fuera de muestra (sin cargar ninguna vela de él).
async fn period(
    store: &PgStore,
    sources: &CandleSources,
    id: &InstrumentId,
    split_cfg: &SplitConfig,
) -> Res<(DateTime<Utc>, DateTime<Utc>)> {
    let last = store
        .last_candle_start_from(
            CandleFeed::Futures(&sources.futures_source),
            id,
            Timeframe::M15,
            Utc::now(),
        )
        .await?
        .ok_or("sin velas de futuros")?;
    // Como `research_round::load`: la separación parte de la primera vela cargada (no del
    // pedido), así las ventanas son las mismas que en H5.2 y en los programas anteriores.
    let first = store
        .futures_candles(
            &sources.futures_source,
            id,
            Timeframe::M15,
            load_from(),
            load_from() + chrono::TimeDelta::days(31),
        )
        .await?
        .first()
        .map(|c| c.start)
        .ok_or("sin velas al inicio")?;
    let split = cfds_market_data::plan(first, last, split_cfg)?;
    Ok((split.windows[0].validation.start, split.out_of_sample.start))
}

fn months_between(lo: DateTime<Utc>, hi: DateTime<Utc>) -> Vec<(i32, u32)> {
    let mut out = Vec::new();
    let mut d = lo.date_naive();
    while d < hi.date_naive() {
        out.push((d.year(), d.month()));
        match d.checked_add_months(chrono::Months::new(1)) {
            Some(n) => d = n,
            None => break,
        }
    }
    out
}

fn timeframe(s: &str) -> Res<Timeframe> {
    match s {
        "15m" => Ok(Timeframe::M15),
        "1h" => Ok(Timeframe::H1),
        _ => Err(format!("timeframe no soportado: {s}").into()),
    }
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let i = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

fn summary(xs: &[f64]) -> String {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    format!(
        "media {:+.4}, mediana {:+.4}, RIC [{:+.4}, {:+.4}] (n {})",
        mean(xs),
        quantile(&s, 0.5),
        quantile(&s, 0.25),
        quantile(&s, 0.75),
        xs.len()
    )
}

fn z() -> Decimal {
    let t: toml::Table = toml::from_str(cfg!("probability.toml")).unwrap_or_default();
    t.get("z")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(Decimal::new(196, 2))
}

fn rate(hits: usize, n: usize) -> String {
    if n == 0 {
        return "—".into();
    }
    let (lo, hi) = wilson(
        u32::try_from(hits).unwrap_or(u32::MAX),
        u32::try_from(n).unwrap_or(u32::MAX),
        z(),
    );
    format!(
        "{:.1} % [{:.1}, {:.1}]",
        100.0 * hits as f64 / n as f64,
        100.0 * f(lo),
        100.0 * f(hi)
    )
}

fn side_name(s: Side) -> &'static str {
    match s {
        Side::Long => "alcista",
        Side::Short => "bajista",
    }
}

// ───────────────────────────── H5.1 ─────────────────────────────

/// Un instrumento y timeframe preparados para el sorteo.
struct Prepared51 {
    id: InstrumentId,
    s: program5::Series,
    atr: Vec<Option<Decimal>>,
    cash: Vec<bool>,
    placebos: Placebos,
    /// Reacción a favor de un alcista por vela (en ATR[s − 1]), para `horizon` y `consistency`.
    react: [Vec<Option<f64>>; 2],
}

impl Prepared51 {
    fn placebo(&self, s: u32, side: Side, h: usize) -> f64 {
        let r = self.react[h][s as usize].unwrap_or(f64::NAN);
        match side {
            Side::Long => r,
            Side::Short => -r,
        }
    }
}

/// Un evento con sus reacciones reales y las de sus placebos sorteados.
struct Row51 {
    month: (i32, u32),
    year: i32,
    side: Side,
    cash: bool,
    wait: usize,
    size: f64,
    real: [f64; 2],
    placebo: Vec<[f64; 2]>,
    /// Reacción real con ATR[t − 1] (descriptivo).
    real_prev_atr: f64,
    /// Real − media de los placebos, en puntos (descriptivo de costos).
    diff_points: f64,
}

#[allow(clippy::too_many_arguments, reason = "orquestación de un ejemplo")]
async fn prepare51(
    store: &PgStore,
    sources: &CandleSources,
    session: &Session,
    id: &InstrumentId,
    tf: Timeframe,
    (lo, hi): (DateTime<Utc>, DateTime<Utc>),
    cfg: &Program5Config,
) -> Res<Prepared51> {
    let s = program5::series(store, sources, id, tf, load_from(), (lo, hi), cfg.horizon).await?;
    let atr_period = LiquidityConfig::from_toml(cfg!("liquidity.toml"))?.atr_period;
    let atr = wilder_atr(&s.bars, atr_period);
    let cash: Vec<bool> = s.bars.iter().map(|c| in_cash_session(session, c)).collect();
    let placebos = placebo_candidates(
        &PlaceboInputs {
            bars: &s.bars,
            contracts: &s.contracts,
            atr: &atr,
            cash: &cash,
            to: hi,
            horizon: cfg.horizon,
        },
        &s.events,
    );
    let react = [cfg.horizon, cfg.consistency_horizon].map(|k| {
        (0..s.bars.len())
            .map(|j| placebo_reaction(&s.bars, &atr, j, k, Side::Long).map(f))
            .collect()
    });
    Ok(Prepared51 {
        id: id.clone(),
        s,
        atr,
        cash,
        placebos,
        react,
    })
}

fn rows51(p: &Prepared51, draws: &[Vec<u32>], cfg: &Program5Config) -> Vec<Row51> {
    let bars = &p.s.bars;
    p.s.events
        .iter()
        .zip(draws)
        .filter(|(_, d)| !d.is_empty())
        .map(|(e, d)| {
            let t = e.touch;
            let real = [cfg.horizon, cfg.consistency_horizon]
                .map(|k| e.reaction(k as u32).map(f).unwrap_or(f64::NAN));
            let placebo: Vec<[f64; 2]> = d
                .iter()
                .map(|&s| [p.placebo(s, e.side, 0), p.placebo(s, e.side, 1)])
                .collect();
            let prev_atr = p.atr[t - 1].map(f).unwrap_or(f64::NAN);
            let atr_i = f(e.atr);
            let placebo_points = mean(
                &d.iter()
                    .map(|&s| {
                        p.placebo(s, e.side, 0) * p.atr[s as usize - 1].map(f).unwrap_or(f64::NAN)
                    })
                    .collect::<Vec<_>>(),
            );
            Row51 {
                month: month_of(&bars[t]),
                year: bars[t].start.year(),
                side: e.side,
                cash: p.cash[t],
                wait: e.wait(),
                size: f(e.size_atr),
                real,
                placebo,
                real_prev_atr: real[0] * atr_i / prev_atr,
                diff_points: real[0] * atr_i - placebo_points,
            }
        })
        .collect()
}

/// Medias mensuales de real y placebo (horizonte `h`), por mes con eventos.
fn monthly51(rows: &[Row51], h: usize) -> BTreeMap<(i32, u32), f64> {
    let mut acc: BTreeMap<(i32, u32), (f64, usize, f64, usize)> = BTreeMap::new();
    for r in rows {
        let a = acc.entry(r.month).or_default();
        a.0 += r.real[h];
        a.1 += 1;
        for p in &r.placebo {
            a.2 += p[h];
            a.3 += 1;
        }
    }
    acc.into_iter()
        .map(|(m, (s, n, ps, pn))| (m, s / n as f64 - ps / pn as f64))
        .collect()
}

/// Serie combinada `D̄_m` en los meses del período (sin los meses sin eventos).
fn combined(per: &[BTreeMap<(i32, u32), f64>], months: &[(i32, u32)]) -> Vec<((i32, u32), f64)> {
    months
        .iter()
        .filter_map(|m| {
            let xs: Vec<f64> = per.iter().filter_map(|d| d.get(m).copied()).collect();
            (!xs.is_empty()).then(|| (*m, mean(&xs)))
        })
        .collect()
}

fn half_means(series: &[((i32, u32), f64)], rule: &RuleConfig) -> (f64, f64, usize, usize) {
    let half = (rule.second_half_from.year(), rule.second_half_from.month());
    let a: Vec<f64> = series
        .iter()
        .filter(|(m, _)| *m < half)
        .map(|x| x.1)
        .collect();
    let b: Vec<f64> = series
        .iter()
        .filter(|(m, _)| *m >= half)
        .map(|x| x.1)
        .collect();
    (mean(&a), mean(&b), a.len(), b.len())
}

/// Prueba y criterios de H5.1 sobre un timeframe; devuelve si pasa (solo vota el principal).
fn verdict51(
    name: &str,
    per: &[(InstrumentId, Vec<Row51>)],
    months: &[(i32, u32)],
    rule: &RuleConfig,
    cfg: &Program5Config,
) -> bool {
    let d8: Vec<BTreeMap<(i32, u32), f64>> = per.iter().map(|(_, r)| monthly51(r, 0)).collect();
    let d4: Vec<BTreeMap<(i32, u32), f64>> = per.iter().map(|(_, r)| monthly51(r, 1)).collect();
    let c8 = combined(&d8, months);
    let c4 = combined(&d4, months);
    let xs: Vec<f64> = c8.iter().map(|x| x.1).collect();
    let alpha = f(rule.alpha);
    println!(
        "\n══ {name}: serie mensual combinada D̄ (real − placebo, r_{}, en ATR)",
        cfg.horizon
    );
    let test = stats::newey_west_t_greater(&xs, cfg.newey_west_lags);
    let c1 = test.is_some_and(|t| t.p_value < alpha);
    match test {
        Some(t) => println!(
            "  1. {} meses, media {:+.5} ATR, √S {:.5}, t (Newey-West {}) = {:.3}, p = {:.5} \
             (nivel {alpha}) → {}",
            t.n,
            t.mean,
            t.std_dev,
            cfg.newey_west_lags,
            t.t,
            t.p_value,
            if c1 { "PASA" } else { "no pasa" }
        ),
        None => println!("  1. sin prueba (S ≤ 0 o sin datos) → no pasa"),
    }
    if let Some(t) = stats::t_test_greater(&xs) {
        println!(
            "     (referencia sin Newey-West: t = {:.3}, p = {:.5})",
            t.t, t.p_value
        );
    }
    let mut c2 = true;
    for ((id, _), d) in per.iter().zip(&d8) {
        let v: Vec<f64> = d.values().copied().collect();
        let m = mean(&v);
        c2 &= m > 0.0;
        println!(
            "  2. {id}: media de D_m {:+.5} ATR en {} meses → {}",
            m,
            v.len(),
            if m > 0.0 { "> 0" } else { "≤ 0" }
        );
    }
    println!("     → {}", if c2 { "PASA" } else { "no pasa" });
    let (a, b, na, nb) = half_means(&c8, rule);
    let c3 = a > 0.0 && b > 0.0;
    println!(
        "  3. mitades: {:+.5} ({na} meses hasta 2020) y {:+.5} ({nb} meses desde 2021) → {}",
        a,
        b,
        if c3 { "PASA" } else { "no pasa" }
    );
    let m4 = mean(&c4.iter().map(|x| x.1).collect::<Vec<_>>());
    let c4ok = m4 > 0.0;
    println!(
        "  4. r_{}: media de D̄ {:+.5} ATR → {}",
        cfg.consistency_horizon,
        m4,
        if c4ok { "PASA" } else { "no pasa" }
    );
    let mut c5 = true;
    for (id, r) in per {
        c5 &= r.len() >= cfg.min_events;
        println!(
            "  5. {id}: {} eventos con placebo (mínimo {})",
            r.len(),
            cfg.min_events
        );
    }
    println!("     → {}", if c5 { "PASA" } else { "no pasa" });
    c1 && c2 && c3 && c4ok && c5
}

fn describe51(per: &[(InstrumentId, Vec<Row51>)], rule: &RuleConfig, cfg: &Program5Config) {
    let mean_placebo =
        |r: &Row51, h: usize| mean(&r.placebo.iter().map(|p| p[h]).collect::<Vec<_>>());
    for (id, rows) in per {
        println!("\n  ── {id}");
        for (h, k) in [(0usize, cfg.horizon), (1, cfg.consistency_horizon)] {
            let real: Vec<f64> = rows.iter().map(|r| r.real[h]).collect();
            let plac: Vec<f64> = rows
                .iter()
                .flat_map(|r| r.placebo.iter().map(move |p| p[h]))
                .collect();
            println!("    r_{k} real:    {}", summary(&real));
            println!("    r_{k} placebo: {}", summary(&plac));
        }
        let diff = |rs: &[&Row51]| {
            mean(
                &rs.iter()
                    .map(|r| r.real[0] - mean_placebo(r, 0))
                    .collect::<Vec<_>>(),
            )
        };
        let group = |title: &str, key: &dyn Fn(&Row51) -> String| {
            let mut g: BTreeMap<String, Vec<&Row51>> = BTreeMap::new();
            for r in rows {
                g.entry(key(r)).or_default().push(r);
            }
            let parts: Vec<String> = g
                .iter()
                .map(|(k, v)| format!("{k}: {:+.4} (n {})", diff(v), v.len()))
                .collect();
            println!(
                "    real − placebo (r_{}) por {title}: {}",
                cfg.horizon,
                parts.join("; ")
            );
        };
        let mut sizes: Vec<f64> = rows.iter().map(|r| r.size).collect();
        sizes.sort_by(f64::total_cmp);
        let (c1, c2) = (quantile(&sizes, 1.0 / 3.0), quantile(&sizes, 2.0 / 3.0));
        group(
            &format!("tercil de tamaño (cortes {c1:.3} y {c2:.3} ATR)"),
            &|r| {
                if r.size <= c1 {
                    "1 chico".into()
                } else if r.size <= c2 {
                    "2 medio".into()
                } else {
                    "3 grande".into()
                }
            },
        );
        group("dirección", &|r| side_name(r.side).into());
        group("franja", &|r| {
            if r.cash {
                "contado".into()
            } else {
                "fuera".into()
            }
        });
        group("espera", &|r| match r.wait {
            1 => "a. 1".into(),
            2..=5 => "b. 2–5".into(),
            6..=20 => "c. 6–20".into(),
            _ => "d. > 20".into(),
        });
        group("año", &|r| r.year.to_string());
        let prev: Vec<f64> = rows
            .iter()
            .map(|r| r.real_prev_atr - mean_placebo(r, 0))
            .filter(|x| x.is_finite())
            .collect();
        println!(
            "    real con ATR[t − 1] − placebo (r_{}): media {:+.4} (n {})",
            cfg.horizon,
            mean(&prev),
            prev.len()
        );
        let up_real = rows.iter().filter(|r| r.real[0] > 0.0).count();
        let plac_n: usize = rows.iter().map(|r| r.placebo.len()).sum();
        let up_plac: usize = rows
            .iter()
            .map(|r| r.placebo.iter().filter(|p| p[0] > 0.0).count())
            .sum();
        println!(
            "    r_{} > 0: real {}; placebo {}",
            cfg.horizon,
            rate(up_real, rows.len()),
            rate(up_plac, plac_n)
        );
        // Costos (descriptivo, §13.3.8).
        let half = rule.second_half_from.year();
        for (label, first) in [("hasta 2020", true), ("desde 2021", false)] {
            let pts: Vec<f64> = rows
                .iter()
                .filter(|r| (r.year < half) == first)
                .map(|r| r.diff_points)
                .filter(|x| x.is_finite())
                .collect();
            println!(
                "    diferencia real − placebo en puntos ({label}): media {:+.3} (n {})",
                mean(&pts),
                pts.len()
            );
        }
    }
}

async fn run_h51(rule: &RuleConfig, cfg: &Program5Config) -> Res<()> {
    let store = read_only_store().await?;
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let costs = BacktestConfig::from_toml(cfg!("backtest.toml"))?.costs;
    let ids: Vec<InstrumentId> = cfg
        .instruments
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;
    let mut period_of = BTreeMap::new();
    for id in &ids {
        period_of.insert(id.clone(), period(&store, &sources, id, &split_cfg).await?);
    }
    let (lo, hi) = period_of[&ids[0]];
    if period_of.values().any(|p| *p != (lo, hi)) {
        return Err("los instrumentos no tienen el mismo período".into());
    }
    let months = months_between(lo, hi);
    println!(
        "══ H5.1: toques en [{lo}, {hi}) ({} meses); fuera de muestra desde {hi}, NO se carga",
        months.len()
    );
    let mut main = Vec::new();
    let mut desc = Vec::new();
    for id in &ids {
        let session = calendar.session_for(id).ok_or("sin bolsa")?;
        for (tf, out) in [
            (timeframe(&cfg.timeframe)?, &mut main),
            (timeframe(&cfg.descriptive_timeframe)?, &mut desc),
        ] {
            let p = prepare51(&store, &sources, session, id, tf, (lo, hi), cfg).await?;
            out.push(p);
        }
    }
    // Conteos antes de calcular reacciones (§10).
    for (name, set) in [(&cfg.timeframe, &main), (&cfg.descriptive_timeframe, &desc)] {
        println!("\n══ eventos ({name}): reales, con placebo y sin placebo, por año");
        for p in set {
            let c = p.s.counts;
            let with: Vec<bool> = (0..p.s.events.len())
                .map(|e| !p.placebos.of(e).is_empty())
                .collect();
            println!(
                "  {}: primeros toques en el período {}; sin vencimiento (F1) {} (pierde {}); \
                 sin llenar en el toque (F2) {} (pierde {}); con placebo {}; sin placebo {}; \
                 candidatas por evento con placebo: media {:.1}",
                p.id,
                c.in_period,
                c.same_contract,
                c.in_period - c.same_contract,
                c.real,
                c.same_contract - c.real,
                with.iter().filter(|w| **w).count(),
                with.iter().filter(|w| !**w).count(),
                p.placebos.candidates.len() as f64
                    / with.iter().filter(|w| **w).count().max(1) as f64
            );
            let mut by: BTreeMap<(i32, &str, &str), (usize, usize)> = BTreeMap::new();
            for (e, w) in p.s.events.iter().zip(&with) {
                let k = (
                    p.s.bars[e.touch].start.year(),
                    side_name(e.side),
                    if p.cash[e.touch] { "contado" } else { "fuera" },
                );
                let x = by.entry(k).or_default();
                if *w {
                    x.0 += 1;
                } else {
                    x.1 += 1;
                }
            }
            let mut years: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
            for ((y, _, _), (a, b)) in &by {
                let x = years.entry(*y).or_default();
                x.0 += a;
                x.1 += b;
            }
            println!("    por año (con placebo, sin placebo): {years:?}");
            let mut cut: BTreeMap<(&str, &str), usize> = BTreeMap::new();
            for ((_, s, c), (a, _)) in &by {
                *cut.entry((s, c)).or_default() += a;
            }
            println!("    con placebo por dirección y franja: {cut:?}");
        }
    }

    // Sorteo: un solo flujo (§13.3.4).
    let mut rng = SplitMix64::new(cfg.luck_seed);
    let main_draws: Vec<Vec<Vec<u32>>> = main
        .iter()
        .map(|p| draw_placebos(&mut rng, &p.placebos, cfg.placebos_per_event))
        .collect();
    let mut luck: Vec<(Vec<f64>, f64)> = Vec::with_capacity(cfg.luck_simulations);
    for _ in 0..cfg.luck_simulations {
        let mut per = Vec::new();
        let (mut sum, mut n) = (0.0, 0usize);
        for p in &main {
            let d = draw_placebos(&mut rng, &p.placebos, cfg.placebos_per_event);
            let (mut s_i, mut n_i) = (0.0, 0usize);
            for (e, ds) in p.s.events.iter().zip(&d) {
                for &s in ds {
                    s_i += p.placebo(s, e.side, 0);
                    n_i += 1;
                }
            }
            per.push(s_i / n_i.max(1) as f64);
            sum += s_i;
            n += n_i;
        }
        luck.push((per, sum / n.max(1) as f64));
    }
    let desc_draws: Vec<Vec<Vec<u32>>> = desc
        .iter()
        .map(|p| draw_placebos(&mut rng, &p.placebos, cfg.placebos_per_event))
        .collect();

    let per_main: Vec<(InstrumentId, Vec<Row51>)> = main
        .iter()
        .zip(&main_draws)
        .map(|(p, d)| (p.id.clone(), rows51(p, d, cfg)))
        .collect();
    let per_desc: Vec<(InstrumentId, Vec<Row51>)> = desc
        .iter()
        .zip(&desc_draws)
        .map(|(p, d)| (p.id.clone(), rows51(p, d, cfg)))
        .collect();

    let pass = verdict51(
        &format!("H5.1 ({}, vota)", cfg.timeframe),
        &per_main,
        &months,
        rule,
        cfg,
    );
    println!(
        "  VEREDICTO H5.1: {}",
        if pass {
            "PASA los 5 criterios"
        } else {
            "NO PASA"
        }
    );

    println!(
        "\n══ test de suerte ({} repeticiones, semilla {}; solo reporte)",
        luck.len(),
        cfg.luck_seed
    );
    let real_all: Vec<f64> = per_main
        .iter()
        .flat_map(|(_, r)| r.iter().map(|x| x.real[0]))
        .collect();
    let pct_below = |xs: &[f64], v: f64| {
        100.0 * xs.iter().filter(|x| **x < v).count() as f64 / xs.len().max(1) as f64
    };
    let comb: Vec<f64> = luck.iter().map(|x| x.1).collect();
    println!(
        "  combinado: media real r_{} {:+.4}; medias de placebo {:+.4} (mín {:+.4}, máx {:+.4}); percentil {:.1}",
        cfg.horizon,
        mean(&real_all),
        mean(&comb),
        comb.iter().copied().fold(f64::INFINITY, f64::min),
        comb.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        pct_below(&comb, mean(&real_all))
    );
    for (k, (id, rows)) in per_main.iter().enumerate() {
        let xs: Vec<f64> = luck.iter().map(|x| x.0[k]).collect();
        let real: Vec<f64> = rows.iter().map(|r| r.real[0]).collect();
        println!(
            "  {id}: media real {:+.4}; medias de placebo {:+.4}; percentil {:.1}",
            mean(&real),
            mean(&xs),
            pct_below(&xs, mean(&real))
        );
    }

    println!("\n══ descriptivo {} (sin voto)", cfg.timeframe);
    describe51(&per_main, rule, cfg);
    // Costos de ida y vuelta por franja (§13.3.8).
    let catalog_costs = |id: &InstrumentId| -> Res<(Decimal, Decimal)> {
        let spec = catalog
            .instruments
            .get(id)
            .ok_or("instrumento fuera del catálogo")?;
        let spread = *spec
            .terms
            .spread_typical_points
            .value()
            .ok_or("spread pendiente")?;
        let tick = *spec.terms.tick.value().ok_or("tick pendiente")?;
        Ok((spread, tick))
    };
    for (id, rows) in &per_main {
        let (spread_in, tick) = catalog_costs(id)?;
        let spread_out = cfg
            .out_of_session_spread
            .get(id.as_str())
            .copied()
            .ok_or("sin spread fuera de sesión")?;
        let rt = |sp: Decimal| {
            Decimal::TWO
                * (sp / Decimal::TWO
                    + costs.slippage_spread_fraction * sp
                    + costs.slippage_ticks * tick)
        };
        for (label, cash, sp) in [("contado", true, spread_in), ("fuera", false, spread_out)] {
            let pts: Vec<f64> = rows
                .iter()
                .filter(|r| r.cash == cash)
                .map(|r| r.diff_points)
                .filter(|x| x.is_finite())
                .collect();
            println!(
                "  {id} {label}: diferencia media {:+.3} puntos (n {}); costo de ida y vuelta {} \
                 (× 1,5: {})",
                mean(&pts),
                pts.len(),
                rt(sp).round_dp(3),
                (rt(sp) * Decimal::new(15, 1)).round_dp(3)
            );
        }
    }
    println!("\n══ descriptivo {} (sin voto)", cfg.descriptive_timeframe);
    let _ = verdict51(
        &format!("{} (descriptivo)", cfg.descriptive_timeframe),
        &per_desc,
        &months,
        rule,
        cfg,
    );
    describe51(&per_desc, rule, cfg);
    Ok(())
}

// ───────────────────────────── H5.2 ─────────────────────────────

struct Row52 {
    id: usize,
    side: Side,
    f: bool,
    /// Toque en `t` o en `t − 1` (variante descriptiva).
    f_or_prev: bool,
    year: i32,
    second_half: bool,
    r_mid: f64,
    r_net: f64,
    r_stress: f64,
    r_double: f64,
    fvg_ob_point: bool,
    triggers: String,
    exits: String,
    /// Tamaño del FVG (en ATR) del primer evento de la vela, si es F.
    size: Option<f64>,
}

fn rows52(
    k: usize,
    trades: &[TradeRecord],
    events: &[RealEvent],
    bars: &[Candle],
    rule: &RuleConfig,
) -> Vec<Row52> {
    let touches = touch_closes(bars, events);
    // Variante "toque en t o en t − 1": el cierre de la vela siguiente al toque.
    let mut prev: BTreeSet<(DateTime<Utc>, bool)> = touches.clone();
    let mut size_of: BTreeMap<(DateTime<Utc>, bool), f64> = BTreeMap::new();
    for e in events {
        let key = (bars[e.touch].end(), e.side == Side::Long);
        size_of.entry(key).or_insert_with(|| f(e.size_atr));
        if let Some(next) = bars.get(e.touch + 1) {
            prev.insert((next.end(), e.side == Side::Long));
        }
    }
    let half = rule.second_half_from;
    trades
        .iter()
        .map(|t| {
            let risk = t.size * (t.entry_price - t.stop).abs();
            let r = |x: Decimal| {
                if risk.is_zero() {
                    f64::NAN
                } else {
                    f(x / risk)
                }
            };
            let costs = t.mid_pnl - t.net_pnl;
            let is_f = is_fvg_trade(&touches, t.signal_at, t.side);
            let mut trig: Vec<String> = t.triggers.iter().map(|g| format!("{g:?}")).collect();
            trig.sort();
            trig.dedup();
            Row52 {
                id: k,
                side: t.side,
                f: is_f,
                f_or_prev: is_fvg_trade(&prev, t.signal_at, t.side),
                year: t.signal_at.year(),
                second_half: t.signal_at.date_naive() >= half,
                r_mid: r(t.mid_pnl),
                r_net: r(t.net_pnl),
                r_stress: r(t.mid_pnl - rule.cost_stress * costs),
                r_double: r(t.mid_pnl - Decimal::TWO * costs),
                fvg_ob_point: t
                    .breakdown
                    .iter()
                    .any(|(c, v)| *c == Component::FvgOrOrderBlock && *v > 0),
                triggers: trig.join(" + "),
                exits: t
                    .exits
                    .iter()
                    .map(|e| format!("{:?}", e.reason))
                    .collect::<Vec<_>>()
                    .join(" → "),
                size: if is_f {
                    size_of.get(&(t.signal_at, t.side == Side::Long)).copied()
                } else {
                    None
                },
            }
        })
        .collect()
}

fn diff52(rows: &[&Row52], r: &dyn Fn(&Row52) -> f64) -> (f64, usize, usize) {
    let fs: Vec<f64> = rows.iter().filter(|x| x.f).map(|x| r(x)).collect();
    let rs: Vec<f64> = rows.iter().filter(|x| !x.f).map(|x| r(x)).collect();
    (mean(&fs) - mean(&rs), fs.len(), rs.len())
}

/// Estrategia filtrada (§5.4, §13.3.13): solo operaciones F, con una posición a la vez.
fn filtered_trades(
    l: &Loaded,
    bars: &[Candle],
    events: &[RealEvent],
) -> Res<(Vec<(usize, TradeRecord)>, usize)> {
    let p = &l.p;
    let eval =
        EvaluationConfig::from_toml(cfg!("evaluation.toml")).map_err(|e| format!("{e:?}"))?;
    let inputs = Inputs {
        m15: &p.m15,
        h1: &p.h1,
        entries: &p.entries,
        force_close: &p.force_close,
        data_errors: &p.data_errors,
        news: &p.news,
        published_from: p.published_from,
    };
    let touches = touch_closes(bars, events);
    let mut allowed: BTreeSet<DateTime<Utc>> = events.iter().map(|e| bars[e.touch].start).collect();
    let mut rounds = 0;
    loop {
        rounds += 1;
        let mut out = Vec::new();
        let mut bad = Vec::new();
        for (w, win) in l.split.windows.iter().enumerate() {
            let period = cfds_backtest::research_period(win.validation, &l.split)
                .map_err(|e| format!("{e:?}"))?;
            let keep = |s: &cfds_backtest::Slice, e: Vec<bool>| {
                Ok(e.iter()
                    .zip(s.m15)
                    .map(|(x, c)| *x && allowed.contains(&c.start))
                    .collect())
            };
            let (_, rep) =
                evaluate_period_with(&inputs, period, &p.setup, eval.warmup, Decimal::ZERO, keep)
                    .map_err(|e| format!("{e:?}"))?;
            let Some(rep) = rep else { continue };
            for t in rep.trades {
                if is_fvg_trade(&touches, t.signal_at, t.side) {
                    out.push((w, t));
                } else {
                    bad.push(t.signal_at);
                }
            }
        }
        if bad.is_empty() {
            return Ok((out, rounds));
        }
        for at in bad {
            // La vela de la señal termina en `at`.
            let j = p.m15.partition_point(|c| c.end() < at);
            if let Some(c) = p.m15.get(j).filter(|c| c.end() == at) {
                allowed.remove(&c.start);
            }
        }
    }
}

async fn run_h52(args: &[String], rule: &RuleConfig, cfg: &Program5Config) -> Res<()> {
    let store = read_only_store().await?;
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let ids: Vec<InstrumentId> = cfg
        .instruments
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;
    let mut rows = Vec::new();
    let mut per_id_n = Vec::new();
    let mut filtered = Vec::new();
    for (k, id) in ids.iter().enumerate() {
        let mut a = vec![id.to_string()];
        a.extend(args.iter().cloned());
        let l = research_round::load_opts(
            id,
            &a,
            load_from(),
            Decimal::from(10_000),
            &sources,
            &store,
            &split_cfg,
            false,
        )
        .await?;
        let (lo, hi) = (
            l.split.windows[0].validation.start,
            l.split.out_of_sample.start,
        );
        let s = program5::series(
            &store,
            &sources,
            id,
            timeframe(&cfg.timeframe)?,
            load_from(),
            (lo, hi),
            cfg.horizon,
        )
        .await?;
        let trades = program5::strategy_trades(&l)?;
        let r = rows52(k, &trades, &s.events, &s.bars, rule);
        per_id_n.push(r.len());
        rows.extend(r);
        let (ft, rounds) = filtered_trades(&l, &s.bars, &s.events)?;
        filtered.push((id.clone(), l, ft, rounds));
    }
    // Conteos antes de los resultados (§10).
    println!("\n══ H5.2: operaciones F y R por instrumento y año");
    for (k, id) in ids.iter().enumerate() {
        let mut by: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
        for r in rows.iter().filter(|r| r.id == k) {
            let e = by.entry(r.year).or_default();
            if r.f {
                e.0 += 1;
            } else {
                e.1 += 1;
            }
        }
        let fs = by.values().map(|x| x.0).sum::<usize>();
        println!(
            "  {id}: {} operaciones, F {fs}, R {}; por año (F, R) {by:?}",
            per_id_n[k],
            per_id_n[k] - fs
        );
    }

    let all: Vec<&Row52> = rows.iter().collect();
    let alpha = f(rule.alpha);
    println!("\n══ H5.2 criterios (R neto por operación)");
    let fs: Vec<f64> = all.iter().filter(|x| x.f).map(|x| x.r_net).collect();
    let rs: Vec<f64> = all.iter().filter(|x| !x.f).map(|x| x.r_net).collect();
    let w = stats::welch_greater(&fs, &rs);
    let c1 = w.is_some_and(|w| w.p_value < alpha);
    match w {
        Some(w) => println!(
            "  1. F {:+.4} R (n {}) − R {:+.4} R (n {}) = {:+.4} R; e.e. {:.4}; t de Welch {:.3} \
             (gl {:.1}); p = {:.5} (nivel {alpha}) → {}",
            mean(&fs),
            fs.len(),
            mean(&rs),
            rs.len(),
            w.diff,
            w.se,
            w.t,
            w.df,
            w.p_value,
            if c1 { "PASA" } else { "no pasa" }
        ),
        None => println!("  1. sin prueba → no pasa"),
    }
    let mut c2 = true;
    for (k, id) in ids.iter().enumerate() {
        let sub: Vec<&Row52> = all.iter().copied().filter(|r| r.id == k).collect();
        let (d, nf, nr) = diff52(&sub, &|x| x.r_net);
        c2 &= d > 0.0;
        println!("  2. {id}: F − R = {d:+.4} R (F {nf}, R {nr})");
    }
    println!("     → {}", if c2 { "PASA" } else { "no pasa" });
    let mut c3 = true;
    for (label, second) in [("02-2013 → 12-2020", false), ("01-2021 → 10-2025", true)] {
        let sub: Vec<&Row52> = all
            .iter()
            .copied()
            .filter(|r| r.second_half == second)
            .collect();
        let (d, nf, nr) = diff52(&sub, &|x| x.r_net);
        c3 &= d > 0.0;
        println!("  3. {label}: F − R = {d:+.4} R (F {nf}, R {nr})");
    }
    println!("     → {}", if c3 { "PASA" } else { "no pasa" });
    let (ds, _, _) = diff52(&all, &|x| x.r_stress);
    let c4 = ds > 0.0;
    println!(
        "  4. costos × {}: F − R = {ds:+.4} R → {}",
        rule.cost_stress,
        if c4 { "PASA" } else { "no pasa" }
    );
    let mut c5 = true;
    for (k, id) in ids.iter().enumerate() {
        let n = rows.iter().filter(|r| r.id == k && r.f).count();
        c5 &= n >= cfg.min_f_trades;
        println!("  5. {id}: {n} operaciones F (mínimo {})", cfg.min_f_trades);
    }
    println!("     → {}", if c5 { "PASA" } else { "no pasa" });
    let pass = c1 && c2 && c3 && c4 && c5;
    println!(
        "  VEREDICTO H5.2 con este escenario: {}",
        if pass {
            "PASA los 5 criterios"
        } else {
            "NO PASA"
        }
    );

    // Test de suerte (§13.3.12): flujo nuevo por escenario.
    let mut rng = SplitMix64::new(cfg.luck_seed);
    let by_id: Vec<Vec<f64>> = (0..ids.len())
        .map(|k| rows.iter().filter(|r| r.id == k).map(|r| r.r_net).collect())
        .collect();
    let n_f: Vec<usize> = (0..ids.len())
        .map(|k| rows.iter().filter(|r| r.id == k && r.f).count())
        .collect();
    let mut sims = Vec::with_capacity(cfg.luck_simulations);
    for _ in 0..cfg.luck_simulations {
        let mut xs = Vec::new();
        for (k, v) in by_id.iter().enumerate() {
            for i in sample_without_replacement(&mut rng, v.len(), n_f[k]) {
                xs.push(v[i]);
            }
        }
        sims.push(mean(&xs));
    }
    let real = mean(&fs);
    println!(
        "\n══ test de suerte ({} subconjuntos, semilla {}): R neto medio de F {:+.4}; de los \
         subconjuntos {:+.4}; percentil {:.1}",
        sims.len(),
        cfg.luck_seed,
        real,
        mean(&sims),
        100.0 * sims.iter().filter(|x| **x < real).count() as f64 / sims.len().max(1) as f64
    );

    // Descriptivo (§5.5).
    println!("\n══ descriptivo H5.2 (sin voto)");
    for (label, sel) in [("F", true), ("R", false)] {
        let sub: Vec<&Row52> = all.iter().copied().filter(|r| r.f == sel).collect();
        let mid: Vec<f64> = sub.iter().map(|x| x.r_mid).collect();
        let net: Vec<f64> = sub.iter().map(|x| x.r_net).collect();
        let dbl: Vec<f64> = sub.iter().map(|x| x.r_double).collect();
        let wins = net.iter().filter(|x| **x > 0.0).count();
        let se = stats::std_dev(&net).unwrap_or(f64::NAN) / (net.len() as f64).sqrt();
        println!(
            "  {label}: {} operaciones; R antes de costos {:+.4}; R neto {:+.4} (IC ± {:.4}); \
             costos × 2 {:+.4}; aciertos {}; con el +1 de FVG u order block {}",
            sub.len(),
            mean(&mid),
            mean(&net),
            f(z()) * se,
            mean(&dbl),
            rate(wins, net.len()),
            sub.iter().filter(|x| x.fvg_ob_point).count()
        );
        let mut exits: BTreeMap<&str, usize> = BTreeMap::new();
        for r in &sub {
            *exits.entry(r.exits.as_str()).or_default() += 1;
        }
        println!("     salidas: {exits:?}");
        for (title, key) in [
            (
                "dirección",
                Box::new(|r: &Row52| side_name(r.side).to_owned()) as Box<dyn Fn(&Row52) -> String>,
            ),
            ("gatillos", Box::new(|r: &Row52| r.triggers.clone())),
            ("año", Box::new(|r: &Row52| r.year.to_string())),
        ] {
            let mut g: BTreeMap<String, Vec<f64>> = BTreeMap::new();
            for r in &sub {
                g.entry(key(r)).or_default().push(r.r_net);
            }
            let parts: Vec<String> = g
                .iter()
                .map(|(k, v)| format!("{k}: {:+.3} (n {})", mean(v), v.len()))
                .collect();
            println!("     R neto por {title}: {}", parts.join("; "));
        }
    }
    let mut sizes: Vec<f64> = rows.iter().filter_map(|r| r.size).collect();
    sizes.sort_by(f64::total_cmp);
    let (c1s, c2s) = (quantile(&sizes, 1.0 / 3.0), quantile(&sizes, 2.0 / 3.0));
    let rest: Vec<f64> = all.iter().filter(|x| !x.f).map(|x| x.r_net).collect();
    for (label, lo_s, hi_s) in [
        ("1 chico", f64::NEG_INFINITY, c1s),
        ("2 medio", c1s, c2s),
        ("3 grande", c2s, f64::INFINITY),
    ] {
        let v: Vec<f64> = rows
            .iter()
            .filter(|r| r.size.is_some_and(|s| s > lo_s && s <= hi_s))
            .map(|r| r.r_net)
            .collect();
        println!(
            "  F − R por tercil de tamaño {label} (cortes {c1s:.3} y {c2s:.3} ATR): {:+.4} (F {})",
            mean(&v) - mean(&rest),
            v.len()
        );
    }
    let fp: Vec<f64> = all
        .iter()
        .filter(|x| x.f_or_prev)
        .map(|x| x.r_net)
        .collect();
    let rp: Vec<f64> = all
        .iter()
        .filter(|x| !x.f_or_prev)
        .map(|x| x.r_net)
        .collect();
    println!(
        "  variante \"toque en t o en t − 1\": F' {} operaciones, F' − R' = {:+.4} R",
        fp.len(),
        mean(&fp) - mean(&rp)
    );

    // Estrategia filtrada (§5.4): solo descriptiva (punto 14).
    println!(
        "\n══ estrategia filtrada (solo operaciones F, una posición a la vez; descriptivo, no vota)"
    );
    for (id, l, ft, rounds) in &filtered {
        let p = &l.p;
        let session = p.calendar.session_for(id).ok_or("sin bolsa")?;
        let rolls = p
            .setup
            .futures_basis
            .as_ref()
            .map(|b| b.rolls.clone())
            .ok_or("sin vencimientos")?;
        let stresses = [Decimal::ONE, rule.cost_stress, Decimal::TWO];
        // Retorno de cada operación sobre el capital del bloque al abrirla.
        let mut closed: [Vec<(DateTime<Utc>, Decimal)>; 3] = Default::default();
        let mut by_block: BTreeMap<usize, Vec<&TradeRecord>> = BTreeMap::new();
        for (w, t) in ft {
            by_block.entry(*w).or_default().push(t);
        }
        for ts in by_block.values() {
            let mut ts = ts.clone();
            ts.sort_by_key(|t| t.closed_at());
            for t in &ts {
                let realized: Decimal = ts
                    .iter()
                    .filter(|o| o.closed_at() <= t.entry_time)
                    .map(|o| {
                        p.setup
                            .to_account(o.net_pnl, o.closed_at())
                            .unwrap_or(Decimal::ZERO)
                    })
                    .sum();
                let equity = p.setup.initial_equity + realized;
                for (k, s) in stresses.iter().enumerate() {
                    let net = t.mid_pnl - *s * (t.mid_pnl - t.net_pnl);
                    let r = p
                        .setup
                        .to_account(net, t.closed_at())
                        .map_err(|e| format!("{e:?}"))?
                        / equity;
                    closed[k].push((t.closed_at(), r));
                }
            }
        }
        let y: Vec<Vec<f64>> = closed
            .iter()
            .map(|c| monthly_returns(c, &l.months).into_iter().map(f).collect())
            .collect();
        let mut xs: [BTreeMap<(i32, u32), Decimal>; 3] = Default::default();
        let hold = [(0usize, Decimal::ONE)];
        for w in &l.split.windows {
            for (k, s) in stresses.iter().enumerate() {
                for (m, r) in managed_returns(
                    &p.m15,
                    session,
                    &rolls,
                    &p.setup,
                    (w.validation.start, w.validation.end),
                    &hold,
                    *s,
                )
                .map_err(|e| format!("{e:?}"))?
                {
                    xs[k].insert(m, r);
                }
            }
        }
        let x_of = |k: usize| -> Res<Vec<f64>> {
            l.months
                .iter()
                .map(|m| {
                    xs[k]
                        .get(m)
                        .copied()
                        .map(f)
                        .ok_or_else(|| format!("falta {m:?}").into())
                })
                .collect()
        };
        let rn: Vec<f64> = ft
            .iter()
            .map(|(_, t)| {
                let risk = t.size * (t.entry_price - t.stop).abs();
                if risk.is_zero() {
                    f64::NAN
                } else {
                    f(t.net_pnl / risk)
                }
            })
            .collect();
        println!(
            "  {id}: {} operaciones F ({rounds} pasadas hasta que todas son F); R neto medio {:+.4}",
            ft.len(),
            mean(&rn)
        );
        let s = alpha_verdict::Series {
            y: y[0].clone(),
            y_stress: y[1].clone(),
            y_double: y[2].clone(),
            x: x_of(0)?,
            x_stress: x_of(1)?,
            x_double: x_of(2)?,
        };
        let _ = alpha_verdict::verdict(&format!("{id} filtrada"), rule, &l.months, &s, false);
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let research = ResearchConfig::from_toml(cfg!("research.toml"))?;
    let (rule, cfg) = (research.rule_program5, research.program5);
    match args.first().map(String::as_str) {
        Some("h51") => run_h51(&rule, &cfg).await,
        Some("h52") => run_h52(&args[1..], &rule, &cfg).await,
        _ => Err(
            "uso: programa_5 h51 | h52 --escenario-financiacion <ETIQUETA>=<archivo.csv>".into(),
        ),
    }
}

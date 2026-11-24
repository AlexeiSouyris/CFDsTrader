//! Programa Nasdaq, H1 (#526): deriva antes del comunicado programado del FOMC en el CFD US100,
//! con la regla congelada en `docs/analysis/2026-11-programa-nasdaq-preregistro.md` (§2 y §3).
//! Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example programa_nasdaq_h1 -- \
//!     US100 [US500 US30] --escenario-financiacion EUR=<archivo.csv> \
//!     [--desde 2011-01-01T00:00:00Z] [--salida <archivo.csv>] [--eventos <archivo.csv>]
//! ```
//! El primer instrumento vota (US100); los demás son descriptivos (§3.5). Velas solo hasta el
//! inicio del fuera de muestra; calendario de `official_releases` publicado antes de ese inicio.
//! Una corrida continua de `managed_returns` sobre la validación concatenada (§2.6); comprar y
//! mantener con una cuenta nueva por ventana de validación (como R018 y la ronda 3).
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
    CalendarRow, EventWindow, ReleaseStatus, ResearchConfig, RuleConfig, Skip, event_schedule,
    managed_returns, pre_fomc_windows, stats,
};
use cfds_core::InstrumentId;
use cfds_market_data::{CandleSources, SplitConfig};
use cfds_storage::PgStore;
use chrono::{DateTime, Datelike, Utc};
use research_round::{f, load, mean};
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

/// Retorno compuesto de los meses de una corrida corta.
fn compound(months: &[((i32, u32), Decimal)]) -> Decimal {
    months
        .iter()
        .fold(Decimal::ONE, |acc, (_, r)| acc * (Decimal::ONE + r))
        - Decimal::ONE
}

/// Series mensuales de H1 y de comprar y mantener en un índice.
struct Series {
    y: Vec<f64>,
    y_stress: Vec<f64>,
    y_double: Vec<f64>,
    x: Vec<f64>,
    x_stress: Vec<f64>,
    x_double: Vec<f64>,
}

/// Un evento operado: fecha, regla de hora, conferencia y retornos (bruto, neto, × 1,5).
struct Traded {
    key: String,
    scheduled_at: DateTime<Utc>,
    entry: DateTime<Utc>,
    exit: DateTime<Utc>,
    press: Option<bool>,
    gross: f64,
    net: f64,
    net_stress: f64,
}

fn t_line(name: &str, xs: &[f64]) -> String {
    let t = stats::t_test_greater(xs);
    format!(
        "{name}: n {}, media {:+.2} pb, desvío {:.2} pb, t {:.2}",
        xs.len(),
        1e4 * mean(xs),
        1e4 * stats::std_dev(xs).unwrap_or(f64::NAN),
        t.map_or(f64::NAN, |t| t.t)
    )
}

/// Criterios de §3.4 sobre una serie. Devuelve si pasa los cinco.
fn verdict(rule: &RuleConfig, months: &[(i32, u32)], s: &Series, voting: bool) -> bool {
    let alpha = f(rule.alpha);
    println!(
        "  H1 {:+.4} % por mes (Sharpe {:.4}, anual ≈ {:.2}; meses con posición {}); comprar y \
         mantener {:+.4} % (Sharpe {:.4}, anual ≈ {:.2})",
        100.0 * mean(&s.y),
        sharpe(&s.y),
        sharpe(&s.y) * 12f64.sqrt(),
        s.y.iter().filter(|v| **v != 0.0).count(),
        100.0 * mean(&s.x),
        sharpe(&s.x),
        sharpe(&s.x) * 12f64.sqrt()
    );
    let Some(a) = stats::alpha_hc1(&s.y, &s.x) else {
        println!("  sin regresión (datos insuficientes) → NO PASA");
        return false;
    };
    let c1 = a.p_value < alpha;
    println!(
        "  1. alfa contra comprar y mantener: α = {:+.4} % por mes (anual ≈ {:+.2} %), β = {:.4}, \
         e.e. HC1 {:.4} %, t = {:.3}, p = {:.5} (nivel {alpha}) → {}",
        100.0 * a.alpha,
        1200.0 * a.alpha,
        a.beta,
        100.0 * a.se_alpha,
        a.t,
        a.p_value,
        if c1 { "PASA" } else { "no pasa" }
    );
    let active: Vec<f64> = s.y.iter().zip(&s.x).map(|(y, x)| y - a.beta * x).collect();
    let dsr = stats::deflated_sharpe(&active, rule.dsr_trials);
    let c2 = dsr.is_some_and(|d| d.dsr >= f(rule.dsr_min));
    match dsr {
        Some(d) => println!(
            "  2. Sharpe deflactado de la ventaja (y − βx, N = {}): Sharpe mensual {:.4} (anual ≈ \
             {:.2}), SR0 {:.4}, asimetría {:.2}, curtosis {:.2}, DSR {:.4} (mínimo {}) → {}",
            rule.dsr_trials,
            d.sharpe,
            d.sharpe * 12f64.sqrt(),
            d.sharpe0,
            d.skew,
            d.kurtosis,
            d.dsr,
            rule.dsr_min,
            if c2 { "PASA" } else { "no pasa" }
        ),
        None => println!("  2. Sharpe deflactado: sin valor → no pasa"),
    }
    for n in [100, 1000] {
        if let Some(r) = stats::deflated_sharpe(&active, n) {
            println!(
                "     (referencia, N = {n}: SR0 {:.4}, DSR {:.4})",
                r.sharpe0, r.dsr
            );
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
    let c3 = mean(&first) > 0.0 && mean(&second) > 0.0;
    println!(
        "  3. mitades: ventaja media {:+.4} % ({} meses hasta {}-{:02}) y {:+.4} % ({} meses desde) → {}",
        100.0 * mean(&first),
        first.len(),
        half.0,
        half.1,
        100.0 * mean(&second),
        second.len(),
        if c3 { "PASA" } else { "no pasa" }
    );
    let stressed = stats::alpha_hc1(&s.y_stress, &s.x_stress);
    let c4 = stressed.is_some_and(|s| s.alpha > 0.0);
    println!(
        "  4. costos × {}: α = {:+.4} % por mes (p {:.4}) → {}",
        rule.cost_stress,
        100.0 * stressed.map_or(f64::NAN, |s| s.alpha),
        stressed.map_or(f64::NAN, |s| s.p_value),
        if c4 { "PASA" } else { "no pasa" }
    );
    let blend: Vec<f64> =
        s.y.iter()
            .zip(&s.x)
            .map(|(y, x)| 0.5 * x + 0.5 * y)
            .collect();
    let c5 = sharpe(&blend) > sharpe(&s.x);
    println!(
        "  5. mezcla 50/50: Sharpe mensual {:.4} > comprar y mantener {:.4} → {}",
        sharpe(&blend),
        sharpe(&s.x),
        if c5 { "PASA" } else { "no pasa" }
    );
    let double = stats::alpha_hc1(&s.y_double, &s.x_double);
    println!(
        "  (costos × 2, solo se informa: α = {:+.4} % por mes)",
        100.0 * double.map_or(f64::NAN, |s| s.alpha)
    );
    let all = c1 && c2 && c3 && c4 && c5;
    if voting {
        println!(
            "  VEREDICTO con este escenario: {}",
            if all {
                "PASA los 5 criterios"
            } else {
                "NO PASA"
            }
        );
    } else {
        println!("  (descriptivo, sin voto)");
    }
    all
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let from: DateTime<Utc> = take(&mut args, "--desde")
        .as_deref()
        .unwrap_or("2011-01-01T00:00:00Z")
        .parse()?;
    let out_csv = take(&mut args, "--salida");
    let events_csv = take(&mut args, "--eventos");
    let research = ResearchConfig::from_toml(cfg!("research.toml"))?;
    let rule = research.rule_nasdaq;
    let pcfg = research.pre_fomc;
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
        return Err("uso: US100 [US500 US30] --escenario-financiacion <ETIQUETA>=<csv>".into());
    }

    let mut csv = String::from("serie,instrumento,mes,retorno,retorno_costos_estresados\n");
    let mut ev_csv = String::from(
        "instrumento,reunion,comunicado_utc,regla_hora,conferencia,estado,operado,motivo,\
         entrada_utc,salida_utc,bruto,neto,neto_estresado\n",
    );
    for (n, id) in instruments.iter().enumerate() {
        let voting = n == 0;
        let l = load(
            id,
            &args,
            from,
            Decimal::from(100_000),
            &sources,
            &store,
            &split_cfg,
        )
        .await?;
        let p = &l.p;
        let session = p.calendar.session_for(id).ok_or("instrumento sin bolsa")?;
        let rolls = p
            .setup
            .futures_basis
            .as_ref()
            .map(|b| b.rolls.clone())
            .ok_or("sin vencimientos: H1 corre sobre futuros")?;
        let (lo, hi) = (
            l.split.windows[0].validation.start,
            l.split.out_of_sample.start,
        );
        if l.months.len() != 153 {
            return Err(format!("{id}: {} meses de validación, no 153", l.months.len()).into());
        }

        // Calendario: solo lo publicado antes del fuera de muestra.
        let raw = store
            .official_releases("fed_fomc", "fomc_statement", hi)
            .await?;
        let mut rows = Vec::with_capacity(raw.len());
        let mut meta: BTreeMap<String, (String, Option<bool>)> = BTreeMap::new();
        for r in &raw {
            let status = ReleaseStatus::parse(&r.status)
                .ok_or_else(|| format!("estado desconocido: {}", r.status))?;
            meta.insert(
                r.reference_period.clone(),
                (r.time_rule.clone(), r.with_press_conference),
            );
            rows.push(CalendarRow {
                key: r.reference_period.clone(),
                status,
                scheduled_at: r.scheduled_at,
                published_at: r.published_at,
            });
        }
        let outcomes = pre_fomc_windows(&p.m15, session, &rows, &pcfg);
        // Eventos del período: la vela de la salida (o el comunicado, si no se opera) en [lo, hi).
        let mut windows: Vec<EventWindow> = Vec::new();
        let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
        println!("══ {id}: eventos del período {lo} → {hi} (lista previa, sin retornos)");
        for o in &outcomes {
            let when = match o.window {
                Ok(w) => p.m15[w.exit()].start,
                Err(_) => o.row.scheduled_at.unwrap_or(o.row.published_at),
            };
            if when < lo || when >= hi {
                continue;
            }
            match o.window {
                Ok(w) => {
                    if p.m15[w.entry()].start < lo {
                        return Err(format!("{}: entrada antes del período", o.row.key).into());
                    }
                    windows.push(w);
                }
                Err(why) => {
                    *skipped.entry(format!("{why:?}")).or_default() += 1;
                    if voting {
                        println!(
                            "   no operado: {} ({:?}) → {why:?}",
                            o.row.key, o.row.status
                        );
                    }
                }
            }
        }
        println!(
            "   operables: {}; no operados: {} {:?}",
            windows.len(),
            skipped.values().sum::<usize>(),
            skipped
        );

        // Serie de H1: corrida continua; comprar y mantener: cuenta nueva por ventana.
        let schedule = event_schedule(&windows);
        let mut ys: [BTreeMap<(i32, u32), Decimal>; 3] = Default::default();
        for (k, stress) in [Decimal::ONE, rule.cost_stress, Decimal::TWO]
            .into_iter()
            .enumerate()
        {
            for (m, r) in managed_returns(
                &p.m15,
                session,
                &rolls,
                &p.setup,
                (lo, hi),
                &schedule,
                stress,
            )
            .map_err(|e| format!("{e:?}"))?
            {
                ys[k].insert(m, r);
            }
        }
        let mut xs: [BTreeMap<(i32, u32), Decimal>; 3] = Default::default();
        let hold = [(0usize, Decimal::ONE)];
        for w in &l.split.windows {
            for (k, stress) in [Decimal::ONE, rule.cost_stress, Decimal::TWO]
                .into_iter()
                .enumerate()
            {
                for (m, r) in managed_returns(
                    &p.m15,
                    session,
                    &rolls,
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
        let mut miss = 0;
        let mut pick = |m: &BTreeMap<(i32, u32), Decimal>| {
            let (v, k) = series_of(m, &l.months);
            miss += k;
            v
        };
        let s = Series {
            y: pick(&ys[0]),
            y_stress: pick(&ys[1]),
            y_double: pick(&ys[2]),
            x: pick(&xs[0]),
            x_stress: pick(&xs[1]),
            x_double: pick(&xs[2]),
        };
        if miss > 0 || ys[0].len() != l.months.len() {
            return Err(format!("{id}: faltan meses ({miss}) o sobran").into());
        }
        for (k, m) in l.months.iter().enumerate() {
            let _ = writeln!(
                csv,
                "H1,{id},{}-{:02},{:.8},{:.8}",
                m.0, m.1, s.y[k], s.y_stress[k]
            );
            let _ = writeln!(
                csv,
                "comprar_y_mantener,{id},{}-{:02},{:.8},{:.8}",
                m.0, m.1, s.x[k], s.x_stress[k]
            );
        }

        // Por evento (descriptivo): bruto (costos × 0), neto y × 1,5 de la misma función.
        let mut traded: Vec<Traded> = Vec::new();
        for o in &outcomes {
            let Ok(w) = o.window else {
                let when = o.row.scheduled_at.unwrap_or(o.row.published_at);
                if when >= lo && when < hi {
                    let (rule_name, press) = meta.get(&o.row.key).cloned().unwrap_or_default();
                    let _ = writeln!(
                        ev_csv,
                        "{id},{},{},{rule_name},{},{:?},no,{:?},,,,,",
                        o.row.key,
                        when.format("%Y-%m-%dT%H:%M:%SZ"),
                        press.map_or(String::new(), |b| b.to_string()),
                        o.row.status,
                        o.window.err().unwrap_or(Skip::Overlap)
                    );
                }
                continue;
            };
            if !windows.contains(&w) {
                continue;
            }
            let one = [
                (w.decision_in, Decimal::ONE),
                (w.decision_out, Decimal::ZERO),
            ];
            let range = (p.m15[w.entry()].start, p.m15[w.exit()].end());
            let mut r = [0.0; 3];
            for (k, stress) in [Decimal::ZERO, Decimal::ONE, rule.cost_stress]
                .into_iter()
                .enumerate()
            {
                let months =
                    managed_returns(&p.m15, session, &rolls, &p.setup, range, &one, stress)
                        .map_err(|e| format!("{e:?}"))?;
                r[k] = f(compound(&months));
            }
            let (rule_name, press) = meta.get(&o.row.key).cloned().unwrap_or_default();
            let t = Traded {
                key: o.row.key.clone(),
                scheduled_at: o.row.scheduled_at.unwrap_or(o.row.published_at),
                entry: p.m15[w.entry()].start,
                exit: p.m15[w.exit()].start,
                press,
                gross: r[0],
                net: r[1],
                net_stress: r[2],
            };
            let _ = writeln!(
                ev_csv,
                "{id},{},{},{rule_name},{},{:?},si,,{},{},{:.8},{:.8},{:.8}",
                t.key,
                t.scheduled_at.format("%Y-%m-%dT%H:%M:%SZ"),
                press.map_or(String::new(), |b| b.to_string()),
                o.row.status,
                t.entry.format("%Y-%m-%dT%H:%M:%SZ"),
                t.exit.format("%Y-%m-%dT%H:%M:%SZ"),
                t.gross,
                t.net,
                t.net_stress
            );
            traded.push(t);
        }

        println!();
        println!(
            "════════ H1 en {id} ({}) ════════",
            if voting { "vota" } else { "descriptivo" }
        );
        let pick_by = |keep: &dyn Fn(&Traded) -> bool, g: &dyn Fn(&Traded) -> f64| -> Vec<f64> {
            traded.iter().filter(|t| keep(t)).map(g).collect()
        };
        let split = rule.second_half_from;
        let all = |_: &Traded| true;
        let first = |t: &Traded| t.scheduled_at.date_naive() < split;
        let second = |t: &Traded| t.scheduled_at.date_naive() >= split;
        let pc = |t: &Traded| t.press == Some(true);
        let no_pc = |t: &Traded| t.press == Some(false);
        for (name, keep) in [
            ("todos", &all as &dyn Fn(&Traded) -> bool),
            ("hasta 2020", &first),
            ("desde 2021", &second),
            ("con conferencia", &pc),
            ("sin conferencia", &no_pc),
        ] {
            println!(
                "  por evento, {}",
                t_line(&format!("{name}, bruto"), &pick_by(keep, &|t| t.gross))
            );
            println!(
                "  por evento, {}",
                t_line(&format!("{name}, neto"), &pick_by(keep, &|t| t.net))
            );
        }
        let costs: Vec<f64> = traded.iter().map(|t| t.gross - t.net).collect();
        println!(
            "  costo medio por evento {:.2} pb (× 1,5: {:.2} pb)",
            1e4 * mean(&costs),
            1e4 * mean(
                &traded
                    .iter()
                    .map(|t| t.gross - t.net_stress)
                    .collect::<Vec<_>>()
            )
        );
        verdict(&rule, &l.months, &s, voting);
    }
    if let Some(path) = out_csv {
        std::fs::write(&path, csv)?;
        println!("retornos mensuales → {path}");
    }
    if let Some(path) = events_csv {
        std::fs::write(&path, ev_csv)?;
        println!("eventos → {path}");
    }
    Ok(())
}

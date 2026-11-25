//! Programa Nasdaq, H2 (#526): días de CPI y empleo del BLS en el CFD US100, con la regla
//! congelada en `docs/analysis/2026-11-programa-nasdaq-preregistro.md` (§2 y §4).
//! Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example programa_nasdaq_h2 -- \
//!     US100 [US500 US30] --escenario-financiacion EUR=<archivo.csv> \
//!     [--desde 2011-01-01T00:00:00Z] [--salida <archivo.csv>] [--eventos <archivo.csv>]
//! ```
//! El primer instrumento vota (US100); los demás son descriptivos (§3.5 y §4.3). Velas solo
//! hasta el inicio del fuera de muestra; calendario de `official_releases` (BLS: `cpi` y
//! `employment_situation`) publicado antes de ese inicio. Una corrida continua de
//! `managed_returns_with_spreads` sobre la validación concatenada (§2.6), con el spread de fuera
//! de sesión en la entrada de las 08:00 ET (§4.2); comprar y mantener con una cuenta nueva por
//! ventana de validación (como R018 y H1).
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
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use alpha_verdict::{Series, verdict};
use cfds_backtest::research::{
    CalendarRow, EventWindow, ReleaseStatus, ResearchConfig, Skip, event_schedule,
    macro_day_windows, managed_returns, managed_returns_with_spreads, session_close_indices, stats,
};
use cfds_core::InstrumentId;
use cfds_market_data::{CandleSources, SplitConfig};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
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

/// Un evento operado: fecha, qué datos salieron ese día y retornos (bruto, neto, × 1,5).
struct Traded {
    key: String,
    scheduled_at: DateTime<Utc>,
    entry: DateTime<Utc>,
    exit: DateTime<Utc>,
    kind: &'static str,
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
    let mcfg = research.macro_day.clone();
    let mrule = mcfg.rule();
    // N vigente al correr (solo se informa, §4.3): el del último programa congelado.
    let n_now = research.rule_program4.dsr_trials;
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
        "instrumento,dato,publicacion_utc,regla_hora,tipo_dia,estado,operado,motivo,\
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
            .ok_or("sin vencimientos: H2 corre sobre futuros")?;
        // Spread de la entrada de las 08:00 ET (fuera de la sesión de contado, §4.2).
        let entry_spread = if voting {
            mcfg.out_of_session_spread
        } else {
            *mcfg
                .descriptive_out_of_session_spread
                .get(&id.to_string())
                .ok_or_else(|| format!("{id}: sin spread de fuera de sesión en [macro_day]"))?
        };
        let (lo, hi) = (
            l.split.windows[0].validation.start,
            l.split.out_of_sample.start,
        );
        if l.months.len() != 153 {
            return Err(format!("{id}: {} meses de validación, no 153", l.months.len()).into());
        }

        // Calendario: solo lo publicado antes del fuera de muestra.
        let mut rows = Vec::new();
        let mut meta: BTreeMap<String, (String, String)> = BTreeMap::new();
        for series in ["cpi", "employment_situation"] {
            for r in store.official_releases("bls", series, hi).await? {
                let status = ReleaseStatus::parse(&r.status)
                    .ok_or_else(|| format!("estado desconocido: {}", r.status))?;
                let key = format!("{series}/{}", r.reference_period);
                meta.insert(key.clone(), (r.time_rule.clone(), series.to_owned()));
                rows.push(CalendarRow {
                    key,
                    status,
                    scheduled_at: r.scheduled_at,
                    published_at: r.published_at,
                });
            }
        }
        let outcomes = macro_day_windows(&p.m15, session, &rows, &mrule);
        // Qué datos salen cada día (en ET): los operados y los que no se operan por ser el mismo día.
        let mut by_day: BTreeMap<chrono::NaiveDate, Vec<String>> = BTreeMap::new();
        for o in &outcomes {
            if let (Ok(_) | Err(Skip::SameDay), Some(t)) = (o.window, o.row.scheduled_at) {
                let d = t.with_timezone(&mrule.timezone).date_naive();
                let series = meta
                    .get(&o.row.key)
                    .map(|m| m.1.clone())
                    .unwrap_or_default();
                by_day.entry(d).or_default().push(series);
            }
        }
        let kind_of = |t: DateTime<Utc>| -> &'static str {
            let d = t.with_timezone(&mrule.timezone).date_naive();
            let v = by_day.get(&d).cloned().unwrap_or_default();
            let cpi = v.iter().any(|s| s == "cpi");
            let emp = v.iter().any(|s| s == "employment_situation");
            match (cpi, emp) {
                (true, true) => "cpi_y_empleo",
                (true, false) => "cpi",
                (false, true) => "empleo",
                _ => "",
            }
        };
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

        // Serie de H2: corrida continua; comprar y mantener: cuenta nueva por ventana.
        let schedule = event_schedule(&windows);
        let spreads: Vec<(usize, Decimal)> =
            windows.iter().map(|w| (w.entry(), entry_spread)).collect();
        let mut ys: [BTreeMap<(i32, u32), Decimal>; 3] = Default::default();
        for (k, stress) in [Decimal::ONE, rule.cost_stress, Decimal::TWO]
            .into_iter()
            .enumerate()
        {
            for (m, r) in managed_returns_with_spreads(
                &p.m15,
                session,
                &rolls,
                &p.setup,
                (lo, hi),
                &schedule,
                stress,
                &spreads,
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
                "H2,{id},{}-{:02},{:.8},{:.8}",
                m.0, m.1, s.y[k], s.y_stress[k]
            );
            let _ = writeln!(
                csv,
                "comprar_y_mantener,{id},{}-{:02},{:.8},{:.8}",
                m.0, m.1, s.x[k], s.x_stress[k]
            );
        }

        let closes = session_close_indices(&p.m15, session);
        // Por evento (descriptivo): bruto (costos × 0), neto y × 1,5 de la misma función.
        let mut traded: Vec<Traded> = Vec::new();
        for o in &outcomes {
            let Ok(w) = o.window else {
                let when = o.row.scheduled_at.unwrap_or(o.row.published_at);
                if when >= lo && when < hi {
                    let (rule_name, _) = meta.get(&o.row.key).cloned().unwrap_or_default();
                    let _ = writeln!(
                        ev_csv,
                        "{id},{},{},{rule_name},{},{:?},no,{:?},,,,,",
                        o.row.key,
                        when.format("%Y-%m-%dT%H:%M:%SZ"),
                        kind_of(when),
                        o.row.status,
                        o.window.err().unwrap_or(Skip::Overlap)
                    );
                }
                continue;
            };
            if !windows.contains(&w) {
                continue;
            }
            // Desde la última vela de sesión del día hábil anterior (sin posición: `managed_returns`
            // arranca en una vela de sesión) hasta el final de la vela de la salida; la entrada es
            // un rebalanceo en la apertura de la vela de las 08:00 ET, con su spread.
            let k = closes.partition_point(|&i| i < w.decision_in);
            let Some(&start) = k.checked_sub(1).and_then(|k| closes.get(k)) else {
                return Err(format!("{}: sin sesión anterior", o.row.key).into());
            };
            let one = [
                (0, Decimal::ZERO),
                (w.decision_in, Decimal::ONE),
                (w.decision_out, Decimal::ZERO),
            ];
            let range = (p.m15[start].start, p.m15[w.exit()].end());
            let one_spread = [(w.entry(), entry_spread)];
            let mut r = [0.0; 3];
            for (k, stress) in [Decimal::ZERO, Decimal::ONE, rule.cost_stress]
                .into_iter()
                .enumerate()
            {
                let months = managed_returns_with_spreads(
                    &p.m15,
                    session,
                    &rolls,
                    &p.setup,
                    range,
                    &one,
                    stress,
                    &one_spread,
                )
                .map_err(|e| format!("{e:?}"))?;
                r[k] = f(compound(&months));
            }
            let (rule_name, _) = meta.get(&o.row.key).cloned().unwrap_or_default();
            let scheduled_at = o.row.scheduled_at.unwrap_or(o.row.published_at);
            let t = Traded {
                key: o.row.key.clone(),
                scheduled_at,
                entry: p.m15[w.entry()].start,
                exit: p.m15[w.exit()].start,
                kind: kind_of(scheduled_at),
                gross: r[0],
                net: r[1],
                net_stress: r[2],
            };
            let _ = writeln!(
                ev_csv,
                "{id},{},{},{rule_name},{},{:?},si,,{},{},{:.8},{:.8},{:.8}",
                t.key,
                t.scheduled_at.format("%Y-%m-%dT%H:%M:%SZ"),
                t.kind,
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
            "════════ H2 en {id} ({}) ════════",
            if voting { "vota" } else { "descriptivo" }
        );
        let pick_by = |keep: &dyn Fn(&Traded) -> bool, g: &dyn Fn(&Traded) -> f64| -> Vec<f64> {
            traded.iter().filter(|t| keep(t)).map(g).collect()
        };
        let split = rule.second_half_from;
        let all = |_: &Traded| true;
        let first = |t: &Traded| t.scheduled_at.date_naive() < split;
        let second = |t: &Traded| t.scheduled_at.date_naive() >= split;
        let cpi = |t: &Traded| t.kind == "cpi";
        let emp = |t: &Traded| t.kind == "empleo";
        let both = |t: &Traded| t.kind == "cpi_y_empleo";
        for (name, keep) in [
            ("todos", &all as &dyn Fn(&Traded) -> bool),
            ("hasta 2020", &first),
            ("desde 2021", &second),
            ("solo CPI", &cpi),
            ("solo empleo", &emp),
            ("CPI y empleo el mismo día", &both),
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
        verdict("H2", &rule, &l.months, &s, voting);
        if let Some(a) = stats::alpha_hc1(&s.y, &s.x) {
            let active: Vec<f64> = s.y.iter().zip(&s.x).map(|(y, x)| y - a.beta * x).collect();
            if let Some(d) = stats::deflated_sharpe(&active, n_now) {
                println!(
                    "     (referencia, N vigente al correr = {n_now}: SR0 {:.4}, DSR {:.4})",
                    d.sharpe0, d.dsr
                );
            }
        }
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

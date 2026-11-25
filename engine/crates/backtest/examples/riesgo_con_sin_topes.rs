//! Backtest de cartera con y sin los topes de riesgo aprobados (#689): la estrategia en uso sobre
//! varios instrumentos en una sola cuenta (`run_portfolio`), con las velas de los futuros
//! (`common/prepare.rs`, igual que `diagnostico --fuente futuros`), corrida cinco veces con todo
//! igual menos `config/risk.toml`:
//!
//! - `sin_topes`: la config del repositorio con todo lo de #670 y #671 apagado (sin colchón por
//!   gaps, sin grupos, sin caída gradual): el comportamiento anterior;
//! - `solo_gaps`, `solo_grupo`, `solo_caida`: una regla por vez, para separar su efecto;
//! - `con_topes`: `config/risk.toml` tal cual.
//!
//! Descriptivo: no es una prueba de hipótesis, no vota y no suma al N del Sharpe deflactado. Solo
//! lee la base (sesión de solo lectura) y no guarda nada en ella. Imprime una tabla en Markdown
//! (retorno, Sharpe diario anualizado, caída máxima, Monte Carlo de la caída por bloques de los
//! retornos mensuales, operaciones achicadas y rechazadas por cada regla) y, con `--csv`, guarda
//! los retornos mensuales (`serie,instrumento,mes,retorno`) para `montecarlo_report`.
//!
//! ```text
//! DATABASE_URL=... cargo run --release -p cfds-backtest --example riesgo_con_sin_topes -- \
//!     <desde UTC> <hasta UTC> <capital inicial> [--instrumentos US500,US100,US30] \
//!     [--csv <archivo>] [--escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv]
//! ```
//! El escenario de financiación (`common/study.rs`) completa en memoria la serie diaria antes de
//! que exista SOFR en la base (EFFR hasta 2018-03), como las rondas y los programas.
#![allow(
    clippy::float_arithmetic,
    reason = "solo el Monte Carlo y los porcentajes para mostrar; el dinero va en Decimal"
)]

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/study.rs"]
mod study;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cfds_backtest::metrics::{daily_returns, max_drawdown, sharpe};
use cfds_backtest::research::ResearchConfig;
use cfds_backtest::research::montecarlo::monte_carlo;
use cfds_backtest::{Market, PortfolioReport, run_portfolio};
use cfds_core::InstrumentId;
use cfds_market_data::CandleSource;
use cfds_risk::{Rejection, RiskConfig};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// La config del repositorio sin lo de #670 y #671 (lo mismo que
/// `engine/crates/risk/tests/neutral_risk.toml`, que un test compara con la del repositorio).
fn neutral(repo: &RiskConfig) -> RiskConfig {
    let mut r = repo.clone();
    r.gap_atr = Decimal::ZERO;
    r.groups.clear();
    r.drawdown_scaling.points.clear();
    r
}

fn variants(repo: &RiskConfig) -> Vec<(&'static str, RiskConfig)> {
    let off = neutral(repo);
    let only = |edit: &dyn Fn(&mut RiskConfig)| {
        let mut r = off.clone();
        edit(&mut r);
        r
    };
    vec![
        ("sin_topes", off.clone()),
        ("solo_gaps", only(&|r| r.gap_atr = repo.gap_atr)),
        ("solo_grupo", only(&|r| r.groups = repo.groups.clone())),
        (
            "solo_caida",
            only(&|r| r.drawdown_scaling = repo.drawdown_scaling.clone()),
        ),
        ("con_topes", repo.clone()),
    ]
}

/// Retorno de cada mes calendario sobre el capital al cierre del mes anterior (el primero, sobre
/// el inicial), desde la curva diaria.
fn monthly_returns(initial: Decimal, curve: &[(NaiveDate, Decimal)]) -> Vec<(String, Decimal)> {
    let mut month_end: BTreeMap<(i32, u32), Decimal> = BTreeMap::new();
    for &(d, e) in curve {
        month_end.insert((d.year(), d.month()), e);
    }
    let mut prev = initial;
    month_end
        .into_iter()
        .map(|((y, m), e)| {
            let r = e / prev - Decimal::ONE;
            prev = e;
            (format!("{y:04}-{m:02}"), r)
        })
        .collect()
}

fn pct(x: Decimal) -> String {
    format!("{:.2}", x * Decimal::ONE_HUNDRED)
}

struct Row {
    name: &'static str,
    trades: usize,
    final_equity: Decimal,
    total_return: Decimal,
    sharpe: Option<Decimal>,
    max_dd: Decimal,
    mc_observed: f64,
    mc_p50: f64,
    mc_p95: f64,
    mc_ruin: f64,
    kill_switches: usize,
    rejected: BTreeMap<String, usize>,
    group_capped: usize,
    scaled: usize,
    /// Parte media del riesgo de cada operación que es colchón por gaps.
    gap_share: Decimal,
    /// Riesgo medio por operación, en % del capital al decidir.
    risk_pct: Decimal,
    months: Vec<(String, Decimal)>,
}

fn summarize(
    name: &'static str,
    report: &PortfolioReport,
    initial: Decimal,
    research: &ResearchConfig,
    metrics: &cfds_backtest::MetricsConfig,
) -> Result<Row, Box<dyn std::error::Error>> {
    let returns = daily_returns(initial, &report.daily_equity);
    let months = monthly_returns(initial, &report.daily_equity);
    let xs: Vec<f64> = months
        .iter()
        .map(|(_, r)| r.to_f64().ok_or("retorno no representable"))
        .collect::<Result<_, _>>()?;
    let mc = monte_carlo(&xs, 1.0, &research.montecarlo).ok_or("Monte Carlo inválido")?;
    let mut rejected = BTreeMap::new();
    for r in &report.rejected {
        let key = match &r.rejected.reason {
            Rejection::InvalidInput(_) => "InvalidInput".to_owned(),
            other => format!("{other:?}"),
        };
        *rejected.entry(key).or_insert(0) += 1;
    }
    let n = report.trades.len();
    let (mut gap_share, mut risk_pct) = (Decimal::ZERO, Decimal::ZERO);
    for t in &report.trades {
        let r = (t.record.reference - t.record.stop).abs();
        let per_unit = t.sized_risk / t.record.size;
        gap_share += (per_unit - r) / per_unit;
        // Monedas: todos los instrumentos de la corrida están en la moneda de la cuenta.
        risk_pct += t.sized_risk / t.equity_at_entry;
    }
    let mean = |x: Decimal| {
        if n == 0 {
            Decimal::ZERO
        } else {
            x / Decimal::from(n)
        }
    };
    Ok(Row {
        name,
        trades: n,
        final_equity: report.final_equity,
        total_return: report.final_equity / initial - Decimal::ONE,
        sharpe: sharpe(&returns, metrics),
        max_dd: max_drawdown(initial, &report.daily_equity),
        mc_observed: mc.observed.max_drawdown,
        mc_p50: mc.max_drawdown.p50,
        mc_p95: mc.max_drawdown.p95,
        mc_ruin: mc.risk_of_ruin,
        kill_switches: report
            .kill_switch_events
            .iter()
            .filter(|e| matches!(e.kind, cfds_backtest::KillSwitchEventKind::Activated))
            .count(),
        rejected,
        group_capped: report.trades.iter().filter(|t| t.group_capped).count(),
        scaled: report
            .trades
            .iter()
            .filter(|t| t.risk_scale < Decimal::ONE)
            .count(),
        gap_share: mean(gap_share),
        risk_pct: mean(risk_pct),
        months,
    })
}

fn take(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    // El estudio se lee de nuevo para cada instrumento (cada preparación se queda con el suyo).
    let (_, mut args) = study::parse(raw.clone())?;
    let instruments = take(&mut args, "--instrumentos").unwrap_or("US500,US100,US30".into());
    let csv = take(&mut args, "--csv");
    let [from, to, equity] = args.as_slice() else {
        return Err(
            "uso: <desde UTC> <hasta UTC> <capital inicial> [--instrumentos A,B] \
                    [--csv <archivo>]"
                .into(),
        );
    };
    let (from, to): (DateTime<Utc>, DateTime<Utc>) = (from.parse()?, to.parse()?);
    let equity: Decimal = equity.parse()?;
    let ids: Vec<InstrumentId> = instruments
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;

    let mut prepared = Vec::new();
    for id in &ids {
        println!("## Preparando {id}");
        prepared.push(
            prepare::prepare_with(
                id,
                CandleSource::Futures,
                (from, to),
                equity,
                None,
                false,
                study::parse(raw.clone())?.0,
            )
            .await?,
        );
    }
    let repo = RiskConfig::from_toml(cfg!("risk.toml"))?;
    let research = ResearchConfig::from_toml(cfg!("research.toml"))?;
    let metrics = prepared[0].setup.backtest.metrics;

    let mut rows = Vec::new();
    for (name, risk) in variants(&repo) {
        let markets: Vec<Market> = prepared
            .iter()
            .map(|p| {
                let mut setup = p.setup.clone();
                setup.risk = risk.clone();
                Market {
                    setup,
                    m15: &p.m15,
                    h1: &p.h1,
                    entries: &p.entries,
                    force_close: &p.force_close,
                    data_errors: &p.data_errors,
                    news: &p.news,
                }
            })
            .collect();
        eprintln!("corriendo {name}…");
        let report = run_portfolio(&markets).map_err(|e| format!("{name}: {e:?}"))?;
        rows.push(summarize(name, &report, equity, &research, &metrics)?);
    }

    let mut out = String::new();
    writeln!(
        out,
        "\nCartera {instruments}, {from} a {to}, capital inicial {equity} (velas de futuros).\n"
    )?;
    writeln!(
        out,
        "| Variante | Operaciones | Capital final | Retorno total (%) | Sharpe diario anualizado \
         | Caída máx. (%) | MC caída p50 (%) | MC caída p95 (%) | MC ruina (%) | Kill switch \
         | Achicadas por grupo | Rechazadas por grupo | Con caída gradual (< 1) | Colchón medio \
         (% del riesgo) | Riesgo medio (% capital) |"
    )?;
    writeln!(
        out,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    )?;
    for r in &rows {
        writeln!(
            out,
            "| {} | {} | {:.2} | {} | {} | {} | {:.2} | {:.2} | {:.2} | {} | {} | {} | {} | {} | {} |",
            r.name,
            r.trades,
            r.final_equity,
            pct(r.total_return),
            r.sharpe.map_or("-".into(), |s| format!("{s:.3}")),
            pct(r.max_dd),
            r.mc_p50 * 100.0,
            r.mc_p95 * 100.0,
            r.mc_ruin * 100.0,
            r.kill_switches,
            r.group_capped,
            r.rejected.get("GroupRiskLimit").copied().unwrap_or(0),
            r.scaled,
            pct(r.gap_share),
            pct(r.risk_pct),
        )?;
    }
    writeln!(out, "\nRechazos del riesgo por motivo:\n")?;
    for r in &rows {
        writeln!(out, "- {}: {:?}", r.name, r.rejected)?;
    }
    writeln!(
        out,
        "\nMonte Carlo: `[montecarlo]` de config/research.toml sobre los {} retornos mensuales \
         de cada variante (caída observada en la serie mensual: {}).",
        rows.first().map_or(0, |r| r.months.len()),
        rows.iter()
            .map(|r| format!("{} {:.2} %", r.name, r.mc_observed * 100.0))
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    println!("{out}");

    if let Some(path) = csv {
        let mut text = String::from("serie,instrumento,mes,retorno\n");
        for r in &rows {
            for (m, x) in &r.months {
                writeln!(text, "{},cartera,{m},{x}", r.name)?;
            }
        }
        std::fs::write(&path, text)?;
        println!("retornos mensuales guardados en {path}");
    }
    Ok(())
}

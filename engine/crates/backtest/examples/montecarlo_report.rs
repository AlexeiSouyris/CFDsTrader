//! Monte Carlo por bloques de una serie de retornos ya calculada (#662): distribución de la caída
//! máxima, la racha perdedora, el tiempo bajo el agua, el retorno final y el riesgo de ruina, con
//! `[montecarlo]` de `config/research.toml`. Descriptivo: no decide nada, no lee la base y no
//! agrega filas al registro de resultados.
//!
//! ```text
//! cargo run --release -p cfds-backtest --example montecarlo_report -- \
//!     docs/analysis/2026-11-ronda3-retornos-eur.csv comprar_y_mantener \
//!     [--instrumento US500] [--columna retorno] [--fraccion 1] [--bloque 5]
//! ```
//! El CSV es el que guardan las rondas y los programas: la primera columna es la serie (familia),
//! y tiene `instrumento`, `mes` y la columna de retornos. Sin `--instrumento`, el retorno de cada
//! mes es el promedio de los instrumentos de ese mes (cartera de igual peso). `--fraccion` es la
//! parte del retorno que se aplica al capital: 1 con retornos mensuales sobre el capital, el
//! riesgo por operación (0.01) con una serie en R.
#![allow(
    clippy::float_arithmetic,
    reason = "solo estadísticos y porcentajes para mostrar; no es dinero"
)]

use std::collections::BTreeMap;

use cfds_backtest::research::{ResearchConfig, monte_carlo};

fn take(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
}

/// Retornos por mes de la serie `label` (promedio de los instrumentos si hay más de uno).
fn monthly(
    text: &str,
    label: &str,
    instrument: Option<&str>,
    column: &str,
) -> Result<Vec<(String, f64)>, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("CSV vacío")?.split(',').collect();
    let col = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .ok_or(format!("falta la columna {name}"))
    };
    let (i_inst, i_month, i_val) = (col("instrumento")?, col("mes")?, col(column)?);
    let mut by_month: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split(',').collect();
        if f.first() != Some(&label) || instrument.is_some_and(|i| f.get(i_inst) != Some(&i)) {
            continue;
        }
        let v: f64 = f
            .get(i_val)
            .and_then(|v| v.parse().ok())
            .ok_or(format!("valor inválido en: {line}"))?;
        let month = f.get(i_month).ok_or(format!("sin mes en: {line}"))?;
        let e = by_month.entry((*month).to_owned()).or_insert((0.0, 0));
        e.0 += v;
        e.1 += 1;
    }
    Ok(by_month
        .into_iter()
        .map(|(m, (s, n))| (m, s / n as f64))
        .collect())
}

fn main() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let instrument = take(&mut args, "--instrumento");
    let column = take(&mut args, "--columna").unwrap_or_else(|| "retorno".into());
    let fraction: f64 = take(&mut args, "--fraccion")
        .map_or(Ok(1.0), |v| v.parse())
        .map_err(|e| format!("--fraccion: {e}"))?;
    let block = take(&mut args, "--bloque")
        .map(|v| v.parse::<usize>())
        .transpose()
        .map_err(|e| format!("--bloque: {e}"))?;
    let [path, label] = args.as_slice() else {
        return Err("uso: montecarlo_report <archivo.csv> <serie> [--instrumento X] [--columna retorno] [--fraccion 1] [--bloque N]".into());
    };
    let research = ResearchConfig::from_toml(include_str!("../../../../config/research.toml"))?;
    let mut cfg = research.montecarlo;
    if let Some(b) = block {
        cfg.block_len = b;
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let series = monthly(&text, label, instrument.as_deref(), &column)?;
    let xs: Vec<f64> = series.iter().map(|(_, v)| *v).collect();
    let report = monte_carlo(&xs, fraction, &cfg)
        .ok_or("serie vacía o inválida, o config de Monte Carlo inválida")?;
    let (first, last) = (series.first().map(|s| &s.0), series.last().map(|s| &s.0));
    println!(
        "Serie {label} ({}), columna {column}: {} meses ({} a {}), fracción {fraction}",
        instrument.as_deref().unwrap_or("promedio de instrumentos"),
        xs.len(),
        first.map_or("-", |s| s.as_str()),
        last.map_or("-", |s| s.as_str()),
    );
    println!(
        "{} caminos, bloques circulares de {}, semilla {}, ruina = −{} % del capital inicial",
        report.paths,
        cfg.block_len,
        cfg.seed,
        cfg.ruin_threshold * rust_decimal::Decimal::ONE_HUNDRED
    );
    println!(
        "{:<28}{:>10}{:>10}{:>10}{:>10}",
        "", "real", "p5", "p50", "p95"
    );
    let o = report.observed;
    let rows = [
        (
            "Caída máxima (%)",
            o.max_drawdown * 100.0,
            report.max_drawdown,
            100.0,
        ),
        (
            "Racha perdedora (períodos)",
            o.longest_losing_streak as f64,
            report.longest_losing_streak,
            1.0,
        ),
        (
            "Bajo el agua (períodos)",
            o.longest_under_water as f64,
            report.longest_under_water,
            1.0,
        ),
        (
            "Retorno final (%)",
            o.final_return * 100.0,
            report.final_return,
            100.0,
        ),
    ];
    for (name, real, s, k) in rows {
        println!(
            "{name:<28}{real:>10.2}{:>10.2}{:>10.2}{:>10.2}",
            s.p5 * k,
            s.p50 * k,
            s.p95 * k
        );
    }
    println!(
        "Riesgo de ruina: {:.2} % de los caminos (la serie real {})",
        report.risk_of_ruin * 100.0,
        if o.ruined {
            "sí la tocó"
        } else {
            "no la tocó"
        }
    );
    Ok(())
}

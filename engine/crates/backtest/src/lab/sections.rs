//! Secciones del informe del laboratorio (`lab_report.v1`): cada una arma un bloque del JSON con
//! lo que ya existe (`stats`, `pbo_cscv`, `monte_carlo`, `wilson`), sin cálculos nuevos de
//! trading. Las junta [`super::report::lab_report`].
//!
//! Números como texto decimal con 6 decimales; `null` cuando algo no está definido (nunca un
//! número inventado). R de una operación = P/L / riesgo (el que dio el módulo de riesgo).
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de una muestra de retornos, como research::stats: no son dinero"
)]

use std::collections::BTreeMap;

use cfds_indicators::wilson;
use cfds_strategy::Side;
use chrono::DateTime;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::research::{
    MonteCarloConfig, StudyExit, StudyRun, StudyTrade, Summary, monte_carlo, pbo_cscv, stats,
};

/// `[report]` de `config/lab.toml`: convenciones del informe, no parámetros de la estrategia.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportConfig {
    /// Nivel de los intervalos (0,95).
    pub confidence: Decimal,
    /// Ancho de los tramos del histograma de R.
    pub r_step: Decimal,
    /// Entradas al azar del test de suerte.
    pub luck_simulations: usize,
}

/// Texto decimal con 6 decimales como mucho, sin ceros de más (y nunca "-0").
pub fn text(d: Decimal) -> Value {
    let d = d.round_dp(6).normalize();
    Value::String(if d.is_zero() {
        "0".into()
    } else {
        d.to_string()
    })
}

/// Un estadístico como texto decimal (6 decimales), o null si no está definido.
pub fn num(x: Option<f64>) -> Value {
    x.filter(|v| v.is_finite())
        .and_then(Decimal::from_f64)
        .map_or(Value::Null, text)
}

/// El decimal como `f64` para la estadística (NaN si no entra).
pub fn f(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(f64::NAN)
}

/// Hora UTC del contrato (`…Z`, en segundos).
pub fn utc(t: DateTime<Utc>) -> Value {
    Value::String(t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Media y cuantiles 5, 50 y 95 % (de Monte Carlo o del PBO).
pub fn summary(s: &Summary) -> Value {
    json!({ "mean": num(Some(s.mean)), "p5": num(Some(s.p5)), "p50": num(Some(s.p50)),
        "p95": num(Some(s.p95)) })
}

/// Cuantiles 25/50/75/90 (tipo 7), o null sin datos.
pub fn quantiles(mut xs: Vec<f64>) -> Value {
    xs.sort_by(f64::total_cmp);
    let q = |p| crate::research::montecarlo::quantile_sorted(&xs, p);
    match q(0.25) {
        None => Value::Null,
        Some(p25) => json!({ "p25": num(Some(p25)), "p50": num(q(0.5)), "p75": num(q(0.75)),
            "p90": num(q(0.9)) }),
    }
}

/// Sharpe y Sortino por período (sin anualizar); `None` sin desvío.
pub fn sharpe_sortino(xs: &[f64]) -> (Option<f64>, Option<f64>) {
    let (m, s) = (stats::mean(xs), stats::std_dev(xs));
    let down =
        stats::mean(&xs.iter().map(|x| x.min(0.0).powi(2)).collect::<Vec<_>>()).map(f64::sqrt);
    let ratio = |a: Option<f64>, b: Option<f64>| a.zip(b.filter(|b| *b > 0.0)).map(|(a, b)| a / b);
    (ratio(m, s), ratio(m, down))
}

/// Capital y caída de cada período de una serie compuesta desde 1, la caída máxima y el retorno
/// final.
pub fn curve(xs: &[f64]) -> (Vec<(f64, f64)>, f64, f64) {
    let (mut eq, mut peak, mut worst) = (1.0_f64, 1.0_f64, 0.0_f64);
    let points = xs
        .iter()
        .map(|r| {
            eq *= 1.0 + r;
            peak = peak.max(eq);
            worst = worst.max(1.0 - eq / peak);
            (eq, 1.0 - eq / peak)
        })
        .collect();
    (points, worst, eq - 1.0)
}

/// Un corte: operaciones, R neto medio y aciertos.
pub fn slices<K: Ord + ToString>(items: impl Iterator<Item = (K, f64)>) -> Value {
    let mut by: BTreeMap<K, Vec<f64>> = BTreeMap::new();
    for (k, r) in items {
        by.entry(k).or_default().push(r);
    }
    let rows: Vec<Value> = by
        .into_iter()
        .map(|(k, rs)| {
            let wins = rs.iter().filter(|r| **r > 0.0).count() as f64;
            json!({ "key": k.to_string(), "trades": rs.len(), "mean_r_net": num(stats::mean(&rs)),
                "win_rate": num(Some(wins / rs.len() as f64)) })
        })
        .collect();
    Value::Array(rows)
}

/// Histograma del R neto y bruto en tramos `[k·step, (k+1)·step)`.
pub fn histogram(net: &[f64], gross: &[f64], step: Decimal) -> Value {
    let w = f(step);
    let all = net.iter().chain(gross);
    let (Some(lo), Some(hi)) = (
        all.clone().map(|x| (x / w).floor() as i64).min(),
        all.map(|x| (x / w).floor() as i64).max(),
    ) else {
        return json!([]);
    };
    let count = |xs: &[f64], k: i64| xs.iter().filter(|x| (*x / w).floor() as i64 == k).count();
    let bins: Vec<Value> = (lo..=hi)
        .map(|k| {
            json!({ "from": text(step * Decimal::from(k)), "to": text(step * Decimal::from(k + 1)),
                "net": count(net, k), "gross": count(gross, k) })
        })
        .collect();
    Value::Array(bins)
}

/// R de un P/L de la operación: P/L / riesgo (NaN si el riesgo es cero).
pub fn r_of(t: &StudyTrade, pnl: Decimal) -> f64 {
    if t.risk.is_zero() {
        f64::NAN
    } else {
        f(pnl / t.risk)
    }
}

/// Mes `AAAA-MM`.
pub fn month(y: i32, m: u32) -> String {
    format!("{y}-{m:02}")
}

/// PBO por CSCV de las variantes corridas juntas (columnas) sobre los mismos meses, desde 4
/// variantes; si no se puede, `null` y el motivo en `notes`.
pub fn pbo_section(variants: &[Vec<f64>], partitions: usize, notes: &mut Vec<String>) -> Value {
    if variants.len() < 4 {
        notes.push(format!(
            "PBO: hacen falta 4 variantes de la familia corridas juntas (hay {})",
            variants.len()
        ));
        return Value::Null;
    }
    let months = variants.iter().map(Vec::len).min().unwrap_or(0);
    let matrix: Vec<Vec<f64>> = (0..months)
        .map(|k| variants.iter().map(|x| x[k]).collect())
        .collect();
    let Some(p) = pbo_cscv(&matrix, partitions) else {
        notes.push("PBO: pocos meses para las particiones de config".into());
        return Value::Null;
    };
    json!({
        "variants": variants.len(), "partitions": partitions, "pbo": num(Some(p.pbo)),
        "combinations": p.combinations, "logits": summary(&p.logits),
        "oos_loss": num(Some(p.oos_loss)),
    })
}

/// Monte Carlo por bloques de los retornos por operación (fracción 1: ya son sobre el capital).
pub fn monte_carlo_block(returns: &[f64], cfg: &MonteCarloConfig) -> Value {
    let Some(r) = monte_carlo(returns, 1.0, cfg) else {
        return Value::Null;
    };
    json!({
        "paths": r.paths, "block_len": cfg.block_len, "ruin_threshold": text(cfg.ruin_threshold),
        "max_drawdown": summary(&r.max_drawdown),
        "longest_losing_streak": summary(&r.longest_losing_streak),
        "longest_under_water": summary(&r.longest_under_water),
        "final_return": summary(&r.final_return), "risk_of_ruin": num(Some(r.risk_of_ruin)),
        "observed_max_drawdown": num(Some(r.observed.max_drawdown)),
    })
}

/// R neto medio con IC normal, R bruto, win rate con IC de Wilson y profit factor.
pub fn expectancy_block(net: &[f64], gross: &[f64], confidence: Decimal) -> Value {
    let n = net.len();
    if n == 0 {
        return Value::Null;
    }
    let wins = net.iter().filter(|r| **r > 0.0).count();
    let z = stats::normal_inv(1.0 - (1.0 - f(confidence)) / 2.0);
    let mean = stats::mean(net);
    let half = stats::std_dev(net).map(|s| z * s / (n as f64).sqrt());
    let ci = mean.zip(half).map_or(
        Value::Null,
        |(m, h)| json!({ "low": num(Some(m - h)), "high": num(Some(m + h)) }),
    );
    let as_u32 = |x: usize| u32::try_from(x).unwrap_or(u32::MAX);
    let z_dec = Decimal::from_f64(z).unwrap_or(Decimal::TWO);
    let (lo, hi) = wilson(as_u32(wins), as_u32(n), z_dec);
    let gain: f64 = net.iter().filter(|r| **r > 0.0).sum();
    let loss: f64 = net.iter().filter(|r| **r < 0.0).sum();
    json!({
        "mean_r_net": num(mean), "mean_r_net_ci": ci, "mean_r_gross": num(stats::mean(gross)),
        "win_rate": num(Some(wins as f64 / n as f64)),
        "win_rate_ci": { "low": text(lo), "high": text(hi) },
        "profit_factor": num((loss < 0.0).then(|| gain / -loss)),
        "confidence": text(confidence),
    })
}

/// Operaciones, lados, salidas por motivo y rechazos del riesgo por motivo.
pub fn trades_block(run: &StudyRun) -> Value {
    let t = &run.trades;
    let side = |s: Side| t.iter().filter(|x| x.side == s).count();
    let exits = |e: StudyExit| t.iter().filter(|x| x.exit == e).count();
    let mut rejected: BTreeMap<String, usize> = BTreeMap::new();
    for r in &run.rejected {
        *rejected.entry(format!("{:?}", r.reason)).or_default() += 1;
    }
    let rejected: Vec<Value> = rejected
        .into_iter()
        .map(|(reason, count)| json!({ "reason": reason, "count": count }))
        .collect();
    json!({
        "count": t.len(), "long": side(Side::Long), "short": side(Side::Short),
        "exits": {
            "stop": exits(StudyExit::Stop), "stop_gap": exits(StudyExit::StopGap),
            "target": exits(StudyExit::Target), "planned": exits(StudyExit::Planned),
            "force_close": exits(StudyExit::ForceClose),
        },
        "rejected": rejected, "through_stop": run.through_stop.len(),
    })
}

/// Avisos fijos del informe (exploratorio, pocas operaciones, spread histórico, cortes).
pub fn notes(n: usize) -> Vec<String> {
    let mut notes =
        vec!["exploratorio, no vota: ninguna cifra cuenta para el N del proyecto".into()];
    if n < 30 {
        notes.push(format!(
            "pocas operaciones ({n}): los intervalos son anchos y nada de esto es concluyente"
        ));
    }
    notes.push(
        "con velas de futuros los costos usan el spread típico de config (sin spread medido del \
         CFD): los años viejos pueden pagar de más (C5 del autoanálisis)"
            .into(),
    );
    notes.push("mirar cortes es probar más: no sirven para elegir".into());
    notes
}

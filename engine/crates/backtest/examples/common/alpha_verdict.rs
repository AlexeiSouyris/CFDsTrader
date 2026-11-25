//! Los cinco criterios del alfa contra comprar y mantener que comparten el programa Nasdaq (§3.4)
//! y el programa 4 (§7). Solo presentación: la estadística vive en `cfds_backtest::research::stats`.
#![allow(
    clippy::float_arithmetic,
    reason = "solo estadísticos y porcentajes para mostrar; el dinero se calcula en Decimal"
)]

use cfds_backtest::research::{RuleConfig, stats};
use chrono::Datelike;

use super::research_round::{f, mean};

pub fn sharpe(xs: &[f64]) -> f64 {
    match (stats::mean(xs), stats::std_dev(xs)) {
        (Some(m), Some(s)) if s > 0.0 => m / s,
        _ => f64::NAN,
    }
}

/// Series mensuales de una hipótesis (`y`) y de comprar y mantener (`x`), con los costos × 1,5 y × 2.
pub struct Series {
    pub y: Vec<f64>,
    pub y_stress: Vec<f64>,
    pub y_double: Vec<f64>,
    pub x: Vec<f64>,
    pub x_stress: Vec<f64>,
    pub x_double: Vec<f64>,
}

/// Los cinco criterios del alfa contra comprar y mantener (programa Nasdaq §3.4, programa 4 §7)
/// sobre una serie `name`. Devuelve si pasa los cinco.
pub fn verdict(
    name: &str,
    rule: &RuleConfig,
    months: &[(i32, u32)],
    s: &Series,
    voting: bool,
) -> bool {
    let alpha = f(rule.alpha);
    println!(
        "  {name} {:+.4} % por mes (Sharpe {:.4}, anual ≈ {:.2}; meses con posición {}); comprar y \
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

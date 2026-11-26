//! Secciones del informe del laboratorio (#756): números como texto, curva, Sharpe y Sortino,
//! histograma, cuantiles, cortes, esperanza, PBO, Monte Carlo, operaciones y avisos.

use cfds_backtest::lab::sections::{
    curve, expectancy_block, histogram, monte_carlo_block, notes, num, pbo_section, quantiles,
    sharpe_sortino, slices, text, trades_block,
};
use cfds_backtest::research::{MonteCarloConfig, StudyRun};
use rust_decimal::dec;
use serde_json::{Value, json};

#[test]
fn numbers_are_decimal_text_or_null() {
    assert_eq!(text(dec!(1.50)), json!("1.5"));
    assert_eq!(text(dec!(0.1234567)), json!("0.123457"));
    assert_eq!(text(dec!(-0.0000001)), json!("0"));
    assert_eq!(num(Some(-0.25)), json!("-0.25"));
    assert_eq!(num(Some(f64::NAN)), Value::Null);
    assert_eq!(num(Some(f64::INFINITY)), Value::Null);
    assert_eq!(num(None), Value::Null);
}

#[test]
fn the_curve_compounds_and_measures_the_drawdown() {
    let (points, worst, total) = curve(&[0.1, -0.5, 0.2]);
    assert!((points[1].0 - 0.55).abs() < 1e-12);
    assert!((points[1].1 - 0.5).abs() < 1e-12);
    assert!((worst - 0.5).abs() < 1e-12);
    assert!((total - (1.1 * 0.5 * 1.2 - 1.0)).abs() < 1e-12);
    assert_eq!(curve(&[]).1, 0.0);
}

#[test]
fn sharpe_and_sortino_need_dispersion() {
    let (sharpe, sortino) = sharpe_sortino(&[0.01, -0.01, 0.03]);
    // Media 0,01, desvío muestral 0,02; caída: √(0,0001 / 3).
    assert!((sharpe.unwrap() - 0.5).abs() < 1e-12);
    assert!((sortino.unwrap() - 0.01 / (0.0001_f64 / 3.0).sqrt()).abs() < 1e-9);
    assert_eq!(sharpe_sortino(&[0.01, 0.01]), (None, None));
    assert_eq!(sharpe_sortino(&[]), (None, None));
}

#[test]
fn the_histogram_counts_net_and_gross_per_step() {
    let h = histogram(&[-1.0, -0.2, 0.7], &[-0.9, 0.1, 0.8], dec!(0.5));
    assert_eq!(
        h,
        json!([
            { "from": "-1", "to": "-0.5", "net": 1, "gross": 1 },
            { "from": "-0.5", "to": "0", "net": 1, "gross": 0 },
            { "from": "0", "to": "0.5", "net": 0, "gross": 1 },
            { "from": "0.5", "to": "1", "net": 1, "gross": 1 }
        ])
    );
    assert_eq!(histogram(&[], &[], dec!(0.5)), json!([]));
}

#[test]
fn quantiles_and_slices_describe_without_inventing() {
    assert_eq!(
        quantiles(vec![4.0, 1.0, 3.0, 2.0, 5.0]),
        json!({ "p25": "2", "p50": "3", "p75": "4", "p90": "4.6" })
    );
    assert_eq!(quantiles(Vec::new()), Value::Null);
    let s = slices([("b", 1.0), ("a", -1.0), ("b", -0.5), ("b", 2.0)].into_iter());
    assert_eq!(
        s,
        json!([
            { "key": "a", "trades": 1, "mean_r_net": "-1", "win_rate": "0" },
            { "key": "b", "trades": 3, "mean_r_net": "0.833333", "win_rate": "0.666667" }
        ])
    );
}

#[test]
fn the_expectancy_has_intervals_and_no_made_up_numbers() {
    let e = expectancy_block(&[1.5, -1.0, -1.0, 2.0], &[1.6, -0.9, -0.9, 2.1], dec!(0.95));
    assert_eq!(e["mean_r_net"], json!("0.375"));
    assert_eq!(e["mean_r_gross"], json!("0.475"));
    assert_eq!(e["win_rate"], json!("0.5"));
    assert_eq!(e["profit_factor"], json!("1.75"));
    assert_eq!(e["confidence"], json!("0.95"));
    let low: f64 = e["mean_r_net_ci"]["low"].as_str().unwrap().parse().unwrap();
    let high: f64 = e["mean_r_net_ci"]["high"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(low < 0.375 && 0.375 < high);
    let w_low: f64 = e["win_rate_ci"]["low"].as_str().unwrap().parse().unwrap();
    assert!(w_low > 0.0 && w_low < 0.5);
    // Una sola operación: sin desvío no hay IC; sin pérdidas no hay profit factor.
    let one = expectancy_block(&[1.0], &[1.1], dec!(0.95));
    assert_eq!(one["mean_r_net_ci"], Value::Null);
    assert_eq!(one["profit_factor"], Value::Null);
    assert_eq!(expectancy_block(&[], &[], dec!(0.95)), Value::Null);
}

#[test]
fn the_pbo_needs_four_variants_and_enough_months() {
    let mut notes = Vec::new();
    let three = vec![vec![0.01; 40]; 3];
    assert_eq!(pbo_section(&three, 4, &mut notes), Value::Null);
    assert!(notes[0].contains("4 variantes"));
    // Cuatro variantes con 40 meses: la mejor del entrenamiento es la de la tendencia.
    let variants: Vec<Vec<f64>> = (0..4)
        .map(|v| {
            (0..40)
                .map(|k| 0.001 * f64::from(v) + 0.01 * f64::from((k * 7 + v * 3) % 5) - 0.02)
                .collect()
        })
        .collect();
    let p = pbo_section(&variants, 4, &mut notes);
    assert_eq!(p["variants"], json!(4));
    assert_eq!(p["partitions"], json!(4));
    assert_eq!(p["combinations"], json!(6));
    assert!(p["pbo"].is_string() && p["logits"]["p50"].is_string());
    // Pocos meses para las particiones: null y el motivo.
    let short: Vec<Vec<f64>> = variants.iter().map(|v| v[..3].to_vec()).collect();
    assert_eq!(pbo_section(&short, 4, &mut notes), Value::Null);
    assert!(notes.last().unwrap().contains("pocos meses"));
}

#[test]
fn monte_carlo_is_null_without_trades() {
    let cfg = MonteCarloConfig {
        paths: 200,
        block_len: 2,
        seed: 7,
        ruin_threshold: dec!(0.3),
    };
    assert_eq!(monte_carlo_block(&[], &cfg), Value::Null);
    let m = monte_carlo_block(&[0.01, -0.02, 0.015, -0.005, 0.02], &cfg);
    assert_eq!(m["paths"], json!(200));
    assert_eq!(m["block_len"], json!(2));
    assert_eq!(m["ruin_threshold"], json!("0.3"));
    assert_eq!(m["risk_of_ruin"], json!("0"));
    assert_eq!(
        m,
        monte_carlo_block(&[0.01, -0.02, 0.015, -0.005, 0.02], &cfg)
    );
}

#[test]
fn an_empty_run_and_the_fixed_notes() {
    let t = trades_block(&StudyRun::default());
    assert_eq!(t["count"], json!(0));
    assert_eq!(t["exits"]["stop"], json!(0));
    assert_eq!(t["rejected"], json!([]));
    let few = notes(5);
    assert!(few[0].contains("exploratorio, no vota"));
    assert!(few.iter().any(|n| n.contains("pocas operaciones")));
    assert!(!notes(30).iter().any(|n| n.contains("pocas operaciones")));
}

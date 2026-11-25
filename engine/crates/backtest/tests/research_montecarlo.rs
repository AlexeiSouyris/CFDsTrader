//! Monte Carlo por bloques (#662): casos calculados a mano, determinismo y bordes.
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de caminos simulados: no son dinero"
)]

use cfds_backtest::research::ResearchConfig;
use cfds_backtest::research::montecarlo::{
    MonteCarloConfig, block_bootstrap, monte_carlo, path_stats, quantile_sorted, summarize,
};
use cfds_backtest::research::program4::SplitMix64;
use rust_decimal::Decimal;

fn cfg(paths: usize, block_len: usize, seed: u64) -> MonteCarloConfig {
    MonteCarloConfig {
        paths,
        block_len,
        seed,
        ruin_threshold: Decimal::new(30, 2),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

/// Serie de prueba con algo de todo: subas, bajas y una racha de tres pérdidas.
fn series() -> Vec<f64> {
    vec![
        0.03, -0.02, 0.01, -0.04, -0.01, -0.03, 0.05, 0.02, -0.02, 0.04, 0.00, -0.05, 0.06,
    ]
}

#[test]
fn path_by_hand_full_fraction() {
    // 1 → 1,1 → 0,55 → 0,66. Pico 1,1; caída 0,5; dos períodos bajo el agua; ruina (≤ 0,70).
    let s = path_stats(&[0.1, -0.5, 0.2], 1.0, 0.7);
    assert!(close(s.max_drawdown, 0.5));
    assert_eq!(s.longest_losing_streak, 1);
    assert_eq!(s.longest_under_water, 2);
    assert!(close(s.final_return, 0.66 - 1.0));
    assert!(s.ruined);
}

#[test]
fn path_by_hand_fixed_fraction() {
    // Fracción 0,1: 1 → 1,01 → 0,9595 → 0,97869. Caída 1 − 0,9595/1,01 = 0,05, sin ruina.
    let s = path_stats(&[0.1, -0.5, 0.2], 0.1, 0.7);
    assert!(close(s.max_drawdown, 0.05));
    assert!(close(s.final_return, 0.97869 - 1.0));
    assert!(!s.ruined);
}

#[test]
fn streak_and_under_water_count_separately() {
    // Retorno 0 no corta lo bajo el agua pero sí la racha perdedora; volver al pico la corta.
    let s = path_stats(&[-0.1, -0.1, 0.0, -0.1, 0.5, -0.01], 1.0, 0.0);
    assert_eq!(s.longest_losing_streak, 2);
    assert_eq!(s.longest_under_water, 4);
    assert!(!s.ruined);
}

#[test]
fn equity_never_below_zero() {
    let s = path_stats(&[-2.0, 0.5], 1.0, 0.7);
    assert!(close(s.final_return, -1.0));
    assert!(close(s.max_drawdown, 1.0));
    assert!(s.ruined);
}

#[test]
fn quantile_type_7() {
    let xs = [1.0, 2.0, 3.0, 4.0, 5.0];
    assert_eq!(quantile_sorted(&xs, 0.5), Some(3.0));
    assert!(close(quantile_sorted(&xs, 0.05).unwrap(), 1.2));
    assert!(close(quantile_sorted(&xs, 0.95).unwrap(), 4.8));
    assert_eq!(quantile_sorted(&[7.0], 0.95), Some(7.0));
    assert_eq!(quantile_sorted(&[], 0.5), None);
    let s = summarize(vec![5.0, 1.0, 3.0, 2.0, 4.0]).unwrap();
    assert_eq!((s.mean, s.p50), (3.0, 3.0));
}

#[test]
fn bootstrap_uses_circular_blocks() {
    let xs: Vec<f64> = (0..10).map(f64::from).collect();
    let mut rng = SplitMix64::new(7);
    let path = block_bootstrap(&mut rng, &xs, 4);
    assert_eq!(path.len(), xs.len());
    // Dentro de cada bloque los valores son consecutivos (módulo 10).
    for block in path.chunks(4) {
        for w in block.windows(2) {
            assert!(close(w[1], (w[0] + 1.0) % 10.0));
        }
    }
}

#[test]
fn whole_series_blocks_keep_the_final_return() {
    // Con bloques del largo de la serie cada camino es una rotación: el producto no cambia.
    let xs = series();
    let r = monte_carlo(&xs, 1.0, &cfg(200, xs.len(), 1)).unwrap();
    let f = r.observed.final_return;
    assert!(close(r.final_return.p5, f) && close(r.final_return.p95, f));
}

#[test]
fn deterministic_with_the_same_seed() {
    let xs = series();
    let a = monte_carlo(&xs, 1.0, &cfg(500, 3, 42)).unwrap();
    let b = monte_carlo(&xs, 1.0, &cfg(500, 3, 42)).unwrap();
    let c = monte_carlo(&xs, 1.0, &cfg(500, 3, 43)).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn larger_fraction_means_more_ruin() {
    let xs = series();
    let low = monte_carlo(&xs, 1.0, &cfg(2000, 2, 9)).unwrap();
    let high = monte_carlo(&xs, 4.0, &cfg(2000, 2, 9)).unwrap();
    assert!(high.risk_of_ruin > low.risk_of_ruin);
    assert!(high.max_drawdown.p50 > low.max_drawdown.p50);
    let s = high.max_drawdown;
    assert!(s.p5 <= s.p50 && s.p50 <= s.p95);
}

#[test]
fn constant_series_has_no_spread() {
    let r = monte_carlo(&[0.01; 24], 1.0, &cfg(100, 5, 3)).unwrap();
    assert_eq!(r.max_drawdown.p95, 0.0);
    assert_eq!(r.longest_under_water.p95, 0.0);
    assert_eq!(r.risk_of_ruin, 0.0);
    assert!(close(r.final_return.p5, 1.01_f64.powi(24) - 1.0));
}

#[test]
fn rejects_bad_input() {
    let ok = cfg(10, 2, 1);
    assert!(monte_carlo(&[], 1.0, &ok).is_none());
    assert!(monte_carlo(&[0.1, f64::NAN], 1.0, &ok).is_none());
    assert!(monte_carlo(&[0.1], 0.0, &ok).is_none());
    assert!(monte_carlo(&[0.1], 1.0, &cfg(0, 2, 1)).is_none());
    assert!(monte_carlo(&[0.1], 1.0, &cfg(10, 0, 1)).is_none());
    let mut bad = ok;
    bad.ruin_threshold = Decimal::new(15, 1);
    assert!(bad.validate().is_err());
}

#[test]
fn reads_the_repo_config() {
    let text = include_str!("../../../../config/research.toml");
    let c = ResearchConfig::from_toml(text).unwrap().montecarlo;
    assert!(c.validate().is_ok());
    let bad = text.replace("ruin_threshold = \"0.30\"", "ruin_threshold = \"0\"");
    assert!(ResearchConfig::from_toml(&bad).is_err());
}

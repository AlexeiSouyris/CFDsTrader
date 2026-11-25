//! PBO por CSCV (#664): caso calculado a mano, respuestas conocidas, determinismo y bordes.
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de una matriz de retornos: no son dinero"
)]

use cfds_backtest::research::ResearchConfig;
use cfds_backtest::research::pbo::{MAX_PARTITIONS, pbo_cscv};
use cfds_backtest::research::program4::SplitMix64;

/// Ruido aproximadamente normal (suma de 12 uniformes − 6), con semilla.
fn noise(rng: &mut SplitMix64) -> f64 {
    (0..12)
        .map(|_| (rng.next_u64() >> 11) as f64 / (1_u64 << 53) as f64)
        .sum::<f64>()
        - 6.0
}

fn noise_matrix(seed: u64, t: usize, n: usize) -> Vec<Vec<f64>> {
    let mut rng = SplitMix64::new(seed);
    (0..t)
        .map(|_| (0..n).map(|_| noise(&mut rng)).collect())
        .collect()
}

#[test]
fn hand_computed_overfit_case() {
    // A = [2, 4 | 1, 1.5], B = [1, 1.5 | 2, 4]. Sharpe de [2, 4] = 3/√2 ≈ 2,12 y de [1, 1.5] =
    // 1,25/√0,125 ≈ 3,54: en cada mitad gana una y en la otra pierde. Las dos combinaciones
    // eligen a la que después queda última: ω = 1/3, λ = ln(1/2), PBO = 1.
    let m = vec![
        vec![2.0, 1.0],
        vec![4.0, 1.5],
        vec![1.0, 2.0],
        vec![1.5, 4.0],
    ];
    let r = pbo_cscv(&m, 2).unwrap();
    assert_eq!(r.combinations, 2);
    assert_eq!(r.rows_used, 4);
    assert_eq!(r.pbo, 1.0);
    assert!((r.logits.p50 - 0.5_f64.ln()).abs() < 1e-12);
    assert_eq!(r.oos_loss, 0.0);
}

#[test]
fn dominant_variant_gives_zero() {
    // La variante 0 tiene +1 por período sobre el mismo ruido: gana en todas las particiones, IS
    // y OOS. ω = N/(N+1), λ = ln N > 0 en todas: PBO = 0.
    let n = 10;
    let mut m = noise_matrix(11, 64, n);
    for row in &mut m {
        row[0] += 1.0;
    }
    let r = pbo_cscv(&m, 8).unwrap();
    assert_eq!(r.combinations, 70);
    assert_eq!(r.pbo, 0.0);
    assert!((r.logits.p5 - (n as f64).ln()).abs() < 1e-12);
}

#[test]
fn pure_noise_is_near_one_half() {
    // Sin ninguna ventaja, la mejor del entrenamiento cae en cualquier lugar de la prueba.
    let seeds = [1_u64, 2, 3, 4, 5, 6];
    let avg = seeds
        .iter()
        .map(|&s| pbo_cscv(&noise_matrix(s, 240, 40), 10).unwrap().pbo)
        .sum::<f64>()
        / seeds.len() as f64;
    assert!((0.35..=0.65).contains(&avg), "PBO medio {avg}");
}

#[test]
fn deterministic() {
    let m = noise_matrix(9, 120, 12);
    assert_eq!(pbo_cscv(&m, 6), pbo_cscv(&m, 6));
}

#[test]
fn leftover_rows_are_dropped() {
    let m = noise_matrix(3, 103, 5);
    let r = pbo_cscv(&m, 4).unwrap();
    assert_eq!((r.rows_used, r.combinations), (100, 6));
    assert_eq!(pbo_cscv(&m[..100], 4), Some(r));
}

#[test]
fn constant_columns_tie_at_the_median() {
    // Todas iguales: rango promedio (N+1)/2, ω = 1/2, λ = 0, que cuenta como sobreajuste.
    let m = vec![vec![0.01; 3]; 8];
    let r = pbo_cscv(&m, 2).unwrap();
    assert_eq!(r.pbo, 1.0);
    assert_eq!(r.logits.p95, 0.0);
}

#[test]
fn rejects_bad_input() {
    let m = noise_matrix(1, 40, 4);
    assert!(pbo_cscv(&[], 4).is_none());
    assert!(pbo_cscv(&m, 3).is_none());
    assert!(pbo_cscv(&m, 0).is_none());
    assert!(pbo_cscv(&noise_matrix(1, 400, 4), MAX_PARTITIONS + 2).is_none());
    assert!(
        pbo_cscv(&noise_matrix(1, 7, 4), 4).is_none(),
        "bloques de menos de 2 filas"
    );
    assert!(pbo_cscv(&noise_matrix(1, 40, 1), 4).is_none());
    let mut ragged = m.clone();
    ragged[5].pop();
    assert!(pbo_cscv(&ragged, 4).is_none());
    let mut nan = m;
    nan[0][0] = f64::NAN;
    assert!(pbo_cscv(&nan, 4).is_none());
}

#[test]
fn reads_the_repo_config() {
    let text = include_str!("../../../../config/research.toml");
    let s = ResearchConfig::from_toml(text).unwrap().pbo_partitions;
    assert!(s.is_multiple_of(2) && (2..=MAX_PARTITIONS).contains(&s));
    let bad = text.replace("partitions = 16", "partitions = 15");
    assert!(ResearchConfig::from_toml(&bad).is_err());
}

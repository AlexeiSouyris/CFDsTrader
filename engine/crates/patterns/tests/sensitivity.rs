//! Sensibilidad de las tolerancias (#80, ROADMAP 3b): cada tolerancia tiene su caso en el borde
//! exacto (justo dentro detecta, justo fuera no), y los umbrales de las velas son monótonos
//! (relajarlos nunca detecta menos).

use std::collections::BTreeSet;

use cfds_core::{Candle, Timeframe};
use cfds_indicators::wilder_atr;
use cfds_patterns::{
    CandleConfig, PatternsConfig, Pivot, PivotKind, candle_patterns_at, confirmed_pivots,
    double_at, levels_at,
};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn cfg() -> PatternsConfig {
    PatternsConfig::from_toml(include_str!("../../../../config/patterns.toml")).unwrap()
}

fn bar(i: usize, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    let t0: DateTime<Utc> = "2026-11-19T14:30:00Z".parse().unwrap();
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: t0 + TimeDelta::minutes(15 * i as i64),
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

// ---------- Doble suelo: diferencia entre suelos y altura mínima ----------

/// Doble suelo: S1 en la vela 5 (mínimo 90), S2 en la 20 (mínimo `s2`), neckline 100 en la
/// vela 12. La vela 25 cierra arriba del neckline.
fn double_bottom(s2: Decimal) -> Vec<Candle> {
    (0..40)
        .map(|i| match i {
            5 => bar(i, dec!(96), dec!(97), dec!(90), dec!(96)),
            20 => bar(i, dec!(96), dec!(97), s2, dec!(96)),
            12 => bar(i, dec!(96), dec!(100), dec!(95), dec!(96)),
            24 => bar(i, dec!(96), dec!(99.5), dec!(95), dec!(99)),
            25 => bar(i, dec!(99), dec!(102), dec!(98), dec!(101)),
            _ => bar(i, dec!(96), dec!(97), dec!(95), dec!(96)),
        })
        .collect()
}

fn doubles(candles: &[Candle], atr: Decimal) -> usize {
    let c = cfg();
    let pivots = confirmed_pivots(candles, c.pivots.strength);
    double_at(
        candles,
        25,
        &pivots,
        &vec![Some(atr); candles.len()],
        &c.double,
    )
    .len()
}

#[test]
fn double_difference_tolerance_is_inclusive_at_the_edge() {
    // |S1 − S2| ≤ max_difference × S1 (0,3 % de 90 = 0,27).
    let edge = dec!(90) + cfg().double.max_difference * dec!(90);
    assert_eq!(edge, dec!(90.27));
    assert_eq!(
        doubles(&double_bottom(edge), dec!(5)),
        1,
        "en el borde: detecta"
    );
    assert_eq!(
        doubles(&double_bottom(edge + dec!(0.0001)), dec!(5)),
        0,
        "apenas fuera: no detecta"
    );
}

#[test]
fn double_min_height_is_inclusive_at_the_edge() {
    // Altura = neckline − suelo más bajo = 100 − 90 = 10 ≥ min_height_atr × ATR.
    let edge_atr = dec!(10) / cfg().double.min_height_atr;
    let c = double_bottom(dec!(90.2));
    assert_eq!(doubles(&c, edge_atr), 1, "altura justo igual: detecta");
    assert_eq!(
        doubles(&c, edge_atr + dec!(0.0001)),
        0,
        "ATR apenas mayor: no"
    );
}

#[test]
fn a_wider_difference_tolerance_never_detects_less() {
    let seconds: Vec<Decimal> = (0..=60).map(|k| dec!(90) + Decimal::new(k, 2)).collect();
    let count = |max_difference: Decimal| -> usize {
        let mut c = cfg();
        c.double.max_difference = max_difference;
        seconds
            .iter()
            .filter(|s2| {
                let candles = double_bottom(**s2);
                let pivots = confirmed_pivots(&candles, c.pivots.strength);
                !double_at(&candles, 25, &pivots, &vec![Some(dec!(5)); 40], &c.double).is_empty()
            })
            .count()
    };
    let counts: Vec<usize> = [
        dec!(0.001),
        dec!(0.002),
        dec!(0.003),
        dec!(0.005),
        dec!(0.01),
    ]
    .into_iter()
    .map(count)
    .collect();
    assert!(counts.windows(2).all(|w| w[0] <= w[1]), "{counts:?}");
    assert!(
        counts[0] < counts[4],
        "la tolerancia tiene efecto: {counts:?}"
    );
}

// ---------- Soportes y resistencias: tolerancia de agrupamiento ----------

fn low(index: usize, price: Decimal) -> Pivot {
    Pivot {
        kind: PivotKind::Low,
        index,
        confirmed_at: index + 3,
        price,
    }
}

#[test]
fn level_tolerance_is_inclusive_at_the_edge() {
    // Tres mínimos simétricos alrededor de 100: el más alejado está a `d` del promedio.
    let tol = cfg().levels.tolerance;
    let edge = tol * dec!(100);
    let level = |d: Decimal| {
        levels_at(
            150,
            &[
                low(10, dec!(100) - d),
                low(40, dec!(100)),
                low(80, dec!(100) + d),
            ],
            &cfg().levels,
        )
    };
    assert_eq!(level(edge).len(), 1, "a {edge} del promedio: un soporte");
    assert!(
        level(edge + dec!(0.0001)).is_empty(),
        "apenas fuera: ninguno"
    );
}

// ---------- Velas: los cuatro umbrales son monótonos ----------

/// Caminata aleatoria determinística, con mechas variadas para que aparezcan todos los patrones.
fn random_walk(n: usize, seed: u64) -> Vec<Candle> {
    let mut state = seed;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let mut close = dec!(1000);
    (0..n)
        .map(|i| {
            let open = close;
            close = open + Decimal::new(next(41) - 20, 1);
            let up = Decimal::new(next(40), 1);
            let down = Decimal::new(next(40), 1);
            bar(i, open, open.max(close) + up, open.min(close) - down, close)
        })
        .collect()
}

fn detections(candles: &[Candle], c: &CandleConfig) -> BTreeSet<(usize, String)> {
    let atr = wilder_atr(candles, c.atr_period);
    (0..candles.len())
        .flat_map(|t| {
            candle_patterns_at(candles, t, &atr, c)
                .into_iter()
                .map(move |p| (t, format!("{p:?}")))
        })
        .collect()
}

/// Relajar un umbral (de `strict` a `loose`) nunca saca detecciones, y en la serie larga las
/// agrega: la tolerancia tiene efecto.
fn assert_monotone(
    name: &str,
    set: impl Fn(&mut CandleConfig, Decimal),
    strict: Decimal,
    loose: Decimal,
) {
    let candles = random_walk(3000, 7);
    let mut c = cfg().candles;
    set(&mut c, strict);
    let tight = detections(&candles, &c);
    set(&mut c, loose);
    let wide = detections(&candles, &c);
    assert!(tight.is_subset(&wide), "{name}: relajar sacó detecciones");
    assert!(
        tight.len() < wide.len(),
        "{name}: sin efecto ({} = {})",
        tight.len(),
        wide.len()
    );
}

#[test]
fn candle_thresholds_are_monotone() {
    assert_monotone(
        "min_wick_to_body",
        |c, v| c.min_wick_to_body = v,
        dec!(3),
        dec!(1.5),
    );
    assert_monotone(
        "max_opposite_wick_to_range",
        |c, v| c.max_opposite_wick_to_range = v,
        dec!(0.2),
        dec!(0.4),
    );
    assert_monotone(
        "min_body_to_range",
        |c, v| c.min_body_to_range = v,
        dec!(0.2),
        dec!(0.01),
    );
    assert_monotone(
        "min_range_to_atr",
        |c, v| c.min_range_to_atr = v,
        dec!(1),
        dec!(0.3),
    );
}

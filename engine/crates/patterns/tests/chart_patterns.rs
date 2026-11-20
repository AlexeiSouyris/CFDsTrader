//! Doble techo/suelo y soportes/resistencias con series construidas a mano (#41).

use cfds_core::{Candle, Timeframe};
use cfds_patterns::{
    DoubleKind, LevelKind, PatternsConfig, Pivot, PivotKind, confirmed_pivots, double_at,
    levels_at, rejections_at,
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

/// 40 velas "normales" (O 96, H 97, L 95, C 96) con un doble suelo:
/// S1 en la vela 5 (mínimo 90), S2 en la 20 (mínimo 90,2: 0,22 % de diferencia),
/// neckline en la vela 12 (máximo 100). S2 se confirma en la 23 (N = 3).
/// El cierre cruza el neckline en la vela `cross` (cierre anterior 99, cierre 101).
fn double_bottom(cross: usize) -> Vec<Candle> {
    (0..40)
        .map(|i| match i {
            5 => bar(i, dec!(96), dec!(97), dec!(90), dec!(96)),
            20 => bar(i, dec!(96), dec!(97), dec!(90.2), dec!(96)),
            12 => bar(i, dec!(96), dec!(100), dec!(95), dec!(96)),
            _ if i + 1 == cross => bar(i, dec!(96), dec!(99.5), dec!(95), dec!(99)),
            _ if i == cross => bar(i, dec!(99), dec!(102), dec!(98), dec!(101)),
            _ => bar(i, dec!(96), dec!(97), dec!(95), dec!(96)),
        })
        .collect()
}

/// El espejo exacto alrededor de 200: suelos ↔ techos, alcista ↔ bajista.
fn mirror(candles: &[Candle]) -> Vec<Candle> {
    let m = |x: Decimal| dec!(200) - x;
    candles
        .iter()
        .enumerate()
        .map(|(i, c)| bar(i, m(c.open), m(c.low), m(c.high), m(c.close)))
        .collect()
}

fn atr(n: usize, value: Decimal) -> Vec<Option<Decimal>> {
    vec![Some(value); n]
}

fn detect(candles: &[Candle], t: usize) -> Vec<cfds_patterns::DoublePattern> {
    let pivots = confirmed_pivots(candles, cfg().pivots.strength);
    double_at(
        candles,
        t,
        &pivots,
        &atr(candles.len(), dec!(5)),
        &cfg().double,
    )
}

#[test]
fn detects_a_double_bottom_on_the_neckline_break() {
    let c = double_bottom(25);
    let found = detect(&c, 25);
    assert_eq!(found.len(), 1);
    let p = found[0];
    assert_eq!(p.kind, DoubleKind::Bottom);
    assert_eq!((p.first.index, p.second.index), (5, 20));
    assert_eq!(p.neckline, dec!(100));
    assert_eq!(p.extreme(), dec!(90));
}

#[test]
fn only_the_breaking_candle_triggers() {
    let c = double_bottom(25);
    assert!(detect(&c, 24).is_empty(), "antes del cruce");
    let mut after = c.clone();
    after[26] = bar(26, dec!(101), dec!(103), dec!(100.5), dec!(102)); // sigue arriba: no es cruce
    assert!(detect(&after, 26).is_empty(), "después del cruce");
}

#[test]
fn never_uses_a_pivot_before_its_confirmation() {
    // El cruce ocurre en la vela 22, pero S2 (vela 20) recién se confirma en la 23.
    let c = double_bottom(22);
    assert!(detect(&c, 22).is_empty());
}

#[test]
fn passing_future_pivots_does_not_leak_into_the_past() {
    // Pivotes calculados con TODA la serie (incluido el futuro): el resultado en t no cambia.
    let c = double_bottom(25);
    let all_pivots = confirmed_pivots(&c, 3);
    let only_past = confirmed_pivots(&c[..=25], 3);
    let a = atr(c.len(), dec!(5));
    assert_eq!(
        double_at(&c, 25, &all_pivots, &a, &cfg().double),
        double_at(&c[..=25], 25, &only_past, &a, &cfg().double)
    );
}

#[test]
fn rejects_pairs_too_different_too_shallow_or_too_old() {
    // Suelos demasiado distintos: 90 y 91 (1,1 % > 0,3 %).
    let mut c = double_bottom(25);
    c[20] = bar(20, dec!(96), dec!(97), dec!(91), dec!(96));
    assert!(detect(&c, 25).is_empty(), "diferencia");

    // Altura 10 < 1 × ATR de 15.
    let c = double_bottom(25);
    let pivots = confirmed_pivots(&c, 3);
    assert!(
        double_at(&c, 25, &pivots, &atr(40, dec!(15)), &cfg().double).is_empty(),
        "altura"
    );

    // El cruce llega a la vela 44: más de 20 velas después de confirmar S2 (vela 23).
    let mut long: Vec<Candle> = double_bottom(99);
    for i in 40..45 {
        long.push(bar(i, dec!(96), dec!(97), dec!(95), dec!(96)));
    }
    long[43] = bar(43, dec!(96), dec!(99.5), dec!(95), dec!(99));
    long[44] = bar(44, dec!(99), dec!(102), dec!(98), dec!(101));
    assert!(detect(&long, 44).is_empty(), "vencido");
}

#[test]
fn rejects_bottoms_too_close_together() {
    // S2 en la vela 12: solo 7 velas después de S1 (mínimo 10).
    let mut c = double_bottom(25);
    c[20] = bar(20, dec!(96), dec!(97), dec!(95), dec!(96));
    c[12] = bar(12, dec!(96), dec!(97), dec!(90.2), dec!(96));
    c[9] = bar(9, dec!(96), dec!(100), dec!(95), dec!(96));
    assert!(detect(&c, 25).is_empty());
}

#[test]
fn the_mirror_is_a_double_top() {
    let c = mirror(&double_bottom(25));
    let found = detect(&c, 25);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, DoubleKind::Top);
    assert_eq!(found[0].neckline, dec!(100)); // 200 − 100
    assert_eq!(found[0].extreme(), dec!(110)); // 200 − 90
}

fn pivot(kind: PivotKind, index: usize, price: Decimal) -> Pivot {
    Pivot {
        kind,
        index,
        confirmed_at: index + 3,
        price,
    }
}

#[test]
fn groups_three_close_lows_into_a_support() {
    let pivots = [
        pivot(PivotKind::Low, 10, dec!(100.00)),
        pivot(PivotKind::Low, 40, dec!(100.10)),
        pivot(PivotKind::Low, 80, dec!(99.95)),
        pivot(PivotKind::Low, 90, dec!(90)), // lejos: no se agrupa
    ];
    let levels = levels_at(150, &pivots, &cfg().levels);
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].kind, LevelKind::Support);
    assert_eq!(levels[0].touches, 3);
    assert_eq!(levels[0].price.round_dp(4), dec!(100.0167));
}

#[test]
fn two_touches_or_old_or_unconfirmed_pivots_are_not_a_level() {
    let cfg = cfg().levels;
    let two = [
        pivot(PivotKind::Low, 10, dec!(100)),
        pivot(PivotKind::Low, 40, dec!(100.05)),
    ];
    assert!(levels_at(150, &two, &cfg).is_empty(), "dos toques");

    let mut three = two.to_vec();
    three.push(pivot(PivotKind::Low, 60, dec!(99.98)));
    assert!(
        levels_at(260, &three, &cfg).is_empty(),
        "el primero quedó fuera de las 200 velas"
    );
    assert!(
        levels_at(62, &three, &cfg).is_empty(),
        "el tercero todavía no se confirmó"
    );
    assert_eq!(levels_at(63, &three, &cfg).len(), 1);
}

#[test]
fn detects_rejections_of_support_and_resistance() {
    let cfg = cfg().levels;
    let pivots = [
        pivot(PivotKind::Low, 10, dec!(100)),
        pivot(PivotKind::Low, 40, dec!(100)),
        pivot(PivotKind::Low, 80, dec!(100)),
        pivot(PivotKind::High, 20, dec!(120)),
        pivot(PivotKind::High, 50, dec!(120)),
        pivot(PivotKind::High, 70, dec!(120)),
    ];
    let levels = levels_at(150, &pivots, &cfg);
    let at = |o, h, l, c| vec![bar(0, o, h, l, c)];

    // Toca el soporte (100,05 ≤ 100,15) y cierra arriba: rechazo alcista.
    let r = rejections_at(
        &at(dec!(103), dec!(104), dec!(100.05), dec!(102)),
        0,
        &levels,
        &cfg,
    );
    assert_eq!(
        r.iter().map(|l| l.kind).collect::<Vec<_>>(),
        vec![LevelKind::Support]
    );

    // Cierra por debajo del soporte: no es rechazo, es ruptura.
    assert!(
        rejections_at(
            &at(dec!(101), dec!(102), dec!(98), dec!(99)),
            0,
            &levels,
            &cfg
        )
        .is_empty()
    );

    // Toca la resistencia y cierra abajo: rechazo bajista.
    let r = rejections_at(
        &at(dec!(117), dec!(119.9), dec!(116), dec!(118)),
        0,
        &levels,
        &cfg,
    );
    assert_eq!(
        r.iter().map(|l| l.kind).collect::<Vec<_>>(),
        vec![LevelKind::Resistance]
    );
}

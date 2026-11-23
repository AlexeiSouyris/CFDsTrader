//! Desequilibrio aproximado desde el precio de la spec §5.4.1 (#231), con series hechas a mano.
//! Los precios son inventados.

use std::num::NonZeroU32;

use cfds_core::{Candle, Direction, ImbalanceLabel, Timeframe};
use cfds_liquidity::{ImbalanceConfig, imbalance_series};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const ATR: usize = 14;

fn cfg() -> ImbalanceConfig {
    ImbalanceConfig::from_toml(include_str!("../../../../config/liquidity.toml")).unwrap()
}

fn window(n: u32) -> ImbalanceConfig {
    ImbalanceConfig {
        window_bars: NonZeroU32::new(n).unwrap(),
        ..cfg()
    }
}

fn t0() -> DateTime<Utc> {
    "2026-11-16T14:00:00Z".parse().unwrap()
}

fn bar(i: i32, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: t0() + TimeDelta::minutes(15) * i,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

fn flat(i: i32, level: Decimal) -> Candle {
    bar(i, level, level + dec!(1), level - dec!(1), level)
}

/// 16 velas chatas (ATR14 = 2) y después la vela 16.
fn after_flat(candle16: Candle, level_after: Decimal) -> Vec<Candle> {
    let mut v: Vec<Candle> = (0..16).map(|i| flat(i, dec!(100))).collect();
    v.push(candle16);
    v.extend((17..19).map(|i| flat(i, level_after)));
    v
}

#[test]
fn index_computed_by_hand_with_a_bullish_displacement() {
    // Vela 16: cuerpo 4, rango 5 (0,8 ≥ 0,7); ATR16 = (2·13 + 5)/14 ≈ 2,214 y 1,5 × ATR ≈ 3,32 ≤ 4.
    let v = after_flat(
        bar(16, dec!(100), dec!(104.5), dec!(99.5), dec!(104)),
        dec!(104),
    );
    let out = imbalance_series(&v, ATR, &window(3));
    let at = |t: usize| out.iter().find(|x| x.at == v[t].end()).unwrap();
    assert_eq!(at(16).displacement, Some(Direction::Bullish));
    assert_eq!(at(17).displacement, None);
    // Ventana [16, 18]: (4 − 0) / (5 + 2 + 2).
    let x = at(18);
    assert_eq!(
        (x.bullish_body_sum, x.bearish_body_sum, x.range_sum),
        (dec!(4), dec!(0), dec!(9))
    );
    assert_eq!(x.index, dec!(4) / dec!(9));
    assert_eq!(x.label, ImbalanceLabel::PriceApproximation);
    assert_eq!(x.window_bars.get(), 3);
}

#[test]
fn a_bearish_displacement_pushes_the_index_below_zero() {
    let v = after_flat(
        bar(16, dec!(100), dec!(100.5), dec!(95.5), dec!(96)),
        dec!(96),
    );
    let x = imbalance_series(&v, ATR, &window(3)).pop().unwrap();
    assert_eq!(x.at, v[18].end());
    assert_eq!(x.bearish_body_sum, dec!(4));
    assert_eq!(x.index, dec!(-4) / dec!(9));
}

#[test]
fn long_wicks_or_a_small_body_are_not_displacement() {
    // Cuerpo 4 con rango 10: 0,4 < 0,7.
    let wicks = after_flat(
        bar(16, dec!(100), dec!(107), dec!(97), dec!(104)),
        dec!(104),
    );
    // Cuerpo 3 con rango 3: 3 < 1,5 × ATR16 (≈ 3,21).
    let small = after_flat(
        bar(16, dec!(100), dec!(103), dec!(100), dec!(103)),
        dec!(103),
    );
    for v in [wicks, small] {
        let out = imbalance_series(&v, ATR, &window(3));
        assert!(
            out.iter()
                .all(|x| x.displacement.is_none() && x.index.is_zero())
        );
    }
}

#[test]
fn zero_denominator_gives_zero() {
    let v: Vec<Candle> = (0..40)
        .map(|i| bar(i, dec!(100), dec!(100), dec!(100), dec!(100)))
        .collect();
    let out = imbalance_series(&v, ATR, &cfg());
    assert!(!out.is_empty());
    for x in out {
        assert_eq!(
            (x.range_sum, x.index, x.displacement),
            (dec!(0), dec!(0), None)
        );
    }
}

#[test]
fn warm_up_waits_for_the_atr_of_the_oldest_candle_in_the_window() {
    let v: Vec<Candle> = (0..40).map(|i| flat(i, dec!(100))).collect();
    let out = imbalance_series(&v, ATR, &cfg());
    // ATR14 desde la vela 13; ventana de 20: la primera es la vela 13 + 19 = 32.
    assert_eq!(out.first().map(|x| x.at), Some(v[32].end()));
    assert_eq!(out.len(), 8);
    assert!(imbalance_series(&v[..32], ATR, &cfg()).is_empty());
}

/// Serie pseudoaleatoria determinista (LCG), con velas grandes y chicas.
fn noisy(n: i32) -> Vec<Candle> {
    let mut seed: u64 = 7;
    let mut next = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        i64::try_from(seed >> 59).unwrap() - 16
    };
    let mut close = dec!(1000);
    (0..n)
        .map(|i| {
            let open = close;
            close = open + Decimal::from(next());
            let high = open.max(close) + Decimal::from(next().abs() / 4);
            let low = open.min(close) - Decimal::from(next().abs() / 4);
            bar(i, open, high, low, close)
        })
        .collect()
}

#[test]
fn no_repaint_and_the_index_stays_between_minus_one_and_one() {
    let v = noisy(300);
    let full = imbalance_series(&v, ATR, &cfg());
    assert!(
        full.iter()
            .any(|x| x.displacement == Some(Direction::Bullish))
    );
    assert!(
        full.iter()
            .any(|x| x.displacement == Some(Direction::Bearish))
    );
    assert!(
        full.iter()
            .all(|x| x.index >= dec!(-1) && x.index <= dec!(1))
    );
    for k in (40..300).step_by(11) {
        let prefix = imbalance_series(&v[..k], ATR, &cfg());
        assert_eq!(prefix[..], full[..prefix.len()]);
    }
}

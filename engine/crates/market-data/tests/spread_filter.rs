//! Filtro de spread (spec §7.3, #90) con casos construidos a mano.

use cfds_core::{Candle, CandleSpread, Timeframe};
use cfds_market_data::{SpreadFilterConfig, spread_flags};
use chrono::{DateTime, TimeDelta, Utc};
use chrono_tz::Europe::Berlin;
use rust_decimal::{Decimal, dec};

fn cfg() -> SpreadFilterConfig {
    SpreadFilterConfig::from_toml(include_str!("../../../../config/spread_filter.toml")).unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn candle(start: DateTime<Utc>) -> Candle {
    Candle {
        instrument_id: "DE40".parse().unwrap(),
        timeframe: Timeframe::M15,
        start,
        open: dec!(24000),
        high: dec!(24010),
        low: dec!(23990),
        close: dec!(24005),
        volume: None,
    }
}

fn spread(start: DateTime<Utc>, value: Decimal) -> CandleSpread {
    CandleSpread {
        instrument_id: "DE40".parse().unwrap(),
        timeframe: Timeframe::M15,
        start,
        spread: value,
        source: "ig_demo".into(),
    }
}

/// Un spread de `value` a las 10:00 UTC (11:00 en Berlín, invierno) de cada uno de los `days`
/// días anteriores a `day`.
fn history(day: DateTime<Utc>, days: i64, value: Decimal) -> Vec<CandleSpread> {
    (1..=days)
        .map(|d| spread(day - TimeDelta::days(d), value))
        .collect()
}

/// ¿Deja abrir en `t` con el spread actual `current` y la historia dada?
fn allows(t: DateTime<Utc>, current: Decimal, mut spreads: Vec<CandleSpread>) -> bool {
    spreads.push(spread(t, current));
    spread_flags(&[candle(t)], &spreads, Berlin, &cfg())[0]
}

const T: &str = "2026-11-30T10:00:00Z";

#[test]
fn the_repository_config_is_the_spec() {
    let c = cfg();
    assert_eq!(c.max_multiple, dec!(2));
    assert_eq!(c.lookback, TimeDelta::days(20));
    assert_eq!((c.slot_minutes, c.min_samples), (60, 10));
}

#[test]
fn more_than_twice_the_average_blocks_and_exactly_twice_does_not() {
    let t = at(T);
    assert!(allows(t, dec!(2.0), history(t, 20, dec!(1.0))));
    assert!(!allows(t, dec!(2.01), history(t, 20, dec!(1.0))));
    // El promedio es exacto: 10 de 1,0 y 10 de 1,6 → 1,3; el borde es 2,6.
    let mut mixed = history(t, 10, dec!(1.0));
    mixed.extend((11..=20).map(|d| spread(t - TimeDelta::days(d), dec!(1.6))));
    assert!(allows(t, dec!(2.6), mixed.clone()));
    assert!(!allows(t, dec!(2.61), mixed));
}

#[test]
fn only_the_last_20_days_count() {
    let t = at(T);
    // Spreads enormes de hace 21 a 30 días: fuera de la ventana, no levantan el promedio.
    let mut h = history(t, 20, dec!(1.0));
    h.extend((21..=30).map(|d| spread(t - TimeDelta::days(d), dec!(100))));
    assert!(!allows(t, dec!(2.01), h));
    // El de hace exactamente 20 días sí cuenta.
    let mut edge = history(t, 19, dec!(1.0));
    edge.push(spread(t - TimeDelta::days(20), dec!(20.0)));
    assert!(allows(t, dec!(2.01), edge));
}

#[test]
fn without_enough_history_or_current_spread_it_does_not_open() {
    let t = at(T);
    assert!(
        !allows(t, dec!(1.0), history(t, 9, dec!(1.0))),
        "9 muestras"
    );
    assert!(
        allows(t, dec!(1.0), history(t, 10, dec!(1.0))),
        "10 muestras"
    );
    let no_current = spread_flags(&[candle(t)], &history(t, 20, dec!(1.0)), Berlin, &cfg());
    assert_eq!(no_current, vec![false]);
}

#[test]
fn other_hours_do_not_count() {
    let t = at(T);
    // Mismos días, pero a las 14:00 UTC: otra franja. Sin historia en la franja de t.
    let other: Vec<_> = history(t, 20, dec!(1.0))
        .into_iter()
        .map(|s| spread(s.start + TimeDelta::hours(4), s.spread))
        .collect();
    assert!(!allows(t, dec!(1.0), other));
    // Dentro de la misma hora local (10:45 UTC = 11:45 en Berlín) sí cuenta.
    let same_hour: Vec<_> = history(t, 20, dec!(1.0))
        .into_iter()
        .map(|s| spread(s.start + TimeDelta::minutes(45), s.spread))
        .collect();
    assert!(allows(t, dec!(2.0), same_hour));
}

#[test]
fn the_hour_is_local_to_the_exchange_across_daylight_saving() {
    // Antes del 25-10-2026 Berlín está en UTC+2: las 10:00 UTC son las 12:00 locales.
    // Después, en UTC+1, las 12:00 locales son las 11:00 UTC.
    let h: Vec<_> = (10..=24)
        .map(|d| spread(at(&format!("2026-10-{d:02}T10:00:00Z")), dec!(1.0)))
        .collect();
    assert!(
        allows(at("2026-10-28T11:00:00Z"), dec!(2.0), h.clone()),
        "misma hora local"
    );
    assert!(
        !allows(at("2026-10-28T10:00:00Z"), dec!(1.0), h),
        "misma hora UTC, otra local"
    );
}

#[test]
fn later_spreads_never_change_an_earlier_decision() {
    let t = at(T);
    let mut later = history(t, 20, dec!(1.0));
    later.extend((1..=5).map(|d| spread(t + TimeDelta::days(d), dec!(100))));
    assert!(!allows(t, dec!(2.01), later));
}

#[test]
fn one_flag_per_candle_in_order() {
    let t = at(T);
    let mut s = history(t, 20, dec!(1.0));
    s.push(spread(t, dec!(1.0)));
    let next = t + TimeDelta::days(1);
    s.push(spread(next, dec!(5.0)));
    let flags = spread_flags(&[candle(t), candle(next)], &s, Berlin, &cfg());
    assert_eq!(flags, vec![true, false]);
}

#[test]
fn invalid_configuration_is_rejected() {
    let real = include_str!("../../../../config/spread_filter.toml");
    for (from, to) in [
        (r#"max_multiple = "2""#, r#"max_multiple = "1""#),
        ("lookback_days = 20", "lookback_days = 0"),
        ("slot_minutes = 60", "slot_minutes = 7"),
        ("min_samples = 10", "min_samples = 0"),
    ] {
        let text = real.replace(from, to);
        assert_ne!(text, real, "{from}");
        assert!(SpreadFilterConfig::from_toml(&text).is_err(), "{to}");
    }
}

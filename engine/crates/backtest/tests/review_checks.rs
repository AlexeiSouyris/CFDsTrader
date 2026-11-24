//! Chequeos descriptivos de la revisión externa (#512): caída fuerte, deriva después de la
//! caída y cartera mezclada. Series sintéticas; nunca la base real.

use cfds_backtest::research::{
    ResearchConfig, blend_return, drift_after_close, session_close_returns, strong_drop_flags,
    vol_match_scale,
};
use cfds_core::{Candle, InstrumentId, Timeframe};
use chrono::{DateTime, NaiveDate, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn candle(start: DateTime<Utc>, o: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US500".parse::<InstrumentId>().unwrap(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: o.max(c) + dec!(1),
        low: o.min(c) - dec!(1),
        close: c,
        volume: None,
    }
}

#[test]
fn the_review_checks_config_loads_and_rejects_bad_values() {
    let text = include_str!("../../../../config/research.toml");
    let rc = ResearchConfig::from_toml(text).unwrap().review_checks;
    assert_eq!(rc.drift_split_from, day("2021-01-01"));
    assert_eq!(rc.drop_lookback_days, 250);
    assert_eq!(rc.drop_quantile, dec!(0.2));
    assert_eq!(rc.blend_hold_weight, dec!(0.5));
    for (from, to) in [
        ("drop_quantile = \"0.2\"", "drop_quantile = \"1\""),
        ("drop_quantile = \"0.2\"", "drop_quantile = \"0\""),
        ("blend_hold_weight = \"0.5\"", "blend_hold_weight = \"1.5\""),
        ("drop_lookback_days = 250", "drop_lookback_days = 1"),
        (
            "drift_split_from = \"2021-01-01\"",
            "drift_split_from = \"2021\"",
        ),
    ] {
        let bad = text.replace(from, to);
        assert!(ResearchConfig::from_toml(&bad).is_err(), "{to}");
    }
}

#[test]
fn session_close_returns_chain_across_a_rollover() {
    // Cierres de sesión en los índices 0, 1 y 3; el vencimiento cae en la vela 3 (el nuevo
    // contrato abre 10 puntos más arriba: el salto no cuenta).
    let t0 = at("2024-03-04T20:45:00Z");
    let m15 = vec![
        candle(t0, dec!(100), dec!(100)),
        candle(t0 + TimeDelta::days(1), dec!(100), dec!(110)),
        candle(t0 + TimeDelta::days(2), dec!(110), dec!(99)),
        candle(t0 + TimeDelta::days(3), dec!(109), dec!(109)),
    ];
    let rolls = [m15[3].start];
    let r = session_close_returns(&m15, &rolls, &[0, 1, 3]);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0], (m15[1].end(), dec!(0.1)));
    // 110 → 99 en el contrato viejo, y 109 → 109 en el nuevo: 99 / 110 − 1.
    assert_eq!(r[1].0, m15[3].end());
    assert_eq!(r[1].1, dec!(99) / dec!(110) - Decimal::ONE);
}

#[test]
fn a_strong_drop_is_below_the_kth_smallest_of_the_previous_days() {
    // 10 días previos −5 … 4 (en %); lookback 10, percentil 20 → k = 2 → umbral = −4.
    let mut r: Vec<Decimal> = (-5..5).map(|x| Decimal::from(x) / dec!(100)).collect();
    r.extend([dec!(-0.045), dec!(-0.04), dec!(0.01)]);
    let flags = strong_drop_flags(&r, 10, dec!(0.2));
    assert!(flags[..10].iter().all(Option::is_none), "calentamiento");
    assert_eq!(flags[10], Some(true));
    // Día 11: los 10 anteriores son −4 … 4 y −4,5; el 2.º menor es −4: −4 no está por debajo.
    assert_eq!(flags[11], Some(false));
    assert_eq!(flags[12], Some(false));
    // Sin repintado: agregar días no cambia las marcas viejas.
    let mut more = r.clone();
    more.push(dec!(-0.5));
    assert_eq!(
        &strong_drop_flags(&more, 10, dec!(0.2))[..r.len()],
        &flags[..]
    );
}

#[test]
fn the_drift_takes_the_flag_of_the_last_close_before_the_window() {
    let ny: chrono_tz::Tz = "America/New_York".parse().unwrap();
    let from = NaiveTime::from_hms_opt(2, 0, 0).unwrap();
    // Cierres del jueves 07-03-2024 (caída) y del viernes 08-03 (no), a las 21:00 UTC.
    let closes = [
        (at("2024-03-06T21:00:00Z"), None),
        (at("2024-03-07T21:00:00Z"), Some(true)),
        (at("2024-03-08T21:00:00Z"), Some(false)),
    ];
    let drift = [
        (day("2024-03-07"), dec!(0.001)), // usa el cierre del miércoles: calentamiento
        (day("2024-03-08"), dec!(0.002)), // usa el del jueves: caída fuerte
        (day("2024-03-11"), dec!(0.003)), // lunes (EDT): usa el del viernes
        (day("2024-03-01"), dec!(0.004)), // sin cierre anterior
    ];
    let r = drift_after_close(&drift, ny, from, &closes);
    assert_eq!(
        r,
        vec![
            (day("2024-03-08"), dec!(0.002), true),
            (day("2024-03-11"), dec!(0.003), false),
        ]
    );
}

#[test]
fn the_blend_scales_v_to_the_volatility_of_buy_and_hold() {
    let hold = [dec!(0.02), dec!(-0.02), dec!(0.02), dec!(-0.02)];
    let v = [dec!(0.01), dec!(-0.01), dec!(0.01), dec!(-0.01)];
    let s = vol_match_scale(&hold, &v).unwrap();
    assert_eq!(s.round_dp(12), dec!(2));
    assert_eq!(vol_match_scale(&hold, &[dec!(0.01); 4]), None);
    assert_eq!(vol_match_scale(&hold[..1], &v[..1]), None);
    assert_eq!(vol_match_scale(&hold, &v[..3]), None);
    // 0,5 × 1 % + 0,5 × 2 × 3 % = 3,5 %.
    assert_eq!(
        blend_return(dec!(0.01), dec!(0.03), dec!(0.5), dec!(2)),
        dec!(0.035)
    );
    // Peso 1: solo comprar y mantener.
    assert_eq!(
        blend_return(dec!(0.01), dec!(0.03), Decimal::ONE, dec!(2)),
        dec!(0.01)
    );
}

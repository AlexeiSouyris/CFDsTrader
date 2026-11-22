//! Indicadores sobre velas diarias (#148): la misma fórmula que en intradía y la configuración.

use cfds_core::{Candle, DailyCandle, Timeframe};
use cfds_indicators::{DailyConfig, TrendConfig, wilder_atr};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const CONFIG: &str = include_str!("../../../../config/indicators.toml");

fn hlc() -> Vec<(Decimal, Decimal, Decimal)> {
    vec![
        (dec!(105), dec!(100), dec!(102)),
        (dec!(104), dec!(101), dec!(103)),
        (dec!(110), dec!(103), dec!(109)),
        (dec!(108), dec!(98), dec!(99)),
        (dec!(101), dec!(97), dec!(100)),
    ]
}

fn days() -> Vec<DailyCandle> {
    let d0: NaiveDate = "2026-11-02".parse().unwrap();
    let t0: DateTime<Utc> = "2026-11-01T23:00:00Z".parse().unwrap();
    hlc()
        .into_iter()
        .enumerate()
        .map(|(i, (high, low, close))| DailyCandle {
            instrument_id: "DE40".parse().unwrap(),
            day: d0 + TimeDelta::days(i as i64),
            start: t0 + TimeDelta::days(i as i64),
            end: t0 + TimeDelta::days(i as i64 + 1),
            open: close,
            high,
            low,
            close,
            volume: None,
        })
        .collect()
}

fn intraday() -> Vec<Candle> {
    let t0: DateTime<Utc> = "2026-11-19T14:30:00Z".parse().unwrap();
    hlc()
        .into_iter()
        .enumerate()
        .map(|(i, (high, low, close))| Candle {
            instrument_id: "DE40".parse().unwrap(),
            timeframe: Timeframe::M15,
            start: t0 + TimeDelta::minutes(15 * i as i64),
            open: close,
            high,
            low,
            close,
            volume: None,
        })
        .collect()
}

#[test]
fn daily_atr_is_the_same_formula_as_intraday() {
    assert_eq!(wilder_atr(&days(), 3), wilder_atr(&intraday(), 3));
    assert_eq!(wilder_atr(&days(), 3)[1], None);
    assert!(wilder_atr(&days(), 3)[2].is_some());
}

#[test]
fn both_sections_of_the_real_config_load() {
    let daily = DailyConfig::from_toml(CONFIG).unwrap();
    assert_eq!((daily.atr_period, daily.realized_vol_window), (14, 20));
    assert!(TrendConfig::from_toml(CONFIG).is_ok());
}

#[test]
fn a_bad_daily_config_is_rejected() {
    let bad = CONFIG.replace("realized_vol_window = 20", "realized_vol_window = 1");
    assert!(DailyConfig::from_toml(&bad).is_err());
    let typo = CONFIG.replace("atr_period = 14", "atr_periodo = 14");
    assert!(DailyConfig::from_toml(&typo).is_err());
}

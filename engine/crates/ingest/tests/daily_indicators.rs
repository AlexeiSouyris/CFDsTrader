//! Indicadores diarios (#148): solo días con sesión, `published_at` al fin de la vela, sin
//! suponer nada fuera del calendario. Sin red ni servicios.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_core::{DailyCandle, InstrumentId};
use cfds_indicators::{DailyConfig, wilder_atr};
use cfds_ingest::{daily_drawdowns, daily_indicators};
use cfds_market_data::ExchangeCalendar;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn de40() -> InstrumentId {
    "DE40".parse().unwrap()
}

fn calendar() -> ExchangeCalendar {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    ExchangeCalendar::load(&dir, HashMap::from([(de40(), "XETR".to_string())])).unwrap()
}

fn cfg() -> DailyConfig {
    DailyConfig {
        atr_period: 2,
        realized_vol_window: 2,
        drawdown_window: 2,
    }
}

/// Días seguidos desde `first`, con cierres `closes` (máximo +1 y mínimo −1).
fn days(first: &str, closes: &[Decimal]) -> Vec<DailyCandle> {
    let d0: NaiveDate = first.parse().unwrap();
    let t0: DateTime<Utc> = format!("{first}T00:00:00Z").parse().unwrap();
    closes
        .iter()
        .enumerate()
        .map(|(i, &close)| DailyCandle {
            instrument_id: de40(),
            day: d0 + TimeDelta::days(i as i64),
            start: t0 + TimeDelta::days(i as i64) - TimeDelta::hours(1),
            end: t0 + TimeDelta::days(i as i64 + 1) - TimeDelta::hours(1),
            open: close,
            high: close + Decimal::ONE,
            low: close - Decimal::ONE,
            close,
            volume: None,
        })
        .collect()
}

#[test]
fn days_without_session_are_ignored() {
    let cal = calendar();
    let session = cal.session_for(&de40()).unwrap();
    // Lunes 16 a lunes 23 de noviembre de 2026: el sábado 21 y el domingo 22 no tienen sesión.
    let closes = [100, 101, 102, 103, 104, 999, 999, 105].map(Decimal::from);
    let rows = daily_indicators(&days("2026-11-16", &closes), session, &cfg()).unwrap();
    let got: Vec<String> = rows.iter().map(|r| r.day.to_string()).collect();
    assert_eq!(
        got,
        [
            "2026-11-16",
            "2026-11-17",
            "2026-11-18",
            "2026-11-19",
            "2026-11-20",
            "2026-11-23"
        ]
    );
    // Los 999 del fin de semana no entran al ATR: es el de los días con sesión.
    let with_session: Vec<_> = days("2026-11-16", &closes)
        .into_iter()
        .filter(|d| d.close != dec!(999))
        .collect();
    let atr: Vec<_> = rows.iter().map(|r| r.atr).collect();
    assert_eq!(atr, wilder_atr(&with_session, 2));
}

#[test]
fn published_when_the_daily_candle_ends_and_warms_up() {
    let cal = calendar();
    let session = cal.session_for(&de40()).unwrap();
    let d = days("2026-11-16", &[100, 101, 102].map(Decimal::from));
    let rows = daily_indicators(&d, session, &cfg()).unwrap();
    assert!(rows.iter().zip(&d).all(|(r, c)| r.published_at == c.end));
    // ATR(2): desde el segundo día. Volatilidad de 2 retornos: desde el tercero.
    assert_eq!((rows[0].atr, rows[1].atr.is_some()), (None, true));
    assert_eq!(
        (rows[1].realized_vol, rows[2].realized_vol.is_some()),
        (None, true)
    );
    assert_eq!((rows[0].atr_period, rows[0].realized_vol_window), (2, 2));
}

#[test]
fn a_day_outside_the_calendar_is_an_error_not_a_guess() {
    let cal = calendar();
    let session = cal.session_for(&de40()).unwrap();
    let d = days("1990-01-02", &[dec!(100)]);
    assert_eq!(
        daily_indicators(&d, session, &cfg()),
        Err("1990-01-02".parse().unwrap())
    );
}

#[test]
fn drawdown_uses_the_same_session_days_and_the_end_of_the_candle() {
    let cal = calendar();
    let session = cal.session_for(&de40()).unwrap();
    // Viernes 20, sábado 21 y domingo 22 (999, sin sesión) y lunes 23 de noviembre de 2026.
    let closes = [100, 999, 999, 90].map(Decimal::from);
    let d = days("2026-11-20", &closes);
    let rows = daily_drawdowns(&d, session, &cfg()).unwrap();
    let got: Vec<_> = rows
        .iter()
        .map(|r| (r.day.to_string(), r.drawdown))
        .collect();
    // Ventana 2: el máximo del lunes es el 100 del viernes, no el 999 del fin de semana.
    assert_eq!(
        got,
        [
            ("2026-11-20".to_owned(), None),
            ("2026-11-23".to_owned(), Some(dec!(-0.1)))
        ]
    );
    assert_eq!(rows[1].published_at, d[3].end);
    assert_eq!(rows[1].drawdown_window, 2);
    let outside = days("1990-01-02", &[dec!(100)]);
    assert_eq!(
        daily_drawdowns(&outside, session, &cfg()),
        Err("1990-01-02".parse().unwrap())
    );
}

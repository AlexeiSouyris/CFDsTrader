//! Calendario de sesiones con los archivos reales de `config/`.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{
    ExchangeCalendar, IssueKind, SessionCalendar, ValidationConfig, validate_series,
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::dec;

fn config_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config")
}

fn id(s: &str) -> InstrumentId {
    s.parse().unwrap()
}

fn calendar() -> ExchangeCalendar {
    let instruments = HashMap::from([(id("US100"), "XNYS".into()), (id("DE40"), "XETR".into())]);
    ExchangeCalendar::load(&config_dir(), instruments).unwrap()
}

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

/// Apertura y cierre en UTC de un día hábil.
fn hours(exchange: &str, day: &str) -> (DateTime<Utc>, DateTime<Utc>) {
    calendar()
        .session(exchange)
        .unwrap()
        .hours(date(day))
        .unwrap()
        .unwrap()
}

#[test]
fn new_york_follows_us_daylight_saving_time() {
    // Invierno (EST, UTC−5): 09:30–16:00 local = 14:30–21:00 UTC.
    assert_eq!(
        hours("XNYS", "2026-01-05"),
        (utc("2026-01-05T14:30:00Z"), utc("2026-01-05T21:00:00Z"))
    );
    // Verano (EDT, UTC−4): 13:30–20:00 UTC.
    assert_eq!(
        hours("XNYS", "2026-07-06"),
        (utc("2026-07-06T13:30:00Z"), utc("2026-07-06T20:00:00Z"))
    );
}

#[test]
fn the_weeks_where_us_and_europe_change_time_on_different_dates() {
    // EE. UU. pasa a verano el 8 de marzo de 2026; Europa recién el 29.
    // Lunes 9 de marzo: Nueva York ya en EDT, Berlín todavía en CET (UTC+1).
    assert_eq!(hours("XNYS", "2026-03-09").0, utc("2026-03-09T13:30:00Z"));
    assert_eq!(hours("XETR", "2026-03-09").0, utc("2026-03-09T08:00:00Z"));
    // Lunes 30 de marzo: los dos en verano.
    assert_eq!(hours("XETR", "2026-03-30").0, utc("2026-03-30T07:00:00Z"));
    // Otoño: Europa vuelve a invierno el 25 de octubre, EE. UU. el 1 de noviembre.
    assert_eq!(hours("XETR", "2026-10-26").0, utc("2026-10-26T08:00:00Z"));
    assert_eq!(hours("XNYS", "2026-10-26").0, utc("2026-10-26T13:30:00Z"));
    assert_eq!(hours("XNYS", "2026-11-02").0, utc("2026-11-02T14:30:00Z"));
}

#[test]
fn holidays_and_weekends_have_no_session() {
    let cal = calendar();
    let nyse = cal.session("XNYS").unwrap();
    assert_eq!(nyse.hours(date("2026-11-26")), Some(None)); // Thanksgiving
    assert_eq!(nyse.hours(date("2025-01-09")), Some(None)); // cierre extraordinario (funeral de J. Carter)
    assert_eq!(nyse.hours(date("2026-11-28")), Some(None)); // sábado
    let xetra = cal.session("XETR").unwrap();
    assert_eq!(xetra.hours(date("2026-04-06")), Some(None)); // lunes de Pascua
    assert!(xetra.hours(date("2026-11-26")).unwrap().is_some()); // Thanksgiving no es feriado en Alemania
}

#[test]
fn early_closes_shorten_the_session() {
    // Viernes después de Thanksgiving: Nueva York cierra a las 13:00 local (18:00 UTC).
    assert_eq!(hours("XNYS", "2026-11-27").1, utc("2026-11-27T18:00:00Z"));
    // Xetra cierra a las 14:00 el 30 de diciembre.
    assert_eq!(hours("XETR", "2026-12-30").1, utc("2026-12-30T13:00:00Z"));
}

#[test]
fn expected_candles_in_a_regular_and_a_short_day() {
    let cal = calendar();
    let starts = |tf, day: &str| {
        let d = utc(&format!("{day}T00:00:00Z"));
        cal.expected_starts(&id("US100"), tf, d, d + TimeDelta::days(1))
            .unwrap()
    };
    // Día normal en Nueva York: 6,5 h → 26 velas de 15m; de 1h, de 10:00 a 15:00 local → 6.
    assert_eq!(starts(Timeframe::M15, "2026-11-25").len(), 26);
    let h1 = starts(Timeframe::H1, "2026-11-25");
    assert_eq!((h1.len(), h1[0]), (6, utc("2026-11-25T15:00:00Z")));
    // Thanksgiving: ninguna. Día siguiente (cierre 13:00): 3,5 h → 14 velas de 15m.
    assert!(starts(Timeframe::M15, "2026-11-26").is_empty());
    assert_eq!(starts(Timeframe::M15, "2026-11-27").len(), 14);
    // Xetra, 09:00–17:30: 34 de 15m y 8 de 1h (09:00 a 16:00).
    let d = utc("2026-11-25T00:00:00Z");
    let xetra = |tf| {
        cal.expected_starts(&id("DE40"), tf, d, d + TimeDelta::days(1))
            .unwrap()
            .len()
    };
    assert_eq!((xetra(Timeframe::M15), xetra(Timeframe::H1)), (34, 8));
}

#[test]
fn is_open_at_respects_the_exact_bounds() {
    let nyse = calendar().session("XNYS").unwrap().clone();
    assert_eq!(nyse.is_open_at(utc("2026-11-25T14:29:59Z")), Some(false));
    assert_eq!(nyse.is_open_at(utc("2026-11-25T14:30:00Z")), Some(true));
    assert_eq!(nyse.is_open_at(utc("2026-11-25T20:59:59Z")), Some(true));
    assert_eq!(nyse.is_open_at(utc("2026-11-25T21:00:00Z")), Some(false));
}

#[test]
fn unknown_dates_or_instruments_are_never_assumed_closed() {
    let cal = calendar();
    let d = utc("2028-03-01T00:00:00Z"); // fuera del rango de los archivos (2021–2027)
    assert_eq!(
        cal.expected_starts(&id("US100"), Timeframe::M15, d, d + TimeDelta::days(1)),
        None
    );
    assert_eq!(
        cal.expected_starts(&id("AAPL"), Timeframe::M15, d, d + TimeDelta::days(1)),
        None
    );
}

fn us100(start: DateTime<Utc>) -> Candle {
    Candle {
        instrument_id: id("US100"),
        timeframe: Timeframe::M15,
        start,
        open: dec!(26700),
        high: dec!(26706),
        low: dec!(26696),
        close: dec!(26701),
        volume: None,
    }
}

fn cfg() -> ValidationConfig {
    ValidationConfig::from_toml(include_str!("../../../../config/data_validation.toml")).unwrap()
}

#[test]
fn validation_accepts_the_thanksgiving_gap_but_not_a_missing_candle() {
    let cal = calendar();
    // Todas las velas esperadas del 25 al 27 de noviembre: el 26 (feriado) no tiene ninguna.
    let from = utc("2026-11-25T00:00:00Z");
    let all = cal
        .expected_starts(
            &id("US100"),
            Timeframe::M15,
            from,
            from + TimeDelta::days(3),
        )
        .unwrap();
    let mut series: Vec<Candle> = all.iter().copied().map(us100).collect();
    assert!(validate_series(&series, &cal, &cfg()).is_empty());

    let removed = series.remove(10).start; // falta una vela en plena sesión
    let issues = validate_series(&series, &cal, &cfg());
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].kind,
        IssueKind::MissingCandles {
            count: 1,
            first_missing: removed
        }
    );
}

#[test]
fn validation_reports_gaps_the_calendar_cannot_judge() {
    let cal = calendar();
    let a = us100(utc("2027-12-31T20:45:00Z")); // última vela cubierta por el calendario
    let b = us100(utc("2028-01-03T14:30:00Z")); // ya fuera del rango
    let issues = validate_series(&[a.clone(), b], &cal, &cfg());
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].kind,
        IssueKind::CalendarUnavailable { gap_from: a.end() }
    );
}

#[test]
fn invalid_configuration_is_rejected_at_load_time() {
    let wrong = HashMap::from([(id("US100"), "XTKS".to_string())]); // bolsa que no existe
    assert!(ExchangeCalendar::load(&config_dir(), wrong).is_err());
}

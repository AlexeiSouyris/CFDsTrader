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

// ---------- Sesión que cruza la medianoche: oro de CME Globex (ADR 0013) ----------

fn gold() -> cfds_market_data::Session {
    calendar().session("CMEGLOBEX_METALS").unwrap().clone()
}

#[test]
fn the_gold_session_opens_the_evening_before() {
    let g = gold();
    assert!(g.is_overnight());
    // Miércoles 25-11-2026 (CST, UTC−6): abre el martes 17:00 CT, cierra el miércoles 16:00 CT.
    assert_eq!(
        g.hours(date("2026-11-25")),
        Some(Some((
            utc("2026-11-24T23:00:00Z"),
            utc("2026-11-25T22:00:00Z")
        )))
    );
    // Verano (CDT, UTC−5): una hora antes en UTC. 18:00–17:00 en Nueva York en los dos casos.
    assert_eq!(
        g.hours(date("2026-07-08")),
        Some(Some((
            utc("2026-07-07T22:00:00Z"),
            utc("2026-07-08T21:00:00Z")
        )))
    );
    // La sesión del lunes abre el domingo; sábado y domingo no son días hábiles.
    assert_eq!(
        g.hours(date("2026-11-23")).unwrap().unwrap().0,
        utc("2026-11-22T23:00:00Z")
    );
    assert_eq!(g.hours(date("2026-11-28")), Some(None));
    assert_eq!(g.hours(date("2026-11-29")), Some(None));
}

#[test]
fn each_instant_belongs_to_the_session_that_contains_it() {
    let g = gold();
    // Domingo 17:30 CT → día hábil lunes.
    assert_eq!(
        g.trading_day(utc("2026-11-22T23:30:00Z")),
        date("2026-11-23")
    );
    // Miércoles 09:00 CT → miércoles.
    assert_eq!(
        g.trading_day(utc("2026-11-25T15:00:00Z")),
        date("2026-11-25")
    );
    // Una sesión normal no cambia: Nueva York 23:30 UTC es el mismo día local.
    let nyse = calendar().session("XNYS").unwrap().clone();
    assert_eq!(
        nyse.trading_day(utc("2026-11-25T23:30:00Z")),
        date("2026-11-25")
    );
}

#[test]
fn the_daily_pause_and_the_weekend_are_closed() {
    let g = gold();
    // Pausa diaria de 60 minutos: 16:00–17:00 CT (22:00–23:00 UTC en invierno).
    assert_eq!(g.is_open_at(utc("2026-11-25T21:59:59Z")), Some(true));
    assert_eq!(g.is_open_at(utc("2026-11-25T22:00:00Z")), Some(false));
    assert_eq!(g.is_open_at(utc("2026-11-25T22:59:59Z")), Some(false));
    assert_eq!(g.is_open_at(utc("2026-11-25T23:00:00Z")), Some(true));
    // Fin de semana: cierra el viernes 16:00 CT y abre el domingo 17:00 CT.
    assert_eq!(g.is_open_at(utc("2026-12-04T22:00:00Z")), Some(false));
    assert_eq!(g.is_open_at(utc("2026-12-05T12:00:00Z")), Some(false));
    assert_eq!(g.is_open_at(utc("2026-12-06T22:59:59Z")), Some(false));
    assert_eq!(g.is_open_at(utc("2026-12-06T23:00:00Z")), Some(true));
}

#[test]
fn cme_holidays_and_early_closes() {
    let g = gold();
    // Navidad: sin sesión. Thanksgiving y el viernes siguiente: cierre anticipado.
    assert_eq!(g.hours(date("2026-12-25")), Some(None));
    assert_eq!(
        g.hours(date("2026-11-26")).unwrap().unwrap().1,
        utc("2026-11-26T19:30:00Z")
    );
    assert_eq!(
        g.hours(date("2026-11-27")).unwrap().unwrap().1,
        utc("2026-11-27T19:45:00Z")
    );
    // Fuera de la cobertura del archivo: "no sé", nunca cerrado ni abierto.
    assert_eq!(g.hours(date("2028-03-01")), None);
}

#[test]
fn expected_gold_candles_across_midnight() {
    let g = gold();
    let day = |d: &str| g.hours(date(d)).unwrap().unwrap();
    let (open, close) = day("2026-11-25");
    // 23 h → 92 velas de 15m y 23 de 1h, la primera a la apertura del día anterior.
    let m15 = g.candle_starts(Timeframe::M15, open, close).unwrap();
    assert_eq!((m15.len(), m15[0]), (92, open));
    assert_eq!(
        g.candle_starts(Timeframe::H1, open, close).unwrap().len(),
        23
    );
    // Un rango de un día UTC toma partes de dos sesiones y ninguna vela de la pausa.
    let d = utc("2026-11-25T00:00:00Z");
    let starts = g
        .candle_starts(Timeframe::M15, d, d + TimeDelta::days(1))
        .unwrap();
    assert_eq!(starts.len(), 92);
    assert!(
        !starts
            .iter()
            .any(|s| *s >= utc("2026-11-25T22:00:00Z") && *s < utc("2026-11-25T23:00:00Z"))
    );
}

#[test]
fn an_overnight_session_needs_distinct_open_and_close() {
    let dir = std::env::temp_dir().join(format!("cfds-cal-oro-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("holidays")).unwrap();
    let holidays = "exchange = \"X\"\nsource = \"prueba\"\ngenerated_on = \"2026-11-24\"\n\
                    from = \"2026-01-01\"\nto = \"2026-12-31\"\nclosed = []\n[early_closes]\n";
    std::fs::write(dir.join("holidays/X.toml"), holidays).unwrap();
    let write = |open: &str, close: &str, extra: &str| {
        std::fs::write(
            dir.join("sessions.toml"),
            format!(
                "[sessions.X]\nname = \"x\"\ntimezone = \"America/Chicago\"\nopen = \"{open}\"\n\
                 close = \"{close}\"\nholidays = \"holidays/X.toml\"\n"
            ),
        )
        .unwrap();
        std::fs::write(dir.join("holidays/X.toml"), format!("{holidays}{extra}")).unwrap();
        ExchangeCalendar::load(&dir, HashMap::new())
    };
    assert!(write("17:00", "16:00", "").is_ok());
    assert!(write("17:00", "17:00", "").is_err());
    // Cierre anticipado de una sesión nocturna: antes del cierre habitual, no después.
    assert!(write("17:00", "16:00", "\"2026-11-26\" = \"12:00\"\n").is_ok());
    assert!(write("17:00", "16:00", "\"2026-11-26\" = \"16:30\"\n").is_err());
    std::fs::remove_dir_all(&dir).ok();
}

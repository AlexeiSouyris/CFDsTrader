//! Velas diarias (#103): límites del día local (incluidos los cambios de horario) y validación.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_core::DailyCandle;
use cfds_market_data::{DailyIssueKind, ExchangeCalendar, local_day_bounds, validate_daily};
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::{Europe::Madrid, Tz};
use rust_decimal::{Decimal, dec};

fn calendar() -> ExchangeCalendar {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    ExchangeCalendar::load(&dir, HashMap::new()).unwrap()
}

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

/// Zona de los precios de la cuenta de IG (medida: UTC+1 en invierno, UTC+2 en verano).
const TZ: Tz = Madrid;

fn daily(day: &str) -> DailyCandle {
    let (start, end) = local_day_bounds(date(day), TZ).unwrap();
    DailyCandle {
        instrument_id: "DE40".parse().unwrap(),
        day: date(day),
        start,
        end,
        open: dec!(24000),
        high: dec!(24100),
        low: dec!(23900),
        close: dec!(24050),
        volume: None,
    }
}

fn kinds(c: &[DailyCandle]) -> Vec<(usize, DailyIssueKind)> {
    let cal = calendar();
    validate_daily(c, TZ, cal.session("XETR").unwrap())
        .into_iter()
        .map(|i| (i.index, i.kind))
        .collect()
}

#[test]
fn the_day_runs_from_local_midnight_to_local_midnight() {
    // Medido en la demo de IG el 21-11-2026: 23:00 UTC en noviembre, 22:00 UTC en julio.
    assert_eq!(
        local_day_bounds(date("2026-11-16"), TZ),
        Some((utc("2026-11-15T23:00:00Z"), utc("2026-11-16T23:00:00Z")))
    );
    assert_eq!(
        local_day_bounds(date("2026-07-13"), TZ),
        Some((utc("2026-07-12T22:00:00Z"), utc("2026-07-13T22:00:00Z")))
    );
    // Cambios de horario: el día dura 23 h en marzo y 25 h en octubre.
    let (s, e) = local_day_bounds(date("2026-03-29"), TZ).unwrap();
    assert_eq!(
        (s, e),
        (utc("2026-03-28T23:00:00Z"), utc("2026-03-29T22:00:00Z"))
    );
    let (s, e) = local_day_bounds(date("2026-10-25"), TZ).unwrap();
    assert_eq!(
        (s, e),
        (utc("2026-10-24T22:00:00Z"), utc("2026-10-25T23:00:00Z"))
    );
}

#[test]
fn a_healthy_week_has_no_issues_and_weekends_are_not_gaps() {
    let days = [
        "2026-11-13",
        "2026-11-16",
        "2026-11-17",
        "2026-11-18",
        "2026-11-19",
        "2026-11-20",
    ];
    let c: Vec<_> = days.iter().map(|d| daily(d)).collect();
    assert_eq!(kinds(&c), vec![]);
    assert_eq!(kinds(&[]), vec![]);
}

#[test]
fn a_missing_session_day_is_reported_but_a_holiday_is_not() {
    let c = [daily("2026-11-17"), daily("2026-11-19")];
    assert_eq!(
        kinds(&c),
        vec![(
            1,
            DailyIssueKind::MissingDays {
                count: 1,
                first: date("2026-11-18")
            }
        )]
    );
    // 24 y 25-12-2026: Xetra cerrado. Del 23 al 28 no falta nada.
    assert_eq!(kinds(&[daily("2026-12-23"), daily("2026-12-28")]), vec![]);
}

#[test]
fn bounds_of_a_fixed_24h_day_are_wrong_on_a_daylight_saving_change() {
    let mut c = daily("2026-10-25");
    let right = (c.start, c.end);
    c.end = c.start + chrono::TimeDelta::hours(24);
    assert_eq!(
        kinds(&[c]),
        vec![(
            0,
            DailyIssueKind::WrongBounds {
                start: right.0,
                end: right.1
            }
        )]
    );
}

#[test]
fn corrupted_candles_are_reported() {
    let mut zero = daily("2026-11-17");
    zero.low = Decimal::ZERO;
    let ks = kinds(&[zero]);
    assert!(
        ks.iter()
            .any(|(_, k)| matches!(k, DailyIssueKind::NonPositivePrice { field: "low", .. }))
    );

    let mut bad = daily("2026-11-17");
    bad.high = dec!(23000);
    assert!(matches!(
        kinds(&[bad])[..],
        [(0, DailyIssueKind::InconsistentOhlc { .. })]
    ));

    let dup = [daily("2026-11-17"), daily("2026-11-17")];
    assert_eq!(
        kinds(&dup),
        vec![(
            1,
            DailyIssueKind::OutOfOrder {
                previous: date("2026-11-17")
            }
        )]
    );
    let back = [daily("2026-11-18"), daily("2026-11-17")];
    assert!(matches!(
        kinds(&back)[..],
        [(1, DailyIssueKind::OutOfOrder { .. })]
    ));

    let mut other = daily("2026-11-18");
    other.instrument_id = "US100".parse().unwrap();
    assert!(matches!(
        kinds(&[daily("2026-11-17"), other])[..],
        [(1, DailyIssueKind::MixedInstruments(_))]
    ));
}

#[test]
fn outside_the_calendar_nothing_is_assumed() {
    // El calendario de feriados cubre hasta 2027: en 2028 no se puede saber si falta un día.
    let c = [daily("2027-12-30"), daily("2028-01-04")];
    assert!(matches!(
        kinds(&c)[..],
        [(1, DailyIssueKind::CalendarUnavailable { .. })]
    ));
}

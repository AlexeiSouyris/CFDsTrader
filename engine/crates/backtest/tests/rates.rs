//! Tipo de referencia vigente en cada noche (#79): sin look-ahead y con correcciones.

use cfds_backtest::{
    Commission, DailyRates, FundingRates, InstrumentCosts, funding, funding_cutoffs, funding_over,
};
use cfds_core::ReferenceRate;
use cfds_strategy::Side;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rust_decimal::{Decimal, dec};

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn rate(ref_date: &str, published: &str, pct: Decimal) -> ReferenceRate {
    ReferenceRate {
        series: "ESTR".into(),
        ref_date: day(ref_date),
        published_at: utc(published),
        rate_pct: pct,
    }
}

/// Jueves 12, viernes 13 y lunes 16 de noviembre, publicados a las 09:00 CET (08:00 UTC) del
/// día hábil siguiente.
fn november() -> DailyRates {
    DailyRates::new(vec![
        rate("2026-11-16", "2026-11-17T08:00:00Z", dec!(2.44)),
        rate("2026-11-12", "2026-11-13T08:00:00Z", dec!(2.443)),
        rate("2026-11-13", "2026-11-16T08:00:00Z", dec!(2.437)),
    ])
    .unwrap()
}

#[test]
fn before_anything_was_published_there_is_no_rate() {
    assert_eq!(november().at(utc("2026-11-13T07:59:59Z")), None);
}

#[test]
fn a_value_counts_from_its_publication_never_before() {
    let r = november();
    assert_eq!(r.at(utc("2026-11-13T08:00:00Z")), Some(dec!(2.443)));
    // Viernes y fin de semana: sigue el del jueves hasta que el lunes se publica el del viernes.
    assert_eq!(r.at(utc("2026-11-15T22:00:00Z")), Some(dec!(2.443)));
    assert_eq!(r.at(utc("2026-11-16T08:00:00Z")), Some(dec!(2.437)));
    assert_eq!(
        r.at_with_day(utc("2026-11-20T22:00:00Z")),
        Some((day("2026-11-16"), dec!(2.44)))
    );
}

#[test]
fn a_correction_of_the_same_day_replaces_it_from_when_it_is_published() {
    let r = DailyRates::new(vec![
        rate("2026-11-12", "2026-11-13T07:00:00Z", dec!(2.443)),
        rate("2026-11-12", "2026-11-13T08:00:00Z", dec!(2.5)),
    ])
    .unwrap();
    assert_eq!(r.at(utc("2026-11-13T07:30:00Z")), Some(dec!(2.443)));
    assert_eq!(r.at(utc("2026-11-13T08:00:00Z")), Some(dec!(2.5)));
}

#[test]
fn a_late_correction_of_an_old_day_does_not_override_a_newer_day() {
    let mut rows = vec![
        rate("2026-11-12", "2026-11-13T08:00:00Z", dec!(2.443)),
        rate("2026-11-13", "2026-11-16T08:00:00Z", dec!(2.437)),
    ];
    // Corrección del jueves leída recién el martes.
    rows.push(rate("2026-11-12", "2026-11-17T10:00:00Z", dec!(9.9)));
    let r = DailyRates::new(rows).unwrap();
    assert_eq!(r.at(utc("2026-11-18T00:00:00Z")), Some(dec!(2.437)));
}

#[test]
fn empty_or_mixed_series_are_rejected() {
    assert!(DailyRates::new(vec![]).is_none());
    let mut other = rate("2026-11-12", "2026-11-13T08:00:00Z", dec!(5));
    other.series = "SOFR".into();
    assert!(
        DailyRates::new(vec![
            rate("2026-11-12", "2026-11-13T08:00:00Z", dec!(2)),
            other
        ])
        .is_none()
    );
}

#[test]
fn a_constant_rate_holds_at_any_time() {
    let c = FundingRates::Constant(dec!(2.441));
    assert_eq!(c.at(utc("2000-01-01T00:00:00Z")), Some(dec!(2.441)));
}

// ---------- Financiación noche por noche ----------

fn index() -> InstrumentCosts {
    InstrumentCosts {
        spread: dec!(1.8),
        tick: dec!(0.1),
        commission: Commission::None,
        funding_admin_fee_pct: dec!(2.5),
        funding_day_count: dec!(360),
    }
}

fn cutoffs(open: &str, close: &str) -> Vec<DateTime<Utc>> {
    funding_cutoffs(
        utc(open),
        utc(close),
        NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
        chrono_tz::Europe::Madrid,
    )
}

#[test]
fn each_night_pays_the_rate_in_force_at_its_cutoff() {
    // Del jueves 12 a las 15:00 al martes 17 a las 15:00 de Madrid: cortes del 12, 13, 14, 15
    // y 16 a las 23:00 (22:00 UTC).
    let nights = cutoffs("2026-11-12T14:00:00Z", "2026-11-17T14:00:00Z");
    assert_eq!(nights.len(), 5);
    let rates = FundingRates::Daily(november());
    // Antes del 13 08:00 UTC no hay nada publicado: el jueves 12 no tiene tasa.
    assert_eq!(
        funding_over(Side::Long, dec!(20000), dec!(1), &nights, &rates, &index()),
        Err(utc("2026-11-12T22:00:00Z"))
    );
    // Desde el viernes: 13, 14 y 15 con 2,443 (jueves); 16 con 2,437 (viernes).
    let later = &nights[1..];
    let got = funding_over(Side::Long, dec!(20000), dec!(1), later, &rates, &index()).unwrap();
    let expected = funding(Side::Long, dec!(20000), dec!(1), 3, dec!(2.443), &index())
        + funding(Side::Long, dec!(20000), dec!(1), 1, dec!(2.437), &index());
    assert_eq!(got, expected);
}

#[test]
fn with_a_constant_rate_it_matches_the_old_formula() {
    let nights = cutoffs("2026-11-12T14:00:00Z", "2026-11-17T14:00:00Z");
    let c = FundingRates::Constant(dec!(1.1));
    for side in [Side::Long, Side::Short] {
        assert_eq!(
            funding_over(side, dec!(20000), dec!(2), &nights, &c, &index()),
            Ok(funding(side, dec!(20000), dec!(2), 5, dec!(1.1), &index()))
        );
    }
}

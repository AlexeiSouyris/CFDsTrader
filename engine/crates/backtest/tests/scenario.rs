//! Escenarios de financiación de un estudio (#405): solo en memoria y sin look-ahead.

use cfds_backtest::{
    DailyRates, SCENARIO_SOURCE, SetupError, add_weekdays, apply_funding_scenario,
    market_terms_from_catalog, rates_from_csv,
};
use cfds_core::InstrumentId;
use cfds_market_data::{Catalog, Sourced};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rust_decimal::dec;

const CATALOG: &str = include_str!("../../../../config/instruments.toml");

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn one_pm() -> NaiveTime {
    NaiveTime::from_hms_opt(13, 0, 0).unwrap()
}

#[test]
fn weekdays_skip_the_weekend() {
    // Jueves + 2 hábiles = lunes; viernes + 1 = lunes; miércoles + 2 = viernes.
    assert_eq!(add_weekdays(day("2026-11-19"), 2), day("2026-11-23"));
    assert_eq!(add_weekdays(day("2026-11-20"), 1), day("2026-11-23"));
    assert_eq!(add_weekdays(day("2026-11-18"), 2), day("2026-11-20"));
    assert_eq!(add_weekdays(day("2026-11-18"), 0), day("2026-11-18"));
}

#[test]
fn csv_values_count_only_after_the_lagged_publication() {
    let text = "# NY Fed\neffective_date,percent_rate\n2026-11-19,3.91\n2026-11-20,3.92\n\n";
    let rates = rates_from_csv("SOFR", text, 2, one_pm()).unwrap();
    assert_eq!(rates.len(), 2);
    assert_eq!(rates[0].series, "SOFR");
    assert_eq!(rates[0].rate_pct, dec!(3.91));
    assert_eq!(rates[0].published_at, utc("2026-11-23T13:00:00Z"));
    assert_eq!(rates[1].published_at, utc("2026-11-24T13:00:00Z"));
    let daily = DailyRates::new(rates).unwrap();
    // Antes de la publicación del jueves 19 no hay tasa: nunca se adelanta.
    assert_eq!(daily.at(utc("2026-11-23T12:59:59Z")), None);
    assert_eq!(daily.at(utc("2026-11-23T13:00:00Z")), Some(dec!(3.91)));
    assert_eq!(daily.at(utc("2026-11-24T13:00:00Z")), Some(dec!(3.92)));
}

#[test]
fn bad_or_empty_csv_is_rejected() {
    assert!(rates_from_csv("SOFR", "effective_date,percent_rate\n", 2, one_pm()).is_err());
    assert!(rates_from_csv("SOFR", "2026-11-19,3.91\n2026-11-xx,3.92\n", 2, one_pm()).is_err());
    assert!(rates_from_csv("SOFR", "2026-11-19,abc\n", 2, one_pm()).is_err());
    assert!(rates_from_csv("SOFR", "2026-11-19,3.91,extra\n", 2, one_pm()).is_err());
    // Solo la primera línea con datos puede ser cabecera.
    assert!(rates_from_csv("SOFR", "2026-11-19,3.91\nfecha,tasa\n", 2, one_pm()).is_err());
}

#[test]
fn the_scenario_completes_a_pending_benchmark_only_in_memory() {
    let original = Catalog::from_toml(CATALOG).unwrap();
    let us500: InstrumentId = "US500".parse().unwrap();
    // Con el catálogo de config, US500 no se puede simular: el tipo de referencia está pendiente.
    assert!(matches!(
        market_terms_from_catalog(&original, &us500),
        Err(SetupError::Pending(_))
    ));

    let mut sofr = original.clone();
    let before = apply_funding_scenario(&mut sofr, &us500, "SOFR", dec!(3.92)).unwrap();
    assert_eq!(before, None);
    let terms = market_terms_from_catalog(&sofr, &us500).unwrap();
    // La tasa administrativa y la base de días siguen siendo las del catálogo.
    assert_eq!(terms.costs.funding_admin_fee_pct, dec!(3));
    assert_eq!(terms.costs.funding_day_count, dec!(360));
    assert_eq!(
        sofr.funding_benchmarks.get("SOFR"),
        Some(&Sourced::Known {
            value: dec!(3.92),
            source: SCENARIO_SOURCE.into()
        })
    );

    // Un tipo que ya está en el catálogo (ESTR) conserva su valor y su fuente.
    let mut estr = original.clone();
    apply_funding_scenario(&mut estr, &us500, "ESTR", dec!(99)).unwrap();
    assert_eq!(
        estr.funding_benchmarks.get("ESTR"),
        original.funding_benchmarks.get("ESTR")
    );
    assert!(market_terms_from_catalog(&estr, &us500).is_ok());

    // El catálogo original no cambió, y un instrumento desconocido es un error.
    assert_eq!(original, Catalog::from_toml(CATALOG).unwrap());
    let unknown: InstrumentId = "XX1".parse().unwrap();
    assert!(apply_funding_scenario(&mut estr, &unknown, "ESTR", dec!(1)).is_err());
}

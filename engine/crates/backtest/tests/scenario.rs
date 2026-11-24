//! Escenarios de financiación (#405) y costos supuestos (#421) de un estudio: solo en memoria,
//! sin look-ahead y sin pisar datos con fuente.

use cfds_backtest::{
    DailyRates, SCENARIO_SOURCE, SetupError, StudyAssumptions, add_weekdays, apply_assumptions,
    apply_funding_scenario, market_terms_from_catalog, rates_from_csv,
};
use cfds_core::InstrumentId;
use cfds_market_data::{CandleSources, Catalog, RollDay, Sourced};
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

const SOURCES: &str = include_str!("../../../../config/candle_sources.toml");

const US2000_ASSUMED: &str = r#"
source = "supuesto de prueba"
[instruments.US2000]
margin_retail_pct = "10"
spread_min_points = "0.8"
spread_typical_points = "0.9"
value_per_point = "1"
min_deal_size = "1"
tick = "0.1"
funding_admin_fee_pct = "3"
roll = "after_expiry_day"
"#;

#[test]
fn assumptions_fill_only_pending_terms_in_memory() {
    let original = Catalog::from_toml(CATALOG).unwrap();
    let mut catalog = original.clone();
    let mut sources = CandleSources::from_toml(SOURCES).unwrap();
    let us2000: InstrumentId = "US2000".parse().unwrap();
    // Sin regla de vencimiento en memoria, el supuesto la completa (#449 la agrega a config).
    sources.roll.remove(&us2000);
    let a = StudyAssumptions::from_toml(US2000_ASSUMED).unwrap();
    let filled = apply_assumptions(&mut catalog, &mut sources, &a).unwrap();
    assert_eq!(filled.len(), 8);
    assert_eq!(sources.roll.get(&us2000), Some(&RollDay::AfterExpiryDay));
    let t = &catalog.instruments[&us2000].terms;
    assert_eq!(
        t.spread_typical_points,
        Sourced::Known {
            value: dec!(0.9),
            source: "supuesto de prueba".into()
        }
    );
    // Falta el tipo de referencia; con el escenario de financiación, el US2000 ya se simula.
    assert!(matches!(
        market_terms_from_catalog(&catalog, &us2000),
        Err(SetupError::Pending(_))
    ));
    apply_funding_scenario(&mut catalog, &us2000, "ESTR", dec!(2)).unwrap();
    let terms = market_terms_from_catalog(&catalog, &us2000).unwrap();
    assert_eq!(terms.costs.spread, dec!(0.9));
    assert_eq!(terms.costs.funding_admin_fee_pct, dec!(3));
    assert_eq!(original, Catalog::from_toml(CATALOG).unwrap());
}

#[test]
fn assumptions_never_overwrite_sourced_data() {
    let mut sources = CandleSources::from_toml(SOURCES).unwrap();
    // US500 ya tiene spread con fuente y regla de vencimiento.
    for text in [
        "source = \"x\"\n[instruments.US500]\nspread_typical_points = \"0.5\"\n",
        "source = \"x\"\n[instruments.US500]\nroll = \"after_expiry_day\"\n",
        "source = \"x\"\n[instruments.US2000]\nspread_typical_points = \"0\"\n",
        "source = \"x\"\n[instruments.XX1]\ntick = \"1\"\n",
    ] {
        let mut catalog = Catalog::from_toml(CATALOG).unwrap();
        let a = StudyAssumptions::from_toml(text).unwrap();
        assert!(
            apply_assumptions(&mut catalog, &mut sources, &a).is_err(),
            "{text}"
        );
    }
    // Campos desconocidos: error al leer.
    assert!(
        StudyAssumptions::from_toml("source = \"x\"\n[instruments.US2000]\nepic = \"1\"\n")
            .is_err()
    );
}

#[test]
fn the_us2000_study_files_apply_cleanly() {
    for text in [
        include_str!("../../../../docs/analysis/2026-11-us2000-supuestos-us500.toml"),
        include_str!("../../../../docs/analysis/2026-11-us2000-supuestos-proporcional.toml"),
    ] {
        let mut catalog = Catalog::from_toml(CATALOG).unwrap();
        let mut sources = CandleSources::from_toml(SOURCES).unwrap();
        let a = StudyAssumptions::from_toml(text).unwrap();
        apply_assumptions(&mut catalog, &mut sources, &a).unwrap();
        let us2000: InstrumentId = "US2000".parse().unwrap();
        apply_funding_scenario(&mut catalog, &us2000, "ESTR", dec!(2)).unwrap();
        assert!(market_terms_from_catalog(&catalog, &us2000).is_ok());
    }
}

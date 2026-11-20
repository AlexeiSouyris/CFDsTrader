//! Catálogo de instrumentos con el `config/instruments.toml` real (#7).

use std::path::PathBuf;

use cfds_core::{AssetClass, Currency, Timeframe};
use cfds_market_data::{Catalog, ExchangeCalendar, SessionCalendar, Sourced};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::dec;

const REAL: &str = include_str!("../../../../config/instruments.toml");

fn catalog() -> Catalog {
    Catalog::from_toml(REAL).unwrap()
}

#[test]
fn the_repository_catalog_loads_with_its_sources() {
    let c = catalog();
    let ids: Vec<&str> = c.instruments.keys().map(|i| i.as_str()).collect();
    assert_eq!(ids, ["DE40", "US100", "US30", "US500"]);

    let us100 = &c.instruments[&"US100".parse().unwrap()];
    assert_eq!(us100.instrument.asset_class, AssetClass::Index);
    // Contrato de 1 € por punto (#67).
    assert_eq!(us100.instrument.currency, Currency::EUR);
    assert_eq!(us100.exchange, "XNYS");
    assert_eq!(
        us100.terms.margin_retail_pct,
        Sourced::Known {
            value: dec!(5),
            source: "ig-es-indices".into()
        }
    );
    assert_eq!(us100.terms.spread_min_points.value(), Some(&dec!(1.6)));
    assert_eq!(c.margin_closeout_pct.value(), Some(&dec!(50)));
}

#[test]
fn own_leverage_cap_is_below_the_regulatory_one() {
    for spec in catalog().instruments.values() {
        let margin = spec.terms.margin_retail_pct.value().unwrap();
        assert!(
            spec.max_leverage <= dec!(100) / margin,
            "{}",
            spec.instrument.id
        );
    }
}

#[test]
fn backtests_are_blocked_while_costs_are_pending() {
    let missing = catalog().ready_for_backtest().unwrap_err();
    let fields: Vec<_> = missing.iter().map(|p| (p.item.as_str(), p.field)).collect();
    // Solo falta confirmar el tipo de referencia de la financiación de los índices de EE. UU.
    assert_eq!(
        fields,
        [
            ("US100", "funding_benchmark"),
            ("US30", "funding_benchmark"),
            ("US500", "funding_benchmark"),
        ]
    );
    assert!(missing.iter().all(|p| !p.reason.is_empty()));
}

#[test]
fn broker_terms_read_from_the_ig_demo_api() {
    // #67: CFD al contado de 1 € por punto, valores de GET /markets/{epic} v3.
    let c = catalog();
    for (id, epic, min, spread) in [
        ("US100", "IX.D.NASDAQ.IFE.IP", dec!(0.5), dec!(1.6)),
        ("US500", "IX.D.SPTRD.IFE.IP", dec!(1), dec!(0.9)),
        ("US30", "IX.D.DOW.IFE.IP", dec!(0.2), dec!(3.7)),
        ("DE40", "IX.D.DAX.IFE.IP", dec!(0.5), dec!(1.8)),
    ] {
        let t = &c.instruments[&id.parse().unwrap()].terms;
        assert_eq!(t.epic.value().map(String::as_str), Some(epic), "{id}");
        assert_eq!(t.value_per_point.value(), Some(&dec!(1)), "{id}");
        assert_eq!(t.min_deal_size.value(), Some(&min), "{id}");
        // Regla de #67: el mayor entre el medido en la demo y el mínimo publicado.
        assert_eq!(t.spread_typical_points.value(), Some(&spread), "{id}");
        assert!(
            t.spread_typical_points.value() >= t.spread_min_points.value(),
            "{id}"
        );
        assert_eq!(
            c.instruments[&id.parse().unwrap()].instrument.currency,
            Currency::EUR
        );
    }
}

#[test]
fn feeds_the_session_calendar() {
    let config = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let c = catalog();
    let cal = ExchangeCalendar::load(&config, c.exchange_map()).unwrap();
    let day: DateTime<Utc> = "2026-11-25T00:00:00Z".parse().unwrap();
    for (id, expected) in [("US100", 26), ("DE40", 34)] {
        let n = cal
            .expected_starts(
                &id.parse().unwrap(),
                Timeframe::M15,
                day,
                day + TimeDelta::days(1),
            )
            .unwrap()
            .len();
        assert_eq!(n, expected, "{id}");
    }
}

#[test]
fn rejects_values_without_a_known_source() {
    let bad = REAL.replacen(r#"source = "ig-es-indices""#, r#"source = "inventada""#, 1);
    assert!(
        Catalog::from_toml(&bad)
            .unwrap_err()
            .to_string()
            .contains("inventada")
    );
}

#[test]
fn rejects_a_value_that_has_no_source_at_all() {
    let bad = REAL.replacen(
        r#"spread_min_points = { value = "1.6", source = "ig-es-indices" }"#,
        r#"spread_min_points = { value = "1.6" }"#,
        1,
    );
    assert!(Catalog::from_toml(&bad).is_err());
}

#[test]
fn rejects_a_leverage_cap_above_the_regulator() {
    let bad = REAL.replacen(r#"max_leverage = "10""#, r#"max_leverage = "30""#, 1);
    assert!(
        Catalog::from_toml(&bad)
            .unwrap_err()
            .to_string()
            .contains("max_leverage")
    );
}

#[test]
fn rejects_invalid_numbers_and_unknown_fields() {
    let negative = REAL.replacen(r#"value = "1.6""#, r#"value = "-1.6""#, 1);
    assert!(Catalog::from_toml(&negative).is_err());
    let day_count = REAL.replacen(r#"value = "360""#, r#"value = "300""#, 1);
    assert!(Catalog::from_toml(&day_count).is_err());
    let typo = REAL.replacen("spread_min_points", "spread_minimo", 1);
    assert!(Catalog::from_toml(&typo).is_err());
    let empty_reason = REAL.replacen(
        r#"{ pending = "Contrato en EUR (#67): probablemente ESTR como el DE40, pero ni la API ni ig.com/es lo confirman para este índice. Confirmar con IG." }"#,
        r#"{ pending = "" }"#,
        1,
    );
    assert_ne!(empty_reason, REAL);
    assert!(Catalog::from_toml(&empty_reason).is_err());
}

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
    assert_eq!(ids, ["DE40", "US100", "US2000", "US30", "US500", "XAUUSD"]);

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
        // Sin margen confirmado no hay tope regulatorio que comparar (US2000, #371).
        let Some(margin) = spec.terms.margin_retail_pct.value() else {
            continue;
        };
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
    // Falta el tipo de referencia de la financiación de los índices de EE. UU., los términos
    // de IG del Russell 2000, que solo está para el estudio (#371), y los del oro (#418).
    assert_eq!(
        fields,
        [
            ("US100", "funding_benchmark"),
            ("US2000", "funding_benchmark"),
            ("US2000", "margin_retail_pct"),
            ("US2000", "spread_min_points"),
            ("US2000", "spread_typical_points"),
            ("US2000", "value_per_point"),
            ("US2000", "min_deal_size"),
            ("US2000", "tick"),
            ("US2000", "funding_admin_fee_pct"),
            ("US30", "funding_benchmark"),
            ("US500", "funding_benchmark"),
            // El oro (ADR 0013, #418): lo que IG no publica y la base de su financiación.
            ("XAUUSD", "funding_benchmark"),
            ("XAUUSD", "spread_typical_points"),
            ("XAUUSD", "value_per_point"),
            ("XAUUSD", "min_deal_size"),
            ("XAUUSD", "tick"),
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
    // El oro (ADR 0013): dos tramos de la sesión de casi 23 h que cruzan ese día UTC.
    for (id, expected) in [("US100", 26), ("DE40", 34), ("XAUUSD", 92)] {
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

#[test]
fn account_and_funding_benchmarks_come_with_their_source() {
    let c = catalog();
    assert_eq!(c.account_currency.value(), Some(&Currency::EUR));
    assert_eq!(
        c.funding_cutoff.value().map(|t| t.to_string()),
        Some("23:00:00".into())
    );
    assert_eq!(c.funding_cutoff_timezone, chrono_tz::Europe::Madrid);
    assert_eq!(
        c.funding_benchmarks["ESTR"],
        Sourced::Known {
            value: dec!(2.441),
            source: "ecb-estr".into()
        }
    );
}

#[test]
fn an_instrument_cannot_use_a_benchmark_without_a_rate_entry() {
    let bad = REAL.replacen(
        r#"funding_benchmark = { value = "ESTR", source = "ig-es-costes" }"#,
        r#"funding_benchmark = { value = "SOFR", source = "ig-es-costes" }"#,
        1,
    );
    assert_ne!(bad, REAL);
    let err = Catalog::from_toml(&bad).unwrap_err().to_string();
    assert!(
        err.contains("SOFR") && err.contains("funding_benchmarks"),
        "{err}"
    );
    // Un tipo de referencia puede ser negativo (el €STR lo fue hasta 2022).
    let negative = REAL.replacen(
        r#"ESTR = { value = "2.441""#,
        r#"ESTR = { value = "-0.5""#,
        1,
    );
    assert_eq!(
        Catalog::from_toml(&negative).unwrap().funding_benchmarks["ESTR"].value(),
        Some(&dec!(-0.5))
    );
}

#[test]
fn the_commodity_funding_model_is_only_for_commodities() {
    use cfds_market_data::FundingModel;
    let us100 = "US100".parse().unwrap();
    // Por omisión, índices y acciones usan el tipo de referencia.
    assert_eq!(
        catalog().instruments[&us100].funding_model,
        FundingModel::Benchmark
    );
    let commodity = REAL.replacen(
        r#"asset_class = "index""#,
        "asset_class = \"commodity\"\nfunding_model = \"commodity_basis\"",
        1,
    );
    assert_eq!(
        Catalog::from_toml(&commodity).unwrap().instruments[&us100].funding_model,
        FundingModel::CommodityBasis
    );
    // Una materia prima sin su fórmula, o un índice con la de materias primas: error.
    let without = REAL.replacen(
        r#"asset_class = "index""#,
        r#"asset_class = "commodity""#,
        1,
    );
    let index_with = REAL.replacen(
        r#"asset_class = "index""#,
        "asset_class = \"index\"\nfunding_model = \"commodity_basis\"",
        1,
    );
    let unknown = REAL.replacen(
        r#"asset_class = "index""#,
        "asset_class = \"index\"\nfunding_model = \"swap\"",
        1,
    );
    for bad in [without, index_with, unknown] {
        let err = Catalog::from_toml(&bad).unwrap_err().to_string();
        assert!(err.contains("funding_model"), "{err}");
    }
}

#[test]
fn gold_is_in_the_catalog_but_cannot_be_simulated_yet() {
    use cfds_market_data::FundingModel;
    let c = catalog();
    let gold = &c.instruments[&"XAUUSD".parse().unwrap()];
    assert_eq!(gold.instrument.asset_class, AssetClass::Commodity);
    assert_eq!(gold.instrument.currency, Currency::USD);
    assert_eq!(gold.exchange, "CMEGLOBEX_METALS");
    assert_eq!(gold.funding_model, FundingModel::CommodityBasis);
    // Tope propio 10:1, la mitad del regulatorio (margen 5 % → 20:1).
    assert_eq!(gold.max_leverage, dec!(10));
    assert_eq!(gold.terms.margin_retail_pct.value(), Some(&dec!(5)));
    assert_eq!(gold.terms.spread_min_points.value(), Some(&dec!(0.9)));
    // Lo que IG no publica queda pendiente, nunca inventado.
    let pending: Vec<_> = gold.pending().into_iter().map(|p| p.field).collect();
    assert_eq!(
        pending,
        [
            "epic",
            "funding_benchmark",
            "spread_typical_points",
            "value_per_point",
            "min_deal_size",
            "tick"
        ]
    );
    assert!(!gold.missing_costs().is_empty());
}

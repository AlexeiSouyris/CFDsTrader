//! `config/futures_contracts.toml` (#672): el archivo del repo es válido y tiene los datos
//! verificados; los errores típicos se rechazan.

use cfds_market_data::{AssetClass, FuturesContracts};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromStr;

const REPO: &str = include_str!("../../../../config/futures_contracts.toml");

fn d(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

#[test]
fn repo_file_is_valid_and_has_the_15_markets() {
    let c = FuturesContracts::from_toml(REPO).unwrap();
    let ids: Vec<&str> = c.contracts.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "ES", "NQ", "YM", "RTY", "NKD", "ZN", "ZB", "6E", "6J", "6B", "6A", "GC", "SI", "HG",
            "CL"
        ]
    );
    let es = c.get("ES").unwrap();
    assert_eq!((es.multiplier, es.tick_size), (d("50"), d("0.25")));
    assert_eq!(es.tick_value, d("12.5"));
    assert_eq!(c.get("ZN").unwrap().tick_value, d("15.625"));
    assert_eq!(c.get("6A").unwrap().tick_size, d("0.00005"));
    assert!(c.contracts.iter().all(|k| k.currency == "USD"));
    for class in [
        AssetClass::Equity,
        AssetClass::Bond,
        AssetClass::Fx,
        AssetClass::Commodity,
    ] {
        assert!(c.contracts.iter().any(|k| k.asset_class == class));
    }
    assert_eq!(c.commission_ibkr_us.fixed_usd, d("0.85"));
    assert!(!c.commission_ibkr_us.exchange_fees_included);
}

fn replace(from: &str, to: &str) -> String {
    assert!(REPO.contains(from), "{from}");
    REPO.replacen(from, to, 1)
}

#[test]
fn rejects_wrong_tick_value() {
    let bad = replace("tick_value = \"12.50\"", "tick_value = \"12.00\"");
    assert!(
        FuturesContracts::from_toml(&bad)
            .unwrap_err()
            .contains("ES")
    );
}

#[test]
fn rejects_duplicate_id_bad_months_bad_hours_and_unknown_fields() {
    for (from, to) in [
        ("id = \"NQ\"", "id = \"ES\""),
        ("listed_months = \"HMUZ\"", "listed_months = \"HMUA\""),
        ("listed_months = \"HMUZ\"", "listed_months = \"HHMZ\""),
        ("session_open = \"17:00\"", "session_open = \"25:00\""),
        ("timezone = \"US/Central\"", "timezone = \"Mars/Base\""),
        ("currency = \"USD\"", "currency = \"usd\""),
        ("multiplier = \"50\"", "multiplier = \"0\""),
        (
            "next_last_trade = \"2026-12-18 08:30\"",
            "next_last_trade = \"mañana\"",
        ),
        ("asset_class = \"equity\"", "asset_class = \"crypto\""),
        (
            "trading_class = \"ES\"",
            "trading_class = \"ES\"\nextra = 1",
        ),
        ("checked_on = \"2026-11-25\"", "checked_on = \"ayer\""),
    ] {
        assert!(
            FuturesContracts::from_toml(&replace(from, to)).is_err(),
            "{to}"
        );
    }
}

#[test]
fn rejects_bad_commission_tiers() {
    for (from, to) in [
        ("up_to_contracts = 10000", "up_to_contracts = 500"),
        (
            "# Sin tope: más de 20.000 contratos.\nusd",
            "up_to_contracts = 30000\nusd",
        ),
        ("fixed_usd = \"0.85\"", "fixed_usd = \"0\""),
    ] {
        assert!(
            FuturesContracts::from_toml(&replace(from, to)).is_err(),
            "{to}"
        );
    }
}

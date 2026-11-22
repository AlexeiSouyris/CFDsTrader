//! Historial del fondo (`fund_movement.v1`, #179): filas, contrato y reconstrucción.

use std::path::PathBuf;

use cfds_fund::{Fund, FundConfig, FundError, MoveKind, Record, Request};
use chrono::{DateTime, Utc};
use rust_decimal::dec;

const REAL: &str = include_str!("../../../../config/fund.toml");

fn cfg() -> FundConfig {
    FundConfig::from_toml(REAL).unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn put(member: &str, amount: rust_decimal::Decimal) -> Request {
    Request::Contribute {
        member: member.into(),
        amount,
    }
}

/// Dos lunes: aportes de Ana y Beto, y después Caro aporta y Beto retira algo.
fn two_mondays() -> (Fund, Vec<Record>) {
    let c = cfg();
    let (f1, s1) = Fund::default()
        .settle(dec!(0), &[put("ana", dec!(100)), put("beto", dec!(50))], &c)
        .unwrap();
    let (f2, s2) = f1
        .settle(
            dec!(165),
            &[
                put("caro", dec!(110)),
                Request::Withdraw {
                    member: "beto".into(),
                    amount: dec!(11),
                },
            ],
            &c,
        )
        .unwrap();
    let mut records = s1.records(at("2026-11-23T07:00:00Z"), &c);
    records.extend(s2.records(at("2026-11-30T07:00:00Z"), &c));
    (f2, records)
}

#[test]
fn records_have_idempotent_ids_and_the_settlement_data() {
    let (_, records) = two_mondays();
    let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "fund:2026-11-23:1",
            "fund:2026-11-23:2",
            "fund:2026-11-30:1",
            "fund:2026-11-30:2"
        ]
    );
    let beto_out = &records[3];
    assert_eq!(beto_out.kind, MoveKind::Withdrawal);
    assert_eq!(
        (beto_out.amount, beto_out.units, beto_out.nav),
        (dec!(11), dec!(10), dec!(1.1))
    );
    assert!(records.iter().all(|r| r.currency == "EUR"));
}

#[test]
fn replaying_the_history_gives_the_same_accounts() {
    let (fund, records) = two_mondays();
    assert_eq!(Fund::replay(&records, &cfg()).unwrap(), fund);
}

#[test]
fn records_validate_against_the_contract() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/fund_movement.v1.schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let (_, records) = two_mondays();
    for r in &records {
        let json = serde_json::to_value(r).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&json)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{}: {errors:?}", r.id);
        let back: Record = serde_json::from_value(json).unwrap();
        assert_eq!(&back, r);
    }
    // Un campo de más no pasa ni el contrato ni Rust.
    let mut extra = serde_json::to_value(&records[0]).unwrap();
    extra["note"] = "x".into();
    assert!(!validator.is_valid(&extra));
    assert!(serde_json::from_value::<Record>(extra).is_err());
}

#[test]
fn an_invalid_history_is_rejected() {
    let (_, records) = two_mondays();
    // Retiro sin aporte previo: Beto vendería unidades que todavía no tiene.
    let mut wrong_order = records.clone();
    wrong_order.swap(1, 3);
    assert!(matches!(
        Fund::replay(&wrong_order, &cfg()),
        Err(FundError::BadHistory { .. })
    ));
    let mut other_currency = records.clone();
    other_currency[0].currency = "USD".into();
    assert!(matches!(
        Fund::replay(&other_currency, &cfg()),
        Err(FundError::BadHistory { .. })
    ));
    let mut zero_units = records;
    zero_units[0].units = dec!(0);
    assert!(Fund::replay(&zero_units, &cfg()).is_err());
}

#[test]
fn a_contribution_too_small_for_one_unit_fraction_is_an_error() {
    let c = cfg();
    let (fund, _) = Fund::default()
        .settle(dec!(0), &[put("a", dec!(10))], &c)
        .unwrap();
    // El fondo vale una fortuna: 10 € no compran ni 0,000001 unidades.
    let huge = dec!(100_000_000_000);
    assert!(matches!(
        fund.settle(huge, &[put("b", dec!(10))], &c),
        Err(FundError::ZeroUnits { .. })
    ));
}

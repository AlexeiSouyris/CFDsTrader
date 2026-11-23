//! Resumen del fondo (`fund_summary.v1`, #312): valuación al cerrar la última liquidación.

use cfds_fund::{Fund, FundConfig, FundError, Request, summary};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};

const REAL: &str = include_str!("../../../../config/fund.toml");

fn cfg() -> FundConfig {
    FundConfig::from_toml(REAL).unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn put(member: &str, amount: Decimal) -> Request {
    Request::Contribute {
        member: member.into(),
        amount,
    }
}

#[test]
fn values_the_fund_at_the_close_of_the_last_settlement() {
    let c = cfg();
    let (f1, s1) = Fund::default()
        .settle(dec!(0), &[put("ana", dec!(100)), put("beto", dec!(50))], &c)
        .unwrap();
    let (_, s2) = f1
        .settle(
            dec!(165),
            &[
                put("caro", dec!(110)),
                Request::WithdrawAll {
                    member: "beto".into(),
                },
            ],
            &c,
        )
        .unwrap();
    let mut records = s1.records(at("2026-11-16T07:00:00Z"), &c);
    records.extend(s2.records(at("2026-11-23T07:00:00Z"), &c));
    let s = summary(&records, &c).unwrap();
    // 165 + 110 − 55 (Beto vende todo a 1,10): lo mismo que dio la liquidación.
    assert_eq!(s.total_value, s2.value_after);
    assert_eq!(s.total_value, dec!(220));
    assert_eq!(s.nav, dec!(1.1));
    assert_eq!(s.valued_at, Some(at("2026-11-23T07:00:00Z")));
    let beto = s.members.iter().find(|m| m.member_id == "beto").unwrap();
    assert_eq!(beto.units, dec!(0));
    assert_eq!(beto.value, dec!(0));
    assert_eq!(beto.net_contributions, dec!(-5));
    assert_eq!(beto.pnl, dec!(5));
    let sum: Decimal = s.members.iter().map(|m| m.value).sum();
    assert_eq!(sum, s.total_value);
}

#[test]
fn one_settlement_with_two_navs_is_a_broken_history() {
    let c = cfg();
    let (_, s1) = Fund::default()
        .settle(dec!(0), &[put("ana", dec!(100)), put("beto", dec!(50))], &c)
        .unwrap();
    let mut records = s1.records(at("2026-11-16T07:00:00Z"), &c);
    records[1].nav = dec!(2);
    records[1].units = dec!(25);
    assert!(matches!(
        summary(&records, &c),
        Err(FundError::BadHistory { .. })
    ));
}

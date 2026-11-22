//! Fondo del grupo por unidades (spec §14, #177).

use cfds_fund::{Fund, FundConfig, FundError, MoveKind, Request};
use rust_decimal::{Decimal, RoundingStrategy, dec};

const REAL: &str = include_str!("../../../../config/fund.toml");

fn cfg() -> FundConfig {
    FundConfig::from_toml(REAL).unwrap()
}

fn put(member: &str, amount: Decimal) -> Request {
    Request::Contribute {
        member: member.into(),
        amount,
    }
}

fn take(member: &str, amount: Decimal) -> Request {
    Request::Withdraw {
        member: member.into(),
        amount,
    }
}

fn value_of(fund: &Fund, value: Decimal, member: &str) -> (Decimal, Decimal) {
    let line = fund
        .statement(value)
        .unwrap()
        .into_iter()
        .find(|l| l.member == member)
        .unwrap();
    (line.value, line.pnl)
}

#[test]
fn the_shipped_config_is_valid() {
    let c = cfg();
    assert_eq!(c.currency, "EUR");
    assert_eq!(c.min_contribution, dec!(10));
    assert_eq!(c.initial_nav, dec!(1));
    assert_eq!(c.unit_decimals, 6);
}

/// El ejemplo del glosario (NAV), exacto: Ana +10 €, Beto +5 €, Caro 0 €.
#[test]
fn glossary_example_ana_beto_caro() {
    let c = cfg();
    // Semana 1: Ana aporta 100 € y Beto 50 €. La unidad vale 1 €.
    let (fund, s1) = Fund::default()
        .settle(dec!(0), &[put("ana", dec!(100)), put("beto", dec!(50))], &c)
        .unwrap();
    assert_eq!(s1.nav, dec!(1));
    assert_eq!(s1.value_after, dec!(150));
    assert_eq!(fund.accounts()["ana"].units, dec!(100));
    assert_eq!(fund.accounts()["beto"].units, dec!(50));
    // El fondo gana 10 % y pasa a 165 €: la unidad vale 1,10 €.
    assert_eq!(fund.nav(dec!(165), &c).unwrap(), dec!(1.1));
    // Semana 2: Caro aporta 110 € y recibe 100 unidades.
    let (fund, s2) = fund
        .settle(dec!(165), &[put("caro", dec!(110))], &c)
        .unwrap();
    assert_eq!(s2.movements[0].units, dec!(100));
    assert_eq!(s2.value_after, dec!(275));
    assert_eq!(value_of(&fund, dec!(275), "ana"), (dec!(110), dec!(10)));
    assert_eq!(value_of(&fund, dec!(275), "beto"), (dec!(55), dec!(5)));
    assert_eq!(value_of(&fund, dec!(275), "caro"), (dec!(110), dec!(0)));
}

#[test]
fn members_always_add_up_to_the_fund_value_to_the_cent() {
    let c = cfg();
    let (fund, _) = Fund::default()
        .settle(
            dec!(0),
            &[
                put("a", dec!(100)),
                put("b", dec!(100)),
                put("c", dec!(100)),
            ],
            &c,
        )
        .unwrap();
    // 100 € / 3 miembros: no divide exacto.
    for value in [
        dec!(100),
        dec!(299.99),
        dec!(300.01),
        dec!(1000.005),
        dec!(0.01),
    ] {
        let lines = fund.statement(value).unwrap();
        let sum: Decimal = lines.iter().map(|l| l.value).sum();
        let expected = value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        assert_eq!(sum, expected, "valor {value}");
        assert!(lines.iter().all(|l| l.value.scale() <= 2));
    }
    // El céntimo que sobra va al primero por id cuando los restos son iguales.
    let lines = fund.statement(dec!(100)).unwrap();
    let values: Vec<Decimal> = lines.iter().map(|l| l.value).collect();
    assert_eq!(values, vec![dec!(33.34), dec!(33.33), dec!(33.33)]);
}

#[test]
fn contribution_units_round_down_and_withdrawal_units_round_up() {
    let c = cfg();
    let (fund, _) = Fund::default()
        .settle(dec!(0), &[put("a", dec!(300))], &c)
        .unwrap();
    // El fondo vale 310: NAV = 1,0333… (no exacto).
    let (fund, s) = fund
        .settle(dec!(310), &[put("b", dec!(100)), take("a", dec!(50))], &c)
        .unwrap();
    let nav = dec!(310) / dec!(300);
    assert_eq!(s.nav, nav);
    // 100 / 1,0333… = 96,774193… → 96,774193 (hacia abajo).
    assert_eq!(s.movements[0].units, dec!(96.774193));
    // 50 / 1,0333… = 48,387096… → 48,387097 (hacia arriba).
    assert_eq!(s.movements[1].units, dec!(48.387097));
    assert_eq!(s.movements[1].kind, MoveKind::Withdrawal);
    assert_eq!(fund.accounts()["a"].units, dec!(300) - dec!(48.387097));
    assert_eq!(s.value_after, dec!(360));
    let sum: Decimal = fund
        .statement(dec!(360))
        .unwrap()
        .iter()
        .map(|l| l.value)
        .sum();
    assert_eq!(sum, dec!(360));
}

#[test]
fn withdraw_all_sells_every_unit() {
    let c = cfg();
    let (fund, _) = Fund::default()
        .settle(dec!(0), &[put("a", dec!(100)), put("b", dec!(100))], &c)
        .unwrap();
    let (fund, s) = fund
        .settle(
            dec!(220),
            &[Request::WithdrawAll { member: "a".into() }],
            &c,
        )
        .unwrap();
    assert_eq!(s.movements[0].amount, dec!(110));
    assert_eq!(fund.accounts()["a"].units, dec!(0));
    assert_eq!(s.value_after, dec!(110));
    let (value, pnl) = value_of(&fund, dec!(110), "a");
    assert_eq!((value, pnl), (dec!(0), dec!(10)));
    assert_eq!(value_of(&fund, dec!(110), "b"), (dec!(110), dec!(10)));
}

#[test]
fn invalid_requests_settle_nothing() {
    let c = cfg();
    let (fund, _) = Fund::default()
        .settle(dec!(0), &[put("a", dec!(100))], &c)
        .unwrap();
    let cases = [
        (vec![put("b", dec!(9.99))], "mínimo"),
        (vec![put("b", dec!(-20))], "negativo"),
        (vec![put("b", dec!(10.001))], "fracción de céntimo"),
        (vec![take("a", dec!(100.01))], "más de lo que vale"),
        (vec![take("z", dec!(10))], "miembro sin unidades"),
        (
            vec![put("b", dec!(50)), take("a", dec!(500))],
            "uno malo anula todos",
        ),
    ];
    for (reqs, why) in cases {
        assert!(fund.settle(dec!(100), &reqs, &c).is_err(), "{why}");
    }
    assert!(matches!(
        fund.settle(dec!(100), &[put("b", dec!(5))], &c),
        Err(FundError::BelowMinimum { .. })
    ));
    assert!(matches!(
        fund.settle(dec!(100), &[take("a", dec!(101))], &c),
        Err(FundError::OverWithdrawal { .. })
    ));
}

#[test]
fn empty_or_broken_fund_values_are_errors() {
    let c = cfg();
    assert_eq!(Fund::default().nav(dec!(0), &c), Ok(dec!(1)));
    assert_eq!(
        Fund::default().nav(dec!(5), &c),
        Err(FundError::ValueWithoutUnits(dec!(5)))
    );
    let (fund, _) = Fund::default()
        .settle(dec!(0), &[put("a", dec!(100))], &c)
        .unwrap();
    assert_eq!(
        fund.nav(dec!(-1), &c),
        Err(FundError::NegativeValue(dec!(-1)))
    );
    assert_eq!(
        fund.settle(dec!(0), &[put("b", dec!(10))], &c).unwrap_err(),
        FundError::ZeroNav
    );
    // Un fondo que perdió todo: todos valen 0.
    assert!(
        fund.statement(dec!(0))
            .unwrap()
            .iter()
            .all(|l| l.value.is_zero())
    );
}

#[test]
fn invalid_config_is_rejected() {
    let base =
        "currency = \"EUR\"\nmin_contribution = \"10\"\ninitial_nav = \"1\"\nunit_decimals = 6\n";
    assert!(FundConfig::from_toml(base).is_ok());
    for (from, to) in [
        ("\"EUR\"", "\"euro\""),
        ("min_contribution = \"10\"", "min_contribution = \"0\""),
        ("initial_nav = \"1\"", "initial_nav = \"x\""),
        ("unit_decimals = 6", "unit_decimals = 1"),
        ("unit_decimals = 6", "unit_decimals = 6\nextra = 1"),
    ] {
        assert!(
            matches!(
                FundConfig::from_toml(&base.replace(from, to)),
                Err(FundError::Config(_))
            ),
            "debería fallar: {to}"
        );
    }
}

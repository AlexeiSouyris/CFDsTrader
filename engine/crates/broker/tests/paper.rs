//! Broker simulado (fase 7, #278): ejecución, idempotencia y cada límite de riesgo que se
//! dispara. Precios sintéticos.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, LiveBroker, MarketSetup, PaperBroker, live_trading_status,
};
use cfds_core::{
    Candle, InstrumentId, KillSwitch, Mode, Order, OrderRejection, OrderSide, OrderStatus,
    PaperMode, Timeframe,
};
use cfds_risk::{InstrumentLimits, RiskConfig};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::Value;

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("../../../../config/broker.toml");

fn t0() -> DateTime<Utc> {
    "2026-11-18T09:00:00Z".parse().unwrap()
}

fn at(i: i64) -> DateTime<Utc> {
    t0() + TimeDelta::minutes(15 * i)
}

fn bar(id: &str, i: i64, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: id.parse().unwrap(),
        timeframe: Timeframe::M15,
        start: at(i),
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

/// 30 velas planas en 100 (rango 2 → ATR 2).
fn flat(id: &str) -> Vec<Candle> {
    (0..30)
        .map(|i| bar(id, i, dec!(100), dec!(101), dec!(99), dec!(100)))
        .collect()
}

fn limits() -> InstrumentLimits {
    InstrumentLimits {
        max_leverage: dec!(10),
        margin_pct: dec!(5),
        min_deal_size: Some(dec!(0.01)),
    }
}

fn costs() -> InstrumentCosts {
    InstrumentCosts {
        spread: dec!(1),
        tick: dec!(0.1),
        commission: Commission::None,
        funding_admin_fee_pct: dec!(2.5),
        funding_day_count: dec!(365),
    }
}

fn config(edit: impl FnOnce(&mut RiskConfig)) -> BrokerConfig {
    let mut risk = RiskConfig::from_toml(RISK).unwrap();
    edit(&mut risk);
    let exits = ExitConfig::from_toml(BACKTEST).unwrap();
    let costs = CostConfig {
        slippage_spread_fraction: dec!(0.1),
        slippage_ticks: dec!(1),
    };
    BrokerConfig::from_toml(BROKER, risk, costs, exits, chrono_tz::Europe::Madrid).unwrap()
}

fn market(candles: Vec<Candle>, limits: InstrumentLimits) -> MarketSetup {
    MarketSetup {
        limits,
        costs: costs(),
        account_to_instrument: Decimal::ONE,
        candles,
        funding: funding(FundingRates::Constant(dec!(2))),
    }
}

/// Corte de las 23:00 de Madrid, como el del catálogo.
fn funding(rates: FundingRates) -> FundingSetup {
    FundingSetup {
        cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
        tz: chrono_tz::Europe::Madrid,
        rates,
    }
}

fn broker_with(
    edit: impl FnOnce(&mut RiskConfig),
    markets: Vec<(&str, Vec<Candle>, InstrumentLimits)>,
) -> PaperBroker {
    let m: BTreeMap<InstrumentId, MarketSetup> = markets
        .into_iter()
        .map(|(id, c, l)| (id.parse().unwrap(), market(c, l)))
        .collect();
    PaperBroker::new(config(edit), m)
}

fn broker() -> PaperBroker {
    broker_with(|_| {}, vec![("US100", flat("US100"), limits())])
}

fn order(id: &str, instrument: &str, side: OrderSide, stop: Decimal, i: i64) -> Order {
    Order {
        order_id: id.parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: instrument.parse().unwrap(),
        side,
        stop,
        tp1: None,
        proposal_id: None,
        created_at: at(i),
    }
}

fn rejected(b: &mut PaperBroker, o: &Order) -> OrderRejection {
    let r = b.submit(o);
    assert_eq!(r.status, OrderStatus::Rejected, "{r:?}");
    assert!(r.position_id.is_none());
    r.rejection.unwrap()
}

#[test]
fn fills_at_the_next_open_with_spread_slippage_and_the_risk_size() {
    let mut b = broker();
    let o = order("o1", "US100", OrderSide::Long, dec!(98), 20);
    let r = b.submit(&o);
    assert_eq!(r.status, OrderStatus::Filled);
    // apertura 100 + medio spread 0,5 + slippage (0,1 × 1 + 1 tick de 0,1) = 100,7
    assert_eq!(r.fill_price, Some(dec!(100.7)));
    assert_eq!(r.at, at(20));
    // 1 % de 1.000 € = 10 €; R = 100 − 98 = 2 → 5 € por punto (lo calcula el riesgo)
    assert_eq!(r.size, Some(dec!(5)));
    assert_eq!(r.commission, Some(dec!(0)));
    let p = b.positions(at(25));
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].entry, dec!(100.7));
    // valuada al último cierre (100): (100 − 100,7) × 5 = −3,5
    assert_eq!(p[0].unrealized_pl, dec!(-3.50));
}

#[test]
fn the_same_order_id_never_opens_twice() {
    let mut b = broker();
    let o = order("o1", "US100", OrderSide::Long, dec!(98), 20);
    let first = b.submit(&o);
    let again = b.submit(&o);
    // Aunque el reenvío llegue más tarde, devuelve el mismo informe.
    let later = b.submit(&Order {
        created_at: at(25),
        ..o.clone()
    });
    assert_eq!(first, again);
    assert_eq!(first, later);
    assert_eq!(b.positions(at(29)).len(), 1);
}

#[test]
fn a_gap_past_the_stop_closes_at_the_open_and_realizes_the_loss() {
    let mut c = flat("US100");
    c[22] = bar("US100", 22, dec!(97), dec!(97.5), dec!(96), dec!(97));
    let mut b = broker_with(|_| {}, vec![("US100", c, limits())]);
    b.submit(&order("o1", "US100", OrderSide::Long, dec!(98), 20));
    assert!(b.positions(at(29)).is_empty());
    let acc = b.account(at(29));
    // salida a la apertura 97 − 0,7 = 96,3; (96,3 − 100,7) × 5 = −22
    assert_eq!(acc.equity, dec!(978.00));
    assert_eq!(acc.realized_pl_today, dec!(-22.00));
}

#[test]
fn stop_on_the_wrong_side_no_price_and_unknown_instrument_are_rejected() {
    let mut b = broker();
    assert_eq!(
        rejected(&mut b, &order("a", "US100", OrderSide::Long, dec!(101), 20)),
        OrderRejection::StopWrongSide
    );
    assert_eq!(
        rejected(&mut b, &order("b", "US100", OrderSide::Short, dec!(99), 20)),
        OrderRejection::StopWrongSide
    );
    // antes de la primera vela cerrada
    assert_eq!(
        rejected(&mut b, &order("c", "US100", OrderSide::Long, dec!(98), 0)),
        OrderRejection::NoPrice
    );
    // después de la última vela: no hay apertura siguiente para ejecutar
    assert_eq!(
        rejected(&mut b, &order("d", "US100", OrderSide::Long, dec!(98), 40)),
        OrderRejection::NoPrice
    );
    assert_eq!(
        rejected(&mut b, &order("e", "DE40", OrderSide::Long, dec!(98), 20)),
        OrderRejection::UnknownInstrument
    );
    // sin ATR todavía (calentamiento)
    assert_eq!(
        rejected(&mut b, &order("f", "US100", OrderSide::Long, dec!(98), 5)),
        OrderRejection::InvalidInput
    );
}

#[test]
fn stop_too_close_and_too_far_in_atr() {
    let mut b = broker();
    assert_eq!(
        rejected(
            &mut b,
            &order("a", "US100", OrderSide::Long, dec!(99.5), 20)
        ),
        OrderRejection::StopTooClose
    );
    assert_eq!(
        rejected(&mut b, &order("b", "US100", OrderSide::Long, dec!(90), 20)),
        OrderRejection::StopTooFar
    );
}

#[test]
fn one_position_per_instrument_and_a_maximum_in_total() {
    let mut b = broker_with(
        |r| r.max_open_positions = 1,
        vec![
            ("US100", flat("US100"), limits()),
            ("DE40", flat("DE40"), limits()),
        ],
    );
    b.submit(&order("a", "US100", OrderSide::Long, dec!(98), 20));
    assert_eq!(
        rejected(&mut b, &order("b", "DE40", OrderSide::Long, dec!(98), 21)),
        OrderRejection::MaxOpenPositions
    );
    let mut b = broker();
    b.submit(&order("a", "US100", OrderSide::Long, dec!(98), 20));
    // Mientras espera su fill ya cuenta: no se abre otra en el mismo instrumento.
    assert_eq!(
        rejected(&mut b, &order("b", "US100", OrderSide::Long, dec!(98), 20)),
        OrderRejection::InstrumentAlreadyOpen
    );
}

#[test]
fn kill_switch_blocks_every_order_and_shows_in_the_account() {
    let mut b = broker();
    b.activate_kill_switch("Error de datos", at(10));
    assert_eq!(
        rejected(&mut b, &order("a", "US100", OrderSide::Long, dec!(98), 20)),
        OrderRejection::KillSwitchActive
    );
    assert!(matches!(
        b.account(at(20)).kill_switch,
        KillSwitch::Active { .. }
    ));
}

fn losing_then_new_order(edit: impl FnOnce(&mut RiskConfig)) -> (PaperBroker, OrderRejection) {
    let mut c = flat("US100");
    c[22] = bar("US100", 22, dec!(97), dec!(97.5), dec!(96), dec!(97));
    let mut b = broker_with(edit, vec![("US100", c, limits())]);
    b.submit(&order("loss", "US100", OrderSide::Long, dec!(96), 20));
    let r = rejected(
        &mut b,
        &order("next", "US100", OrderSide::Short, dec!(102), 26),
    );
    (b, r)
}

#[test]
fn daily_loss_stops_new_entries() {
    // pérdida del día −2,2 % con un máximo de 0,5 % (drawdown alto para que no salte antes)
    let (b, r) = losing_then_new_order(|r| {
        r.max_daily_loss_pct = dec!(0.5);
        r.max_drawdown_pct = dec!(50);
    });
    assert_eq!(r, OrderRejection::DailyLossLimit);
    assert!(!b.kill_switch_active());
}

#[test]
fn drawdown_rejects_and_activates_the_kill_switch() {
    let (mut b, r) = losing_then_new_order(|r| r.max_drawdown_pct = dec!(0.5));
    assert_eq!(r, OrderRejection::DrawdownLimit);
    assert!(b.kill_switch_active());
    assert_eq!(
        rejected(
            &mut b,
            &order("after", "US100", OrderSide::Long, dec!(94), 27)
        ),
        OrderRejection::KillSwitchActive
    );
}

#[test]
fn margin_and_broker_minimum() {
    let mut b = broker_with(
        |_| {},
        vec![(
            "US100",
            flat("US100"),
            InstrumentLimits {
                margin_pct: dec!(300),
                ..limits()
            },
        )],
    );
    assert_eq!(
        rejected(&mut b, &order("a", "US100", OrderSide::Long, dec!(98), 20)),
        OrderRejection::InsufficientMargin
    );
    let mut b = broker_with(
        |_| {},
        vec![(
            "US100",
            flat("US100"),
            InstrumentLimits {
                min_deal_size: Some(dec!(100)),
                ..limits()
            },
        )],
    );
    assert_eq!(
        rejected(&mut b, &order("b", "US100", OrderSide::Long, dec!(98), 20)),
        OrderRejection::BelowMinDealSize
    );
    let mut b = broker_with(
        |_| {},
        vec![(
            "US100",
            flat("US100"),
            InstrumentLimits {
                min_deal_size: None,
                ..limits()
            },
        )],
    );
    assert_eq!(
        rejected(&mut b, &order("c", "US100", OrderSide::Long, dec!(98), 20)),
        OrderRejection::MinDealSizeUnknown
    );
}

#[test]
fn the_preview_answers_like_submit_but_changes_nothing() {
    let b = broker();
    let o = order("p", "US100", OrderSide::Long, dec!(98), 20);
    let v = b.preview(&o, at(20));
    assert!(v.accepted);
    assert_eq!(v.size, Some(dec!(5)));
    assert_eq!(v.risk, Some(dec!(10.00)));
    assert_eq!(v.risk_pct, Some(dec!(1.00)));
    assert_eq!(v.reference, Some(dec!(100)));
    let mut b = b;
    assert!(
        b.positions(at(29)).is_empty(),
        "la vista previa no abre nada"
    );
    assert!(b.report(&o.order_id).is_none());
    let bad = b.preview(
        &order("q", "US100", OrderSide::Long, dec!(99.5), 20),
        at(20),
    );
    assert!(!bad.accepted);
    assert_eq!(bad.rejection, Some(OrderRejection::StopTooClose));
}

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn assert_valid(schema: &str, doc: Value) {
    let errors: Vec<String> = validator(schema)
        .iter_errors(&doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{schema}: {errors:?} en {doc}");
}

#[test]
fn every_message_follows_its_contract() {
    let mut b = broker();
    let ok = b.submit(&order("o1", "US100", OrderSide::Long, dec!(98), 20));
    let no = b.submit(&order("o2", "US100", OrderSide::Long, dec!(101), 20));
    assert_valid(
        "order_report.v1.schema.json",
        serde_json::to_value(ok).unwrap(),
    );
    assert_valid(
        "order_report.v1.schema.json",
        serde_json::to_value(no).unwrap(),
    );
    let v = b.preview(
        &order("o3", "US100", OrderSide::Short, dec!(102), 26),
        at(26),
    );
    assert_valid(
        "ticket_verdict.v1.schema.json",
        serde_json::to_value(v).unwrap(),
    );
    for p in b.positions(at(29)) {
        assert_valid("position.v1.schema.json", serde_json::to_value(p).unwrap());
    }
    assert_valid(
        "paper_account.v1.schema.json",
        serde_json::to_value(b.account(at(29))).unwrap(),
    );
    b.activate_kill_switch("Reconciliación", at(29));
    assert_valid(
        "paper_account.v1.schema.json",
        serde_json::to_value(b.account(at(29))).unwrap(),
    );
}

/// No existe forma de enviar una orden real: el único modo es paper, el broker real no tiene
/// valores y la autorización no se puede construir (lo prueba el doctest `compile_fail` de
/// `cfds_broker::live`).
#[test]
fn there_is_no_way_to_send_a_real_order() {
    match Mode::default() {
        Mode::Paper => {} // si apareciera otro modo, este match deja de compilar
    }
    assert_eq!(PaperMode::default(), PaperMode::Paper);
    assert!(live_trading_status().contains("dos personas"));
    // `LiveBroker` es un tipo sin valores: su tamaño en memoria es cero y nadie puede tener uno.
    assert_eq!(std::mem::size_of::<LiveBroker>(), 0);
    let json = serde_json::to_value(order("x", "US100", OrderSide::Long, dec!(98), 20)).unwrap();
    assert_eq!(json["mode"], "paper");
}

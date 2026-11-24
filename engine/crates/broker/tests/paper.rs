//! Broker simulado (fase 7, #278): ejecución, idempotencia y cada límite de riesgo que se
//! dispara. Precios sintéticos.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, BrokerEvent, FundingSetup, JournalStore, LiveBroker, MarketSetup,
    PaperBroker, flush_journal, live_trading_status,
};
use cfds_core::OrderReport;
use cfds_core::{
    Candle, InstrumentId, KillSwitch, Mode, Order, OrderRejection, OrderSide, OrderStatus,
    PaperMode, Timeframe,
};
use cfds_risk::{InstrumentLimits, RiskConfig};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::Value;
use std::sync::Mutex;

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
        account_to_instrument: cfds_backtest::AccountFx::Same,
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
    // después de la última vela: todavía no hay apertura siguiente para ejecutar, queda
    // pendiente hasta esa vela (#471), fuera del registro
    let d = b.submit(&order("d", "US100", OrderSide::Long, dec!(98), 40));
    assert_eq!(d.status, OrderStatus::Pending, "{d:?}");
    assert!(b.journal().iter().all(|(o, _)| o.order_id.as_str() != "d"));
    assert_eq!(
        rejected(&mut b, &order("e", "DE40", OrderSide::Long, dec!(98), 20)),
        OrderRejection::UnknownInstrument
    );
    // sin ATR todavía (calentamiento): faltan velas recientes, no es un dato inválido (#361)
    assert_eq!(
        rejected(&mut b, &order("f", "US100", OrderSide::Long, dec!(98), 5)),
        OrderRejection::NoRecentCandles
    );
    // La vista previa da el mismo motivo, con el precio de referencia de la última vela cerrada.
    let v = b.preview(&order("g", "US100", OrderSide::Long, dec!(98), 5), at(5));
    assert!(!v.accepted);
    assert_eq!(v.rejection, Some(OrderRejection::NoRecentCandles));
    assert!(v.reference.is_some());
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

#[test]
fn every_decision_report_and_kill_switch_leaves_an_event() {
    let mut b = broker();
    let ok = order("o1", "US100", OrderSide::Long, dec!(98), 20);
    let report = b.submit(&ok);
    b.submit(&ok);
    b.activate_kill_switch("Error de datos", at(21));
    b.activate_kill_switch("otra vez", at(22));
    let events = b.take_events();
    assert!(b.take_events().is_empty(), "take_events vacía la lista");
    let names: Vec<String> = events
        .iter()
        .map(|e| {
            serde_json::to_value(e).unwrap()["event"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        names,
        [
            "order_received",
            "risk_decision",
            "order_report",
            "order_duplicate",
            "kill_switch"
        ]
    );
    assert!(matches!(
        &events[1],
        BrokerEvent::RiskDecision { accepted: true, size: Some(s), .. } if *s == dec!(5)
    ));
    assert_eq!(events[2], BrokerEvent::OrderReport { report });
    // Una orden rechazada deja la decisión con el motivo.
    rejected(&mut b, &order("o2", "US100", OrderSide::Long, dec!(98), 23));
    let events = b.take_events();
    assert!(matches!(
        &events[1],
        BrokerEvent::RiskDecision {
            accepted: false,
            rejection: Some(OrderRejection::KillSwitchActive),
            ..
        }
    ));
}

/// Un registro en memoria; `fail` hace fallar los guardados de esos `order_id`.
#[derive(Default)]
struct MemJournal {
    saved: Mutex<Vec<(Order, OrderReport)>>,
    fail: Vec<String>,
}

impl JournalStore for MemJournal {
    async fn save(&self, order: &Order, report: &OrderReport) -> Result<(), String> {
        if self.fail.contains(&order.order_id.to_string()) {
            return Err("sin base".into());
        }
        let mut s = self.saved.lock().unwrap();
        if !s.iter().any(|(o, _)| o.order_id == order.order_id) {
            s.push((order.clone(), report.clone()));
        }
        Ok(())
    }
    async fn saved_ids(&self) -> Result<Vec<String>, String> {
        Ok(self
            .saved
            .lock()
            .unwrap()
            .iter()
            .map(|(o, _)| o.order_id.to_string())
            .collect())
    }
}

#[tokio::test]
async fn the_shutdown_flush_saves_what_was_missing_and_reports_orphans() {
    let mut b = broker();
    let o1 = order("o1", "US100", OrderSide::Long, dec!(98), 20);
    let o2 = order("o2", "US100", OrderSide::Long, dec!(98), 21);
    let r1 = b.submit(&o1);
    b.submit(&o2);
    // o1 llegó a guardarse; o2 no (el guardado falló en su momento).
    let store = MemJournal::default();
    store.saved.lock().unwrap().push((o1.clone(), r1));
    let flush = flush_journal(&b, &store).await.unwrap();
    assert!(flush.is_clean());
    assert_eq!(flush.saved_now, ["o2"]);
    assert_eq!(store.saved_ids().await.unwrap(), ["o1", "o2"]);
    // Otra vez: nada que guardar.
    let again = flush_journal(&b, &store).await.unwrap();
    assert!(again.is_clean() && again.saved_now.is_empty());
    // Si la base sigue sin aceptar una orden, queda huérfana y la parada no es limpia.
    let broken = MemJournal {
        fail: vec!["o2".into()],
        ..Default::default()
    };
    let flush = flush_journal(&b, &broken).await.unwrap();
    assert!(!flush.is_clean());
    assert_eq!(flush.saved_now, ["o1"]);
    assert_eq!(flush.orphans, ["o2"]);
}

/// #319, spec §12: si la apertura de la entrada ya saltó el stop, la operación sale entera a esa
/// apertura (como el backtest), no queda abierta.
#[test]
fn a_gap_through_the_stop_at_the_entry_exits_at_that_open() {
    let mut candles = flat("US100");
    // La vela 20 abre en 97, debajo del stop de 98 (el riesgo decidió con el cierre de 19 = 100).
    candles[20] = bar("US100", 20, dec!(97), dec!(98.5), dec!(96.5), dec!(98));
    let mut b = broker_with(|_| {}, vec![("US100", candles, limits())]);
    let r = b.submit(&order("g", "US100", OrderSide::Long, dec!(98), 20));
    assert_eq!(r.status, OrderStatus::Filled, "{r:?}");
    // 97 + medio spread 0,5 + slippage 0,2 = 97,7: ya debajo del stop.
    assert_eq!(r.fill_price, Some(dec!(97.7)));
    let size = r.size.unwrap();
    assert!(b.positions(at(25)).is_empty(), "no queda abierta");
    let out = b.trade_outcomes(at(25));
    assert_eq!(out.len(), 1);
    let e = &out[0].exits;
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].mid, dec!(97));
    assert_eq!(e[0].price, dec!(96.3), "97 − 0,7");
    assert_eq!(e[0].time, at(21), "fin de la vela de la entrada");
    assert!(!e[0].end_of_data);
    assert_eq!(out[0].gross_pnl, size * dec!(-1.4));
}

/// #319: TP1 se calcula desde el precio ejecutado, como en el backtest.
#[test]
fn tp1_is_measured_from_the_executed_entry() {
    let mut candles = flat("US100");
    // Fill en 100,7 con stop 98: R = 2,7 desde el ejecutado (2 desde el medio). Con TP1 a
    // `tp1_r` R, la vela 22 llega justo al TP1 medido desde el medio, pero no desde el ejecutado.
    let tp1_r = cfds_backtest::ExitConfig::from_toml(BACKTEST)
        .unwrap()
        .tp1_r;
    let from_mid = dec!(100) + tp1_r * dec!(2);
    let from_fill = dec!(100.7) + tp1_r * dec!(2.7);
    candles[22] = bar("US100", 22, dec!(100), from_mid, dec!(99.5), dec!(100));
    let mut b = broker_with(|_| {}, vec![("US100", candles, limits())]);
    let r = b.submit(&order("t", "US100", OrderSide::Long, dec!(98), 20));
    assert_eq!(r.fill_price, Some(dec!(100.7)));
    let out = b.trade_outcomes(at(23));
    assert!(
        out[0].exits.iter().all(|e| e.end_of_data),
        "no tocó el TP1 ({from_fill}): {:?}",
        out[0].exits
    );
}

// ---------- Contrato en USD con la cuenta en EUR (ADR 0013 §4, #418) ----------

/// Serie EUR/USD: (publicación, tipo).
fn eurusd(points: &[(DateTime<Utc>, Decimal)]) -> cfds_backtest::AccountFx {
    let rates = points
        .iter()
        .map(|&(at, rate)| cfds_core::FxRate {
            pair: "EURUSD".into(),
            ref_date: at.date_naive(),
            published_at: at,
            rate,
        })
        .collect();
    cfds_backtest::AccountFx::Daily(cfds_backtest::FxRates::new(rates).unwrap())
}

fn usd_broker(fx: cfds_backtest::AccountFx, candles: Vec<Candle>) -> PaperBroker {
    let mut m = market(candles, limits());
    m.account_to_instrument = fx;
    PaperBroker::new(
        config(|_| {}),
        [("XAUUSD".parse().unwrap(), m)].into_iter().collect(),
    )
}

#[test]
fn a_usd_contract_is_sized_and_valued_at_the_rate_in_force() {
    let mut b = usd_broker(
        eurusd(&[(at(0) - TimeDelta::days(1), dec!(1.25))]),
        flat("XAUUSD"),
    );
    let r = b.submit(&order("g1", "XAUUSD", OrderSide::Long, dec!(98), 20));
    assert_eq!(r.status, OrderStatus::Filled, "{r:?}");
    // 1 % de 1.000 € = 10 € = 12,5 $; R = 2 → 6,25 $ por punto.
    assert_eq!(r.size, Some(dec!(6.25)));
    let p = b.positions(at(25));
    // En dólares: (100 − 100,7) × 6,25 = −4,375.
    assert_eq!(p[0].unrealized_pl, dec!(-4.38));
    // En la cuenta, al tipo de ese momento: −4,375 / 1,25 = −3,5 €.
    assert_eq!(b.account(at(25)).equity, dec!(996.50));
}

#[test]
fn a_new_rate_changes_the_valuation_from_its_publication_on() {
    let mut b = usd_broker(
        eurusd(&[(at(0) - TimeDelta::days(1), dec!(1.25)), (at(24), dec!(2))]),
        flat("XAUUSD"),
    );
    b.submit(&order("g1", "XAUUSD", OrderSide::Long, dec!(98), 20));
    // Antes de la publicación vale 1,25; desde ahí, 2.
    assert_eq!(b.account(at(23)).equity, dec!(996.50));
    assert_eq!(b.account(at(25)).equity, dec!(997.81));
    // En dólares no cambia: lo que cambia es la conversión.
    assert_eq!(b.positions(at(25))[0].unrealized_pl, dec!(-4.38));
}

#[test]
fn without_a_rate_at_the_signal_the_order_is_rejected() {
    // Primer tipo publicado después de la señal: no se usa antes (sin look-ahead).
    let mut b = usd_broker(eurusd(&[(at(25), dec!(1.25))]), flat("XAUUSD"));
    let o = order("g1", "XAUUSD", OrderSide::Long, dec!(98), 20);
    assert_eq!(
        b.preview(&o, at(20)).rejection,
        Some(OrderRejection::MissingFxRate)
    );
    assert_eq!(rejected(&mut b, &o), OrderRejection::MissingFxRate);
    // Sin serie: nunca hay tipo.
    let mut b = usd_broker(
        cfds_backtest::AccountFx::Missing {
            pair: "EURUSD".into(),
        },
        flat("XAUUSD"),
    );
    assert_eq!(rejected(&mut b, &o), OrderRejection::MissingFxRate);
    assert_eq!(b.account(at(25)).equity, dec!(1000.00));
}

#[test]
fn a_realized_loss_in_usd_uses_the_rate_of_its_exit() {
    let mut c = flat("XAUUSD");
    c[22] = bar("XAUUSD", 22, dec!(97), dec!(97.5), dec!(96), dec!(97));
    let mut b = usd_broker(
        eurusd(&[(at(0) - TimeDelta::days(1), dec!(1.25)), (at(22), dec!(2))]),
        c,
    );
    b.submit(&order("g1", "XAUUSD", OrderSide::Long, dec!(98), 20));
    let acc = b.account(at(29));
    // Salida a 96,3: (96,3 − 100,7) × 6,25 = −27,5 $, al tipo de la vela de salida (2): −13,75 €.
    assert_eq!(acc.equity, dec!(986.25));
    assert_eq!(acc.kill_switch, KillSwitch::Inactive);
}

#[test]
fn a_rate_published_after_the_start_is_used_once_read_again() {
    // #439: al arrancar solo se conocía 1,25; a at(24) se publica 2.
    let start = [(at(0) - TimeDelta::days(1), dec!(1.25))];
    let later = [(at(0) - TimeDelta::days(1), dec!(1.25)), (at(24), dec!(2))];
    let mut b = usd_broker(eurusd(&start), flat("XAUUSD"));
    b.submit(&order("g1", "XAUUSD", OrderSide::Long, dec!(98), 20));
    let before = b.account(at(23));
    b.update_account_fx(&"XAUUSD".parse().unwrap(), eurusd(&later))
        .unwrap();
    // Lo valuado antes de la publicación no cambia; después, el tipo nuevo, como la
    // reconstrucción con la serie completa.
    assert_eq!(b.account(at(23)), before);
    let mut rebuilt = usd_broker(eurusd(&later), flat("XAUUSD"));
    rebuilt.submit(&order("g1", "XAUUSD", OrderSide::Long, dec!(98), 20));
    assert_eq!(b.account(at(25)), rebuilt.account(at(25)));
    assert_eq!(b.account(at(25)).equity, dec!(997.81));
    assert!(
        b.update_account_fx(&"DE40".parse().unwrap(), eurusd(&later))
            .is_err()
    );
}

/// US100 sube después de la entrada y vuelve: el capital pasa por un máximo entre dos órdenes.
fn up_and_back() -> PaperBroker {
    let mut c = flat("US100");
    c[22] = bar("US100", 22, dec!(100), dec!(103), dec!(100), dec!(103));
    c[23] = bar("US100", 23, dec!(103), dec!(103), dec!(100), dec!(100));
    broker_with(
        |r| r.max_drawdown_pct = dec!(0.5),
        vec![("US100", c, limits()), ("DE40", flat("DE40"), limits())],
    )
}

#[test]
fn reading_the_account_does_not_change_the_peak_nor_later_decisions() {
    // #464: `account()` es una lectura (la UI la llama cuando quiere). No mueve el máximo del
    // capital: si lo moviera, una lectura en el pico cambiaría la decisión siguiente y la
    // reconstrucción (que no repite lecturas) daría otro informe.
    let first = order("up", "US100", OrderSide::Long, dec!(96), 20);
    let second = order("next", "DE40", OrderSide::Long, dec!(96), 26);

    let mut quiet = up_and_back();
    quiet.submit(&first);
    let quiet_report = quiet.submit(&second);

    let mut read = up_and_back();
    read.submit(&first);
    let peak = read.account(at(23));
    assert!(peak.equity > dec!(1005), "{peak:?}");
    let read_report = read.submit(&second);

    assert_eq!(read_report, quiet_report);
    assert_eq!(read.kill_switch_active(), quiet.kill_switch_active());
    assert_eq!(read.journal(), quiet.journal());

    let positions = read.positions(at(28));
    let (_, rec) = cfds_broker::restore_and_reconcile(
        read.config().clone(),
        up_and_back().markets().clone(),
        read.journal(),
        &positions,
        at(28),
    );
    assert!(rec.is_clean(), "{rec:?}");
}

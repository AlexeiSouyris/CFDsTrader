//! Reconciliación al arrancar (fase 7, #278). Precios sintéticos.

use std::collections::BTreeMap;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, KillSwitchChange, KillSwitchMark, MarketSetup, Mismatch,
    PaperBroker, kill_switch_stored_active, reconcile, restore_and_reconcile,
    restore_with_kill_switch,
};
use cfds_core::{
    Candle, InstrumentId, KillSwitch, Order, OrderRejection, OrderSide, OrderStatus, PaperMode,
    Position, Timeframe,
};
use cfds_risk::{InstrumentLimits, RiskConfig};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("../../../../config/broker.toml");

fn at(i: i64) -> DateTime<Utc> {
    "2026-11-18T09:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(15 * i)
}

fn flat(id: &str, close: Decimal) -> Vec<Candle> {
    (0..30)
        .map(|i| Candle {
            instrument_id: id.parse().unwrap(),
            timeframe: Timeframe::M15,
            start: at(i),
            open: close,
            high: close + dec!(1),
            low: close - dec!(1),
            close,
            volume: None,
        })
        .collect()
}

fn setup(close: Decimal) -> (BrokerConfig, BTreeMap<InstrumentId, MarketSetup>) {
    let cfg = BrokerConfig::from_toml(
        BROKER,
        RiskConfig::from_toml(RISK).unwrap(),
        CostConfig {
            slippage_spread_fraction: dec!(0.1),
            slippage_ticks: dec!(1),
        },
        ExitConfig::from_toml(BACKTEST).unwrap(),
        chrono_tz::Europe::Madrid,
    )
    .unwrap();
    let markets = ["US100", "DE40"]
        .into_iter()
        .map(|id| {
            (
                id.parse().unwrap(),
                MarketSetup {
                    limits: InstrumentLimits {
                        max_leverage: dec!(10),
                        margin_pct: dec!(5),
                        min_deal_size: Some(dec!(0.01)),
                    },
                    costs: InstrumentCosts {
                        spread: dec!(1),
                        tick: dec!(0.1),
                        commission: Commission::None,
                        funding_admin_fee_pct: dec!(2.5),
                        funding_day_count: dec!(365),
                    },
                    account_to_instrument: cfds_backtest::AccountFx::Same,
                    candles: flat(id, close),
                    funding: FundingSetup {
                        cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
                        tz: chrono_tz::Europe::Madrid,
                        rates: FundingRates::Constant(dec!(2)),
                    },
                },
            )
        })
        .collect();
    (cfg, markets)
}

fn order(id: &str, instrument: &str, stop: Decimal) -> Order {
    Order {
        order_id: id.parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: instrument.parse().unwrap(),
        side: OrderSide::Long,
        stop,
        tp1: None,
        proposal_id: None,
        created_at: at(20),
    }
}

/// Un broker que trabajó, su registro y sus posiciones (lo que se guarda al parar).
fn session() -> (PaperBroker, Vec<Position>) {
    let (cfg, markets) = setup(dec!(100));
    let mut b = PaperBroker::new(cfg, markets);
    b.submit(&order("a", "US100", dec!(98)));
    b.submit(&order("b", "DE40", dec!(98)));
    b.submit(&order("c", "DE40", dec!(97))); // rechazada: ya hay una en DE40
    let p = b.positions(at(29));
    (b, p)
}

#[test]
fn a_clean_restart_rebuilds_the_same_positions_without_kill_switch() {
    let (b, positions) = session();
    assert_eq!(positions.len(), 2);
    let (cfg, markets) = setup(dec!(100));
    let (mut restored, rec) = restore_and_reconcile(cfg, markets, b.journal(), &positions, at(29));
    assert!(rec.is_clean(), "{rec:?}");
    assert_eq!(rec.matched, 2);
    assert_eq!(restored.positions(at(29)), positions);
    assert!(!restored.kill_switch_active());
    assert_eq!(restored.journal(), b.journal());
}

#[test]
fn a_position_the_broker_does_not_have_activates_the_kill_switch() {
    let (b, positions) = session();
    let (cfg, markets) = setup(dec!(100));
    let (mut restored, rec) =
        restore_and_reconcile(cfg, markets, b.journal(), &positions[..1], at(29));
    assert_eq!(
        rec.mismatches,
        vec![Mismatch::MissingInBroker(positions[1].position_id.clone())]
    );
    assert!(matches!(
        restored.account(at(29)).kill_switch,
        KillSwitch::Active { ref reason, .. } if reason.contains("Reconciliación")
    ));
}

#[test]
fn unknown_or_different_positions_are_mismatches() {
    let (_, positions) = session();
    let mut other = positions[0].clone();
    other.position_id = "pos:extra".parse().unwrap();
    let mut changed = positions[1].clone();
    changed.size += dec!(1);
    let rec = reconcile(&positions, &[positions[0].clone(), changed, other]);
    assert_eq!(rec.matched, 1);
    assert_eq!(
        rec.mismatches,
        vec![
            Mismatch::Different(positions[1].position_id.clone()),
            Mismatch::UnknownInBroker("pos:extra".parse().unwrap()),
        ]
    );
}

#[test]
fn candles_that_changed_underneath_are_drift_and_stop_trading() {
    let (b, positions) = session();
    // Las velas guardadas cambiaron (repintado): el fill reconstruido sale a otro precio.
    let (cfg, markets) = setup(dec!(101));
    let (mut restored, rec) = restore_and_reconcile(cfg, markets, b.journal(), &positions, at(29));
    assert!(!rec.drifts.is_empty());
    assert!(restored.kill_switch_active());
    assert!(matches!(
        restored.account(at(29)).kill_switch,
        KillSwitch::Active { .. }
    ));
}

/// Un kill switch guardado (#441) después de la orden `a`: `b` sale rechazada por él.
fn session_with_kill_switch() -> (PaperBroker, Vec<KillSwitchMark>) {
    let (cfg, markets) = setup(dec!(100));
    let mut b = PaperBroker::new(cfg, markets);
    b.submit(&order("a", "US100", dec!(98)));
    b.activate_kill_switch("error de datos en DE40", at(20));
    let r = b.submit(&order("b", "DE40", dec!(98)));
    assert_eq!(r.rejection, Some(OrderRejection::KillSwitchActive));
    let marks = vec![KillSwitchMark {
        after_orders: 1,
        change: KillSwitchChange::Activated {
            reason: "error de datos en DE40".into(),
            at: at(20),
        },
    }];
    (b, marks)
}

#[test]
fn a_saved_kill_switch_survives_a_restart_at_the_same_point_of_the_journal() {
    let (mut b, marks) = session_with_kill_switch();
    let positions = b.positions(at(29));
    let (cfg, markets) = setup(dec!(100));
    let (mut restored, rec) =
        restore_with_kill_switch(cfg, markets, b.journal(), &marks, &positions, at(29));
    assert!(rec.is_clean(), "{rec:?}");
    assert!(kill_switch_stored_active(&marks));
    assert!(matches!(
        restored.account(at(29)).kill_switch,
        KillSwitch::Active { ref reason, since } if reason == "error de datos en DE40" && since == at(20)
    ));
}

#[test]
fn without_the_saved_kill_switch_the_restart_would_reopen_trading() {
    // Lo que pasaba antes de #441: sin el cambio guardado, la orden rechazada por el kill
    // switch se reconstruye distinta (drift) y el motivo del kill switch se pierde.
    let (mut b, _) = session_with_kill_switch();
    let positions = b.positions(at(29));
    let (cfg, markets) = setup(dec!(100));
    let (_, rec) = restore_and_reconcile(cfg, markets, b.journal(), &positions, at(29));
    assert_eq!(rec.drifts.len(), 1);
    assert_eq!(rec.drifts[0].order_id.as_str(), "b");
}

#[test]
fn only_a_saved_release_turns_the_kill_switch_off_and_later_orders_rebuild_the_same() {
    let (mut b, mut marks) = session_with_kill_switch();
    marks.push(KillSwitchMark {
        after_orders: 2,
        change: KillSwitchChange::Released,
    });
    // El broker en marcha no se libera solo: la liberación vale desde el reinicio.
    let before = b.positions(at(20));
    let (cfg, markets) = setup(dec!(100));
    let (mut after_vote, rec) =
        restore_with_kill_switch(cfg, markets, b.journal(), &marks, &before, at(20));
    assert!(rec.is_clean(), "{rec:?}");
    assert!(!after_vote.kill_switch_active());
    assert!(!kill_switch_stored_active(&marks));
    let r = after_vote.submit(&order("c", "DE40", dec!(98)));
    assert_eq!(r.status, OrderStatus::Filled);
    // Y se reconstruye igual: rechazada con el kill switch, llenada después de liberarlo.
    let positions = after_vote.positions(at(29));
    let (cfg, markets) = setup(dec!(100));
    let (again, rec) = restore_with_kill_switch(
        cfg,
        markets,
        after_vote.journal(),
        &marks,
        &positions,
        at(29),
    );
    assert!(rec.is_clean(), "{rec:?}");
    assert!(!again.kill_switch_active());
    assert_eq!(again.journal(), after_vote.journal());
}

/// 20 velas anchas (rango 20) y después 60 angostas (rango 2), desde `at(0)`.
fn wide_then_narrow(id: &str) -> Vec<Candle> {
    (0..80)
        .map(|i| {
            let half = if i < 20 { dec!(10) } else { dec!(1) };
            Candle {
                instrument_id: id.parse().unwrap(),
                timeframe: Timeframe::M15,
                start: at(i),
                open: dec!(100),
                high: dec!(100) + half,
                low: dec!(100) - half,
                close: dec!(100),
                volume: None,
            }
        })
        .collect()
}

fn markets_from(first: usize) -> BTreeMap<InstrumentId, MarketSetup> {
    let (_, mut markets) = setup(dec!(100));
    for (id, m) in &mut markets {
        m.candles = wide_then_narrow(id.as_str())[first..].to_vec();
    }
    markets
}

/// #438: el ATR de Wilder depende de la primera vela. Una sesión que arrancó con más historia
/// que la que carga la reconstrucción decide distinto (acá, el stop a 6,4 puntos pasa con el
/// ATR de toda la serie y es `stop_too_far` con el de las últimas 60 velas).
#[test]
fn the_candles_start_at_the_first_order_so_a_restart_rebuilds_the_same() {
    let (cfg, _) = setup(dec!(100));
    let mut o = order("a", "DE40", dec!(93.6));
    o.created_at = at(80);
    let window = TimeDelta::minutes(15 * 60);

    // Sin fijar el comienzo: la sesión tiene velas desde at(0), la reconstrucción desde at(20).
    let mut loose = PaperBroker::new(cfg.clone(), markets_from(0));
    let r = loose.submit(&o);
    let (_, rec) =
        restore_and_reconcile(cfg.clone(), markets_from(20), loose.journal(), &[], at(80));
    assert_eq!(rec.drifts.len(), 1, "{r:?}");

    // Fijando el comienzo antes de la primera orden: igual que la reconstrucción.
    let mut anchored = PaperBroker::new(cfg.clone(), markets_from(0));
    anchored.start_candles_at(o.created_at - window).unwrap();
    anchored.submit(&o);
    let positions = anchored.positions(at(80));
    let (_, rec) = restore_and_reconcile(
        cfg,
        markets_from(20),
        anchored.journal(),
        &positions,
        at(80),
    );
    assert!(rec.is_clean(), "{rec:?}");
    // Con órdenes ya procesadas las velas no se tocan.
    assert!(anchored.start_candles_at(at(40)).is_err());
    assert_eq!(anchored.markets()[&o.instrument_id].candles.len(), 60);
}

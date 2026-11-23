//! Reconciliación al arrancar (fase 7, #278). Precios sintéticos.

use std::collections::BTreeMap;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, MarketSetup, Mismatch, PaperBroker, reconcile,
    restore_and_reconcile,
};
use cfds_core::{
    Candle, InstrumentId, KillSwitch, Order, OrderSide, PaperMode, Position, Timeframe,
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
                    account_to_instrument: Decimal::ONE,
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

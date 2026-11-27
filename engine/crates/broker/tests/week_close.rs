//! Cierre de fin de semana en el broker simulado (spec §8, #334): lo abierto se cierra al cierre
//! de la última vela de 15m de la sesión del último día hábil de la semana, con las mismas marcas
//! que el backtest. Sin look-ahead ni repintado. Precios sintéticos.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_backtest::{
    Commission, CostConfig, Execution, ExitConfig, FundingRates, InstrumentCosts, fill_price,
};
use cfds_broker::{Broker, BrokerConfig, FundingSetup, MarketSetup, PaperBroker, PaperTrade};
use cfds_core::{Candle, InstrumentId, Order, OrderSide, OrderStatus, PaperMode, Timeframe};
use cfds_market_data::{Catalog, ExchangeCalendar, trading_window};
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_strategy::Side;
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("fixtures/broker.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn id() -> InstrumentId {
    "DE40".parse().unwrap()
}

fn calendar() -> ExchangeCalendar {
    let catalog = Catalog::from_toml(CATALOG).unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap()
}

/// Velas de 15m seguidas (también fuera de sesión) del jueves 19-11-2026 a las 00:00 UTC al lunes
/// 23: casi planas, sin tocar stop ni TP1, con el cierre distinto en cada vela.
fn candles(until: DateTime<Utc>) -> Vec<Candle> {
    let t0 = utc("2026-11-19T00:00:00Z");
    (0..)
        .map(|i: i64| (i, t0 + TimeDelta::minutes(15 * i)))
        .take_while(|(_, s)| *s + TimeDelta::minutes(15) <= until)
        .map(|(i, start)| {
            let close = dec!(20000) + Decimal::new(i % 7, 1);
            Candle {
                instrument_id: id(),
                timeframe: Timeframe::M15,
                start,
                open: close - dec!(0.1),
                high: close + dec!(1),
                low: close - dec!(1),
                close,
                volume: None,
            }
        })
        .collect()
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

fn cost_cfg() -> CostConfig {
    CostConfig {
        slippage_spread_fraction: dec!(0.1),
        slippage_ticks: dec!(1),
    }
}

fn broker(m15: Vec<Candle>, with_sessions: bool) -> PaperBroker {
    let mut cfg = BrokerConfig::from_toml(
        BROKER,
        RiskConfig::from_toml(RISK).unwrap(),
        cost_cfg(),
        ExitConfig::from_toml(BACKTEST).unwrap(),
        chrono_tz::Europe::Madrid,
    )
    .unwrap();
    if with_sessions {
        cfg = cfg.with_calendar(&calendar(), [&id()]).unwrap();
    }
    let market = MarketSetup {
        limits: InstrumentLimits {
            max_leverage: dec!(10),
            margin_pct: dec!(5),
            min_deal_size: Some(dec!(0.01)),
        },
        costs: costs(),
        account_to_instrument: cfds_backtest::AccountFx::Same,
        candles: m15,
        funding: FundingSetup {
            cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            tz: chrono_tz::Europe::Madrid,
            rates: FundingRates::Constant(dec!(2)),
        },
    };
    PaperBroker::new(cfg, BTreeMap::from([(id(), market)]))
}

/// Largo decidido el viernes 20-11 a las 13:00 UTC, con el stop a ~2 ATR.
fn friday_order() -> Order {
    Order {
        order_id: "wk-1".parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: id(),
        side: OrderSide::Long,
        stop: dec!(19996),
        tp1: None,
        proposal_id: None,
        created_at: utc("2026-11-20T13:00:00Z"),
    }
}

fn opened(m15: Vec<Candle>, with_sessions: bool) -> PaperBroker {
    let mut b = broker(m15, with_sessions);
    let r = b.submit(&friday_order());
    assert_eq!(r.status, OrderStatus::Filled, "{r:?}");
    b
}

fn only(trades: Vec<PaperTrade>) -> PaperTrade {
    assert_eq!(trades.len(), 1);
    trades.into_iter().next().unwrap()
}

/// La vela del cierre de la semana de DE40 (Xetra cierra a las 17:30 de Berlín = 16:30 UTC en
/// noviembre): la de 16:15 a 16:30 UTC del viernes 20.
const WEEK_CLOSE_END: &str = "2026-11-20T16:30:00Z";

#[test]
fn a_friday_position_closes_at_the_close_of_the_week_close_candle() {
    let m15 = candles(utc("2026-11-23T12:00:00Z"));
    let close_candle = m15
        .iter()
        .find(|c| c.end() == utc(WEEK_CLOSE_END))
        .unwrap()
        .clone();
    // Es la misma vela que marca el backtest.
    let session = calendar().session_for(&id()).unwrap().clone();
    assert_eq!(
        trading_window::is_week_close_candle(&session, &close_candle),
        Some(true)
    );
    let mut b = opened(m15.clone(), true);
    let now = m15.last().unwrap().end();
    let t = only(b.trade_outcomes(now));
    assert_eq!(t.exits.len(), 1, "{:#?}", t.exits);
    let e = &t.exits[0];
    assert_eq!(e.time, utc(WEEK_CLOSE_END));
    assert_eq!(e.mid, close_candle.close);
    assert!(!e.end_of_data);
    assert_eq!(e.fraction, Decimal::ONE);
    // Al precio de la regla con los costos de salida de siempre.
    assert_eq!(
        e.price,
        fill_price(Side::Long, Execution::Exit, e.mid, &costs(), &cost_cfg())
    );
    // Ya no está abierta el lunes.
    assert!(b.positions(now).is_empty());
}

#[test]
fn no_look_ahead_and_no_repaint() {
    // Antes de que cierre la vela del cierre de la semana, sigue abierta.
    let before = utc(WEEK_CLOSE_END) - TimeDelta::minutes(15);
    let mut b = opened(candles(before), true);
    let t = only(b.trade_outcomes(before));
    assert!(t.exits.iter().all(|e| e.end_of_data), "{:#?}", t.exits);
    assert_eq!(b.positions(before).len(), 1);
    // Con la vela del cierre ya cerrada, se cierra ahí; agregar las velas del lunes no cambia nada.
    let at_close = opened(candles(utc(WEEK_CLOSE_END)), true).trade_outcomes(utc(WEEK_CLOSE_END));
    let later = opened(candles(utc("2026-11-23T12:00:00Z")), true)
        .trade_outcomes(utc("2026-11-23T12:00:00Z"));
    assert_eq!(at_close, later);
    assert!(!only(at_close).exits[0].end_of_data);
}

#[test]
fn without_sessions_nothing_is_forced() {
    let m15 = candles(utc("2026-11-23T12:00:00Z"));
    let now = m15.last().unwrap().end();
    let t = only(opened(m15, false).trade_outcomes(now));
    assert!(t.exits.iter().all(|e| e.time != utc(WEEK_CLOSE_END)));
}

#[test]
fn an_instrument_without_exchange_is_an_error() {
    let cfg = BrokerConfig::from_toml(
        BROKER,
        RiskConfig::from_toml(RISK).unwrap(),
        cost_cfg(),
        ExitConfig::from_toml(BACKTEST).unwrap(),
        chrono_tz::Europe::Madrid,
    )
    .unwrap();
    let unknown: InstrumentId = "ZZ99".parse().unwrap();
    assert!(cfg.with_calendar(&calendar(), [&unknown]).is_err());
}

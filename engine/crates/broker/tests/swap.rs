//! Financiación nocturna (swap) del broker simulado (#278). Precios y tasas sintéticos. El
//! importe esperado se calcula con `cfds_backtest::funding`, el mismo cálculo del backtest.

use std::collections::BTreeMap;

use cfds_backtest::{
    Commission, CostConfig, DailyRates, ExitConfig, FundingRates, InstrumentCosts, funding,
};
use cfds_broker::{Broker, BrokerConfig, FundingSetup, MarketSetup, PaperBroker};
use cfds_core::{
    Candle, InstrumentId, KillSwitch, Order, OrderRejection, OrderSide, OrderStatus, PaperMode,
    ReferenceRate, Timeframe,
};
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_strategy::Side;
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("../../../../config/broker.toml");

fn t(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn bar(start: DateTime<Utc>, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

fn flat(start: DateTime<Utc>) -> Candle {
    bar(start, dec!(100), dec!(101), dec!(99), dec!(100))
}

/// Miércoles 18-11-2026: 21 velas planas desde las 09:00 UTC (la 20 es la del fill, 14:00).
/// Después, una vela por mañana: jueves, viernes y lunes (no hay velas el fin de semana).
fn candles(monday: Candle) -> Vec<Candle> {
    let wed = t("2026-11-18T09:00:00Z");
    let mut c: Vec<Candle> = (0..21)
        .map(|i| flat(wed + TimeDelta::minutes(15 * i)))
        .collect();
    c.push(flat(t("2026-11-19T09:00:00Z")));
    c.push(flat(t("2026-11-20T09:00:00Z")));
    c.push(monday);
    c
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

fn broker(rates: FundingRates, monday: Candle) -> PaperBroker {
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
    let market = MarketSetup {
        limits: InstrumentLimits {
            max_leverage: dec!(10),
            margin_pct: dec!(5),
            min_deal_size: Some(dec!(0.01)),
        },
        costs: costs(),
        account_to_instrument: cfds_backtest::AccountFx::Same,
        candles: candles(monday),
        funding: FundingSetup {
            cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            tz: chrono_tz::Europe::Madrid,
            rates,
        },
    };
    let markets: BTreeMap<InstrumentId, MarketSetup> =
        [("US100".parse().unwrap(), market)].into_iter().collect();
    PaperBroker::new(cfg, markets)
}

fn open(b: &mut PaperBroker, side: OrderSide) {
    let stop = match side {
        OrderSide::Long => dec!(98),
        OrderSide::Short => dec!(102),
    };
    let r = b.submit(&Order {
        order_id: "o1".parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: "US100".parse().unwrap(),
        side,
        stop,
        tp1: None,
        proposal_id: None,
        created_at: t("2026-11-18T14:00:00Z"),
    });
    assert_eq!(r.status, OrderStatus::Filled, "{r:?}");
    assert_eq!(r.size, Some(dec!(5)));
}

fn monday_flat() -> Candle {
    flat(t("2026-11-23T09:00:00Z"))
}

/// Swap esperado de la posición de 5 € por punto abierta en 100, con el cálculo del backtest.
fn swap(side: Side, nights: u32, benchmark: Decimal) -> Decimal {
    funding(side, dec!(100), dec!(5), nights, benchmark, &costs())
}

#[test]
fn no_swap_before_the_first_cutoff() {
    let mut b = broker(FundingRates::Constant(dec!(2)), monday_flat());
    open(&mut b, OrderSide::Long);
    // miércoles 21:00 UTC = 22:00 de Madrid: el corte de las 23:00 todavía no pasó
    let p = b.positions(t("2026-11-18T21:00:00Z"));
    assert_eq!(p[0].unrealized_pl, dec!(-3.50));
}

#[test]
fn one_night_is_charged_on_the_open_position() {
    let mut b = broker(FundingRates::Constant(dec!(2)), monday_flat());
    open(&mut b, OrderSide::Long);
    let p = b.positions(t("2026-11-19T10:00:00Z"));
    // (100 − 100,7) × 5 = −3,5, menos una noche de swap a (2 + 2,5) % anual
    assert_eq!(
        p[0].unrealized_pl,
        (dec!(-3.5) - swap(Side::Long, 1, dec!(2))).round_dp(2)
    );
    let acc = b.account(t("2026-11-19T10:00:00Z"));
    assert_eq!(
        acc.equity,
        (dec!(1000) - dec!(3.5) - swap(Side::Long, 1, dec!(2))).round_dp(2)
    );
}

#[test]
fn the_weekend_pays_friday_saturday_and_sunday() {
    let mut b = broker(FundingRates::Constant(dec!(2)), monday_flat());
    open(&mut b, OrderSide::Long);
    let friday = b.positions(t("2026-11-20T10:00:00Z"));
    let monday = b.positions(t("2026-11-23T10:00:00Z"));
    // miércoles y jueves; después, además, viernes, sábado y domingo
    assert_eq!(
        friday[0].unrealized_pl,
        (dec!(-3.5) - swap(Side::Long, 2, dec!(2))).round_dp(2)
    );
    assert_eq!(
        monday[0].unrealized_pl,
        (dec!(-3.5) - swap(Side::Long, 5, dec!(2))).round_dp(2)
    );
}

#[test]
fn a_short_collects_when_the_benchmark_is_above_the_fee() {
    // corto: paga 2,5 − 3 = −0,5 % anual → cobra
    let mut b = broker(FundingRates::Constant(dec!(3)), monday_flat());
    open(&mut b, OrderSide::Short);
    let p = b.positions(t("2026-11-19T10:00:00Z"));
    let s = swap(Side::Short, 1, dec!(3));
    assert!(s < Decimal::ZERO);
    // (99,3 − 100) × 5 = −3,5, y el swap negativo suma
    assert_eq!(p[0].unrealized_pl, (dec!(-3.5) - s).round_dp(2));
}

#[test]
fn a_position_closed_after_the_weekend_realizes_its_swap() {
    // el lunes abre en 97, debajo del stop de 98: sale a la apertura
    let gap = bar(
        t("2026-11-23T09:00:00Z"),
        dec!(97),
        dec!(97.5),
        dec!(96),
        dec!(97),
    );
    let mut b = broker(FundingRates::Constant(dec!(2)), gap);
    open(&mut b, OrderSide::Long);
    let acc = b.account(t("2026-11-23T10:00:00Z"));
    assert_eq!(acc.open_positions, 0);
    // (96,3 − 100,7) × 5 = −22, menos 5 noches (miércoles a domingo)
    let expected = dec!(1000) - dec!(22) - swap(Side::Long, 5, dec!(2));
    assert_eq!(acc.equity, expected.round_dp(2));
    assert_eq!(
        acc.realized_pl_today,
        (dec!(-22) - swap(Side::Long, 5, dec!(2))).round_dp(2)
    );
}

#[test]
fn the_daily_rate_in_force_at_each_cutoff_is_used() {
    let rate = |day: &str, published: &str, pct: Decimal| ReferenceRate {
        series: "ESTR".into(),
        ref_date: day.parse().unwrap(),
        published_at: t(published),
        rate_pct: pct,
    };
    // 2 % para la noche del miércoles; el jueves a la mañana se publica 4 %
    let daily = DailyRates::new(vec![
        rate("2026-11-17", "2026-11-18T08:00:00Z", dec!(2)),
        rate("2026-11-18", "2026-11-19T08:00:00Z", dec!(4)),
    ])
    .unwrap();
    let mut b = broker(FundingRates::Daily(daily), monday_flat());
    open(&mut b, OrderSide::Long);
    let p = b.positions(t("2026-11-20T10:00:00Z"));
    let expected = dec!(-3.5) - swap(Side::Long, 1, dec!(2)) - swap(Side::Long, 1, dec!(4));
    assert_eq!(p[0].unrealized_pl, expected.round_dp(2));
    assert!(!b.kill_switch_active());
}

#[test]
fn a_missing_rate_is_not_invented_and_activates_the_kill_switch() {
    // La única tasa se publica el jueves: la noche del miércoles no tiene tasa vigente.
    let daily = DailyRates::new(vec![ReferenceRate {
        series: "ESTR".into(),
        ref_date: "2026-11-18".parse().unwrap(),
        published_at: t("2026-11-19T08:00:00Z"),
        rate_pct: dec!(2),
    }])
    .unwrap();
    let mut b = broker(FundingRates::Daily(daily), monday_flat());
    open(&mut b, OrderSide::Long);
    let p = b.positions(t("2026-11-19T10:00:00Z"));
    // no se cobra la noche sin tasa (no se inventa), y se frena todo por error de datos
    assert_eq!(p[0].unrealized_pl, dec!(-3.50));
    assert!(b.kill_switch_active());
    assert!(matches!(
        b.account(t("2026-11-19T10:00:00Z")).kill_switch,
        KillSwitch::Active { ref reason, .. } if reason.contains("tasa de referencia")
    ));
    let r = b.submit(&Order {
        order_id: "o2".parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: "US100".parse().unwrap(),
        side: OrderSide::Short,
        stop: dec!(102),
        tp1: None,
        proposal_id: None,
        created_at: t("2026-11-20T09:00:00Z"),
    });
    assert_eq!(r.rejection, Some(OrderRejection::KillSwitchActive));
}

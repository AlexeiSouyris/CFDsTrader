//! MAE y MFE de las operaciones de estudio (#728, C6 del autoanálisis): cuánto llegó a ir en
//! contra y a favor cada operación antes de cerrar, con cada motivo de salida.

use cfds_backtest::research::{Plan, StudyExit, StudyTrade, excursions, execute_plan};
use cfds_backtest::{BacktestConfig, Setup, apply_funding_scenario, setup_from_catalog};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::Catalog;
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, Side, StrategyConfig};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn us500() -> InstrumentId {
    "US500".parse().unwrap()
}

fn setup() -> Setup {
    let mut catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    apply_funding_scenario(&mut catalog, &us500(), "TEST", dec!(4)).unwrap();
    setup_from_catalog(
        &catalog,
        &us500(),
        Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
            trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
        },
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        dec!(100000),
    )
    .unwrap()
}

/// Martes 2024-03-05 15:00 UTC: sesión de contado de EE. UU., sin financiación en el medio.
fn start() -> DateTime<Utc> {
    "2024-03-05T15:00:00Z".parse().unwrap()
}

/// Velas de 15m seguidas `(open, high, low, close)` desde `start()`.
fn candles(ohlc: &[(i64, i64, i64, i64)]) -> Vec<Candle> {
    ohlc.iter()
        .enumerate()
        .map(|(k, &(o, h, l, c))| Candle {
            instrument_id: us500(),
            timeframe: Timeframe::M15,
            start: start() + TimeDelta::minutes(15 * k as i64),
            open: Decimal::from(o),
            high: Decimal::from(h),
            low: Decimal::from(l),
            close: Decimal::from(c),
            volume: None,
        })
        .collect()
}

/// Plan decidido al cierre de la vela 0 (cierre 100), salida planificada al cierre de la 2.
fn plan(side: Side, stop: Decimal, target_r: Option<Decimal>) -> Plan {
    Plan {
        event: 0,
        signal_index: 0,
        side,
        reference: dec!(100),
        stop,
        risk_atr: dec!(3),
        exit_index: 2,
        target_r,
        target_cap: None,
    }
}

fn run(p: &Plan, m15: &[Candle]) -> StudyTrade {
    let force_close = vec![false; m15.len()];
    execute_plan(
        p,
        dec!(1),
        dec!(3),
        dec!(100000),
        m15,
        &force_close,
        &setup(),
    )
    .unwrap()
}

#[test]
fn a_planned_exit_counts_every_candle_whole() {
    let m15 = candles(&[(99, 100, 98, 100), (100, 103, 98, 101), (101, 105, 99, 104)]);
    let t = run(&plan(Side::Long, dec!(97), None), &m15);
    assert_eq!(t.exit, StudyExit::Planned);
    assert_eq!((t.mae, t.mfe), (dec!(2), dec!(5)));
    assert_eq!(
        t.excursions_r(),
        Some((dec!(2) / dec!(3), dec!(5) / dec!(3)))
    );
    // El resto de la operación es el de siempre.
    assert_eq!((t.entry_mid, t.exit_mid), (dec!(100), dec!(104)));
    assert_eq!(t.mid_pnl, dec!(4));
}

#[test]
fn a_stop_caps_the_adverse_side_at_the_stop() {
    let m15 = candles(&[(99, 100, 98, 100), (100, 102, 99, 100), (99, 104, 96, 98)]);
    let t = run(&plan(Side::Long, dec!(97), None), &m15);
    assert_eq!(t.exit, StudyExit::Stop);
    // Adversa: hasta el stop (97), no hasta el mínimo de la vela (96). Favorable: la vela entera
    // (cota: no se sabe si el 104 fue antes del stop).
    assert_eq!((t.mae, t.mfe), (dec!(3), dec!(4)));
    assert_eq!(t.excursions_r(), Some((dec!(1), dec!(4) / dec!(3))));
}

#[test]
fn a_stop_gap_only_counts_the_open_of_the_exit_candle() {
    let m15 = candles(&[(99, 100, 98, 100), (100, 102, 99, 100), (95, 110, 90, 92)]);
    let t = run(&plan(Side::Long, dec!(97), None), &m15);
    assert_eq!(t.exit, StudyExit::StopGap);
    assert_eq!((t.mae, t.mfe), (dec!(5), dec!(2)));
}

#[test]
fn a_target_caps_the_favorable_side_at_the_target() {
    // Corto a 100 con stop 103: objetivo 1 R = 97.
    let m15 = candles(&[(101, 101, 99, 100), (100, 101, 98, 99), (99, 102, 96, 97)]);
    let t = run(&plan(Side::Short, dec!(103), Some(dec!(1))), &m15);
    assert_eq!(t.exit, StudyExit::Target);
    assert_eq!(t.exit_mid, dec!(97));
    assert_eq!((t.mae, t.mfe), (dec!(2), dec!(3)));
}

#[test]
fn an_exit_on_the_entry_candle_uses_only_that_candle() {
    let m15 = candles(&[(99, 100, 98, 100), (100, 101, 96, 97), (97, 120, 80, 100)]);
    let t = run(&plan(Side::Long, dec!(97), None), &m15);
    assert_eq!((t.exit, t.exit_index), (StudyExit::Stop, 1));
    assert_eq!((t.mae, t.mfe), (dec!(3), dec!(1)));
}

#[test]
fn excursions_are_never_negative_and_need_every_candle() {
    let m15 = candles(&[(99, 100, 98, 100), (100, 100, 100, 100)]);
    assert_eq!(
        excursions(
            Side::Short,
            dec!(100),
            1,
            1,
            dec!(100),
            StudyExit::Planned,
            &m15
        ),
        Some((Decimal::ZERO, Decimal::ZERO))
    );
    assert_eq!(
        excursions(
            Side::Long,
            dec!(100),
            1,
            5,
            dec!(100),
            StudyExit::Planned,
            &m15
        ),
        None
    );
    // Sin distancia al stop no hay R.
    let mut t = run(
        &plan(Side::Long, dec!(97), None),
        &candles(&[(99, 100, 98, 100), (100, 103, 98, 101), (101, 105, 99, 104)]),
    );
    t.stop = t.entry_mid;
    assert_eq!(t.excursions_r(), None);
}

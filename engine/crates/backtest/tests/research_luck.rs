//! Test de suerte general (#729): `research::luck` da exactamente lo mismo que la implementación
//! del programa 4 de antes (copiada abajo como referencia) y se comporta bien sin objetivo y sin
//! candidatas. Series sintéticas sobre el calendario real de XNYS; nunca la base real.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use cfds_backtest::research::luck::{self, LuckConfig, LuckSeries, LuckTest};
use cfds_backtest::research::program4::{
    SplitMix64, execution_exit, luck_test, r_summary, trade_net_r,
};
use cfds_backtest::research::{
    Plan, Program4Config, ResearchConfig, StudyTrade, run_plans, simulate_leg,
};
use cfds_backtest::{
    BacktestConfig, Execution, Setup, apply_funding_scenario, fill_price, setup_from_catalog,
};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Catalog, ExchangeCalendar, Session};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, Side, StrategyConfig};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn us100() -> InstrumentId {
    "US100".parse().unwrap()
}

fn program4() -> Program4Config {
    let mut p = ResearchConfig::from_toml(cfg!("research.toml"))
        .unwrap()
        .program4;
    p.luck_simulations = 300;
    p
}

fn session() -> Session {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    let cal = ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap();
    cal.session_for(&us100()).unwrap().clone()
}

fn setup() -> Setup {
    let mut catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    apply_funding_scenario(&mut catalog, &us100(), "TEST", dec!(4)).unwrap();
    setup_from_catalog(
        &catalog,
        &us100(),
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

/// Velas de 15m de la sesión de contado de cada día con sesión en `[from, to]`, con un camino
/// al azar determinista (LCG) alrededor de 15 000.
fn session_candles(session: &Session, from: &str, to: &str) -> Vec<Candle> {
    let (from, to): (NaiveDate, NaiveDate) = (from.parse().unwrap(), to.parse().unwrap());
    let mut out = Vec::new();
    let mut state: u64 = 42;
    let mut price = dec!(15000);
    let mut d = from;
    while d <= to {
        if let Some(Some((open, close))) = session.hours(d) {
            let mut s = open;
            while s + TimeDelta::minutes(15) <= close {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let step = Decimal::from((state >> 33) % 41) - dec!(20);
                let o = price;
                let c = price + step;
                let wick = Decimal::from((state >> 40) % 9);
                out.push(Candle {
                    instrument_id: us100(),
                    timeframe: Timeframe::M15,
                    start: s,
                    open: o,
                    high: o.max(c) + wick,
                    low: o.min(c) - wick,
                    close: c,
                    volume: None,
                });
                price = c;
                s += TimeDelta::minutes(15);
            }
        }
        d = d.succ_opt().unwrap();
    }
    out
}

/// Dos planes por sesión (barras 4 y 14), de lados alternados, con objetivo de 1,5 R y la
/// salida planificada del programa 4.
fn trades(m15: &[Candle], session: &Session, cfg: &Program4Config, s: &Setup) -> Vec<StudyTrade> {
    let none = BTreeSet::new();
    let mut plans = Vec::new();
    let mut k = 0usize;
    let mut day: Option<NaiveDate> = None;
    for i in 0..m15.len().saturating_sub(1) {
        let d = session.trading_day(m15[i].start);
        if day != Some(d) {
            day = Some(d);
            k = 0;
        }
        k += 1;
        if k != 4 && k != 14 {
            continue;
        }
        let Some(exit_index) = execution_exit(m15, session, &none, cfg, i + 1) else {
            continue;
        };
        let side = if plans.len() % 3 == 0 {
            Side::Short
        } else {
            Side::Long
        };
        let distance = dec!(30);
        let reference = m15[i].close;
        plans.push(Plan {
            event: plans.len(),
            signal_index: i,
            side,
            reference,
            stop: match side {
                Side::Long => reference - distance,
                Side::Short => reference + distance,
            },
            risk_atr: distance,
            exit_index,
            target_r: Some(cfg.target_r),
            target_cap: None,
        });
    }
    let from: DateTime<Utc> = "2024-01-01T00:00:00Z".parse().unwrap();
    let to: DateTime<Utc> = "2025-01-01T00:00:00Z".parse().unwrap();
    run_plans(&plans, m15, &[], s, from, to).unwrap().trades
}

// ── Implementación anterior del programa 4 (hasta #729), copiada tal cual como referencia ──

fn net_r(side: Side, entry_price: Decimal, exit_price: Decimal, distance: Decimal) -> Decimal {
    let s = match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    };
    (exit_price - entry_price) * s / distance
}

fn decision_allowed(candles: &[Candle], entries: &[bool], i: usize) -> bool {
    entries.get(i).copied().unwrap_or(false)
        && candles
            .get(i + 1)
            .is_some_and(|n| n.start == candles[i].end())
}

#[allow(clippy::too_many_arguments)]
fn reference_luck_test(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    excluded: &BTreeSet<NaiveDate>,
    force_close: &[bool],
    cfg: &Program4Config,
    trades: &[StudyTrade],
    setup: &Setup,
    risk: Decimal,
) -> LuckTest {
    let real: Vec<Decimal> = trades.iter().filter_map(trade_net_r).collect();
    let strategy = r_summary(&real, risk);
    let longs = trades.iter().filter(|t| t.side == Side::Long).count();
    let mut per_day: BTreeMap<NaiveDate, Vec<Decimal>> = BTreeMap::new();
    for t in trades {
        let d = (t.entry_mid - t.stop).abs();
        if d > Decimal::ZERO {
            per_day
                .entry(session.trading_day(t.entry_time))
                .or_default()
                .push(d);
        }
    }
    let mut candidates: BTreeMap<NaiveDate, Vec<(usize, usize)>> = BTreeMap::new();
    for j in 1..m15.len() {
        if !decision_allowed(m15, entries, j - 1) {
            continue;
        }
        if let Some(exit) = execution_exit(m15, session, excluded, cfg, j) {
            let day = session.trading_day(m15[j].start);
            if per_day.contains_key(&day) {
                candidates.entry(day).or_default().push((j, exit));
            }
        }
    }
    let mut rng = SplitMix64::new(cfg.luck_seed);
    let mut simulations = Vec::with_capacity(cfg.luck_simulations);
    for _ in 0..cfg.luck_simulations {
        let mut rs = Vec::new();
        for (day, distances) in &per_day {
            let Some(pool) = candidates.get(day) else {
                continue;
            };
            let mut idx: Vec<usize> = (0..pool.len()).collect();
            let n = distances.len().min(pool.len());
            for k in 0..n {
                let r = k + rng.below(pool.len() - k);
                idx.swap(k, r);
            }
            let mut picked: Vec<(usize, usize)> = idx[..n].iter().map(|&k| pool[k]).collect();
            picked.sort_unstable();
            let mut busy_until: Option<usize> = None;
            for (k, (entry_index, exit_index)) in picked.into_iter().enumerate() {
                let side = if rng.chance(longs, trades.len()) {
                    Side::Long
                } else {
                    Side::Short
                };
                if busy_until.is_some_and(|b| entry_index <= b) {
                    continue;
                }
                let distance = distances[k];
                let entry_mid = m15[entry_index].open;
                let (stop, target) = match side {
                    Side::Long => (entry_mid - distance, entry_mid + cfg.target_r * distance),
                    Side::Short => (entry_mid + distance, entry_mid - cfg.target_r * distance),
                };
                let Some((out_index, exit_mid, _)) = simulate_leg(
                    side,
                    entry_index,
                    stop,
                    Some(target),
                    exit_index,
                    m15,
                    force_close,
                ) else {
                    continue;
                };
                let c = &setup.backtest.costs;
                let entry_price = fill_price(side, Execution::Entry, entry_mid, &setup.costs, c);
                let exit_price = fill_price(side, Execution::Exit, exit_mid, &setup.costs, c);
                rs.push(net_r(side, entry_price, exit_price, distance));
                busy_until = Some(out_index);
            }
        }
        simulations.push(r_summary(&rs, risk));
    }
    LuckTest {
        strategy,
        simulations,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_general_test_gives_exactly_the_old_program_4_result() {
    let (session, s, cfg) = (session(), setup(), program4());
    let m15 = session_candles(&session, "2024-01-08", "2024-02-09");
    let trades = trades(&m15, &session, &cfg, &s);
    assert!(trades.len() > 20, "pocas operaciones: {}", trades.len());
    let entries = vec![true; m15.len()];
    let mut excluded_days = BTreeSet::new();
    excluded_days.insert(session.trading_day(trades[3].entry_time));
    for excluded in [BTreeSet::new(), excluded_days] {
        for risk in [dec!(0.01), dec!(0.005)] {
            let old = reference_luck_test(
                &m15,
                &session,
                &entries,
                &excluded,
                &[],
                &cfg,
                &trades,
                &s,
                risk,
            );
            let new = luck_test(
                &m15,
                &session,
                &entries,
                &excluded,
                &[],
                &cfg,
                &trades,
                &s,
                risk,
            );
            assert_eq!(new, old);
            assert!(old.simulations.iter().any(|x| x.0 != Decimal::ZERO));
            assert_eq!(new.percentiles(), old.percentiles());
        }
    }
}

#[test]
fn without_a_target_the_random_entries_exit_by_stop_or_plan() {
    let (session, s, cfg) = (session(), setup(), program4());
    let m15 = session_candles(&session, "2024-01-08", "2024-01-26");
    let trades = trades(&m15, &session, &cfg, &s);
    let entries = vec![true; m15.len()];
    let series = LuckSeries {
        m15: &m15,
        session: &session,
        entries: &entries,
        force_close: &[],
    };
    let none = BTreeSet::new();
    let exit = |j| execution_exit(&m15, &session, &none, &cfg, j);
    let luck_cfg = |target_r| LuckConfig {
        simulations: 100,
        seed: 7,
        target_r,
    };
    let a = luck::luck_test(&series, exit, &luck_cfg(None), &trades, &s, dec!(0.01));
    let b = luck::luck_test(&series, exit, &luck_cfg(None), &trades, &s, dec!(0.01));
    assert_eq!(a, b, "misma semilla, mismo resultado");
    assert_eq!(a.simulations.len(), 100);
    // La estrategia se mide igual con o sin objetivo; las simulaciones cambian.
    let with_target = luck::luck_test(
        &series,
        exit,
        &luck_cfg(Some(dec!(1.5))),
        &trades,
        &s,
        dec!(0.01),
    );
    assert_eq!(a.strategy, with_target.strategy);
    assert_ne!(a.simulations, with_target.simulations);
    let (p_mean, p_total) = a.percentiles();
    assert!((Decimal::ZERO..=Decimal::ONE_HUNDRED).contains(&p_mean));
    assert!((Decimal::ZERO..=Decimal::ONE_HUNDRED).contains(&p_total));
}

#[test]
fn without_candidates_or_trades_every_simulation_is_empty() {
    let (session, s, cfg) = (session(), setup(), program4());
    let m15 = session_candles(&session, "2024-01-08", "2024-01-12");
    let trades = trades(&m15, &session, &cfg, &s);
    let entries = vec![true; m15.len()];
    let series = LuckSeries {
        m15: &m15,
        session: &session,
        entries: &entries,
        force_close: &[],
    };
    let cfg = LuckConfig {
        simulations: 5,
        seed: 1,
        target_r: None,
    };
    // Ninguna vela candidata (la regla de salida no deja entrar en ninguna).
    let none = luck::luck_test(&series, |_| None, &cfg, &trades, &s, dec!(0.01));
    assert!(none.simulations.iter().all(|x| *x == (dec!(0), dec!(0))));
    // Sin operaciones: la estrategia y las simulaciones son cero.
    let empty = luck::luck_test(&series, Some, &cfg, &[], &s, dec!(0.01));
    assert_eq!(empty.strategy, (dec!(0), dec!(0)));
    assert_eq!(empty.simulations.len(), 5);
    assert_eq!(empty.percentiles(), (dec!(0), dec!(0)));
}

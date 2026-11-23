//! Evaluación de la Fase 5 (#298) con series sintéticas: tramos, fold vacío y sin operaciones,
//! huecos, guardia del fuera de muestra y la prueba nunca alimenta el entrenamiento.

use cfds_backtest::{
    BacktestConfig, Commission, EvalError, EvaluationConfig, FoldResult, FundingRates, Inputs,
    InstrumentCosts, Setup, aggregate, evaluate_holdout, evaluate_period, research_period,
    walk_forward,
};
use cfds_core::{Candle, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{DataSplit, Period, Window};
use cfds_patterns::PatternsConfig;
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn setup() -> Setup {
    Setup {
        instrument: "US100".parse().unwrap(),
        strategy: Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
            trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
        },
        risk: RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        limits: InstrumentLimits {
            max_leverage: dec!(10),
            margin_pct: dec!(5),
            min_deal_size: Some(dec!(0.0001)),
        },
        costs: InstrumentCosts {
            spread: dec!(1),
            tick: dec!(0.1),
            commission: Commission::None,
            funding_admin_fee_pct: dec!(2.5),
            funding_day_count: dec!(360),
        },
        backtest: BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        initial_equity: dec!(1000),
        account_to_instrument: dec!(1),
        funding_rates: FundingRates::Constant(dec!(1.1)),
        funding_cutoff: chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
        funding_tz: chrono_tz::Europe::Madrid,
        day_tz: chrono_tz::Europe::Madrid,
    }
}

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn cfg() -> EvaluationConfig {
    EvaluationConfig::from_toml(cfg!("evaluation.toml")).unwrap()
}

/// Caminata sintética de 15m, 24 h por día, desde el 05-01-2026 (precios inventados).
fn walk(n: usize, seed: u64) -> Vec<Candle> {
    let mut state = seed;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let t0 = utc("2026-01-05T00:00:00Z");
    let mut close = dec!(1000);
    (0..n)
        .map(|i| {
            let drift = if (i / 150) % 2 == 0 { 1 } else { -1 };
            let open = close;
            close = open + Decimal::new(next(41) - 20 + drift * 2, 1);
            Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * i as i64),
                open,
                high: open.max(close) + Decimal::new(next(25), 1),
                low: open.min(close) - Decimal::new(next(25), 1),
                close,
                volume: None,
            }
        })
        .collect()
}

fn h1_of(m15: &[Candle]) -> Vec<Candle> {
    m15.chunks_exact(4)
        .map(|g| Candle {
            instrument_id: g[0].instrument_id.clone(),
            timeframe: Timeframe::H1,
            start: g[0].start,
            open: g[0].open,
            high: g.iter().map(|c| c.high).max().unwrap(),
            low: g.iter().map(|c| c.low).min().unwrap(),
            close: g[3].close,
            volume: None,
        })
        .collect()
}

fn inputs<'a>(m15: &'a [Candle], h1: &'a [Candle]) -> Inputs<'a> {
    Inputs {
        m15,
        h1,
        entries: &[],
        force_close: &[],
        data_errors: &[],
        news: &[],
        published_from: utc("2020-01-01T00:00:00Z"),
    }
}

fn period(a: &str, b: &str) -> Period {
    Period {
        start: utc(a),
        end: utc(b),
    }
}

/// Dos ventanas de 10 días de entrenamiento y 5 de prueba, y el fuera de muestra al final
/// (la serie de 6000 velas cubre del 05-01 al 06-03-2026).
fn split() -> DataSplit {
    DataSplit {
        windows: vec![
            Window {
                train: period("2026-01-12T00:00:00Z", "2026-01-22T00:00:00Z"),
                validation: period("2026-01-22T00:00:00Z", "2026-01-27T00:00:00Z"),
            },
            Window {
                train: period("2026-01-27T00:00:00Z", "2026-02-06T00:00:00Z"),
                validation: period("2026-02-06T00:00:00Z", "2026-02-11T00:00:00Z"),
            },
        ],
        out_of_sample: period("2026-02-11T00:00:00Z", "2026-03-01T00:00:00Z"),
    }
}

const WARMUP_DAYS: i64 = 5;

#[test]
fn a_period_only_opens_trades_inside_it_and_measures_from_its_start() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let p = period("2026-01-20T00:00:00Z", "2026-02-05T00:00:00Z");
    let r = evaluate_period(
        &inputs(&m15, &h1),
        p,
        &setup(),
        TimeDelta::days(WARMUP_DAYS),
        dec!(1),
    )
    .unwrap();
    let m = r.metrics.clone().unwrap();
    assert!(m.trades > 0, "la serie tiene que operar");
    assert_eq!(r.candles, 16 * 96);
    // Comprar y mantener con las velas del tramo: abre en la primera y cierra en la última.
    let bh = r.buy_and_hold.unwrap();
    assert!(bh.nights > 0 && bh.funding != Decimal::ZERO);
    assert_eq!(
        r.buy_and_hold_return.unwrap(),
        bh.net_pnl / dec!(1000),
        "sobre el capital inicial"
    );
    // Lo que pasa después del tramo no cambia nada (sin look-ahead ni repintado).
    let longer = walk(7000, 11);
    let h1b = h1_of(&longer);
    let r2 = evaluate_period(
        &inputs(&longer, &h1b),
        p,
        &setup(),
        TimeDelta::days(WARMUP_DAYS),
        dec!(1),
    )
    .unwrap();
    assert_eq!(r, r2);
}

#[test]
fn an_empty_fold_has_no_metrics_and_never_invents_numbers() {
    let m15 = walk(600, 3);
    let h1 = h1_of(&m15);
    let r = evaluate_period(
        &inputs(&m15, &h1),
        period("2027-01-01T00:00:00Z", "2027-02-01T00:00:00Z"),
        &setup(),
        TimeDelta::days(WARMUP_DAYS),
        dec!(1),
    )
    .unwrap();
    assert_eq!((r.candles, &r.metrics, r.buy_and_hold), (0, &None, None));
    assert_eq!(r.final_equity, dec!(1000));
    let a = aggregate([&r]);
    assert_eq!((a.periods, a.with_data, a.mean_return), (1, 0, None));
}

#[test]
fn a_fold_without_trades_has_zero_return_and_undefined_ratios() {
    let m15 = walk(3000, 5);
    let h1 = h1_of(&m15);
    let closed = vec![false; m15.len()];
    let mut i = inputs(&m15, &h1);
    i.entries = &closed;
    let r = evaluate_period(
        &i,
        period("2026-01-12T00:00:00Z", "2026-01-30T00:00:00Z"),
        &setup(),
        TimeDelta::days(WARMUP_DAYS),
        dec!(1),
    )
    .unwrap();
    let m = r.metrics.unwrap();
    assert_eq!(m.trades, 0);
    assert_eq!(m.total_return, Decimal::ZERO);
    assert_eq!(m.max_drawdown, Decimal::ZERO);
    assert_eq!((m.win_rate, m.profit_factor, m.sharpe), (None, None, None));
    // Comprar y mantener sí existe: la estrategia sin operaciones se compara igual.
    assert!(r.buy_and_hold.is_some());
}

#[test]
fn gaps_are_skipped_and_never_filled() {
    let mut m15 = walk(4000, 7);
    // Un hueco de dos días dentro del tramo.
    let gap_from = utc("2026-01-20T00:00:00Z");
    let gap_to = utc("2026-01-22T00:00:00Z");
    m15.retain(|c| c.start < gap_from || c.start >= gap_to);
    let h1 = h1_of(&m15);
    let r = evaluate_period(
        &inputs(&m15, &h1),
        period("2026-01-15T00:00:00Z", "2026-01-30T00:00:00Z"),
        &setup(),
        TimeDelta::days(WARMUP_DAYS),
        dec!(1),
    )
    .unwrap();
    assert_eq!(r.candles, 13 * 96, "15 días menos los 2 del hueco");
    assert!(r.metrics.is_some());
}

#[test]
fn research_never_reaches_the_holdout() {
    let s = split();
    assert!(research_period(s.windows[1].validation, &s).is_ok());
    let mut bad = s.clone();
    bad.windows[1].validation.end = utc("2026-02-12T00:00:00Z");
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let i = inputs(&m15, &h1);
    let touches = |e: EvalError| matches!(e, EvalError::TouchesHoldout { .. });
    assert!(touches(
        walk_forward(&i, &bad, &setup(), &cfg(), dec!(1)).unwrap_err()
    ));
}

#[test]
fn changing_the_holdout_never_changes_the_research_and_vice_versa() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let s = split();
    let mut c = cfg();
    c.warmup = TimeDelta::days(WARMUP_DAYS);
    // Otro fuera de muestra: desde su inicio, otra caminata.
    let mut other = m15.clone();
    let cut = other.partition_point(|x| x.start < s.out_of_sample.start);
    let tail = walk(6000, 99);
    for (k, x) in other[cut..].iter_mut().enumerate() {
        let src = &tail[cut + k];
        (x.open, x.high, x.low, x.close) = (src.open, src.high, src.low, src.close);
    }
    let h1o = h1_of(&other);
    let (a, b) = (inputs(&m15, &h1), inputs(&other, &h1o));
    assert_eq!(
        walk_forward(&a, &s, &setup(), &c, dec!(1)).unwrap(),
        walk_forward(&b, &s, &setup(), &c, dec!(1)).unwrap()
    );
    assert_ne!(
        evaluate_holdout(&a, &s, &setup(), &c, dec!(1)).unwrap(),
        evaluate_holdout(&b, &s, &setup(), &c, dec!(1)).unwrap()
    );
}

#[test]
fn the_test_window_never_feeds_its_training() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let s = split();
    let mut c = cfg();
    c.warmup = TimeDelta::days(WARMUP_DAYS);
    // Otra prueba en la primera ventana: su entrenamiento queda igual, y su prueba cambia.
    let mut other = m15.clone();
    let from = other.partition_point(|x| x.start < s.windows[0].validation.start);
    let to = other.partition_point(|x| x.start < s.windows[0].validation.end);
    for x in &mut other[from..to] {
        x.close = (x.open + x.close) / dec!(2);
        x.high = x.high.max(x.close);
        x.low = x.low.min(x.close);
    }
    let h1o = h1_of(&other);
    let p = walk_forward(&inputs(&m15, &h1), &s, &setup(), &c, dec!(1)).unwrap();
    let q = walk_forward(&inputs(&other, &h1o), &s, &setup(), &c, dec!(1)).unwrap();
    assert_eq!(p[0].train, q[0].train);
    assert_ne!(p[0].test, q[0].test);
    let tests: Vec<_> = p.iter().map(|f: &FoldResult| &f.test).collect();
    let a = aggregate(tests);
    assert_eq!((a.periods, a.with_data), (2, 2));
    assert_eq!(a.with_buy_and_hold, 2);
}

#[test]
fn the_config_is_validated() {
    for bad in [
        "[walk_forward]\nwarmup_days = 0\n",
        "[walk_forward]\nwarmup_days = 30\nextra = 1\n",
    ] {
        assert!(EvaluationConfig::from_toml(bad).is_err(), "{bad}");
    }
}

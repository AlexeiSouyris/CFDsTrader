//! Sensibilidad de la Fase 5 (#298) con series sintéticas: solo con el entrenamiento, nunca con
//! la prueba ni el fuera de muestra, parámetros que existen con su tipo, y estabilidad sin elegir.

use cfds_backtest::{
    BacktestConfig, Commission, ConfigTexts, EvalError, EvaluationConfig, FundingRates, Inputs,
    InstrumentCosts, Metrics, ParamFile, SensitivityPoint, Setup, override_toml, sensitivity,
    setup_with, stability,
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

const TEXTS: ConfigTexts = ConfigTexts {
    strategy: cfg!("strategy.toml"),
    backtest: cfg!("backtest.toml"),
    risk: cfg!("risk.toml"),
};

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
fn the_sensitivity_never_reaches_the_holdout() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let mut bad = split();
    bad.windows[0].train.end = utc("2026-02-20T00:00:00Z");
    assert!(matches!(
        sensitivity(&inputs(&m15, &h1), &bad, &setup(), &TEXTS, &cfg()).unwrap_err(),
        EvalError::TouchesHoldout { .. }
    ));
}

#[test]
fn the_test_window_never_feeds_the_sensitivity() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let s = split();
    let mut c = cfg();
    c.warmup = TimeDelta::days(WARMUP_DAYS);
    c.sensitivity.truncate(1);
    // Otra prueba en la primera ventana (y lo que sigue): el entrenamiento de esa ventana, igual.
    let mut other = m15.clone();
    let from = other.partition_point(|x| x.start < s.windows[0].validation.start);
    let to = other.partition_point(|x| x.start < s.windows[0].validation.end);
    for x in &mut other[from..to] {
        x.close = (x.open + x.close) / dec!(2);
        x.high = x.high.max(x.close);
        x.low = x.low.min(x.close);
    }
    let h1o = h1_of(&other);
    let p = sensitivity(&inputs(&m15, &h1), &s, &setup(), &TEXTS, &c).unwrap();
    let q = sensitivity(&inputs(&other, &h1o), &s, &setup(), &TEXTS, &c).unwrap();
    let fold0 = |v: &[SensitivityPoint]| -> Vec<SensitivityPoint> {
        v.iter().filter(|x| x.fold == 0).cloned().collect()
    };
    assert_eq!(fold0(&p), fold0(&q));
    assert_eq!(p.len(), 2 * c.sensitivity[0].values.len());
}

#[test]
fn every_sensitivity_parameter_of_config_exists_with_its_type() {
    let base = setup();
    for p in &cfg().sensitivity {
        // Cada valor se aplica, y los distintos del de config cambian el recorrido.
        let changed = p
            .values
            .iter()
            .filter(|v| {
                let s = setup_with(&base, &TEXTS, p, v)
                    .unwrap_or_else(|e| panic!("{} {}: {e:?}", p.file.name(), p.key));
                format!("{s:?}") != format!("{base:?}")
            })
            .count();
        assert!(
            changed + 1 >= p.values.len() && changed > 0,
            "{} {}: solo {changed} valores cambian algo",
            p.file.name(),
            p.key
        );
    }
}

#[test]
fn overrides_never_add_keys_nor_change_types() {
    let t = "[a]\nb = \"1\"\nc = 2\n";
    let s = override_toml(t, "a.b", &toml::Value::String("3".into())).unwrap();
    assert!(s.contains("b = \"3\"") && s.contains("c = 2"));
    assert!(override_toml(t, "a.x", &toml::Value::Integer(1)).is_err());
    assert!(override_toml(t, "z.b", &toml::Value::Integer(1)).is_err());
    assert!(override_toml(t, "a.c", &toml::Value::String("2".into())).is_err());
    assert!(override_toml(t, "a.b", &toml::Value::Integer(2)).is_err());
}

#[test]
fn the_config_is_validated() {
    for bad in [
        "[walk_forward]\nwarmup_days = 0\n",
        "[walk_forward]\nwarmup_days = 30\n[[sensitivity]]\nfile = \"strategy.toml\"\nkey = \"score.min_score\"\nvalues = [5]\n",
        "[walk_forward]\nwarmup_days = 30\n[[sensitivity]]\nfile = \"patterns.toml\"\nkey = \"a\"\nvalues = [1, 2]\n",
        "[walk_forward]\nwarmup_days = 30\nextra = 1\n",
    ] {
        assert!(EvaluationConfig::from_toml(bad).is_err(), "{bad}");
    }
}

fn metrics(ret: Decimal, sharpe: Option<Decimal>, trades: usize) -> Option<Metrics> {
    Some(Metrics {
        total_return: ret,
        max_drawdown: dec!(0.1),
        sharpe,
        sortino: None,
        trades,
        win_rate: None,
        profit_factor: None,
        exposure: None,
    })
}

#[test]
fn stability_measures_the_spread_and_never_picks_a_value() {
    let mut c = cfg();
    c.sensitivity.truncate(1);
    let point = |value: &str, fold, m| SensitivityPoint {
        file: ParamFile::Strategy,
        key: "score.min_score".into(),
        value: value.into(),
        fold,
        metrics: m,
    };
    let pts = vec![
        point("4", 0, metrics(dec!(0.02), Some(dec!(1)), 10)),
        point("4", 1, metrics(dec!(0.04), Some(dec!(2)), 6)),
        point("5", 0, metrics(dec!(-0.01), None, 3)),
        point("5", 1, None),
        point("6", 0, metrics(dec!(0.01), Some(dec!(0.5)), 1)),
    ];
    let s = &stability(&pts, &c)[0];
    assert_eq!(
        s.by_value,
        vec![
            ("4".into(), Some(dec!(0.03)), Some(dec!(1.5)), 16),
            ("5".into(), Some(dec!(-0.01)), None, 3),
            ("6".into(), Some(dec!(0.01)), Some(dec!(0.5)), 1),
        ]
    );
    assert_eq!(s.return_spread, Some(dec!(0.04)));
    assert!(!s.same_sign);
}

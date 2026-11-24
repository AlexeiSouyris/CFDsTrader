//! Estudio de variantes (#359) con series sintéticas: lectura de variantes, overrides
//! acumulados, bloques antes del fuera de muestra, filtros de hora y de gatillos y agregados.

use std::collections::BTreeSet;

use cfds_backtest::{
    BacktestConfig, Commission, ConfigTexts, EvalError, FundingRates, Inputs, InstrumentCosts,
    Setup, TradeStats, Variant, entry_filter, evaluate_period, evaluate_period_with,
    research_blocks, setup_for, variants_from_toml,
};
use cfds_core::{Candle, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Period, SplitConfig};
use cfds_patterns::PatternsConfig;
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_strategy::{Configs, StrategyConfig, TriggerKind};
use chrono::{DateTime, TimeDelta, Timelike, Utc};
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

const WARMUP: TimeDelta = TimeDelta::days(5);

fn texts() -> ConfigTexts<'static> {
    ConfigTexts {
        strategy: cfg!("strategy.toml"),
        backtest: cfg!("backtest.toml"),
        risk: cfg!("risk.toml"),
    }
}

fn base() -> Variant {
    variants_from_toml("[[variant]]\nname = \"base\"\n")
        .unwrap()
        .remove(0)
}

#[test]
fn variants_are_read_and_bad_ones_are_refused() {
    let v = variants_from_toml(
        r#"
[[variant]]
name = "base"

[[variant]]
name = "todo"
overrides = [
  { file = "risk.toml", key = "stop_distance.min_atr", value = "3" },
  { file = "risk.toml", key = "stop_distance.max_atr", value = "3.5" },
]
skip_entry_hours_utc = [7, 8, 10]
skip_trigger_sets = [["candle_pattern", "level_rejection"]]
"#,
    )
    .unwrap();
    assert_eq!(v.len(), 2);
    assert!(v[0].overrides.is_empty() && v[0].skip_trigger_sets.is_empty());
    assert_eq!(v[1].skip_entry_hours_utc, BTreeSet::from([7, 8, 10]));
    assert_eq!(
        v[1].skip_trigger_sets,
        vec![BTreeSet::from([
            TriggerKind::CandlePattern,
            TriggerKind::LevelRejection
        ])]
    );
    for bad in [
        "[[variant]]\nname = \"a\"\n[[variant]]\nname = \"a\"\n",
        "[[variant]]\nname = \"a\"\nskip_entry_hours_utc = [24]\n",
        "[[variant]]\nname = \"a\"\nskip_trigger_sets = [[\"martillo\"]]\n",
        "[[variant]]\nname = \"a\"\nskip_trigger_sets = [[]]\n",
        "[[variant]]\nname = \"a\"\notra = 1\n",
    ] {
        assert!(
            matches!(variants_from_toml(bad), Err(EvalError::Config(_))),
            "{bad}"
        );
    }
}

#[test]
fn overrides_of_the_same_file_accumulate_and_the_base_is_untouched() {
    let v = variants_from_toml(
        r#"
[[variant]]
name = "stop 3-3.5"
overrides = [
  { file = "risk.toml", key = "stop_distance.min_atr", value = "3" },
  { file = "risk.toml", key = "stop_distance.max_atr", value = "3.5" },
  { file = "backtest.toml", key = "exits.tp1_r", value = "2.5" },
]
"#,
    )
    .unwrap();
    let mut b = setup();
    b.backtest.kill_switch_reset_days = Some(20);
    let s = setup_for(&b, &texts(), &v[0]).unwrap();
    assert_eq!(
        (s.risk.min_stop_atr, s.risk.max_stop_atr),
        (dec!(3), dec!(3.5))
    );
    assert_eq!(s.backtest.exits.tp1_r, dec!(2.5));
    // El reinicio del kill switch que pisó la corrida se conserva.
    assert_eq!(s.backtest.kill_switch_reset_days, Some(20));
    assert_eq!(b.risk, setup().risk, "la base no cambia");
    assert_eq!(setup_for(&b, &texts(), &base()).unwrap().risk, b.risk);
    // Una sola clave que deja min ≥ max: la config lo rechaza.
    let bad = variants_from_toml(
        "[[variant]]\nname = \"x\"\noverrides = [{ file = \"risk.toml\", key = \"stop_distance.min_atr\", value = \"3\" }]\n",
    )
    .unwrap();
    assert!(setup_for(&b, &texts(), &bad[0]).is_err());
    let missing = variants_from_toml(
        "[[variant]]\nname = \"x\"\noverrides = [{ file = \"risk.toml\", key = \"stop_distance.nada\", value = \"3\" }]\n",
    )
    .unwrap();
    assert!(setup_for(&b, &texts(), &missing[0]).is_err());
}

#[test]
fn research_blocks_end_before_the_holdout() {
    let cfg = SplitConfig {
        out_of_sample_months: 12,
        train_months: 24,
        validation_months: 3,
        step_months: 3,
    };
    // Como las velas de DE40.FUT: del 11-03-2025 al 30-10-2026.
    let (blocks, oos) = research_blocks(
        utc("2025-03-11T00:15:00Z"),
        utc("2026-10-30T21:00:00Z"),
        &cfg,
    );
    assert_eq!(oos, Some(utc("2025-10-01T00:00:00Z")));
    assert_eq!(
        blocks,
        vec![
            period("2025-04-01T00:00:00Z", "2025-07-01T00:00:00Z"),
            period("2025-07-01T00:00:00Z", "2025-10-01T00:00:00Z"),
        ]
    );
    // Un primer día que empieza justo a medianoche del día 1 cuenta entero.
    let (b, _) = research_blocks(
        utc("2025-04-01T00:00:00Z"),
        utc("2026-10-30T21:00:00Z"),
        &cfg,
    );
    assert_eq!(b.len(), 2);
    // Sin lugar antes del fuera de muestra: ningún bloque.
    let (none, _) = research_blocks(
        utc("2025-09-15T00:00:00Z"),
        utc("2026-10-30T21:00:00Z"),
        &cfg,
    );
    assert!(none.is_empty());
}

#[test]
fn the_base_variant_is_exactly_the_normal_evaluation() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let p = period("2026-01-20T00:00:00Z", "2026-02-05T00:00:00Z");
    let s = setup();
    let i = inputs(&m15, &h1);
    let plain = evaluate_period(&i, p, &s, WARMUP, dec!(1)).unwrap();
    let v = base();
    let (with, report) = evaluate_period_with(&i, p, &s, WARMUP, dec!(1), |sl, e| {
        entry_filter(sl, &e, &s, &v)
    })
    .unwrap();
    assert_eq!(plain, with);
    assert_eq!(
        report.unwrap().trades.len(),
        plain.metrics.unwrap().trades,
        "el reporte es el de la corrida"
    );
}

#[test]
fn skipped_hours_never_open_and_the_filter_only_removes_entries() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let p = period("2026-01-20T00:00:00Z", "2026-02-05T00:00:00Z");
    let s = setup();
    let i = inputs(&m15, &h1);
    let (_, all) = evaluate_period_with(&i, p, &s, WARMUP, dec!(1), |_, e| Ok(e)).unwrap();
    let all = all.unwrap();
    let hours: BTreeSet<u32> = all.trades.iter().map(|t| t.entry_time.hour()).collect();
    let skip: BTreeSet<u32> = hours.iter().copied().take(3).collect();
    assert!(!skip.is_empty(), "la serie tiene que operar");
    let mut v = base();
    v.skip_entry_hours_utc = skip.clone();
    let (_, r) = evaluate_period_with(&i, p, &s, WARMUP, dec!(1), |sl, e| {
        entry_filter(sl, &e, &s, &v)
    })
    .unwrap();
    let r = r.unwrap();
    assert!(
        r.trades
            .iter()
            .all(|t| !skip.contains(&t.entry_time.hour()))
    );
    // Un filtro que devuelve marcas de menos nunca abre de más: la marca que falta es "no".
    let (_, none) =
        evaluate_period_with(&i, p, &s, WARMUP, dec!(1), |_, _| Ok(Vec::new())).unwrap();
    assert!(none.unwrap().trades.is_empty());
}

#[test]
fn a_skipped_trigger_set_removes_exactly_those_signals() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let p = period("2026-01-20T00:00:00Z", "2026-02-05T00:00:00Z");
    let s = setup();
    let i = inputs(&m15, &h1);
    let (_, all) = evaluate_period_with(&i, p, &s, WARMUP, dec!(1), |_, e| Ok(e)).unwrap();
    let all = all.unwrap();
    let set = |t: &cfds_backtest::TradeRecord| -> BTreeSet<TriggerKind> {
        t.triggers.iter().copied().collect()
    };
    let target = set(&all.trades[0]);
    let mut v = base();
    v.skip_trigger_sets = vec![target.clone()];
    let (_, r) = evaluate_period_with(&i, p, &s, WARMUP, dec!(1), |sl, e| {
        entry_filter(sl, &e, &s, &v)
    })
    .unwrap();
    let r = r.unwrap();
    assert!(r.trades.iter().all(|t| set(t) != target));
    // La primera operación de la base ya no está.
    assert!(r.trades.first().map(|t| t.signal_index) != Some(all.trades[0].signal_index));
}

#[test]
fn trade_stats_split_gross_costs_and_net() {
    let m15 = walk(6000, 11);
    let h1 = h1_of(&m15);
    let p = period("2026-01-20T00:00:00Z", "2026-02-05T00:00:00Z");
    let s = setup();
    let (_, r) =
        evaluate_period_with(&inputs(&m15, &h1), p, &s, WARMUP, dec!(1), |_, e| Ok(e)).unwrap();
    let trades = r.unwrap().trades;
    let st = TradeStats::of(&trades, dec!(1));
    assert_eq!(st.trades, trades.len());
    assert_eq!(
        st.wins + st.losses,
        trades.iter().filter(|t| !t.net_pnl.is_zero()).count()
    );
    assert_eq!(st.net, trades.iter().map(|t| t.net_pnl).sum::<Decimal>());
    assert_eq!(st.mid_pnl - st.costs, st.net);
    assert!(st.costs > Decimal::ZERO, "los costos nunca son cero");
    let mut twice = st;
    twice.add(&st);
    assert_eq!((twice.trades, twice.net), (2 * st.trades, dec!(2) * st.net));
    assert_eq!(twice.win_rate(), st.win_rate());
    assert_eq!(TradeStats::default().win_rate(), None);
    assert_eq!(TradeStats::default().payoff(), None);
}

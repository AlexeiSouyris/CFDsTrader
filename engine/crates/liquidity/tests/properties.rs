//! Tests de propiedades de patrones y liquidez (#663, plan B1 de
//! `docs/analysis/2026-11-autoanalisis.md`).
//!
//! **Sin mirar el futuro:** lo que el sistema sabe en la vela `t` tiene que ser idéntico si las
//! velas posteriores a `t` no existen. Para cada detector se compara:
//! - la salida sobre la serie entera, quedándose con lo que ya se conocía en `t`
//!   (`confirmed_at ≤ t`, `known_at ≤ t` o `index ≤ t`), con
//! - la salida sobre la serie cortada en `t`, recalculando todo lo que depende de las velas
//!   (ATR, pivotes, tendencia, rupturas), como pasaría en vivo.
//!
//! Los umbrales se aflojan respecto de `config/` para que aparezcan patrones en series cortas;
//! la propiedad no depende de los umbrales. Pocos casos para que el CI no tarde.

use cfds_core::{Candle, Timeframe};
use cfds_indicators::{TrendConfig, trend, wilder_atr};
use cfds_liquidity::{
    ImbalanceConfig, LiquidityConfig, fvg_touches_at, fvgs_up_to, imbalance_series, order_blocks,
    structure_breaks,
};
use cfds_patterns::{
    BottomConfig, FibConfig, LevelConfig, PatternsConfig, Pivot, WyckoffConfig, bottoms,
    candle_patterns_at, confirmed_pivots, double_at, fib_legs, golden_pocket_events, levels_at,
    wyckoff_events,
};
use chrono::{DateTime, TimeDelta, Utc};
use proptest::prelude::*;
use rust_decimal::{Decimal, dec};

const CASES: u32 = 40;
const PATTERNS: &str = include_str!("../../../../config/patterns.toml");
const LIQUIDITY: &str = include_str!("../../../../config/liquidity.toml");

/// Una vela en centésimos: (gap contra el cierre anterior, cuerpo, mecha superior, mecha inferior).
type Step = (i64, i64, i64, i64);

fn step() -> impl Strategy<Value = Step> {
    (
        prop_oneof![4 => Just(0i64), 1 => -3_000i64..3_000],
        -2_000i64..2_000,
        0i64..1_500,
        0i64..1_500,
    )
}

/// Velas de 15m consecutivas desde 10 000,00 (siempre positivas en 160 velas).
fn build(steps: &[Step]) -> Vec<Candle> {
    let t0: DateTime<Utc> = "2026-11-19T14:30:00Z".parse().unwrap();
    let mut prev_close = Decimal::new(1_000_000, 2);
    steps
        .iter()
        .enumerate()
        .map(|(i, &(gap, body, up, down))| {
            let open = prev_close + Decimal::new(gap, 2);
            let close = open + Decimal::new(body, 2);
            prev_close = close;
            Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * i as i64),
                open,
                high: open.max(close) + Decimal::new(up, 2),
                low: open.min(close) - Decimal::new(down, 2),
                close,
                volume: None,
            }
        })
        .collect()
}

/// Serie de velas y una vela `t` dentro de ella.
fn series_and_t() -> impl Strategy<Value = (Vec<Candle>, usize)> {
    prop::collection::vec(step(), 20..160).prop_flat_map(|steps| {
        let n = steps.len();
        (Just(build(&steps)), 0..n)
    })
}

fn patterns_cfg() -> PatternsConfig {
    let mut cfg = PatternsConfig::from_toml(PATTERNS).unwrap();
    cfg.double.min_bars_between = 3;
    cfg.double.max_difference = dec!(0.02);
    cfg.double.min_height_atr = dec!(0.3);
    cfg.levels.min_touches = 2;
    cfg.levels.tolerance = dec!(0.003);
    cfg
}

fn liquidity_cfg() -> LiquidityConfig {
    let mut cfg = LiquidityConfig::from_toml(LIQUIDITY).unwrap();
    cfg.min_impulse_body_atr = dec!(0.5);
    cfg
}

const TREND: TrendConfig = TrendConfig {
    sma_period: 10,
    slope_lookback: 3,
};

const ATR: usize = 14;

/// Todo lo que el motor deriva de las velas, recalculado sobre `candles`.
struct Derived {
    atr: Vec<Option<Decimal>>,
    pivots: Vec<Pivot>,
}

fn derive(candles: &[Candle], strength: usize) -> Derived {
    Derived {
        atr: wilder_atr(candles, ATR),
        pivots: confirmed_pivots(candles, strength),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    #[test]
    fn pivots_and_levels_do_not_look_ahead((candles, t) in series_and_t(), strength in 1usize..5) {
        let cfg = patterns_cfg();
        let full = derive(&candles, strength);
        let past = derive(&candles[..=t], strength);
        let known: Vec<Pivot> =
            full.pivots.iter().copied().filter(|p| p.confirmed_at <= t).collect();
        prop_assert_eq!(&past.pivots, &known);
        prop_assert_eq!(
            levels_at(t, &full.pivots, &cfg.levels),
            levels_at(t, &past.pivots, &cfg.levels)
        );
    }

    #[test]
    fn candle_and_double_patterns_do_not_look_ahead(
        (candles, t) in series_and_t(),
        strength in 1usize..5,
    ) {
        let cfg = patterns_cfg();
        let full = derive(&candles, strength);
        let past = derive(&candles[..=t], strength);
        prop_assert_eq!(
            candle_patterns_at(&candles, t, &full.atr, &cfg.candles),
            candle_patterns_at(&candles[..=t], t, &past.atr, &cfg.candles)
        );
        prop_assert_eq!(
            double_at(&candles, t, &full.pivots, &full.atr, &cfg.double),
            double_at(&candles[..=t], t, &past.pivots, &past.atr, &cfg.double)
        );
    }

    #[test]
    fn bottoms_wyckoff_and_fibonacci_do_not_look_ahead(
        (candles, t) in series_and_t(),
        strength in 1usize..5,
    ) {
        let full = derive(&candles, strength);
        let past = derive(&candles[..=t], strength);

        let bottom_cfg = BottomConfig {
            lookback: 5,
            min_decline: dec!(0.002),
            min_decline_atr: dec!(1),
            atr_period: 5,
            min_rebound_atr: dec!(0.5),
            max_bars_to_rebound: 6,
        };
        let known: Vec<_> = bottoms(&candles, &full.pivots, &bottom_cfg)
            .into_iter()
            .filter(|b| b.confirmed_at <= t)
            .collect();
        prop_assert_eq!(bottoms(&candles[..=t], &past.pivots, &bottom_cfg), known);

        let wyckoff_cfg = WyckoffConfig {
            levels: LevelConfig { lookback_bars: 200, min_touches: 2, tolerance: dec!(0.003) },
            max_bars_to_reclaim: 3,
            max_penetration: dec!(0.02),
        };
        let known: Vec<_> = wyckoff_events(&candles, &full.pivots, &wyckoff_cfg)
            .into_iter()
            .filter(|e| e.confirmed_at <= t)
            .collect();
        prop_assert_eq!(wyckoff_events(&candles[..=t], &past.pivots, &wyckoff_cfg), known);

        let fib_cfg = FibConfig {
            retracements: vec![dec!(0.382), dec!(0.5), dec!(0.618)],
            extensions: vec![dec!(1.272), dec!(1.618)],
            golden_pocket: (dec!(0.618), dec!(0.65)),
        };
        let full_legs = fib_legs(&full.pivots, &fib_cfg);
        let past_legs = fib_legs(&past.pivots, &fib_cfg);
        let known: Vec<_> =
            full_legs.iter().filter(|l| l.confirmed_at() <= t).cloned().collect();
        prop_assert_eq!(&past_legs, &known);
        let known: Vec<_> = golden_pocket_events(&candles, &full_legs)
            .into_iter()
            .filter(|e| e.index <= t)
            .collect();
        prop_assert_eq!(golden_pocket_events(&candles[..=t], &past_legs), known);
    }

    #[test]
    fn fair_value_gaps_do_not_look_ahead((candles, t) in series_and_t()) {
        let cfg = liquidity_cfg();
        prop_assert_eq!(fvgs_up_to(&candles, t), fvgs_up_to(&candles[..=t], t));
        prop_assert_eq!(
            fvg_touches_at(&candles, t, &cfg),
            fvg_touches_at(&candles[..=t], t, &cfg)
        );
    }

    #[test]
    fn structure_and_order_blocks_do_not_look_ahead(
        (candles, t) in series_and_t(),
        strength in 1usize..5,
    ) {
        let cfg = liquidity_cfg();
        let run = |c: &[Candle]| {
            let d = derive(c, strength);
            let breaks = structure_breaks(c, &d.pivots, &trend(c, &TREND));
            let obs = order_blocks(c, &breaks, &d.atr, &cfg);
            (breaks, obs)
        };
        let (full_breaks, full_obs) = run(&candles);
        let (past_breaks, past_obs) = run(&candles[..=t]);
        let known: Vec<_> = full_breaks.into_iter().filter(|b| b.index <= t).collect();
        prop_assert_eq!(past_breaks, known);
        let known: Vec<_> = full_obs.into_iter().filter(|o| o.known_at <= t).collect();
        prop_assert_eq!(past_obs, known);
    }

    #[test]
    fn imbalance_does_not_look_ahead((candles, t) in series_and_t(), window in 1u32..25) {
        let mut cfg = ImbalanceConfig::from_toml(LIQUIDITY).unwrap();
        cfg.window_bars = window.try_into().unwrap();
        let full = imbalance_series(&candles, ATR, &cfg);
        let past = imbalance_series(&candles[..=t], ATR, &cfg);
        let known: Vec<_> =
            full.into_iter().filter(|i| i.at <= candles[t].end()).collect();
        prop_assert_eq!(past, known);
    }
}

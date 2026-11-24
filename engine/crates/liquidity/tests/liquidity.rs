//! Sweep, FVG, rupturas de estructura y order blocks con series construidas a mano (#43).

use cfds_core::{Candle, Timeframe};
use cfds_indicators::Trend;
use cfds_liquidity::{
    BreakKind, LiquidityConfig, Side, fvg_touches_at, fvgs_up_to, order_block_touches_at,
    order_blocks, recent_choch, structure_breaks, sweeps_at,
};
use cfds_patterns::{PivotKind, confirmed_pivots};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};

fn cfg() -> LiquidityConfig {
    LiquidityConfig::from_toml(include_str!("../../../../config/liquidity.toml")).unwrap()
}

fn t0() -> DateTime<Utc> {
    "2026-11-19T14:00:00Z".parse().unwrap()
}

fn bar_tf(tf: Timeframe, i: usize, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: tf,
        start: t0() + tf.duration() * i as i32,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

fn bar(i: usize, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    bar_tf(Timeframe::M15, i, o, h, l, c)
}

fn flat(i: usize) -> Candle {
    bar(i, dec!(100), dec!(101), dec!(99), dec!(100))
}

/// El espejo exacto alrededor de 200: alcista ↔ bajista.
fn mirror(candles: &[Candle]) -> Vec<Candle> {
    let m = |x: Decimal| dec!(200) - x;
    candles
        .iter()
        .enumerate()
        .map(|(i, c)| bar(i, m(c.open), m(c.low), m(c.high), m(c.close)))
        .collect()
}

// ---------- Sweep ----------

/// 10 velas de 1h con un pivote bajo (90) en la 3; con N = 3 se confirma al cierre de la 6,
/// o sea a las t0 + 7h.
fn h1_with_low_pivot() -> Vec<Candle> {
    (0..10)
        .map(|i| {
            let l = if i == 3 { dec!(90) } else { dec!(95) };
            bar_tf(Timeframe::H1, i, dec!(96), dec!(97), l, dec!(96))
        })
        .collect()
}

/// Vela de 15m que empieza `quarters` cuartos de hora después de t0 y barre 90.
fn m15_sweep_below_90(quarters: usize) -> Candle {
    bar(quarters, dec!(92), dec!(93), dec!(89), dec!(91))
}

#[test]
fn sweep_of_a_confirmed_1h_low() {
    let h1 = h1_with_low_pivot();
    let piv = confirmed_pivots(&h1, 3);
    // 15m que cierra a las t0 + 7h15: el pivote ya estaba confirmado.
    let s = sweeps_at(&m15_sweep_below_90(28), &h1, &piv);
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].side, s[0].level), (Side::Long, dec!(90)));
    // Cierra justo a las t0 + 7h, el mismo instante en que cierra la vela de confirmación.
    assert_eq!(sweeps_at(&m15_sweep_below_90(27), &h1, &piv).len(), 1);
}

#[test]
fn a_1h_pivot_not_yet_confirmed_is_invisible() {
    let h1 = h1_with_low_pivot();
    let piv = confirmed_pivots(&h1, 3);
    // Cierra a las t0 + 6h45: la vela de 1h que confirma el pivote todavía está abierta.
    assert!(sweeps_at(&m15_sweep_below_90(26), &h1, &piv).is_empty());
}

#[test]
fn piercing_without_closing_back_inside_is_not_a_sweep() {
    let h1 = h1_with_low_pivot();
    let piv = confirmed_pivots(&h1, 3);
    let broke = bar(28, dec!(92), dec!(93), dec!(88), dec!(89.5));
    assert!(sweeps_at(&broke, &h1, &piv).is_empty());
    let no_touch = bar(28, dec!(92), dec!(93), dec!(90), dec!(91));
    assert!(sweeps_at(&no_touch, &h1, &piv).is_empty());
}

#[test]
fn sweep_of_a_1h_high_is_the_mirror() {
    let h1: Vec<Candle> = h1_with_low_pivot()
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let m = |x: Decimal| dec!(200) - x;
            bar_tf(Timeframe::H1, i, m(c.open), m(c.low), m(c.high), m(c.close))
        })
        .collect();
    let piv = confirmed_pivots(&h1, 3);
    assert_eq!(piv[0].kind, PivotKind::High);
    let c = bar(28, dec!(108), dec!(111), dec!(107), dec!(109));
    let s = sweeps_at(&c, &h1, &piv);
    assert_eq!(s.len(), 1);
    assert_eq!((s[0].side, s[0].level), (Side::Short, dec!(110)));
}

// ---------- FVG ----------

/// FVG alcista creado en la vela 10: H[8] = 101 < L[10] = 103 → zona [101, 103].
/// Desde la 11 el precio sigue arriba (L 104) y en la 15 vuelve a la zona.
fn bullish_fvg(fill_at: Option<usize>) -> Vec<Candle> {
    (0..20)
        .map(|i| match i {
            9 => bar(i, dec!(100), dec!(104), dec!(100), dec!(104)),
            10 => bar(i, dec!(104), dec!(106), dec!(103), dec!(105)),
            _ if Some(i) == fill_at => bar(i, dec!(105), dec!(105), dec!(100), dec!(100.5)),
            15 => bar(i, dec!(105), dec!(105), dec!(102), dec!(104)),
            11..=19 => bar(i, dec!(105), dec!(106), dec!(104), dec!(105)),
            _ => flat(i),
        })
        .collect()
}

#[test]
fn touch_of_an_open_bullish_fvg() {
    let f = fvg_touches_at(&bullish_fvg(None), 15, &cfg());
    assert_eq!(f.len(), 1);
    assert_eq!(
        (f[0].side, f[0].created_at, f[0].bottom, f[0].top),
        (Side::Long, 10, dec!(101), dec!(103))
    );
    // Una vela que no baja hasta la zona no la toca.
    assert!(fvg_touches_at(&bullish_fvg(None), 14, &cfg()).is_empty());
}

#[test]
fn a_filled_fvg_no_longer_counts() {
    assert!(fvg_touches_at(&bullish_fvg(Some(13)), 15, &cfg()).is_empty());
}

#[test]
fn an_old_fvg_falls_out_of_the_lookback() {
    let short = LiquidityConfig {
        fvg_lookback_bars: 5,
        ..cfg()
    };
    assert!(fvg_touches_at(&bullish_fvg(None), 15, &short).is_empty());
    let enough = LiquidityConfig {
        fvg_lookback_bars: 6,
        ..cfg()
    };
    assert_eq!(fvg_touches_at(&bullish_fvg(None), 15, &enough).len(), 1);
}

#[test]
fn bearish_fvg_is_the_mirror() {
    let f = fvg_touches_at(&mirror(&bullish_fvg(None)), 15, &cfg());
    assert_eq!(f.len(), 1);
    assert_eq!(
        (f[0].side, f[0].bottom, f[0].top),
        (Side::Short, dec!(97), dec!(99))
    );
    assert!(fvg_touches_at(&mirror(&bullish_fvg(Some(13))), 15, &cfg()).is_empty());
}

#[test]
fn the_fvg_result_at_t_does_not_depend_on_later_candles() {
    let full = bullish_fvg(Some(17));
    let cut = &full[..=15];
    assert_eq!(
        fvg_touches_at(&full, 15, &cfg()),
        fvg_touches_at(cut, 15, &cfg())
    );
}

// ---------- Estructura y order blocks ----------

/// Pivote alto de 15m en la vela 5 (H 105), confirmado en la 8. La vela 11 es bajista
/// (el futuro order block, zona [99, 101]) y la `cross` cierra en 106 con cuerpo 6.
/// Después el precio queda arriba, y en la 16 vuelve a la zona del OB.
fn breakout(cross: usize, invalidate_at: Option<usize>) -> Vec<Candle> {
    (0..24)
        .map(|i| match i {
            5 => bar(i, dec!(100), dec!(105), dec!(99), dec!(100)),
            _ if i + 1 == cross => bar(i, dec!(100.5), dec!(101), dec!(99), dec!(99.5)),
            _ if i == cross => bar(i, dec!(100), dec!(106.5), dec!(99.5), dec!(106)),
            _ if Some(i) == invalidate_at => bar(i, dec!(106), dec!(106), dec!(98), dec!(98.5)),
            16 => bar(i, dec!(103), dec!(103), dec!(100.5), dec!(101)),
            _ if i > cross => bar(i, dec!(106), dec!(107), dec!(105), dec!(106)),
            _ => flat(i),
        })
        .collect()
}

fn breaks(candles: &[Candle], trend: Trend) -> Vec<cfds_liquidity::StructureBreak> {
    let piv = confirmed_pivots(candles, 3);
    structure_breaks(candles, &piv, &vec![Some(trend); candles.len()])
}

fn long_breaks(candles: &[Candle], trend: Trend) -> Vec<cfds_liquidity::StructureBreak> {
    breaks(candles, trend)
        .into_iter()
        .filter(|b| b.side == Side::Long)
        .collect()
}

#[test]
fn break_with_the_1h_trend_is_a_bos_and_against_it_a_choch() {
    let c = breakout(12, None);
    let bos = long_breaks(&c, Trend::Up);
    assert_eq!(bos.len(), 1);
    assert_eq!(
        (bos[0].index, bos[0].kind, bos[0].level, bos[0].pivot_index),
        (12, BreakKind::Bos, dec!(105), 5)
    );
    assert_eq!(long_breaks(&c, Trend::Down)[0].kind, BreakKind::Choch);
    assert!(long_breaks(&c, Trend::Neutral).is_empty());
    // Sin tendencia calculada (calentamiento) tampoco hay ruptura clasificada.
    let piv = confirmed_pivots(&c, 3);
    assert!(structure_breaks(&c, &piv, &vec![None; c.len()]).is_empty());
}

#[test]
fn a_break_before_the_pivot_is_confirmed_does_not_count() {
    // La vela 7 cierra arriba de 105, pero el pivote de la 5 recién se confirma en la 8.
    assert!(long_breaks(&breakout(7, None), Trend::Up).is_empty());
}

#[test]
fn bearish_break_is_the_mirror() {
    let c = mirror(&breakout(12, None));
    let b: Vec<_> = breaks(&c, Trend::Down)
        .into_iter()
        .filter(|b| b.side == Side::Short)
        .collect();
    assert_eq!(b.len(), 1);
    assert_eq!(
        (b[0].index, b[0].kind, b[0].level),
        (12, BreakKind::Bos, dec!(95))
    );
}

#[test]
fn choch_is_recent_for_12_candles() {
    let b = breaks(&breakout(12, None), Trend::Down);
    for t in 12..=23 {
        assert!(recent_choch(&b, t, Side::Long, &cfg()), "t = {t}");
    }
    assert!(!recent_choch(&b, 11, Side::Long, &cfg()));
    assert!(!recent_choch(&b, 24, Side::Long, &cfg()));
    assert!(!recent_choch(&b, 15, Side::Short, &cfg()));
}

fn atr(n: usize, v: Decimal) -> Vec<Option<Decimal>> {
    vec![Some(v); n]
}

#[test]
fn order_block_is_the_last_opposite_candle_before_the_impulse() {
    let c = breakout(12, None);
    let obs = order_blocks(
        &c,
        &long_breaks(&c, Trend::Up),
        &atr(c.len(), dec!(1)),
        &cfg(),
    );
    assert_eq!(obs.len(), 1);
    assert_eq!(
        (
            obs[0].side,
            obs[0].candle_index,
            obs[0].low,
            obs[0].high,
            obs[0].known_at
        ),
        (Side::Long, 11, dec!(99), dec!(101), 12)
    );
    let touches = order_block_touches_at(&c, 16, &obs);
    assert_eq!(touches, obs);
    // La vela de la ruptura no puede "tocar" su propio OB: se conoce recién a su cierre.
    assert!(order_block_touches_at(&c, 12, &obs).is_empty());
    assert!(order_block_touches_at(&c, 15, &obs).is_empty());
}

#[test]
fn a_weak_impulse_does_not_make_an_order_block() {
    let c = breakout(12, None);
    // Cuerpo 6 < 1,5 × ATR 5 = 7,5.
    let obs = order_blocks(
        &c,
        &long_breaks(&c, Trend::Up),
        &atr(c.len(), dec!(5)),
        &cfg(),
    );
    assert!(obs.is_empty());
    // Sin ATR (calentamiento) tampoco.
    let obs = order_blocks(
        &c,
        &long_breaks(&c, Trend::Up),
        &vec![None; c.len()],
        &cfg(),
    );
    assert!(obs.is_empty());
}

#[test]
fn a_close_beyond_the_order_block_invalidates_it() {
    let c = breakout(12, Some(14));
    let obs = order_blocks(
        &c,
        &long_breaks(&c, Trend::Up),
        &atr(c.len(), dec!(1)),
        &cfg(),
    );
    assert_eq!(obs.len(), 1);
    assert!(order_block_touches_at(&c, 16, &obs).is_empty());
}

#[test]
fn bearish_order_block_is_the_mirror() {
    let c = mirror(&breakout(12, None));
    let b: Vec<_> = breaks(&c, Trend::Down)
        .into_iter()
        .filter(|b| b.side == Side::Short)
        .collect();
    let obs = order_blocks(&c, &b, &atr(c.len(), dec!(1)), &cfg());
    assert_eq!(obs.len(), 1);
    assert_eq!(
        (obs[0].side, obs[0].candle_index, obs[0].low, obs[0].high),
        (Side::Short, 11, dec!(99), dec!(101))
    );
    assert_eq!(order_block_touches_at(&c, 16, &obs).len(), 1);
    let inv = mirror(&breakout(12, Some(14)));
    let b: Vec<_> = breaks(&inv, Trend::Down)
        .into_iter()
        .filter(|b| b.side == Side::Short)
        .collect();
    let obs = order_blocks(&inv, &b, &atr(inv.len(), dec!(1)), &cfg());
    assert!(order_block_touches_at(&inv, 16, &obs).is_empty());
}

#[test]
fn breaks_and_order_blocks_do_not_repaint() {
    let full = breakout(12, None);
    for t in 12..full.len() {
        let cut = &full[..=t];
        let past = |c: &[Candle]| -> Vec<_> {
            breaks(c, Trend::Up)
                .into_iter()
                .filter(|b| b.index <= 12)
                .collect()
        };
        assert_eq!(past(cut), past(&full), "t = {t}");
    }
}

#[test]
fn an_unconfirmed_newer_pivot_does_not_hide_the_confirmed_one() {
    // Pivote A en la 5 (H 103, confirmado en la 8) y pivote B en la 12 (H 105, se confirma en
    // la 15). La 13 cierra en 104: rompe A. B todavía no existe para el sistema.
    let c: Vec<Candle> = (0..20)
        .map(|i| match i {
            5 => bar(i, dec!(100), dec!(103), dec!(99), dec!(100)),
            12 => bar(i, dec!(100), dec!(105), dec!(99), dec!(100)),
            13 => bar(i, dec!(100), dec!(104.5), dec!(99.5), dec!(104)),
            14.. => bar(i, dec!(104), dec!(104.4), dec!(103), dec!(104)),
            _ => flat(i),
        })
        .collect();
    let b = long_breaks(&c, Trend::Up);
    assert_eq!(b.len(), 1);
    assert_eq!(
        (b[0].index, b[0].level, b[0].pivot_index),
        (13, dec!(103), 5)
    );
}

// ---------- Sensibilidad (#80): el impulso mínimo del order block es monótono ----------

#[test]
fn a_lower_impulse_threshold_never_finds_fewer_order_blocks() {
    // Serie larga con tendencias alternadas (determinística).
    let mut state = 11u64;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let mut close = dec!(1000);
    let candles: Vec<Candle> = (0..2000)
        .map(|i| {
            let drift = if (i / 120) % 2 == 0 { 3 } else { -3 };
            let open = close;
            close = open + Decimal::new(next(41) - 20 + drift, 1);
            let hi = open.max(close) + Decimal::new(next(20), 1);
            let lo = open.min(close) - Decimal::new(next(20), 1);
            Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: "2026-11-19T14:30:00Z"
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .unwrap()
                    + chrono::TimeDelta::minutes(15 * i as i64),
                open,
                high: hi,
                low: lo,
                close,
                volume: None,
            }
        })
        .collect();
    let base =
        LiquidityConfig::from_toml(include_str!("../../../../config/liquidity.toml")).unwrap();
    let piv = confirmed_pivots(&candles, 3);
    let atr = cfds_indicators::wilder_atr(&candles, base.atr_period);
    let mut breaks = structure_breaks(&candles, &piv, &vec![Some(Trend::Up); candles.len()]);
    breaks.extend(structure_breaks(
        &candles,
        &piv,
        &vec![Some(Trend::Down); candles.len()],
    ));
    let found = |min: Decimal| {
        let mut c = base;
        c.min_impulse_body_atr = min;
        order_blocks(&candles, &breaks, &atr, &c)
            .into_iter()
            .map(|ob| (ob.known_at, ob.low, ob.high))
            .collect::<std::collections::BTreeSet<_>>()
    };
    let counts: Vec<usize> = [dec!(3), dec!(2), dec!(1.5), dec!(1), dec!(0.5)]
        .into_iter()
        .map(|m| found(m).len())
        .collect();
    assert!(counts.windows(2).all(|w| w[0] <= w[1]), "{counts:?}");
    assert!(counts[0] < counts[4], "el umbral tiene efecto: {counts:?}");
}

/// Serie seudoaleatoria con huecos frecuentes (para que haya FVG).
fn jumpy(n: usize, seed: u64) -> Vec<Candle> {
    let mut state = seed;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let mut close = dec!(1000);
    (0..n)
        .map(|i| {
            let open = close + Decimal::new(next(21) - 10, 1);
            close = open + Decimal::new(next(61) - 30, 1);
            let high = open.max(close) + Decimal::new(next(10), 1);
            let low = open.min(close) - Decimal::new(next(10), 1);
            bar(i, open, high, low, close)
        })
        .collect()
}

#[test]
fn fvg_touches_only_look_at_the_window_and_match_the_full_history() {
    // #390: `fvg_touches_at` ya no arma todos los FVG desde el principio; el resultado tiene que
    // ser el mismo que filtrar los de `fvgs_up_to`.
    let c = jumpy(600, 7);
    let cfg = cfg();
    let mut touched = 0;
    for t in 0..c.len() {
        let from = (t + 1).saturating_sub(cfg.fvg_lookback_bars);
        let x = &c[t];
        let expected: Vec<_> = fvgs_up_to(&c, t)
            .into_iter()
            .filter(|f| f.created_at < t && f.created_at >= from)
            .filter(|f| match f.side {
                Side::Long => x.low <= f.top && x.close >= f.bottom,
                Side::Short => x.high >= f.bottom && x.close <= f.top,
            })
            .collect();
        touched += expected.len();
        assert_eq!(fvg_touches_at(&c, t, &cfg), expected, "t = {t}");
    }
    assert!(touched > 0, "la serie tiene que tocar algún FVG");
}

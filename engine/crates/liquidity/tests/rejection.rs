//! Rechazo de la spec §9.1 (#220): reúne martillo/estrella fugaz con niveles, FVG, order blocks
//! y sweeps. Sin red ni servicios.

use cfds_core::{Candle, Timeframe};
use cfds_indicators::wilder_atr;
use cfds_liquidity::{
    LiquidityConfig, OrderBlock, Rejection, RejectionInputs, RejectionVia, Side, rejection_state_at,
};
use cfds_patterns::{PatternsConfig, Pivot, PivotKind};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const PATTERNS: &str = include_str!("../../../../config/patterns.toml");
const LIQUIDITY: &str = include_str!("../../../../config/liquidity.toml");

fn candle(i: usize, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    let t0: DateTime<Utc> = "2026-11-16T14:00:00Z".parse().unwrap();
    Candle {
        instrument_id: "US500".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: t0 + TimeDelta::minutes(15 * i as i64),
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

/// 19 velas que bajan de a 1 (de 119 a 101, rango ±1) y en t = 19 la vela `last`.
fn falling_then(last: (Decimal, Decimal, Decimal, Decimal)) -> Vec<Candle> {
    let mut v: Vec<Candle> = (0..19)
        .map(|i| {
            let c = Decimal::from(119 - i as i64);
            candle(i, c, c + Decimal::ONE, c - Decimal::ONE, c)
        })
        .collect();
    let (o, h, l, c) = last;
    v.push(candle(19, o, h, l, c));
    v
}

fn low_pivot(index: usize, price: Decimal) -> Pivot {
    Pivot {
        kind: PivotKind::Low,
        index,
        confirmed_at: index + 3,
        price,
    }
}

fn cfgs() -> (PatternsConfig, LiquidityConfig) {
    (
        PatternsConfig::from_toml(PATTERNS).unwrap(),
        LiquidityConfig::from_toml(LIQUIDITY).unwrap(),
    )
}

fn at(
    candles: &[Candle],
    pivots: &[Pivot],
    obs: &[OrderBlock],
    h1_pivots: &[Pivot],
) -> Vec<Rejection> {
    let (p, l) = cfgs();
    let atr = wilder_atr(candles, p.candles.atr_period);
    let inp = RejectionInputs {
        candles,
        atr: &atr,
        pivots,
        order_blocks: obs,
        h1: candles,
        h1_pivots,
    };
    rejection_state_at(&inp, candles.len() - 1, &p, &l)
}

/// Martillo: cuerpo 1, mecha inferior 5, mecha superior 0,2, después de la caída.
fn hammer() -> Vec<Candle> {
    falling_then((dec!(100), dec!(101.2), dec!(95), dec!(101)))
}

#[test]
fn a_hammer_alone_is_not_a_rejection() {
    assert_eq!(at(&hammer(), &[], &[], &[]), []);
}

#[test]
fn a_hammer_whose_wick_touches_a_support_or_an_order_block() {
    let support = [
        low_pivot(2, dec!(96)),
        low_pivot(5, dec!(96.05)),
        low_pivot(8, dec!(95.98)),
    ];
    let long = |via| Rejection {
        side: Side::Long,
        via,
    };
    assert_eq!(
        at(&hammer(), &support, &[], &[]),
        [long(RejectionVia::Level)]
    );
    let ob = OrderBlock {
        side: Side::Long,
        candle_index: 4,
        low: dec!(94),
        high: dec!(97),
        known_at: 10,
    };
    assert_eq!(
        at(&hammer(), &support, &[ob], &[]),
        [long(RejectionVia::Level), long(RejectionVia::OrderBlock)]
    );
    // Un order block bajista no cuenta para un martillo.
    let short_ob = OrderBlock {
        side: Side::Short,
        ..ob
    };
    assert_eq!(at(&hammer(), &[], &[short_ob], &[]), []);
}

#[test]
fn a_sweep_that_closes_back_inside_is_a_rejection_without_a_candle_pattern() {
    // Vela común (sin martillo): pincha el pivote bajo de 96 y cierra arriba.
    let c = falling_then((dec!(99), dec!(100), dec!(95.5), dec!(97)));
    let pivot = [low_pivot(10, dec!(96))];
    assert_eq!(
        at(&c, &[], &[], &pivot),
        [Rejection {
            side: Side::Long,
            via: RejectionVia::Sweep
        }]
    );
    // Sin cerrar de vuelta adentro no hay rechazo.
    let through = falling_then((dec!(99), dec!(100), dec!(95), dec!(95.5)));
    assert_eq!(at(&through, &[], &[], &pivot), []);
}

#[test]
fn the_mirror_shooting_star_at_a_resistance() {
    // Sube de a 1 (de 80 a 98) y en t = 19 una estrella fugaz que pincha la resistencia de 104.
    let mut v: Vec<Candle> = (0..19)
        .map(|i| {
            let c = Decimal::from(80 + i as i64);
            candle(i, c, c + Decimal::ONE, c - Decimal::ONE, c)
        })
        .collect();
    v.push(candle(19, dec!(99), dec!(105), dec!(97.8), dec!(98)));
    let resistance: Vec<Pivot> = [2, 5, 8]
        .map(|i| Pivot {
            kind: PivotKind::High,
            index: i,
            confirmed_at: i + 3,
            price: dec!(104),
        })
        .to_vec();
    assert_eq!(
        at(&v, &resistance, &[], &[]),
        [Rejection {
            side: Side::Short,
            via: RejectionVia::Level
        }]
    );
}

#[test]
fn never_repaints() {
    let full = hammer();
    let support = [
        low_pivot(2, dec!(96)),
        low_pivot(5, dec!(96.05)),
        low_pivot(8, dec!(95.98)),
    ];
    let (p, l) = cfgs();
    let atr = wilder_atr(&full, p.candles.atr_period);
    for t in 0..full.len() {
        let inp = |candles: &'_ [Candle], atr: &'_ [Option<Decimal>]| {
            rejection_state_at(
                &RejectionInputs {
                    candles,
                    atr,
                    pivots: &support,
                    order_blocks: &[],
                    h1: candles,
                    h1_pivots: &support,
                },
                t,
                &p,
                &l,
            )
        };
        assert_eq!(inp(&full[..=t], &atr[..=t]), inp(&full, &atr));
    }
}

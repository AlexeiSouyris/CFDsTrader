//! Programa 5 (#649): eventos reales de FVG (filtros F1 a F3) y etiqueta "F" de las operaciones
//! (`docs/analysis/2026-11-programa-5-fvg-preregistro.md` §3 y §5.2). Series sintéticas; nunca
//! la base real.

use cfds_backtest::research::program5::{is_fvg_trade, real_events, touch_closes};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_liquidity::{LiquidityConfig, StatsConfig, fvg_events};
use cfds_strategy::Side;
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;

const LIQUIDITY: &str = include_str!("../../../../config/liquidity.toml");

fn t0() -> DateTime<Utc> {
    "2020-01-06T00:00:00Z".parse().unwrap()
}

fn bar(k: i64, h: i64, l: i64, c: i64) -> Candle {
    // Precios en décimas.
    let d = |x: i64| Decimal::new(x, 1);
    Candle {
        instrument_id: "US100".parse::<InstrumentId>().unwrap(),
        timeframe: Timeframe::M15,
        start: t0() + TimeDelta::minutes(15 * k),
        open: d(c),
        high: d(h),
        low: d(l),
        close: d(c),
        volume: None,
    }
}

/// 20 velas planas, un FVG alcista creado en la vela 22 (zona [101, 102]) y su primer toque en
/// la 25 (`touch_close` es el cierre de esa vela, en décimas); `n` velas en total.
fn series(n: i64, touch_close: i64) -> Vec<Candle> {
    let mut v: Vec<Candle> = (0..20).map(|k| bar(k, 1005, 995, 1000)).collect();
    v.push(bar(20, 1010, 996, 1008));
    v.push(bar(21, 1060, 1004, 1050));
    v.push(bar(22, 1080, 1020, 1070));
    v.push(bar(23, 1080, 1030, 1060));
    v.push(bar(24, 1080, 1030, 1060));
    v.push(bar(25, 1040, 1015.min(touch_close), touch_close));
    for k in 26..n {
        v.push(bar(k, 1060, 1030, 1050));
    }
    v
}

fn events(bars: &[Candle]) -> Vec<cfds_core::FvgEvent> {
    let atr = LiquidityConfig::from_toml(LIQUIDITY).unwrap().atr_period;
    fvg_events(bars, atr, &StatsConfig::from_toml(LIQUIDITY).unwrap())
}

fn far() -> DateTime<Utc> {
    t0() + TimeDelta::days(30)
}

#[test]
fn a_clean_first_touch_is_a_real_event() {
    let bars = series(40, 1030);
    let contracts = vec![1; bars.len()];
    let (ev, n) = real_events(&bars, &contracts, &events(&bars), (t0(), far()), 8);
    assert_eq!((n.in_period, n.same_contract, n.real), (1, 1, 1));
    let e = &ev[0];
    assert_eq!(
        (e.side, e.created, e.touch, e.wait()),
        (Side::Long, 22, 25, 3)
    );
    assert_eq!((e.bottom, e.top), (Decimal::from(101), Decimal::from(102)));
    // Llegada desde el cierre de la vela 24 (106) hasta el techo (102), en ATR[i].
    assert_eq!(e.arrival_atr(&bars), Some(Decimal::from(4) / e.atr));
    // Las reacciones son las del motor, con signo a favor.
    assert_eq!(
        e.reaction(8),
        Some((Decimal::from(105) - Decimal::from(103)) / e.atr)
    );
    assert!(e.reaction(4).is_some());
}

#[test]
fn a_contract_change_up_to_the_reaction_excludes_the_event() {
    let bars = series(40, 1030);
    let mut contracts = vec![1; bars.len()];
    // Cambio en la vela 33 = t + 8: todavía cuenta.
    for c in contracts.iter_mut().skip(33) {
        *c = 2;
    }
    let (ev, n) = real_events(&bars, &contracts, &events(&bars), (t0(), far()), 8);
    assert_eq!((n.in_period, n.same_contract, n.real), (1, 0, 0));
    assert!(ev.is_empty());
    // Cambio en la vela 34: ya no toca la ventana i − 2 … t + 8.
    let mut contracts = vec![1; bars.len()];
    for c in contracts.iter_mut().skip(34) {
        *c = 2;
    }
    let (_, n) = real_events(&bars, &contracts, &events(&bars), (t0(), far()), 8);
    assert_eq!(n.real, 1);
}

#[test]
fn a_touch_that_fills_in_the_same_candle_is_not_a_real_event() {
    let bars = series(40, 1009);
    let contracts = vec![1; bars.len()];
    let (ev, n) = real_events(&bars, &contracts, &events(&bars), (t0(), far()), 8);
    assert_eq!((n.in_period, n.same_contract, n.real), (1, 1, 0));
    assert!(ev.is_empty());
}

#[test]
fn the_touch_and_its_reaction_must_fit_in_the_period() {
    let bars = series(40, 1030);
    let contracts = vec![1; bars.len()];
    let all = events(&bars);
    // t + 8 = vela 33: tiene que empezar antes de `to`.
    let (_, n) = real_events(&bars, &contracts, &all, (t0(), bars[33].start), 8);
    assert_eq!(n.in_period, 0);
    let (_, n) = real_events(&bars, &contracts, &all, (t0(), bars[34].start), 8);
    assert_eq!(n.in_period, 1);
    // Toque antes de `from`.
    let (_, n) = real_events(&bars, &contracts, &all, (bars[26].start, far()), 8);
    assert_eq!(n.in_period, 0);
    // Sin las 8 velas cargadas, no hay evento.
    let short = series(33, 1030);
    let (_, n) = real_events(&short, &vec![1; 33], &events(&short), (t0(), far()), 8);
    assert_eq!(n.in_period, 0);
}

#[test]
fn events_do_not_repaint_when_more_candles_arrive() {
    let a = series(40, 1030);
    let b = series(80, 1030);
    let (ea, _) = real_events(&a, &vec![1; 40], &events(&a), (t0(), far()), 8);
    let (eb, _) = real_events(&b, &vec![1; 80], &events(&b), (t0(), far()), 8);
    assert_eq!(ea, eb);
}

#[test]
fn missing_candles_are_not_filled_in() {
    // Sin la vela 24: el toque queda 2 velas después de la creación (se cuentan las presentes).
    let mut bars = series(40, 1030);
    bars.remove(24);
    let contracts = vec![1; bars.len()];
    let (ev, _) = real_events(&bars, &contracts, &events(&bars), (t0(), far()), 8);
    assert_eq!((ev[0].created, ev[0].touch, ev[0].wait()), (22, 24, 2));
}

#[test]
fn a_trade_is_f_only_on_the_touch_candle_and_in_the_same_direction() {
    let bars = series(40, 1030);
    let contracts = vec![1; bars.len()];
    let (ev, _) = real_events(&bars, &contracts, &events(&bars), (t0(), far()), 8);
    let touches = touch_closes(&bars, &ev);
    assert!(is_fvg_trade(&touches, bars[25].end(), Side::Long));
    assert!(!is_fvg_trade(&touches, bars[25].end(), Side::Short));
    assert!(!is_fvg_trade(&touches, bars[26].end(), Side::Long));
    assert!(!is_fvg_trade(&touches, bars[24].end(), Side::Long));
}

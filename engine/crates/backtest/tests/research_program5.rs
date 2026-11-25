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

#[test]
fn the_frozen_program5_config_is_parsed() {
    let c = cfds_backtest::research::ResearchConfig::from_toml(include_str!(
        "../../../../config/research.toml"
    ))
    .unwrap();
    let (r, p) = (c.rule_program5, c.program5);
    assert_eq!(r.alpha, Decimal::new(25, 3));
    assert_eq!(r.dsr_trials, 361);
    assert_eq!(r.cost_stress, Decimal::new(15, 1));
    assert_eq!(r.second_half_from.to_string(), "2021-01-01");
    assert_eq!(p.instruments, ["US100", "US500"]);
    assert_eq!(
        (p.timeframe.as_str(), p.descriptive_timeframe.as_str()),
        ("15m", "1h")
    );
    assert_eq!((p.horizon, p.consistency_horizon), (8, 4));
    assert_eq!((p.placebos_per_event, p.newey_west_lags), (5, 3));
    assert_eq!((p.min_events, p.min_f_trades), (1000, 100));
    assert_eq!((p.luck_simulations, p.luck_seed), (1000, 20261125));
    assert_eq!(p.out_of_session_spread["US100"], Decimal::new(25, 1));
    assert_eq!(p.out_of_session_spread["US500"], Decimal::new(16, 1));
}

// ── Placebo (§4.1, §13.3.3) y sorteo (§13.3.4) ──

use cfds_backtest::research::program4::SplitMix64;
use cfds_backtest::research::program5::{
    PlaceboInputs, RealEvent, draw_placebos, placebo_candidates, placebo_reaction,
    sample_without_replacement,
};

/// 60 velas planas (H 101, L 99, C 100) con bajadas (L 97, C 99) y subidas (H 103, C 101) en
/// las velas indicadas.
fn flat(dips: &[i64], spikes: &[i64]) -> Vec<Candle> {
    (0..60)
        .map(|k| {
            if dips.contains(&k) {
                bar(k, 1000, 970, 990)
            } else if spikes.contains(&k) {
                bar(k, 1030, 1000, 1010)
            } else {
                bar(k, 1010, 990, 1000)
            }
        })
        .collect()
}

/// Evento con espera 3, llegada 1 ATR y tamaño 1 ATR (ATR 2), con toque en `t`.
fn event(side: Side, t: usize) -> RealEvent {
    let (bottom, top) = match side {
        Side::Long => (Decimal::from(96), Decimal::from(98)),
        Side::Short => (Decimal::from(102), Decimal::from(104)),
    };
    RealEvent {
        side,
        created: t - 3,
        touch: t,
        bottom,
        top,
        atr: Decimal::TWO,
        size_atr: Decimal::ONE,
        reactions: Vec::new(),
    }
}

fn candidates(bars: &[Candle], contracts: &[u64], events: &[RealEvent]) -> Vec<Vec<u32>> {
    let atr = vec![Some(Decimal::TWO); bars.len()];
    let cash = vec![false; bars.len()];
    let p = placebo_candidates(
        &PlaceboInputs {
            bars,
            contracts,
            atr: &atr,
            cash: &cash,
            to: far(),
            horizon: 8,
        },
        events,
    );
    (0..events.len()).map(|e| p.of(e).to_vec()).collect()
}

#[test]
fn placebos_touch_a_level_at_the_same_distance_for_the_first_time() {
    // Bajadas en 30, 40 y 41: 41 no es primer toque (la 40 ya llegó al nivel).
    let bars = flat(&[30, 40, 41], &[]);
    let c = candidates(&bars, &vec![1; 60], &[event(Side::Long, 50)]);
    assert_eq!(c[0], vec![30, 40]);
}

#[test]
fn a_real_event_of_the_same_direction_is_not_a_placebo() {
    let bars = flat(&[30, 40], &[]);
    let c = candidates(
        &bars,
        &vec![1; 60],
        &[event(Side::Long, 30), event(Side::Short, 40)],
    );
    // La 30 es toque real alcista: no es placebo del alcista. La 40 es toque real bajista, pero
    // el placebo alcista no la excluye.
    assert_eq!(c[0], vec![40]);
}

#[test]
fn placebos_need_the_same_contract_and_their_reaction_candles() {
    let bars = flat(&[30, 40], &[]);
    let mut contracts = vec![1; 60];
    for c in contracts.iter_mut().skip(45) {
        *c = 2;
    }
    // La 40 cruza el vencimiento en 40 + 5; la 30 no (30 + 8 = 38).
    let c = candidates(&bars, &contracts, &[event(Side::Long, 50)]);
    assert_eq!(c[0], vec![30]);
    // Sin 8 velas después, no hay placebo.
    let bars = flat(&[30, 55], &[]);
    let c = candidates(&bars, &vec![1; 60], &[event(Side::Long, 50)]);
    assert_eq!(c[0], vec![30]);
}

#[test]
fn bearish_placebos_are_the_mirror() {
    let bars = flat(&[], &[20, 33]);
    let c = candidates(&bars, &vec![1; 60], &[event(Side::Short, 50)]);
    assert_eq!(c[0], vec![20, 33]);
    assert!(candidates(&bars, &vec![1; 60], &[event(Side::Long, 50)])[0].is_empty());
}

#[test]
fn placebo_reactions_use_the_atr_before_the_touch_and_the_event_side() {
    let bars = flat(&[30], &[38]);
    let atr = vec![Some(Decimal::TWO); 60];
    // C[38] = 101, C[30] = 99: +2 / 2.
    assert_eq!(
        placebo_reaction(&bars, &atr, 30, 8, Side::Long),
        Some(Decimal::ONE)
    );
    assert_eq!(
        placebo_reaction(&bars, &atr, 30, 8, Side::Short),
        Some(Decimal::NEGATIVE_ONE)
    );
    assert_eq!(placebo_reaction(&bars, &atr, 55, 8, Side::Long), None);
}

#[test]
fn draws_are_reproducible_and_skip_events_without_candidates() {
    let bars = flat(&[30, 40], &[]);
    let atr = vec![Some(Decimal::TWO); 60];
    let cash = vec![false; 60];
    let contracts = vec![1; 60];
    let inp = PlaceboInputs {
        bars: &bars,
        contracts: &contracts,
        atr: &atr,
        cash: &cash,
        to: far(),
        horizon: 8,
    };
    let events = [event(Side::Short, 50), event(Side::Long, 50)];
    let p = placebo_candidates(&inp, &events);
    let mut a = SplitMix64::new(20261125);
    let mut b = SplitMix64::new(20261125);
    let da = draw_placebos(&mut a, &p, 5);
    let db = draw_placebos(&mut b, &p, 5);
    assert_eq!(da, db);
    assert!(da[0].is_empty());
    assert_eq!(da[1].len(), 5);
    assert!(da[1].iter().all(|s| [30, 40].contains(s)));
    // El evento sin candidatas no consumió números: el siguiente número es el sexto del flujo.
    let mut c = SplitMix64::new(20261125);
    for _ in 0..5 {
        c.next_u64();
    }
    assert_eq!(a.next_u64(), c.next_u64());
}

#[test]
fn samples_without_replacement_are_distinct() {
    let mut r = SplitMix64::new(1);
    let s = sample_without_replacement(&mut r, 10, 4);
    let mut d = s.clone();
    d.sort();
    d.dedup();
    assert_eq!((s.len(), d.len()), (4, 4));
    assert!(s.iter().all(|x| *x < 10));
    assert_eq!(sample_without_replacement(&mut r, 3, 5).len(), 3);
}

#[test]
fn newey_west_and_welch_match_hand_calculations() {
    use cfds_backtest::research::stats::{newey_west_t_greater, student_t_upper, welch_greater};
    // x = 1, 2, 3, 4: γ0 = 1,25, γ1 = 0,3125; con 1 rezago S = 1,25 + 2 · ½ · 0,3125 = 1,5625.
    let t = newey_west_t_greater(&[1.0, 2.0, 3.0, 4.0], 1).unwrap();
    assert!((t.std_dev - 1.25).abs() < 1e-12);
    assert!((t.t - 4.0).abs() < 1e-12);
    assert!((t.p_value - student_t_upper(4.0, 3.0)).abs() < 1e-12);
    // Sin rezagos: S = γ0 = 1,25 → t = 2,5 / √(1,25 / 4).
    let t0 = newey_west_t_greater(&[1.0, 2.0, 3.0, 4.0], 0).unwrap();
    assert!((t0.t - 2.5 / (1.25f64 / 4.0).sqrt()).abs() < 1e-12);
    assert!(newey_west_t_greater(&[1.0], 3).is_none());
    // a = 1, 2, 3 (media 2, var 1); b = 0, 0, 1, 1 (media 0,5, var 1/3):
    // ee² = 1/3 + 1/12 = 5/12, t = 1,5 / √(5/12), gl = (5/12)² / ((1/3)²/2 + (1/12)²/3) = 3.
    let w = welch_greater(&[1.0, 2.0, 3.0], &[0.0, 0.0, 1.0, 1.0]).unwrap();
    assert!((w.diff - 1.5).abs() < 1e-12);
    assert!((w.t - 1.5 / (5.0f64 / 12.0).sqrt()).abs() < 1e-12);
    assert!((w.df - 3.0).abs() < 1e-9);
    assert!((w.p_value - student_t_upper(w.t, 3.0)).abs() < 1e-12);
}

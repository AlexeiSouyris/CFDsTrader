//! Programa 4 (#561): estrategia A de Bruni (`docs/analysis/2026-11-programa-4-preregistro.md`
//! §3 a §5 y §13). Series sintéticas sobre el calendario real de XNYS; nunca la base real.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cfds_backtest::research::program4::{
    BodyThreshold, SplitMix64, Structure, body_atr_sample, direction, equivalent_k, impulse_plans,
    is_valid, luck_test, r_summary, release_days, roll_days, segment_atr, structure,
};
use cfds_backtest::research::{Plan, ResearchConfig, StudyExit, run_plans, simulate_leg};
use cfds_backtest::{BacktestConfig, Setup, apply_funding_scenario, setup_from_catalog};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::TrendConfig as IndicatorTrend;
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

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn us100() -> InstrumentId {
    "US100".parse().unwrap()
}

fn research() -> ResearchConfig {
    ResearchConfig::from_toml(cfg!("research.toml")).unwrap()
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
            trend: IndicatorTrend::from_toml(cfg!("indicators.toml")).unwrap(),
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
        },
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        dec!(100000),
    )
    .unwrap()
}

fn c(start: DateTime<Utc>, o: Decimal, h: Decimal, l: Decimal, cl: Decimal) -> Candle {
    Candle {
        instrument_id: us100(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: h,
        low: l,
        close: cl,
        volume: None,
    }
}

/// Velas seguidas de 15m desde `start` con (apertura, máximo, mínimo, cierre).
fn seq(start: &str, ohlc: &[(Decimal, Decimal, Decimal, Decimal)]) -> Vec<Candle> {
    ohlc.iter()
        .enumerate()
        .map(|(k, &(o, h, l, cl))| c(at(start) + TimeDelta::minutes(15 * k as i64), o, h, l, cl))
        .collect()
}

fn flat(p: Decimal) -> (Decimal, Decimal, Decimal, Decimal) {
    (p, p + dec!(0.25), p - dec!(0.25), p)
}

const BEAR: (Decimal, Decimal, Decimal, Decimal) =
    (dec!(1000), dec!(1000.5), dec!(994.5), dec!(995));
const BEAR2: (Decimal, Decimal, Decimal, Decimal) =
    (dec!(1003), dec!(1003.2), dec!(998.8), dec!(999));

/// Miércoles 10-01-2024 (EST: sesión 14:30–21:00 UTC). Seis velas planas desde las 11:30 UTC
/// (calentamiento del ATR), dos rupturas alcistas (13:00–15:00), una pausa, el impulso alcista
/// en 15:30 y 15:45 UTC (`t − 1` y `t`, índices 16 y 17) y velas planas hasta el cierre.
fn day_with_impulse() -> Vec<Candle> {
    let mut v = vec![flat(dec!(1000)); 6];
    v.push((dec!(1000), dec!(1002), dec!(998), dec!(1000))); // 13:00 zona (doji)
    v.extend([BEAR, BEAR, BEAR]); // 3 bajistas válidas
    v.push((dec!(995), dec!(1003.5), dec!(994.8), dec!(1003))); // 14:00 ruptura alcista 1
    v.extend([BEAR2, BEAR2, BEAR2]);
    v.push((dec!(999), dec!(1004.2), dec!(998.9), dec!(1004))); // 15:00 ruptura alcista 2
    v.push(flat(dec!(1004))); // 15:15
    v.push((dec!(1004), dec!(1020.5), dec!(1003.5), dec!(1020))); // 15:30 t−1, cuerpo 16
    v.push((dec!(1020), dec!(1036.5), dec!(1019.5), dec!(1036))); // 15:45 t, cuerpo 16
    while v.len() < 6 + 4 * 8 + 2 {
        v.push(flat(dec!(1036))); // hasta las 21:00 UTC
    }
    seq("2024-01-10T11:30:00Z", &v)
}

#[test]
fn a_valid_candle_has_a_body_larger_than_its_longest_wick() {
    let t = at("2024-01-10T15:00:00Z");
    assert!(is_valid(&c(t, dec!(10), dec!(12.9), dec!(9), dec!(12))));
    // Cuerpo 2 = mecha inferior 2: no es válida (estrictamente mayor).
    assert!(!is_valid(&c(t, dec!(10), dec!(12), dec!(8), dec!(12))));
    let doji = c(t, dec!(10), dec!(11), dec!(9), dec!(10));
    assert!(!is_valid(&doji));
    assert_eq!(direction(&doji), None);
    assert_eq!(
        direction(&c(t, dec!(10), dec!(10), dec!(8), dec!(9))),
        Some(Side::Short)
    );
}

#[test]
fn a_zone_breaks_only_with_three_valid_opposite_candles_and_a_close_outside() {
    // Mecha arriba del techo sin cierre afuera: no rompe; con 2 bajistas válidas tampoco.
    let m15 = seq(
        "2024-01-10T13:00:00Z",
        &[
            (dec!(1000), dec!(1002), dec!(998), dec!(1000)),
            BEAR,
            BEAR,
            (dec!(995), dec!(1010), dec!(994.8), dec!(1001.9)), // mecha a 1010, cierre adentro
            (dec!(1001), dec!(1012), dec!(1000.8), dec!(1011)), // cierre afuera, solo 2 bajistas
        ],
    );
    let (b, _) = structure(&m15, &[], 3);
    assert!(b.is_empty());
    // Con la tercera bajista válida, el cierre afuera rompe al alza.
    let m15 = seq(
        "2024-01-10T13:00:00Z",
        &[
            (dec!(1000), dec!(1002), dec!(998), dec!(1000)),
            BEAR,
            BEAR,
            BEAR,
            (dec!(995), dec!(1003.5), dec!(994.8), dec!(1003)),
        ],
    );
    let (b, s) = structure(&m15, &[], 3);
    assert_eq!(b.len(), 1);
    assert_eq!((b[0].index, b[0].side), (4, Side::Long));
    assert_eq!((b[0].high, b[0].low), (dec!(1002), dec!(994.5)));
    assert_eq!(
        s[4],
        Structure {
            current: Some(Side::Long),
            peripheral: None
        }
    );
    assert_eq!(s[3], Structure::default());
}

#[test]
fn the_structure_needs_two_breakouts_and_restarts_at_a_roll() {
    let m15 = day_with_impulse();
    let (b, s) = structure(&m15, &[], 3);
    assert_eq!(b.iter().map(|x| x.index).collect::<Vec<_>>(), vec![10, 14]);
    assert!(s[14].aligned(Side::Long));
    assert!(!s[13].aligned(Side::Long));
    // Un vencimiento entre las dos rupturas borra la primera.
    let (_, s) = structure(&m15, &[m15[12].start], 3);
    assert!(!s[17].aligned(Side::Long));
    assert_eq!(s[17].peripheral, None);
}

#[test]
fn nothing_repaints_when_new_candles_arrive() {
    let m15 = day_with_impulse();
    let rolls = [m15[30].start];
    let (_, full) = structure(&m15, &rolls, 3);
    let atr = segment_atr(&m15, &rolls, 14);
    for n in 1..m15.len() {
        let (_, part) = structure(&m15[..n], &rolls, 3);
        assert_eq!(part[..], full[..n]);
        assert_eq!(segment_atr(&m15[..n], &rolls, 14)[..], atr[..n]);
    }
}

#[test]
fn the_atr_warms_up_again_after_a_roll() {
    let m15 = day_with_impulse();
    let atr = segment_atr(&m15, &[m15[20].start], 14);
    assert!(atr[13].is_some() && atr[19].is_some());
    assert!(atr[20..33].iter().all(Option::is_none));
    assert!(atr[33].is_some());
}

#[test]
fn the_roll_day_is_the_first_session_of_the_new_contract() {
    let s = session();
    // NQ: el contrato nuevo empieza el domingo 17-03-2024 a las 22:00 UTC → lunes 18.
    let days = roll_days(&s, &[at("2024-03-17T22:00:00Z")]);
    assert_eq!(days, BTreeSet::from([day("2024-03-18")]));
    // FOMC a las 18:00 UTC (14:00 ET) cae en la sesión; un dato a las 13:30 UTC no.
    let days = release_days(
        &s,
        &[at("2024-03-20T18:00:00Z"), at("2024-03-12T12:30:00Z")],
    );
    assert_eq!(days, BTreeSet::from([day("2024-03-20")]));
}

fn plans_for(m15: &[Candle], excluded: &BTreeSet<NaiveDate>, th: BodyThreshold) -> Vec<Plan> {
    let cfg = research().program4;
    let atr = segment_atr(m15, &[], cfg.atr_period);
    let (_, states) = structure(m15, &[], cfg.min_valid_in_zone);
    let entries = vec![true; m15.len()];
    impulse_plans(m15, &session(), &entries, &atr, &states, excluded, th, &cfg)
}

#[test]
fn two_valid_impulse_candles_with_aligned_structure_give_one_long() {
    let m15 = day_with_impulse();
    let plans = plans_for(&m15, &BTreeSet::new(), BodyThreshold::Points(dec!(15)));
    assert_eq!(plans.len(), 1);
    let p = &plans[0];
    assert_eq!((p.signal_index, p.side), (17, Side::Long));
    assert_eq!((p.reference, p.stop), (dec!(1036), dec!(1003.5)));
    assert_eq!(p.target_r, Some(dec!(1.5)));
    // Salida al cierre de la vela de las 20:30 UTC (la apertura de la última, 20:45).
    assert_eq!(m15[p.exit_index].start, at("2024-01-10T20:30:00Z"));
    // Objetivo desde la entrada real: 1036 + 1,5 × 32,5.
    assert_eq!(p.target_from(dec!(1036)), Some(dec!(1084.75)));
}

#[test]
fn a_smaller_body_a_gap_or_an_excluded_day_give_no_signal() {
    let m15 = day_with_impulse();
    assert!(plans_for(&m15, &BTreeSet::new(), BodyThreshold::Points(dec!(16.5))).is_empty());
    assert!(
        plans_for(
            &m15,
            &BTreeSet::from([day("2024-01-10")]),
            BodyThreshold::Points(dec!(15))
        )
        .is_empty()
    );
    // Falta la vela t − 1: t ya no tiene una anterior contigua.
    let mut gap = m15.clone();
    gap.remove(16);
    assert!(plans_for(&gap, &BTreeSet::new(), BodyThreshold::Points(dec!(15))).is_empty());
    // Sin la vela de ejecución (dato faltante): no hay señal.
    let mut missing = m15.clone();
    missing.remove(18);
    assert!(plans_for(&missing, &BTreeSet::new(), BodyThreshold::Points(dec!(15))).is_empty());
    // Velas del impulso no válidas (mecha más larga que el cuerpo): no hay señal.
    let mut wick = m15.clone();
    wick[16].high = dec!(1040);
    assert!(plans_for(&wick, &BTreeSet::new(), BodyThreshold::Points(dec!(15))).is_empty());
}

#[test]
fn the_normalized_threshold_uses_the_atr_before_the_impulse() {
    let m15 = day_with_impulse();
    let atr = segment_atr(&m15, &[], 14)[15].unwrap();
    let k_ok = (dec!(16) / atr).floor();
    let k_no = (dec!(16) / atr).ceil() + dec!(1);
    assert_eq!(
        plans_for(&m15, &BTreeSet::new(), BodyThreshold::Atr(k_ok)).len(),
        1
    );
    assert!(plans_for(&m15, &BTreeSet::new(), BodyThreshold::Atr(k_no)).is_empty());
}

fn bracket(stop: Decimal) -> Plan {
    Plan {
        event: 0,
        signal_index: 0,
        side: Side::Long,
        reference: dec!(1000),
        stop,
        risk_atr: dec!(5),
        exit_index: 5,
        target_r: Some(dec!(1.5)),
        target_cap: None,
    }
}

#[test]
fn a_trade_leaves_at_the_target_and_the_stop_wins_a_tie() {
    let s = setup();
    let base = [flat(dec!(1000)); 7];
    let (from, to) = (at("2024-01-01T00:00:00Z"), at("2025-01-01T00:00:00Z"));
    // Objetivo 1000 + 1,5 × 10 = 1015, tocado en la vela 3.
    let mut v = base;
    v[3] = (dec!(1001), dec!(1016), dec!(1000.5), dec!(1010));
    let m15 = seq("2024-01-10T15:00:00Z", &v);
    let run = run_plans(&[bracket(dec!(990))], &m15, &[], &s, from, to).unwrap();
    let t = &run.trades[0];
    assert_eq!(
        (t.exit, t.exit_index, t.exit_mid),
        (StudyExit::Target, 3, dec!(1015))
    );
    assert_eq!(t.target, Some(dec!(1015)));
    // La misma vela toca el stop y el objetivo: se supone el stop.
    v[3] = (dec!(1001), dec!(1016), dec!(989), dec!(1010));
    let m15 = seq("2024-01-10T15:00:00Z", &v);
    let run = run_plans(&[bracket(dec!(990))], &m15, &[], &s, from, to).unwrap();
    assert_eq!(run.trades[0].exit, StudyExit::Stop);
    // Apertura de la entrada ya debajo del stop: no se opera.
    let mut v = base;
    v[1] = (dec!(989), dec!(990), dec!(985), dec!(986));
    let m15 = seq("2024-01-10T15:00:00Z", &v);
    let run = run_plans(&[bracket(dec!(990))], &m15, &[], &s, from, to).unwrap();
    assert!(run.trades.is_empty());
    assert_eq!(run.through_stop, vec![0]);
}

#[test]
fn a_gap_over_the_target_exits_at_the_target() {
    let m15 = seq(
        "2024-01-10T15:00:00Z",
        &[
            flat(dec!(1000)),
            flat(dec!(1000)),
            (dec!(1020), dec!(1021), dec!(1019), dec!(1020)),
        ],
    );
    assert_eq!(
        simulate_leg(Side::Long, 1, dec!(990), Some(dec!(1015)), 2, &m15, &[]),
        Some((2, dec!(1015), StudyExit::Target))
    );
}

#[test]
fn k_is_the_ratio_that_passes_the_same_share_as_fifteen_points() {
    // 10 velas: cuerpos 1..10 con ATR 4 (cociente cuerpo / 4). Con 8 puntos pasan 3 (q = 0,3);
    // el valor número ⌈0,7 × 10⌉ = 7 es 7 / 4 = 1,75.
    let sample: Vec<(Decimal, Decimal)> = (1..=10)
        .map(|b| (Decimal::from(b), Decimal::from(b) / dec!(4)))
        .collect();
    assert_eq!(
        equivalent_k(&sample, dec!(8), dec!(0.05)),
        Some((dec!(0.3), dec!(1.75)))
    );
    // Redondeo al 0,05 más cercano, las mitades hacia arriba.
    let sample = vec![(dec!(20), dec!(0.425)), (dec!(1), dec!(0.1))];
    assert_eq!(
        equivalent_k(&sample, dec!(15), dec!(0.05)),
        Some((dec!(0.5), dec!(0.1)))
    );
    let sample = vec![(dec!(20), dec!(0.425))];
    assert_eq!(
        equivalent_k(&sample, dec!(15), dec!(0.05)),
        Some((dec!(1), dec!(0.45)))
    );
    assert_eq!(equivalent_k(&[], dec!(15), dec!(0.05)), None);
}

#[test]
fn the_k_sample_takes_only_session_candles_with_a_previous_atr() {
    let m15 = day_with_impulse();
    let atr = segment_atr(&m15, &[], 14);
    let sample = body_atr_sample(
        &m15,
        &session(),
        &atr,
        (at("2024-01-10T00:00:00Z"), at("2024-01-11T00:00:00Z")),
    );
    // Sesión 14:30–21:00 UTC: velas de las 14:30 (índice 12) a las 20:45 (índice 37), sin las
    // dos primeras (la vela anterior todavía no tiene ATR de 14) ni las de las 21:00 y 21:15.
    assert_eq!(sample.len(), 24);
    assert_eq!(sample[2].0, dec!(16)); // la vela de las 15:30
}

#[test]
fn splitmix64_matches_the_reference_sequence() {
    // Primer valor de SplitMix64 con semilla 0 (implementación de referencia de Vigna).
    let mut r = SplitMix64::new(0);
    assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
    let mut r = SplitMix64::new(7);
    assert!((0..1000).all(|_| r.below(10) < 10));
    assert!(!SplitMix64::new(1).chance(0, 5));
    assert!(SplitMix64::new(1).chance(5, 5));
}

#[test]
fn the_luck_test_is_deterministic_and_counts_the_strategy_the_same_way() {
    assert_eq!(
        r_summary(&[dec!(1.5), dec!(-1)], dec!(0.01)),
        (dec!(0.25), dec!(1.015) * dec!(0.99) - dec!(1))
    );
    let s = setup();
    let m15 = day_with_impulse();
    let cfg = research().program4;
    let mut plan = plans_for(&m15, &BTreeSet::new(), BodyThreshold::Points(dec!(15)))[0].clone();
    plan.risk_atr = dec!(20); // para que el riesgo la acepte en el ejemplo
    let run = run_plans(
        &[plan],
        &m15,
        &[],
        &s,
        at("2024-01-01T00:00:00Z"),
        at("2025-01-01T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(run.trades.len(), 1);
    let entries = vec![true; m15.len()];
    let none = BTreeSet::new();
    let a = luck_test(
        &m15,
        &session(),
        &entries,
        &none,
        &[],
        &cfg,
        &run.trades,
        &s,
        dec!(0.01),
    );
    let b = luck_test(
        &m15,
        &session(),
        &entries,
        &none,
        &[],
        &cfg,
        &run.trades,
        &s,
        dec!(0.01),
    );
    assert_eq!(a, b);
    assert_eq!(a.simulations.len(), cfg.luck_simulations);
}

#[test]
fn the_program_4_rule_is_read_from_config() {
    let c = research();
    assert_eq!(c.rule_program4.alpha, dec!(0.0125));
    assert_eq!(c.rule_program4.dsr_trials, 359);
    assert_eq!(c.rule_program4.second_half_from, day("2021-01-01"));
    let p = c.program4;
    assert_eq!((p.atr_period, p.min_valid_in_zone), (14, 3));
    assert_eq!((p.body_points, p.target_r), (dec!(15), dec!(1.5)));
    assert_eq!(p.body_atr_k, Some(dec!(0.35)));
    assert_eq!(
        (
            p.last_entry_minutes_before_close,
            p.exit_minutes_before_close
        ),
        (30, 15)
    );
    assert_eq!(p.k_reference, (day("2024-11-01"), day("2025-11-01")));
    assert_eq!(
        p.strong_releases,
        vec![("fed_fomc".to_owned(), "fomc_statement".to_owned())]
    );
    assert_eq!(
        (p.min_trades, p.luck_simulations, p.luck_seed),
        (100, 1000, 20261125)
    );
    for (from, to) in [
        ("min_valid_in_zone = 3", "min_valid_in_zone = 0"),
        (
            "exit_minutes_before_close = 15",
            "exit_minutes_before_close = 30",
        ),
        (
            "k_reference_to = \"2025-11-01\"",
            "k_reference_to = \"2024-10-01\"",
        ),
        ("\"fed_fomc/fomc_statement\"", "\"fed_fomc\""),
        ("body_points = \"15\"", "body_points = \"0\""),
    ] {
        let bad = cfg!("research.toml").replace(from, to);
        assert!(ResearchConfig::from_toml(&bad).is_err(), "{to}");
    }
}

#[test]
fn the_short_side_is_the_exact_mirror() {
    let m = dec!(2100);
    let mirror: Vec<Candle> = day_with_impulse()
        .into_iter()
        .map(|x| c(x.start, m - x.open, m - x.low, m - x.high, m - x.close))
        .collect();
    let plans = plans_for(&mirror, &BTreeSet::new(), BodyThreshold::Points(dec!(15)));
    assert_eq!(plans.len(), 1);
    let p = &plans[0];
    assert_eq!((p.signal_index, p.side), (17, Side::Short));
    assert_eq!((p.reference, p.stop), (m - dec!(1036), m - dec!(1003.5)));
    assert_eq!(p.target_from(m - dec!(1036)), Some(m - dec!(1084.75)));
}

#[test]
fn minute_bars_say_which_level_came_first() {
    use cfds_backtest::research::program4::target_first;
    let bars = [
        (dec!(101), dec!(99.5)),
        (dec!(106), dec!(100)),
        (dec!(101), dec!(94)),
    ];
    assert_eq!(
        target_first(Side::Long, dec!(95), dec!(105), &bars),
        Some(true)
    );
    let bars = [(dec!(101), dec!(94)), (dec!(106), dec!(100))];
    assert_eq!(
        target_first(Side::Long, dec!(95), dec!(105), &bars),
        Some(false)
    );
    assert_eq!(
        target_first(Side::Long, dec!(95), dec!(105), &[(dec!(106), dec!(94))]),
        None
    );
    assert_eq!(
        target_first(Side::Short, dec!(105), dec!(95), &[(dec!(101), dec!(94))]),
        Some(true)
    );
}

#[test]
fn outcomes_after_a_win_and_after_a_loss_are_counted_apart() {
    use cfds_backtest::research::program4::{outcome_after, rolling_after_loss_below};
    let wins = [true, false, false, true, true, false];
    // Tras una ganadora: F, T, F → 1 de 3; tras una perdedora: F, T → 1 de 2.
    assert_eq!(outcome_after(&wins), [(3, 6), (1, 3), (1, 2)]);
    assert_eq!(rolling_after_loss_below(&wins, 6), (0, 1));
    assert_eq!(
        rolling_after_loss_below(&[true, false, false, false], 4),
        (1, 1)
    );
    assert_eq!(rolling_after_loss_below(&wins, 0), (0, 0));
}

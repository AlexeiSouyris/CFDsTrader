//! Programa 4 (#561), H4.3: estrategia C de Bruni (`docs/analysis/2026-11-programa-4-preregistro.md`
//! §3, §6 y §13). Series sintéticas sobre el calendario real de XNYS; nunca la base real.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cfds_backtest::research::program4::{Breakout, Structure};
use cfds_backtest::research::program4_c::{
    ChampionshipInputs, Profile, ProfileBuilder, ProfileContext, atr_percentile, bin_width,
    championship_plans, profile_contexts, profile_session, regime_band, release_dates,
    session_minutes,
};
use cfds_backtest::research::{
    ChampionshipConfig, Plan, Program4Config, ResearchConfig, StudyExit, run_plans,
};
use cfds_backtest::{BacktestConfig, Setup, apply_funding_scenario, setup_from_catalog};
use cfds_core::{Candle, FuturesBar, InstrumentId, Timeframe};
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

fn h43() -> ChampionshipConfig {
    research().program4_c
}

fn p4() -> Program4Config {
    research().program4
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

type Ohlcv = (Decimal, Decimal, Decimal, Decimal, Decimal);

fn candle(start: DateTime<Utc>, (o, h, l, c, v): Ohlcv) -> Candle {
    Candle {
        instrument_id: us100(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: Some(v),
    }
}

fn bar(start: DateTime<Utc>, h: Decimal, l: Decimal, v: u64, contract: u64) -> FuturesBar {
    FuturesBar {
        source: "test".into(),
        dataset: "GLBX.MDP3".into(),
        symbol: "NQ.c.0".into(),
        instrument_id: us100(),
        contract_id: contract,
        start,
        open: l,
        high: h,
        low: l,
        close: h,
        volume: v,
    }
}

// ── configuración ──────────────────────────────────────────────────────────────────────────

#[test]
fn the_h43_config_loads_with_the_frozen_values() {
    let c = h43();
    assert_eq!(c.timezone, chrono_tz::America::New_York);
    assert_eq!(session_minutes(&c), 23 * 60);
    assert_eq!(c.profile_min_bars_fraction, dec!(0.9));
    assert_eq!(c.trigger_margin, dec!(1.5));
    assert_eq!(c.tick, dec!(0.25));
    assert_eq!(c.value_area, dec!(0.7));
    assert_eq!(c.volume_sma, 20);
    assert_eq!(c.target_r, dec!(2));
    assert_eq!(c.regime_window, 200);
    assert_eq!(c.strong_releases.len(), 3);
    assert_eq!(research().rule_program4.dsr_trials, 359);
    for (from, to) in [
        ("value_area = \"0.7\"", "value_area = \"1.2\""),
        (
            "profile_session_end = \"17:00\"",
            "profile_session_end = \"19:00\"",
        ),
        (
            "regime_bands = [\"25\", \"50\", \"75\"]",
            "regime_bands = [\"50\", \"25\"]",
        ),
        ("volume_sma = 20", "volume_sma = 1"),
        ("\"bls/cpi\"", "\"bls\""),
    ] {
        let bad = cfg!("research.toml").replace(from, to);
        assert!(ResearchConfig::from_toml(&bad).is_err(), "{to}");
    }
}

// ── perfil de volumen ──────────────────────────────────────────────────────────────────────

#[test]
fn the_bin_width_is_two_basis_points_rounded_to_ticks() {
    let c = h43();
    assert_eq!(bin_width(dec!(20000), &c), dec!(4.00));
    assert_eq!(bin_width(dec!(2800), &c), dec!(0.50));
    // Nunca menos de un tick.
    assert_eq!(bin_width(dec!(500), &c), dec!(0.25));
}

#[test]
fn volume_is_spread_evenly_and_the_value_area_grows_from_the_poc() {
    let mut p = ProfileBuilder::new(dec!(1));
    // 40 repartido en 100–104 (10 por casillero) y 5 justo en 102.
    p.add(dec!(104), dec!(100), dec!(40));
    p.add(dec!(102), dec!(102), dec!(5));
    // POC 102 (15); 70 % de 45 = 31,5: arriba (103) y abajo (101) empatan con 10 → se suman los
    // dos (35).
    assert_eq!(
        p.profile(dec!(0.7)),
        Some(Profile {
            poc: dec!(102),
            vah: dec!(104),
            val: dec!(101)
        })
    );
    // Con más volumen arriba, el área crece hacia arriba primero.
    let mut q = ProfileBuilder::new(dec!(1));
    q.add(dec!(100), dec!(100), dec!(50));
    q.add(dec!(101), dec!(101), dec!(30));
    q.add(dec!(99), dec!(99), dec!(20));
    assert_eq!(
        q.profile(dec!(0.7)),
        Some(Profile {
            poc: dec!(100),
            vah: dec!(102),
            val: dec!(100)
        })
    );
    assert_eq!(ProfileBuilder::new(dec!(1)).profile(dec!(0.7)), None);
}

#[test]
fn a_poc_tie_goes_to_the_bin_nearest_the_middle_of_the_range() {
    let mut p = ProfileBuilder::new(dec!(1));
    p.add(dec!(100), dec!(100), dec!(10));
    p.add(dec!(103), dec!(103), dec!(10));
    p.add(dec!(110), dec!(110), dec!(1));
    // Rango 100–110, medio 105: el casillero 103 (centro 103,5) está más cerca que el 100.
    assert_eq!(p.profile(dec!(0.5)).unwrap().poc, dec!(103));
}

#[test]
fn the_profile_session_is_the_cme_day_in_new_york_time() {
    let c = h43();
    // Invierno: 18:00 EST = 23:00 UTC del día anterior.
    assert_eq!(
        profile_session(at("2024-01-09T23:00:00Z"), &c),
        Some(day("2024-01-10"))
    );
    assert_eq!(
        profile_session(at("2024-01-09T21:59:00Z"), &c),
        Some(day("2024-01-09"))
    );
    // 17:00–18:00 ET: la pausa, ninguna sesión.
    assert_eq!(profile_session(at("2024-01-10T22:00:00Z"), &c), None);
    assert_eq!(profile_session(at("2024-01-10T22:59:00Z"), &c), None);
    // Verano: 18:00 EDT = 22:00 UTC.
    assert_eq!(
        profile_session(at("2024-07-09T22:00:00Z"), &c),
        Some(day("2024-07-10"))
    );
    // Domingo a la tarde → lunes; viernes a la tarde también.
    assert_eq!(
        profile_session(at("2024-01-14T23:30:00Z"), &c),
        Some(day("2024-01-15"))
    );
    assert_eq!(
        profile_session(at("2024-01-12T23:30:00Z"), &c),
        Some(day("2024-01-15"))
    );
}

/// Barras de 1 minuto de la sesión del CME de `d` (EST), `skip` minutos sin barra al principio.
fn session_bars(d: &str, skip: usize, contract: u64, price: Decimal) -> Vec<FuturesBar> {
    let start = at(&format!("{d}T23:00:00Z")) - TimeDelta::days(1);
    (skip..23 * 60)
        .map(|k| {
            bar(
                start + TimeDelta::minutes(k as i64),
                price + dec!(1),
                price,
                10,
                contract,
            )
        })
        .collect()
}

fn one_candle(start: &str) -> Vec<Candle> {
    vec![candle(
        at(start),
        (dec!(100), dec!(101), dec!(99), dec!(100), dec!(10)),
    )]
}

#[test]
fn the_previous_profile_needs_ninety_percent_of_its_bars_one_contract_and_no_roll() {
    let c = h43();
    let m15 = one_candle("2024-01-11T15:00:00Z");
    let ctx = |bars: Vec<FuturesBar>, rolls: &[DateTime<Utc>]| {
        profile_contexts(&m15, &bars, rolls, &c)[0]
    };
    let two = |skip, c1, c2| {
        [
            session_bars("2024-01-10", skip, c1, dec!(100)),
            session_bars("2024-01-11", 0, c2, dec!(110)),
        ]
        .concat()
    };
    // 1242 barras (90 % de 1380): vale. 1241: no.
    let ok = ctx(two(138, 1, 1), &[]).unwrap();
    assert_eq!(ok.session, day("2024-01-11"));
    assert!(ok.previous.poc >= dec!(100) && ok.previous.poc < dec!(101));
    assert!(ok.developing.poc >= dec!(110) && ok.developing.poc < dec!(111));
    assert!(ctx(two(139, 1, 1), &[]).is_none());
    // Contratos distintos o un vencimiento en el medio: no.
    assert!(ctx(two(0, 1, 2), &[]).is_none());
    assert!(ctx(two(0, 1, 1), &[at("2024-01-11T03:00:00Z")]).is_none());
    // La sesión anterior tiene que ser el día hábil anterior.
    let gap = [
        session_bars("2024-01-09", 0, 1, dec!(100)),
        session_bars("2024-01-11", 0, 1, dec!(110)),
    ]
    .concat();
    assert!(ctx(gap, &[]).is_none());
}

#[test]
fn the_developing_profile_uses_only_bars_before_the_candle_closes() {
    let c = h43();
    let mut bars = session_bars("2024-01-10", 0, 1, dec!(100));
    let mut today = session_bars("2024-01-11", 0, 1, dec!(110));
    // Un pico de volumen en 120 a las 15:05 UTC (dentro de la vela de las 15:00).
    let spike = today
        .iter()
        .position(|b| b.start == at("2024-01-11T15:05:00Z"))
        .unwrap();
    today[spike] = bar(at("2024-01-11T15:05:00Z"), dec!(120), dec!(120), 100_000, 1);
    bars.extend(today);
    let m15 = [
        one_candle("2024-01-11T14:45:00Z"),
        one_candle("2024-01-11T15:00:00Z"),
    ]
    .concat();
    let ctx = profile_contexts(&m15, &bars, &[], &c);
    // Al cierre de las 14:45 el pico todavía no pasó; al cierre de las 15:00, sí.
    assert_ne!(
        ctx[0].unwrap().developing.poc,
        ctx[1].unwrap().developing.poc
    );
    assert!(ctx[1].unwrap().developing.poc >= dec!(119));
    assert!(ctx[0].unwrap().developing.poc < dec!(112));
}

// ── días con datos fuertes ──────────────────────────────────────────────────────────────────

#[test]
fn a_cpi_at_0830_excludes_its_day_like_an_fomc_inside_the_session() {
    let s = session();
    let days = release_dates(
        &s,
        &[
            at("2024-01-11T13:30:00Z"), // CPI 08:30 EST
            at("2024-01-31T19:00:00Z"), // FOMC 14:00 EST
            at("2024-03-29T12:30:00Z"), // empleo en Viernes Santo: sin sesión
        ],
    );
    assert_eq!(days, BTreeSet::from([day("2024-01-11"), day("2024-01-31")]));
}

// ── señales ────────────────────────────────────────────────────────────────────────────────

/// Velas de 15 m de todo el día desde las 00:00 UTC del 10-01-2024 (miércoles, EST) hasta las
/// 21:45 UTC: planas en 120, salvo las que se pisan.
fn base_day() -> Vec<Ohlcv> {
    vec![(dec!(120), dec!(121), dec!(119), dec!(120), dec!(100)); 88]
}

fn idx(hhmm: &str) -> usize {
    let (h, m) = hhmm.split_once(':').unwrap();
    (h.parse::<usize>().unwrap() * 60 + m.parse::<usize>().unwrap()) / 15
}

/// Escenario largo: ruptura alcista a las 13:00 UTC (la vela anterior es bajista válida con
/// mínimo 95), máximo 130 a las 13:30, vuelta al área (mínimo 108 ≤ VAH 110) a las 14:45 y
/// disparador a las 15:00 UTC (10:00 EST), con menos volumen que la anterior.
fn long_day() -> Vec<Ohlcv> {
    let mut v = base_day();
    v[idx("12:45")] = (dec!(122), dec!(122.5), dec!(95), dec!(96), dec!(100));
    v[idx("13:00")] = (dec!(96), dec!(125), dec!(96), dec!(124), dec!(100));
    v[idx("13:30")] = (dec!(124), dec!(130), dec!(120), dec!(121), dec!(100));
    v[idx("14:45")] = (dec!(120), dec!(112), dec!(108), dec!(109), dec!(100));
    v[idx("15:00")] = (dec!(109), dec!(114.5), dec!(108.5), dec!(114), dec!(50));
    v
}

fn mirror(v: &[Ohlcv]) -> Vec<Ohlcv> {
    let f = |p: Decimal| dec!(240) - p;
    v.iter()
        .map(|&(o, h, l, c, vol)| (f(o), f(l), f(h), f(c), vol))
        .collect()
}

fn build(v: &[Ohlcv]) -> Vec<Candle> {
    v.iter()
        .enumerate()
        .map(|(k, x)| {
            candle(
                at("2024-01-10T00:00:00Z") + TimeDelta::minutes(15 * k as i64),
                *x,
            )
        })
        .collect()
}

struct World {
    m15: Vec<Candle>,
    entries: Vec<bool>,
    atr: Vec<Option<Decimal>>,
    breakouts: Vec<Breakout>,
    states: Vec<Structure>,
    contexts: Vec<Option<ProfileContext>>,
}

fn world(v: &[Ohlcv], side: Side) -> World {
    let m15 = build(v);
    let n = m15.len();
    let b = idx("13:00");
    let breakouts = vec![Breakout {
        index: b,
        side,
        high: dec!(0),
        low: dec!(0),
    }];
    let states = (0..n)
        .map(|k| Structure {
            current: (k >= b).then_some(side),
            peripheral: None,
        })
        .collect();
    let (prev, dev) = match side {
        Side::Long => (
            Profile {
                poc: dec!(100),
                vah: dec!(110),
                val: dec!(90),
            },
            Profile {
                poc: dec!(105),
                vah: dec!(115),
                val: dec!(95),
            },
        ),
        Side::Short => (
            Profile {
                poc: dec!(140),
                vah: dec!(150),
                val: dec!(130),
            },
            Profile {
                poc: dec!(135),
                vah: dec!(145),
                val: dec!(125),
            },
        ),
    };
    let contexts = (0..n)
        .map(|_| {
            Some(ProfileContext {
                session: day("2024-01-10"),
                previous: prev,
                developing: dev,
            })
        })
        .collect();
    World {
        m15,
        entries: vec![true; n],
        atr: vec![Some(dec!(5)); n],
        breakouts,
        states,
        contexts,
    }
}

fn plans_of(w: &World, excluded: &BTreeSet<NaiveDate>) -> Vec<(Plan, bool)> {
    let s = session();
    let i = ChampionshipInputs {
        m15: &w.m15,
        session: &s,
        entries: &w.entries,
        atr: &w.atr,
        breakouts: &w.breakouts,
        states: &w.states,
        rolls: &[],
        contexts: &w.contexts,
        excluded,
    };
    let (plans, infos, _) = championship_plans(&i, &p4(), &h43());
    plans
        .into_iter()
        .zip(infos)
        .map(|(p, info)| (p, info.first))
        .collect()
}

#[test]
fn the_long_signal_follows_every_condition() {
    let w = world(&long_day(), Side::Long);
    let plans = plans_of(&w, &BTreeSet::new());
    assert_eq!(plans.len(), 1, "{plans:?}");
    let (p, first) = &plans[0];
    assert!(*first);
    assert_eq!(p.signal_index, idx("15:00"));
    assert_eq!(p.side, Side::Long);
    assert_eq!((p.reference, p.stop), (dec!(114), dec!(108.5)));
    // Objetivo: el menor entre 2 R y el máximo de la estructura (130).
    assert_eq!(p.target_cap, Some(dec!(130)));
    assert_eq!(p.target_r, Some(dec!(2)));
    assert_eq!(p.target_from(dec!(114)), Some(dec!(125)));
    assert_eq!(p.target_from(dec!(124)), Some(dec!(130)));
}

#[test]
fn the_short_signal_is_the_exact_mirror() {
    let w = world(&mirror(&long_day()), Side::Short);
    let plans = plans_of(&w, &BTreeSet::new());
    assert_eq!(plans.len(), 1, "{plans:?}");
    let (p, first) = &plans[0];
    assert!(*first);
    assert_eq!(p.side, Side::Short);
    assert_eq!((p.reference, p.stop), (dec!(126), dec!(131.5)));
    assert_eq!(p.target_cap, Some(dec!(110)));
}

#[test]
fn each_condition_can_block_the_signal() {
    let none = |v: Vec<Ohlcv>, tweak: &dyn Fn(&mut World)| {
        let mut w = world(&v, Side::Long);
        tweak(&mut w);
        plans_of(&w, &BTreeSet::new()).is_empty()
    };
    let t = idx("15:00");
    // 1. Sin desplazamiento del perfil (VAL en desarrollo no más alto).
    assert!(none(long_day(), &|w| {
        for x in w.contexts.iter_mut().flatten() {
            x.developing.val = dec!(90);
        }
    }));
    // 2. Estructura bajista.
    assert!(none(long_day(), &|w| w.states[t].current = Some(Side::Short)));
    // 3. Sin vuelta al área de valor.
    let mut v = long_day();
    v[idx("14:45")].2 = dec!(110.5);
    assert!(none(v, &|_| {}));
    // 4. Mínimo de la estructura (95) perforado.
    let mut v = long_day();
    v[idx("14:00")].2 = dec!(94);
    assert!(none(v, &|_| {}));
    // 5a. Cierre debajo del máximo anterior + m (112 + 1,5).
    let mut v = long_day();
    v[t].3 = dec!(113.25);
    assert!(none(v, &|_| {}));
    // 5c. Volumen no eficiente.
    let mut v = long_day();
    v[t].4 = dec!(150);
    assert!(none(v, &|_| {}));
    // 6. Primera operación: la vela anterior ya cerró sobre el VAH + m.
    let mut v = long_day();
    v[idx("14:45")].3 = dec!(111.5);
    v[idx("14:45")].1 = dec!(111.6);
    v[t].1 = dec!(114.5);
    assert!(none(v, &|_| {}));
    // 7. Día con dato fuerte.
    let w = world(&long_day(), Side::Long);
    assert!(plans_of(&w, &BTreeSet::from([day("2024-01-10")])).is_empty());
    // 8. Sin recorrido: el máximo de la estructura no supera el cierre.
    let mut v = long_day();
    v[idx("13:00")] = (dec!(96), dec!(113), dec!(96), dec!(112), dec!(100));
    for x in &mut v[idx("13:15")..idx("14:45")] {
        *x = (dec!(111), dec!(112.5), dec!(110.5), dec!(112), dec!(100));
    }
    assert!(none(v, &|_| {}));
    // Una vuelta antes de la ruptura no cuenta (el máximo de la estructura se mide entre la
    // ruptura y la vuelta).
    let mut v = long_day();
    v[idx("14:45")].2 = dec!(110.5);
    v[idx("12:30")].2 = dec!(108);
    assert!(none(v, &|_| {}));
}

#[test]
fn reentries_skip_the_first_trade_rules_until_the_structure_high_is_touched() {
    let mut v = long_day();
    // Después de la primera, otra vela que cierra sobre el máximo anterior + m, con la anterior
    // ya sobre el VAH (sería inválida como primera) y menos volumen: reentrada.
    v[idx("15:30")] = (dec!(114), dec!(116), dec!(113), dec!(115), dec!(100));
    v[idx("15:45")] = (dec!(115), dec!(118), dec!(114.5), dec!(117.75), dec!(40));
    let w = world(&v, Side::Long);
    let plans = plans_of(&w, &BTreeSet::new());
    let firsts: Vec<bool> = plans.iter().map(|(_, f)| *f).collect();
    assert_eq!(firsts, vec![true, false]);
    assert_eq!(plans[1].0.signal_index, idx("15:45"));
    // Si antes el precio toca el máximo de la estructura (130), la secuencia termina.
    v[idx("15:15")] = (dec!(114), dec!(130), dec!(113), dec!(114), dec!(100));
    let w = world(&v, Side::Long);
    let plans = plans_of(&w, &BTreeSet::new());
    assert_eq!(plans.len(), 1);
}

#[test]
fn new_candles_do_not_change_past_signals() {
    let w = world(&long_day(), Side::Long);
    let full = plans_of(&w, &BTreeSet::new());
    let cut = idx("15:15") + 1;
    let short = World {
        m15: w.m15[..cut].to_vec(),
        entries: w.entries[..cut].to_vec(),
        atr: w.atr[..cut].to_vec(),
        breakouts: w.breakouts.clone(),
        states: w.states[..cut].to_vec(),
        contexts: w.contexts[..cut].to_vec(),
    };
    // Lo decidido en una vela no cambia (la salida planificada depende de hasta dónde hay
    // velas, igual que en H4.1 y H4.2).
    let key = |v: Vec<(Plan, bool)>| -> Vec<_> {
        v.into_iter()
            .map(|(p, f)| (p.signal_index, p.side, p.reference, p.stop, p.target_cap, f))
            .collect()
    };
    assert_eq!(key(plans_of(&short, &BTreeSet::new())), key(full));
}

// ── ejecución con tope del objetivo ────────────────────────────────────────────────────────

#[test]
fn the_capped_target_and_an_entry_already_at_the_cap() {
    let s = setup();
    let flat = (dec!(1000), dec!(1000.25), dec!(999.75), dec!(1000), dec!(1));
    let mut v = vec![flat; 7];
    // Entrada 1000, stop 990: 2 R = 1020, pero el tope es 1008.
    v[3] = (dec!(1001), dec!(1009), dec!(1000.5), dec!(1005), dec!(1));
    let m15: Vec<Candle> = v
        .iter()
        .enumerate()
        .map(|(k, x)| {
            candle(
                at("2024-01-10T15:00:00Z") + TimeDelta::minutes(15 * k as i64),
                *x,
            )
        })
        .collect();
    let plan = Plan {
        event: 0,
        signal_index: 0,
        side: Side::Long,
        reference: dec!(1000),
        stop: dec!(990),
        risk_atr: dec!(5),
        exit_index: 5,
        target_r: Some(dec!(2)),
        target_cap: Some(dec!(1008)),
    };
    let (from, to) = (at("2024-01-01T00:00:00Z"), at("2025-01-01T00:00:00Z"));
    let run = run_plans(std::slice::from_ref(&plan), &m15, &[], &s, from, to).unwrap();
    let t = &run.trades[0];
    assert_eq!((t.exit, t.exit_mid), (StudyExit::Target, dec!(1008)));
    // La apertura de la entrada ya en el tope: no se opera.
    let capped = Plan {
        target_cap: Some(dec!(1000)),
        ..plan
    };
    let run = run_plans(&[capped], &m15, &[], &s, from, to).unwrap();
    assert!(run.trades.is_empty());
    assert_eq!(run.through_cap, vec![0]);
}

// ── régimen ────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_regime_percentile_and_bands() {
    let m15 = build(&base_day());
    let atr: Vec<Option<Decimal>> = (0..m15.len()).map(|k| Some(Decimal::from(k))).collect();
    // Ventana de 4: el ATR de t es el mayor → 100.
    assert_eq!(atr_percentile(&atr, &m15, &[], 4, 10), Some(dec!(100)));
    assert_eq!(atr_percentile(&atr, &m15, &[], 4, 2), None);
    let bands = [dec!(25), dec!(50), dec!(75)];
    assert_eq!(regime_band(dec!(25), &bands), 0);
    assert_eq!(regime_band(dec!(25.5), &bands), 1);
    assert_eq!(regime_band(dec!(100), &bands), 3);
}

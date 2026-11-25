//! Estrategias de estudio de la ronda 1 (#472): ruptura del rango de apertura (C), cambio de mes
//! (D1), su ejecución con costos y la estadística de la regla pre-registrada. Series sintéticas
//! sobre el calendario real de XNYS.
#![allow(
    clippy::float_arithmetic,
    reason = "valores de referencia de la estadística, no dinero"
)]

use std::path::PathBuf;

use cfds_backtest::research::{
    OrbConfig, Plan, ResearchConfig, StudyExit, TurnOfMonthConfig, execute_plan, monthly_returns,
    orb_plans, run_plans, session_bars, simulate_leg, stats, trade_return, turn_of_month_plans,
};
use cfds_backtest::{BacktestConfig, Setup, apply_funding_scenario, setup_from_catalog};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::{TrendConfig, wilder_atr};
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Catalog, ExchangeCalendar, Session, TradingWindowConfig, trading_window};
use cfds_patterns::PatternsConfig;
use cfds_risk::{Rejection, RiskConfig};
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

fn us500() -> InstrumentId {
    "US500".parse().unwrap()
}

fn calendar() -> ExchangeCalendar {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap()
}

fn research() -> ResearchConfig {
    ResearchConfig::from_toml(cfg!("research.toml")).unwrap()
}

fn setup(equity: Decimal) -> Setup {
    let mut catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    apply_funding_scenario(&mut catalog, &us500(), "TEST", dec!(4)).unwrap();
    setup_from_catalog(
        &catalog,
        &us500(),
        Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
            trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
        },
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        equity,
    )
    .unwrap()
}

fn candle(start: DateTime<Utc>, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: us500(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

/// Velas de 15m de la sesión de contado de cada día con sesión en `[from, to]`, con un precio
/// determinista que oscila alrededor de 1000 (sin tendencia).
fn session_candles(session: &Session, from: NaiveDate, to: NaiveDate) -> Vec<Candle> {
    let mut out = Vec::new();
    let mut d = from;
    let mut i: i64 = 0;
    while d <= to {
        if let Some(Some((open, close))) = session.hours(d) {
            let mut s = open;
            while s + TimeDelta::minutes(15) <= close {
                let base = Decimal::from(1000 + (i * 7) % 13 - 6);
                let wiggle = Decimal::from((i * 5) % 4);
                out.push(candle(
                    s,
                    base,
                    base + wiggle + dec!(1),
                    base - wiggle - dec!(1),
                    base + Decimal::from((i * 3) % 3 - 1),
                ));
                s += TimeDelta::minutes(15);
                i += 1;
            }
        }
        d = d.succ_opt().unwrap();
    }
    out
}

fn flags(session: &Session, m15: &[Candle]) -> (Vec<bool>, Vec<bool>) {
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml")).unwrap();
    trading_window::flags(session, m15, &window).unwrap()
}

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

/// Un día de NY (martes 04-06-2024, 13:30–20:00 UTC) plano en 100, con el rango de apertura
/// 99–101 y la vela `breakout` (índice desde la apertura) cerrando en `close`.
fn orb_day(breakout: usize, close: Decimal) -> Vec<Candle> {
    let open = at("2024-06-04T13:30:00Z");
    (0..26)
        .map(|k| {
            let s = open + TimeDelta::minutes(15 * k as i64);
            match k {
                0 => candle(s, dec!(100), dec!(101), dec!(99.5), dec!(100)),
                1 => candle(s, dec!(100), dec!(100.5), dec!(99), dec!(100)),
                k if k == breakout => candle(
                    s,
                    dec!(100),
                    close.max(dec!(100)),
                    close.min(dec!(100)),
                    close,
                ),
                _ => candle(s, dec!(100), dec!(100.2), dec!(99.8), dec!(100)),
            }
        })
        .collect()
}

fn atr_const(n: usize, v: Decimal) -> Vec<Option<Decimal>> {
    vec![Some(v); n]
}

// ── configuración ──────────────────────────────────────────────────────────────────────────

#[test]
fn the_research_config_loads_and_rejects_bad_values() {
    let c = research();
    assert_eq!(c.orb.range_candles, 2);
    assert_eq!(c.turn_of_month.exit_business_day, 3);
    assert_eq!(c.rule.dsr_trials, 350);
    let bad = cfg!("research.toml").replace("cost_stress = \"1.5\"", "cost_stress = \"0.5\"");
    assert!(ResearchConfig::from_toml(&bad).is_err());
    let bad = cfg!("research.toml").replace("range_candles = 2", "range_candles = 0");
    assert!(ResearchConfig::from_toml(&bad).is_err());
    let extra = format!("{}\n[otra]\nx = 1\n", cfg!("research.toml"));
    assert!(ResearchConfig::from_toml(&extra).is_err());
}

// ── familia C ──────────────────────────────────────────────────────────────────────────────

#[test]
fn a_close_above_the_opening_range_is_a_long_with_the_stop_at_the_range_low() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = orb_day(4, dec!(101.5));
    let (entries, _) = flags(s, &m15);
    let plans = orb_plans(
        &m15,
        s,
        &entries,
        &atr_const(m15.len(), dec!(1)),
        &research().orb,
    );
    assert_eq!(plans.len(), 1);
    let p = &plans[0];
    assert_eq!((p.signal_index, p.side), (4, Side::Long));
    assert_eq!((p.reference, p.stop), (dec!(101.5), dec!(99)));
    // Sale al cierre de la última vela de la sesión (15:45–16:00 ET).
    assert_eq!(m15[p.exit_index].end(), at("2024-06-04T20:00:00Z"));
}

#[test]
fn a_close_below_is_a_short_and_a_close_on_the_extreme_is_not_a_breakout() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let cfg = research().orb;
    let m15 = orb_day(3, dec!(98.9));
    let (entries, _) = flags(s, &m15);
    let p = orb_plans(&m15, s, &entries, &atr_const(m15.len(), dec!(1)), &cfg);
    assert_eq!((p[0].side, p[0].stop), (Side::Short, dec!(101)));
    let m15 = orb_day(3, dec!(101));
    let (entries, _) = flags(s, &m15);
    assert!(orb_plans(&m15, s, &entries, &atr_const(m15.len(), dec!(1)), &cfg).is_empty());
}

#[test]
fn a_breakout_after_three_pm_or_inside_the_range_candles_does_not_trade() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let cfg = research().orb;
    // Vela 22 = 15:00–15:15 ET: cierra después de las 15:00.
    let m15 = orb_day(22, dec!(102));
    let (entries, _) = flags(s, &m15);
    assert!(orb_plans(&m15, s, &entries, &atr_const(m15.len(), dec!(1)), &cfg).is_empty());
    // Vela 21 = 14:45–15:00 ET: todavía vale.
    let m15 = orb_day(21, dec!(102));
    let (entries, _) = flags(s, &m15);
    assert_eq!(
        orb_plans(&m15, s, &entries, &atr_const(m15.len(), dec!(1)), &cfg).len(),
        1
    );
    // Sin la segunda vela del rango no hay rango: ese día no se opera.
    let mut m15 = orb_day(4, dec!(102));
    m15.remove(1);
    let (entries, _) = flags(s, &m15);
    assert!(orb_plans(&m15, s, &entries, &atr_const(m15.len(), dec!(1)), &cfg).is_empty());
}

#[test]
fn only_the_first_breakout_counts_and_a_blocked_decision_skips_the_day() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let cfg = research().orb;
    let mut m15 = orb_day(4, dec!(101.5));
    m15[6] = candle(m15[6].start, dec!(100), dec!(100), dec!(98), dec!(98));
    let (mut entries, _) = flags(s, &m15);
    let atr = atr_const(m15.len(), dec!(1));
    let p = orb_plans(&m15, s, &entries, &atr, &cfg);
    assert_eq!((p.len(), p[0].signal_index), (1, 4));
    entries[4] = false;
    assert!(orb_plans(&m15, s, &entries, &atr, &cfg).is_empty());
}

#[test]
fn orb_signals_do_not_repaint_when_more_candles_arrive() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let cfg = research().orb;
    let m15 = session_candles(s, day("2024-03-01"), day("2024-04-30"));
    let (entries, _) = flags(s, &m15);
    let atr = wilder_atr(&m15, 14);
    let full = orb_plans(&m15, s, &entries, &atr, &cfg);
    assert!(
        full.len() > 5,
        "la serie tiene que tener rupturas: {}",
        full.len()
    );
    for cut in [200, 600, 1000] {
        let prefix = orb_plans(&m15[..cut], s, &entries[..cut], &atr[..cut], &cfg);
        for p in &prefix {
            let f = full
                .iter()
                .find(|f| f.signal_index == p.signal_index)
                .unwrap();
            assert_eq!(
                (f.side, f.reference, f.stop, f.risk_atr),
                (p.side, p.reference, p.stop, p.risk_atr)
            );
        }
        // Todo lo que se pudo decidir y entrar antes del corte ya estaba.
        for f in full.iter().filter(|f| f.signal_index + 1 < cut) {
            assert!(prefix.iter().any(|p| p.signal_index == f.signal_index));
        }
    }
}

// ── familia D1 ─────────────────────────────────────────────────────────────────────────────

#[test]
fn session_bars_use_only_the_cash_session() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let mut m15 = orb_day(4, dec!(101.5));
    // Una vela fuera de la sesión (futuros de noche) no entra en la diaria.
    m15.insert(
        0,
        candle(
            at("2024-06-04T08:00:00Z"),
            dec!(50),
            dec!(200),
            dec!(10),
            dec!(60),
        ),
    );
    let bars = session_bars(&m15, s);
    assert_eq!(bars.len(), 1);
    assert_eq!(
        (bars[0].open, bars[0].high, bars[0].low, bars[0].close),
        (dec!(100), dec!(101.5), dec!(99), dec!(100))
    );
}

fn tom_cfg() -> TurnOfMonthConfig {
    research().turn_of_month
}

#[test]
fn the_turn_of_month_enters_at_the_last_candle_and_exits_on_the_third_business_day() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = session_candles(s, day("2024-03-01"), day("2024-05-10"));
    let (entries, force) = flags(s, &m15);
    let plans = turn_of_month_plans(&m15, s, &entries, &force, &tom_cfg());
    // Martes 30-04-2024: decisión al cierre de 15:30–15:45 ET, salida el viernes 03-05 al cierre.
    let p = plans
        .iter()
        .find(|p| m15[p.signal_index].end() == at("2024-04-30T19:45:00Z"))
        .expect("evento de abril");
    assert_eq!(p.side, Side::Long);
    assert_eq!(m15[p.signal_index + 1].start, at("2024-04-30T19:45:00Z"));
    assert_eq!(m15[p.exit_index].end(), at("2024-05-03T20:00:00Z"));
    // Stop a 2 ATR(20) de las diarias anteriores al 30-04.
    let bars = session_bars(&m15, s);
    let atr = wilder_atr(&bars, 20);
    let i = bars
        .iter()
        .position(|b| b.day == day("2024-04-30"))
        .unwrap();
    let a = atr[i - 1].unwrap();
    assert_eq!((p.risk_atr, p.stop), (a, p.reference - dec!(2) * a));
}

#[test]
fn a_weekend_inside_the_window_closes_on_friday_and_reopens_on_monday() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = session_candles(s, day("2023-12-01"), day("2024-02-09"));
    let (entries, force) = flags(s, &m15);
    let plans = turn_of_month_plans(&m15, s, &entries, &force, &tom_cfg());
    // Miércoles 31-01-2024 → viernes 02-02 (cierre de la semana) y lunes 05-02 (tercer día).
    let legs: Vec<&Plan> = plans
        .iter()
        .filter(|p| {
            let t = m15[p.signal_index].end();
            t >= at("2024-01-31T00:00:00Z") && t < at("2024-02-06T00:00:00Z")
        })
        .collect();
    assert_eq!(legs.len(), 2);
    assert_eq!(legs[0].event, legs[1].event);
    assert_eq!(m15[legs[0].signal_index].end(), at("2024-01-31T20:45:00Z"));
    assert_eq!(m15[legs[0].exit_index].end(), at("2024-02-02T21:00:00Z"));
    // Lunes: primera decisión permitida, al cierre de 09:30–09:45 ET.
    assert_eq!(m15[legs[1].signal_index].end(), at("2024-02-05T14:45:00Z"));
    assert_eq!(m15[legs[1].exit_index].end(), at("2024-02-05T21:00:00Z"));
}

#[test]
fn a_month_ending_on_friday_starts_with_the_monday_reopening() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = session_candles(s, day("2024-04-01"), day("2024-06-10"));
    let (entries, force) = flags(s, &m15);
    let plans = turn_of_month_plans(&m15, s, &entries, &force, &tom_cfg());
    // Viernes 31-05-2024: la spec no deja abrir en los últimos 30 minutos.
    let legs: Vec<&Plan> = plans
        .iter()
        .filter(|p| {
            let t = m15[p.signal_index].end();
            t >= at("2024-05-31T00:00:00Z") && t < at("2024-06-06T00:00:00Z")
        })
        .collect();
    assert_eq!(legs.len(), 1);
    assert_eq!(m15[legs[0].signal_index].end(), at("2024-06-03T13:45:00Z"));
    // Tercer día hábil de junio: miércoles 05-06.
    assert_eq!(m15[legs[0].exit_index].end(), at("2024-06-05T20:00:00Z"));
}

#[test]
fn an_event_without_its_exit_candle_is_not_traded() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // Las velas terminan el jueves 02-05-2024: falta la salida del viernes 03-05.
    let m15 = session_candles(s, day("2024-03-01"), day("2024-05-02"));
    let (entries, force) = flags(s, &m15);
    let plans = turn_of_month_plans(&m15, s, &entries, &force, &tom_cfg());
    assert!(
        plans
            .iter()
            .all(|p| m15[p.signal_index].end() < at("2024-04-30T00:00:00Z"))
    );
}

#[test]
fn turn_of_month_signals_do_not_repaint_when_more_candles_arrive() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = session_candles(s, day("2023-09-01"), day("2024-03-29"));
    let (entries, force) = flags(s, &m15);
    let full = turn_of_month_plans(&m15, s, &entries, &force, &tom_cfg());
    assert!(full.len() >= 4);
    for cut in [2000, 2700, 3500] {
        let prefix =
            turn_of_month_plans(&m15[..cut], s, &entries[..cut], &force[..cut], &tom_cfg());
        for p in &prefix {
            let f = full
                .iter()
                .find(|f| f.signal_index == p.signal_index)
                .unwrap();
            assert_eq!(
                (f.side, f.reference, f.stop, f.risk_atr, f.exit_index),
                (p.side, p.reference, p.stop, p.risk_atr, p.exit_index)
            );
        }
    }
}

// ── ejecución y costos ─────────────────────────────────────────────────────────────────────

fn flat(n: usize, start: &str) -> Vec<Candle> {
    (0..n)
        .map(|k| {
            candle(
                at(start) + TimeDelta::minutes(15 * k as i64),
                dec!(100),
                dec!(100.5),
                dec!(99.5),
                dec!(100),
            )
        })
        .collect()
}

#[test]
fn a_leg_checks_the_gap_then_the_stop_then_the_exits() {
    let mut m15 = flat(6, "2024-06-04T14:00:00Z");
    let none = vec![false; 6];
    assert_eq!(
        simulate_leg(Side::Long, 1, dec!(99), None, 4, &m15, &none),
        Some((4, dec!(100), StudyExit::Planned))
    );
    m15[2].low = dec!(98);
    assert_eq!(
        simulate_leg(Side::Long, 1, dec!(99), None, 4, &m15, &none),
        Some((2, dec!(99), StudyExit::Stop))
    );
    m15[2].open = dec!(98.5);
    assert_eq!(
        simulate_leg(Side::Long, 1, dec!(99), None, 4, &m15, &none),
        Some((2, dec!(98.5), StudyExit::StopGap))
    );
    let mut force = none.clone();
    force[1] = true;
    assert_eq!(
        simulate_leg(
            Side::Short,
            1,
            dec!(101),
            None,
            4,
            &flat(6, "2024-06-04T14:00:00Z"),
            &force
        ),
        Some((1, dec!(100), StudyExit::ForceClose))
    );
}

fn plan(signal: usize, exit: usize, stop: Decimal) -> Plan {
    Plan {
        event: 1,
        signal_index: signal,
        side: Side::Long,
        reference: dec!(100),
        stop,
        risk_atr: dec!(2),
        exit_index: exit,
        target_r: None,
        target_cap: None,
    }
}

#[test]
fn an_intraday_trade_pays_spread_and_slippage_but_no_funding() {
    let s = setup(dec!(100000));
    let m15 = flat(10, "2024-06-04T14:00:00Z");
    let t = execute_plan(
        &plan(0, 8, dec!(98)),
        dec!(10),
        dec!(20),
        dec!(100000),
        &m15,
        &[],
        &s,
    )
    .unwrap();
    // US500: spread 0,9, slippage 0,1 × 0,9 + 1 tick (0,01) = 0,1 por lado → 0,55 por ejecución.
    assert_eq!((t.entry_price, t.exit_price), (dec!(100.55), dec!(99.45)));
    assert_eq!(t.funding, dec!(0));
    assert_eq!(t.mid_pnl, dec!(0));
    assert_eq!(t.net_pnl, dec!(-11));
    assert_eq!(t.total_costs(), dec!(11));
    assert_eq!(t.exit, StudyExit::Planned);
}

#[test]
fn an_overnight_trade_pays_funding_for_each_cutoff() {
    let s = setup(dec!(100000));
    // Desde el martes 04-06-2024 19:00 UTC hasta el miércoles 20:00 UTC: un corte (23:00 Madrid).
    let m15 = flat(30 * 4, "2024-06-04T19:00:00Z");
    let exit = m15
        .iter()
        .position(|c| c.end() == at("2024-06-05T20:00:00Z"))
        .unwrap();
    let t = execute_plan(
        &plan(0, exit, dec!(90)),
        dec!(10),
        dec!(100),
        dec!(100000),
        &m15,
        &[],
        &s,
    )
    .unwrap();
    // Largo: (4 % + 3 %) × 100 × 10 / 100 / 360 por una noche.
    assert_eq!(
        t.funding,
        dec!(7) * dec!(100) * dec!(10) / dec!(100) / dec!(360)
    );
}

#[test]
fn the_risk_module_sizes_and_rejects_and_a_stop_ends_the_event() {
    let s = setup(dec!(100000));
    let mut m15 = flat(20, "2024-06-04T14:00:00Z");
    m15[3].low = dec!(97);
    let plans = vec![
        plan(0, 5, dec!(98)),
        // Misma pata del evento después del stop: no se abre.
        Plan {
            signal_index: 6,
            exit_index: 8,
            ..plan(6, 8, dec!(98))
        },
        // Otro evento con el stop a 10 ATR: el riesgo lo rechaza.
        Plan {
            event: 2,
            signal_index: 10,
            exit_index: 12,
            stop: dec!(80),
            ..plan(10, 12, dec!(80))
        },
        Plan {
            event: 3,
            signal_index: 13,
            exit_index: 15,
            ..plan(13, 15, dec!(98))
        },
    ];
    let run = run_plans(
        &plans,
        &m15,
        &[],
        &s,
        at("2024-01-01T00:00:00Z"),
        at("2025-01-01T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(run.trades.len(), 2);
    // 1 % de 100 000 sobre 2 puntos de riesgo.
    assert_eq!(run.trades[0].size, dec!(500));
    assert_eq!(run.trades[0].exit, StudyExit::Stop);
    assert_eq!(run.trades[1].event, 3);
    // El capital de la segunda ya descuenta la pérdida de la primera.
    assert_eq!(
        run.trades[1].equity_at_entry,
        dec!(100000) + run.trades[0].net_pnl
    );
    assert_eq!(run.rejected.len(), 1);
    assert_eq!(run.rejected[0].reason, Rejection::StopTooFar);
}

#[test]
fn only_plans_decided_inside_the_window_run() {
    let s = setup(dec!(100000));
    let m15 = flat(20, "2024-06-04T14:00:00Z");
    let plans = vec![
        plan(0, 2, dec!(98)),
        Plan {
            event: 2,
            ..plan(5, 7, dec!(98))
        },
    ];
    let from = m15[3].end();
    let run = run_plans(&plans, &m15, &[], &s, from, at("2025-01-01T00:00:00Z")).unwrap();
    assert_eq!(run.trades.len(), 1);
    assert_eq!(run.trades[0].signal_index, 5);
}

#[test]
fn returns_compound_by_closing_month_and_stress_scales_all_costs() {
    let s = setup(dec!(100000));
    let m15 = flat(10, "2024-06-04T14:00:00Z");
    let t = execute_plan(
        &plan(0, 8, dec!(98)),
        dec!(10),
        dec!(20),
        dec!(100000),
        &m15,
        &[],
        &s,
    )
    .unwrap();
    assert_eq!(
        trade_return(&t, dec!(1), &s).unwrap(),
        dec!(-11) / dec!(100000)
    );
    assert_eq!(
        trade_return(&t, dec!(1.5), &s).unwrap(),
        dec!(-16.5) / dec!(100000)
    );
    let closed = vec![
        (at("2024-06-04T20:00:00Z"), dec!(0.1)),
        (at("2024-06-28T20:00:00Z"), dec!(0.1)),
        (at("2024-07-01T20:00:00Z"), dec!(-0.05)),
    ];
    assert_eq!(
        monthly_returns(&closed, &[(2024, 5), (2024, 6), (2024, 7)]),
        vec![dec!(0), dec!(0.21), dec!(-0.05)]
    );
}

#[test]
fn the_orb_config_is_what_the_plans_use() {
    let cfg = OrbConfig {
        range_candles: 4,
        ..research().orb
    };
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // Con 4 velas de rango, la vela 3 (que rompe el de 2) es parte del rango.
    let m15 = orb_day(3, dec!(102));
    let (entries, _) = flags(s, &m15);
    assert!(orb_plans(&m15, s, &entries, &atr_const(m15.len(), dec!(1)), &cfg).is_empty());
}

// ── estadística ────────────────────────────────────────────────────────────────────────────

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

#[test]
fn the_distributions_match_known_values() {
    // Tablas: P(T > 2) con 10 g. l. = 0,036694; P(T > 1,812461) con 10 g. l. = 0,05.
    assert!(close(stats::student_t_upper(2.0, 10.0), 0.036_694, 1e-5));
    assert!(close(stats::student_t_upper(1.812_461, 10.0), 0.05, 1e-6));
    assert!(close(
        stats::student_t_upper(-2.0, 10.0),
        1.0 - 0.036_694,
        1e-5
    ));
    assert!(close(stats::normal_cdf(1.96), 0.975_002, 1e-6));
    assert!(close(stats::normal_cdf(-1.0), 0.158_655, 1e-6));
    assert!(close(stats::normal_inv(0.975), 1.959_964, 1e-6));
    assert!(close(stats::normal_inv(0.001), -3.090_232, 1e-6));
}

#[test]
fn the_t_test_and_the_moments_of_a_small_sample() {
    let xs = [1.0, 2.0, 3.0, 4.0, 5.0];
    let t = stats::t_test_greater(&xs).unwrap();
    // media 3, desvío √2,5, t = 3 / (√2,5 / √5) = 4,2426.
    assert!(close(t.t, 4.242_641, 1e-6));
    assert!(close(
        t.p_value,
        stats::student_t_upper(4.242_641, 4.0),
        1e-6
    ));
    let (skew, kurt) = stats::skew_kurtosis(&xs).unwrap();
    assert!(close(skew, 0.0, 1e-12));
    assert!(close(kurt, 1.7, 1e-12));
    assert!(stats::t_test_greater(&[1.0]).is_none());
}

#[test]
fn the_deflated_sharpe_penalizes_more_trials() {
    // 153 retornos mensuales alternados con media positiva.
    let xs: Vec<f64> = (0..153)
        .map(|i| if i % 2 == 0 { 0.03 } else { -0.01 })
        .collect();
    let few = stats::deflated_sharpe(&xs, 2).unwrap();
    let many = stats::deflated_sharpe(&xs, 1000).unwrap();
    assert!(many.sharpe0 > few.sharpe0);
    assert!(many.dsr < few.dsr);
    // SR0 con N = 350 y T = 153, a mano: √(1/152) × (0,4228 × Φ⁻¹(1 − 1/350) + 0,5772 × Φ⁻¹(1 − 1/(350e))).
    let d = stats::deflated_sharpe(&xs, 350).unwrap();
    let by_hand = (1.0f64 / 152.0).sqrt()
        * ((1.0 - 0.577_215_664_901_532_9) * stats::normal_inv(1.0 - 1.0 / 350.0)
            + 0.577_215_664_901_532_9
                * stats::normal_inv(1.0 - 1.0 / (350.0 * std::f64::consts::E)));
    assert!(close(d.sharpe0, by_hand, 1e-12));
    assert!(d.sharpe0 > 0.2 && d.sharpe0 < 0.3);
}

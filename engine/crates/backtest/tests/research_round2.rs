//! Estrategias de estudio de la ronda 2 (#486): tendencia mensual (A), momentum intradía (B) y
//! comprar y mantener (vara del criterio 5 de A). Series sintéticas sobre el calendario real de
//! XNYS; nunca la base real.

use std::path::PathBuf;

use cfds_backtest::research::{
    IntradayMomentumConfig, Plan, ResearchConfig, TrendConfig, buy_and_hold_returns,
    chained_return, intraday_momentum_plans, monthly_reviews, run_plans, session_bars, trend_plans,
};
use cfds_backtest::{
    BacktestConfig, Execution, Setup, apply_funding_scenario, commission, fill_price,
    setup_from_catalog,
};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::{TrendConfig as IndicatorTrend, wilder_atr};
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Catalog, ExchangeCalendar, Session, TradingWindowConfig, trading_window};
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

fn trend_cfg() -> TrendConfig {
    research().trend
}

fn momentum_cfg() -> IntradayMomentumConfig {
    research().intraday_momentum
}

fn setup() -> Setup {
    let mut catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    apply_funding_scenario(&mut catalog, &us500(), "TEST", dec!(4)).unwrap();
    setup_from_catalog(
        &catalog,
        &us500(),
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

fn candle(start: DateTime<Utc>, o: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: us500(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: o.max(c) + dec!(1),
        low: o.min(c) - dec!(1),
        close: c,
        volume: None,
    }
}

/// Velas de 15m de la sesión de contado de cada día con sesión en `[from, to]`. `price(k)` da el
/// cierre de la vela número `k`; la apertura es el cierre anterior.
fn series(
    session: &Session,
    from: NaiveDate,
    to: NaiveDate,
    price: impl Fn(usize) -> Decimal,
) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();
    let mut d = from;
    while d <= to {
        if let Some(Some((open, close))) = session.hours(d) {
            let mut s = open;
            while s + TimeDelta::minutes(15) <= close {
                let k = out.len();
                let o = out.last().map_or(price(0), |c| c.close);
                out.push(candle(s, o, price(k)));
                s += TimeDelta::minutes(15);
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

fn index_ending(m15: &[Candle], end: &str) -> usize {
    m15.iter().position(|c| c.end() == at(end)).unwrap()
}

// ── configuración ──────────────────────────────────────────────────────────────────────────

#[test]
fn the_round2_config_loads_and_rejects_bad_values() {
    let c = research();
    assert_eq!(
        (
            c.trend.lookback_months,
            c.trend.atr_period,
            c.trend.stop_atr
        ),
        (12, 20, dec!(3))
    );
    let m = c.intraday_momentum;
    assert_eq!(
        (
            m.signal_minutes_after_open,
            m.decision_minutes_before_close,
            m.atr_period,
            m.stop_atr
        ),
        (30, 30, 14, dec!(2))
    );
    assert_eq!(c.rule_round2.alpha, dec!(0.0125));
    assert_eq!(c.rule_round2.dsr_trials, 352);
    // La regla de la ronda 1 no cambia.
    assert_eq!(c.rule.dsr_trials, 350);
    let bad = cfg!("research.toml").replace(
        "signal_minutes_after_open = 30",
        "signal_minutes_after_open = 20",
    );
    assert!(ResearchConfig::from_toml(&bad).is_err());
    let bad = cfg!("research.toml").replace("lookback_months = 12", "lookback_months = 0");
    assert!(ResearchConfig::from_toml(&bad).is_err());
    let bad = cfg!("research.toml").replace("stop_atr = \"3\"", "stop_atr = \"0\"");
    assert!(ResearchConfig::from_toml(&bad).is_err());
}

// ── retorno encadenado en los vencimientos ─────────────────────────────────────────────────

#[test]
fn the_chained_return_skips_the_jump_of_each_rollover() {
    let t0 = at("2024-06-03T13:30:00Z");
    let step = TimeDelta::minutes(15);
    let m15 = vec![
        candle(t0, dec!(100), dec!(100)),
        candle(t0 + step, dec!(100), dec!(110)),
        // Contrato nuevo: abre 50 puntos más arriba (la base), sube 10 %.
        candle(t0 + step * 2, dec!(160), dec!(176)),
        candle(t0 + step * 3, dec!(176), dec!(176)),
    ];
    assert_eq!(chained_return(&m15, &[], 0, 1), Some(dec!(0.1)));
    let roll = [t0 + step * 2];
    // 100 → 110 (+10 %) y 160 → 176 (+10 %): 1,1 × 1,1 − 1.
    assert_eq!(chained_return(&m15, &roll, 0, 3), Some(dec!(0.21)));
    // Sin el vencimiento, el salto entraría.
    assert_eq!(chained_return(&m15, &[], 0, 3), Some(dec!(0.76)));
    // Un vencimiento antes de `i0` no cambia nada.
    assert_eq!(chained_return(&m15, &roll, 2, 3), Some(dec!(0)));
    assert_eq!(chained_return(&m15, &roll, 3, 1), None);
}

// ── familia A ──────────────────────────────────────────────────────────────────────────────

#[test]
fn the_monthly_review_is_the_last_but_one_candle_or_the_monday_after_a_friday_month_end() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = series(s, day("2024-04-01"), day("2024-07-31"), |_| dec!(100));
    let (entries, _) = flags(s, &m15);
    let reviews = monthly_reviews(&m15, s, &entries);
    let ends: Vec<DateTime<Utc>> = reviews.iter().map(|&(_, t)| m15[t].end()).collect();
    // Martes 30-04: cierre de 15:30–15:45 ET. Viernes 31-05: lunes 03-06 a las 09:45 ET.
    // Viernes 28-06: lunes 01-07 a las 09:45 ET. Julio termina el miércoles 31.
    assert_eq!(
        ends,
        vec![
            at("2024-04-30T19:45:00Z"),
            at("2024-06-03T13:45:00Z"),
            at("2024-07-01T13:45:00Z"),
            at("2024-07-31T19:45:00Z"),
        ]
    );
    // Las claves son los meses calendario (año × 12 + mes − 1), no el de la decisión.
    assert_eq!(reviews[1].0, 2024 * 12 + 4);
}

/// Precio que sube un poco en cada vela (tendencia alcista).
fn up(k: usize) -> Decimal {
    dec!(1000) + Decimal::from(k) / dec!(10)
}

#[test]
fn the_first_year_without_twelve_months_of_history_has_no_position() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = series(s, day("2023-01-02"), day("2024-03-29"), up);
    let (entries, force) = flags(s, &m15);
    let plans = trend_plans(&m15, s, &entries, &force, &[], &trend_cfg());
    assert!(!plans.is_empty());
    // La primera revisión con historia es la de enero de 2024 (base: enero de 2023).
    let first = plans
        .iter()
        .map(|p| m15[p.signal_index].end())
        .min()
        .unwrap();
    assert_eq!(first, at("2024-01-31T20:45:00Z"));
    assert!(plans.iter().all(|p| p.side == Side::Long));
}

#[test]
fn a_falling_market_is_short_with_the_stop_three_daily_atr_above() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = series(s, day("2023-01-02"), day("2024-03-29"), |k| {
        dec!(3000) - Decimal::from(k) / dec!(10)
    });
    let (entries, force) = flags(s, &m15);
    let plans = trend_plans(&m15, s, &entries, &force, &[], &trend_cfg());
    let p = plans
        .iter()
        .find(|p| m15[p.signal_index].end() == at("2024-01-31T20:45:00Z"))
        .unwrap();
    assert_eq!(p.side, Side::Short);
    let bars = session_bars(&m15, s);
    let atr = wilder_atr(&bars, 20);
    let i = bars
        .iter()
        .position(|b| b.day == day("2024-01-31"))
        .unwrap();
    let a = atr[i - 1].unwrap().round_dp(8);
    assert_eq!((p.risk_atr, p.stop), (a, p.reference + dec!(3) * a));
}

#[test]
fn a_month_of_a_closes_every_friday_reopens_on_monday_and_ends_at_the_next_review() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = series(s, day("2023-01-02"), day("2024-03-29"), up);
    let (entries, force) = flags(s, &m15);
    let plans = trend_plans(&m15, s, &entries, &force, &[], &trend_cfg());
    // Revisión del miércoles 31-01-2024 → revisión del jueves 29-02-2024.
    let month: Vec<&Plan> = plans
        .iter()
        .filter(|p| {
            let t = m15[p.signal_index].end();
            t >= at("2024-01-31T00:00:00Z") && t < at("2024-02-29T00:00:00Z")
        })
        .collect();
    assert!(month.iter().all(|p| p.event == month[0].event));
    let decided: Vec<DateTime<Utc>> = month.iter().map(|p| m15[p.signal_index].end()).collect();
    let exits: Vec<DateTime<Utc>> = month.iter().map(|p| m15[p.exit_index].end()).collect();
    // Lunes 19-02 es feriado (Presidents' Day): se reabre el martes 20-02.
    assert_eq!(
        decided,
        vec![
            at("2024-01-31T20:45:00Z"),
            at("2024-02-05T14:45:00Z"),
            at("2024-02-12T14:45:00Z"),
            at("2024-02-20T14:45:00Z"),
            at("2024-02-26T14:45:00Z"),
        ]
    );
    assert_eq!(
        exits,
        vec![
            at("2024-02-02T21:00:00Z"),
            at("2024-02-09T21:00:00Z"),
            at("2024-02-16T21:00:00Z"),
            at("2024-02-23T21:00:00Z"),
            // La última pata sale al cierre de la vela de la revisión siguiente.
            at("2024-02-29T20:45:00Z"),
        ]
    );
    // La revisión siguiente decide en esa misma vela y entra en la siguiente.
    assert!(
        plans
            .iter()
            .any(|p| m15[p.signal_index].end() == at("2024-02-29T20:45:00Z"))
    );
}

#[test]
fn the_rollover_jump_does_not_flip_the_trend_signal() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // Cae dentro de cada contrato, pero el contrato nuevo abre muy arriba (la base).
    let raw = series(s, day("2023-01-02"), day("2024-02-29"), |k| {
        dec!(3000) - Decimal::from(k) / dec!(10)
    });
    let roll_at = at("2023-06-20T13:30:00Z");
    let r = raw.iter().position(|c| c.start == roll_at).unwrap();
    let m15: Vec<Candle> = raw
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i >= r {
                let mut c = c.clone();
                for x in [&mut c.open, &mut c.high, &mut c.low, &mut c.close] {
                    *x += dec!(2000);
                }
                c
            } else {
                c.clone()
            }
        })
        .collect();
    let (entries, force) = flags(s, &m15);
    let review = |rolls: &[DateTime<Utc>]| {
        trend_plans(&m15, s, &entries, &force, rolls, &trend_cfg())
            .into_iter()
            .find(|p| m15[p.signal_index].end() == at("2024-01-31T20:45:00Z"))
            .unwrap()
            .side
    };
    assert_eq!(review(&[roll_at]), Side::Short);
    // Sin encadenar, el salto daría vuelta la señal.
    assert_eq!(review(&[]), Side::Long);
}

#[test]
fn a_missing_base_review_leaves_the_month_without_position() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let full = series(s, day("2023-01-02"), day("2024-03-29"), up);
    // Faltan las velas del 31-01-2023 al 28-02-2023: enero de 2023 no tiene revisión (la de
    // febrero pasa al 01-03).
    let m15: Vec<Candle> = full
        .into_iter()
        .filter(|c| c.start < at("2023-01-31T00:00:00Z") || c.start >= at("2023-03-01T00:00:00Z"))
        .collect();
    let (entries, force) = flags(s, &m15);
    let plans = trend_plans(&m15, s, &entries, &force, &[], &trend_cfg());
    let first = plans
        .iter()
        .map(|p| m15[p.signal_index].end())
        .min()
        .unwrap();
    // La de enero de 2024 no tiene base: la primera es la de febrero de 2024.
    assert_eq!(first, at("2024-02-29T20:45:00Z"));
}

#[test]
fn trend_signals_do_not_repaint_when_more_candles_arrive() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = series(s, day("2023-01-02"), day("2024-06-28"), |k| {
        let k = i64::try_from(k).unwrap();
        dec!(2000) + Decimal::from((k * 37) % 101) - Decimal::from(k / 300)
    });
    let (entries, force) = flags(s, &m15);
    let full = trend_plans(&m15, s, &entries, &force, &[], &trend_cfg());
    assert!(full.len() >= 10);
    for cut in [7000, 7600, 8200] {
        let prefix = trend_plans(
            &m15[..cut],
            s,
            &entries[..cut],
            &force[..cut],
            &[],
            &trend_cfg(),
        );
        for p in &prefix {
            let f = full
                .iter()
                .find(|f| f.signal_index == p.signal_index)
                .unwrap();
            assert_eq!(
                (f.side, f.reference, f.stop, f.risk_atr),
                (p.side, p.reference, p.stop, p.risk_atr)
            );
            // La salida solo se conoce cuando llega la revisión siguiente o un cierre forzado.
            if p.exit_index < cut - 1 {
                assert_eq!(f.exit_index, p.exit_index);
            }
        }
    }
}

#[test]
fn a_stop_at_three_daily_atr_passes_the_risk_module_and_pays_funding_overnight() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = series(s, day("2023-01-02"), day("2024-03-29"), up);
    let (entries, force) = flags(s, &m15);
    let plans = trend_plans(&m15, s, &entries, &force, &[], &trend_cfg());
    let run = run_plans(
        &plans,
        &m15,
        &force,
        &setup(),
        at("2024-02-01T00:00:00Z"),
        at("2024-03-01T00:00:00Z"),
    )
    .unwrap();
    // Justo en el tope de 3 ATR (inclusive) de config/risk.toml: no se rechaza ninguna.
    assert!(run.rejected.is_empty(), "{:?}", run.rejected);
    // Las reaperturas del 05, 12, 20 y 26 de febrero y la revisión del 29.
    assert_eq!(run.trades.len(), 5);
    for t in &run.trades {
        assert_eq!(t.side, Side::Long);
        // Cada pata pasa noches: paga financiación (sin base del futuro en el test).
        assert!(t.funding > Decimal::ZERO);
        assert!(t.net_pnl < t.mid_pnl);
    }
}

// ── familia B ──────────────────────────────────────────────────────────────────────────────

/// ATR de 15m constante.
fn atr_const(n: usize, v: Decimal) -> Vec<Option<Decimal>> {
    vec![Some(v); n]
}

/// Serie plana en 100 entre `from` y `to`, salvo que el precio de la vela que empieza en
/// `moved` en adelante, ese día, es `level`.
fn day_move(from: &str, to: &str, moved: &str, level: Decimal) -> Vec<Candle> {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let base = series(s, day(from), day(to), |_| dec!(100));
    let m = at(moved);
    base.into_iter()
        .map(|c| {
            if c.start >= m && s.trading_day(c.start) == s.trading_day(m) {
                candle(c.start, level, level)
            } else {
                c
            }
        })
        .collect()
}

#[test]
fn a_rise_in_the_first_half_hour_is_a_long_in_the_last_half_hour() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // Martes 04-06-2024 (EDT): sube desde las 09:30 ET.
    let m15 = day_move(
        "2024-06-03",
        "2024-06-05",
        "2024-06-04T13:30:00Z",
        dec!(101),
    );
    let (entries, _) = flags(s, &m15);
    let plans = intraday_momentum_plans(
        &m15,
        s,
        &entries,
        &atr_const(m15.len(), dec!(2)),
        &[],
        &momentum_cfg(),
    );
    let p = plans
        .iter()
        .find(|p| s.trading_day(m15[p.signal_index].start) == day("2024-06-04"))
        .unwrap();
    assert_eq!(p.side, Side::Long);
    // Decisión al cierre de 15:15–15:30 ET, entrada 15:30, salida al cierre de 15:45–16:00.
    assert_eq!(p.signal_index, index_ending(&m15, "2024-06-04T19:30:00Z"));
    assert_eq!(p.exit_index, index_ending(&m15, "2024-06-04T20:00:00Z"));
    assert_eq!(
        (p.reference, p.stop, p.risk_atr),
        (dec!(101), dec!(97), dec!(2))
    );
    // El lunes no tiene el cierre anterior; el miércoles vuelve a 100 a la apertura: corto.
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[1].side, Side::Short);
}

#[test]
fn a_fall_is_a_short_and_daylight_saving_time_moves_the_hours_in_utc() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // Martes 05-03-2024 (EST, UTC−5): baja después de las 09:30 ET.
    let winter = day_move("2024-03-04", "2024-03-06", "2024-03-05T14:30:00Z", dec!(99));
    let (entries, _) = flags(s, &winter);
    let plans = intraday_momentum_plans(
        &winter,
        s,
        &entries,
        &atr_const(winter.len(), dec!(1)),
        &[],
        &momentum_cfg(),
    );
    // Martes corto; el miércoles vuelve a 100: largo.
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].side, Side::Short);
    assert_eq!(plans[0].stop, dec!(101));
    assert_eq!(
        winter[plans[0].signal_index].end(),
        at("2024-03-05T20:30:00Z")
    );
    // Martes 12-03-2024, después del cambio de horario de EE. UU. (EDT, UTC−4).
    let summer = day_move("2024-03-11", "2024-03-13", "2024-03-12T13:30:00Z", dec!(99));
    let (entries, _) = flags(s, &summer);
    let plans = intraday_momentum_plans(
        &summer,
        s,
        &entries,
        &atr_const(summer.len(), dec!(1)),
        &[],
        &momentum_cfg(),
    );
    assert_eq!(
        summer[plans[0].signal_index].end(),
        at("2024-03-12T19:30:00Z")
    );
}

#[test]
fn the_last_day_of_the_week_is_not_traded() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // Viernes 07-06-2024.
    let m15 = day_move(
        "2024-06-06",
        "2024-06-07",
        "2024-06-07T13:30:00Z",
        dec!(101),
    );
    let (entries, _) = flags(s, &m15);
    let plans = intraday_momentum_plans(
        &m15,
        s,
        &entries,
        &atr_const(m15.len(), dec!(2)),
        &[],
        &momentum_cfg(),
    );
    assert!(plans.is_empty());
    // Jueves 27-03-2024: el viernes 29 es feriado (Viernes Santo).
    let m15 = day_move(
        "2024-03-26",
        "2024-03-28",
        "2024-03-28T13:30:00Z",
        dec!(101),
    );
    let (entries, _) = flags(s, &m15);
    let plans = intraday_momentum_plans(
        &m15,
        s,
        &entries,
        &atr_const(m15.len(), dec!(2)),
        &[],
        &momentum_cfg(),
    );
    assert!(plans.is_empty());
}

#[test]
fn an_early_close_uses_the_last_half_hour_before_that_close() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // Miércoles 03-07-2024: cierre anticipado a las 13:00 ET.
    let m15 = day_move(
        "2024-07-02",
        "2024-07-03",
        "2024-07-03T13:30:00Z",
        dec!(101),
    );
    let (entries, _) = flags(s, &m15);
    let plans = intraday_momentum_plans(
        &m15,
        s,
        &entries,
        &atr_const(m15.len(), dec!(2)),
        &[],
        &momentum_cfg(),
    );
    assert_eq!(plans.len(), 1);
    assert_eq!(m15[plans[0].signal_index].end(), at("2024-07-03T16:30:00Z"));
    assert_eq!(m15[plans[0].exit_index].end(), at("2024-07-03T17:00:00Z"));
}

#[test]
fn a_missing_candle_or_a_flat_first_half_hour_skips_the_day() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = day_move(
        "2024-06-03",
        "2024-06-05",
        "2024-06-04T13:30:00Z",
        dec!(101),
    );
    // ¿Hay operación el martes 04-06?
    let run = |m15: &[Candle]| {
        let (entries, _) = flags(s, m15);
        intraday_momentum_plans(
            m15,
            s,
            &entries,
            &atr_const(m15.len(), dec!(2)),
            &[],
            &momentum_cfg(),
        )
        .iter()
        .any(|p| s.trading_day(m15[p.signal_index].start) == day("2024-06-04"))
    };
    assert!(run(&m15));
    // Sin el cierre del lunes (15:45–16:00 ET).
    let no_previous: Vec<Candle> = m15
        .iter()
        .filter(|c| c.start != at("2024-06-03T19:45:00Z"))
        .cloned()
        .collect();
    assert!(!run(&no_previous));
    // Sin la vela de la señal (09:45–10:00 ET).
    let no_signal: Vec<Candle> = m15
        .iter()
        .filter(|c| c.start != at("2024-06-04T13:45:00Z"))
        .cloned()
        .collect();
    assert!(!run(&no_signal));
    // Sin la vela de la salida.
    let no_exit: Vec<Candle> = m15
        .iter()
        .filter(|c| c.start != at("2024-06-04T19:45:00Z"))
        .cloned()
        .collect();
    assert!(!run(&no_exit));
    // Plano: retorno cero, no se opera.
    let flat = series(s, day("2024-06-03"), day("2024-06-05"), |_| dec!(100));
    assert!(!run(&flat));
}

#[test]
fn a_rollover_inside_the_signal_window_is_chained() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // El contrato nuevo empieza el martes 04-06 a las 09:30 ET, 5 puntos más arriba, y cae 1.
    let m15 = day_move(
        "2024-06-03",
        "2024-06-05",
        "2024-06-04T13:30:00Z",
        dec!(104),
    );
    let roll = [at("2024-06-04T13:30:00Z")];
    let m15: Vec<Candle> = m15
        .into_iter()
        .map(|c| {
            if c.start == roll[0] {
                candle(c.start, dec!(105), dec!(104))
            } else {
                c
            }
        })
        .collect();
    let (entries, _) = flags(s, &m15);
    let run = |rolls: &[DateTime<Utc>]| {
        intraday_momentum_plans(
            &m15,
            s,
            &entries,
            &atr_const(m15.len(), dec!(2)),
            rolls,
            &momentum_cfg(),
        )
    };
    assert_eq!(run(&roll)[0].side, Side::Short);
    assert_eq!(run(&[])[0].side, Side::Long);
}

#[test]
fn momentum_signals_do_not_repaint_when_more_candles_arrive() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = series(s, day("2024-04-01"), day("2024-06-28"), |k| {
        let k = i64::try_from(k).unwrap();
        dec!(1000) + Decimal::from((k * 37) % 23)
    });
    let (entries, _) = flags(s, &m15);
    let atr = wilder_atr(&m15, 14);
    let full = intraday_momentum_plans(&m15, s, &entries, &atr, &[], &momentum_cfg());
    assert!(full.len() >= 20);
    for cut in [500, 900, 1300] {
        let prefix = intraday_momentum_plans(
            &m15[..cut],
            s,
            &entries[..cut],
            &atr[..cut],
            &[],
            &momentum_cfg(),
        );
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

// ── comprar y mantener ─────────────────────────────────────────────────────────────────────

#[test]
fn buy_and_hold_pays_its_costs_and_chains_the_rollover_without_the_jump() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let setup = setup();
    let (from, to) = (at("2024-04-01T00:00:00Z"), at("2024-07-01T00:00:00Z"));
    let flat = series(s, day("2024-04-01"), day("2024-06-28"), |_| dec!(5000));
    let r = buy_and_hold_returns(&flat, s, &[], &setup, from, to).unwrap();
    let months: Vec<(i32, u32)> = r.iter().map(|(m, _)| *m).collect();
    assert_eq!(months, vec![(2024, 4), (2024, 5), (2024, 6)]);
    // Precio plano: cada mes pierde solo sus costos (financiación y spread).
    assert!(r.iter().all(|(_, x)| *x < Decimal::ZERO));
    // Abril: el spread y el slippage de la entrada y 30 noches (del 01-04 al 30-04, 23:00 de
    // Madrid) de (4 % + tasa administrativa) / base de días sobre 5000.
    let c = &setup.costs;
    let entry = fill_price(
        Side::Long,
        Execution::Entry,
        dec!(5000),
        c,
        &setup.backtest.costs,
    ) - dec!(5000)
        + commission(c, Decimal::ONE, dec!(5000));
    let funding = dec!(30) * dec!(5000) * (dec!(4) + c.funding_admin_fee_pct)
        / dec!(100)
        / c.funding_day_count;
    assert_eq!(r[0].1, -(entry + funding) / dec!(5000));
    // Con un salto de vencimiento a mitad de mayo, encadenado: lo mismo.
    let roll = at("2024-05-15T13:30:00Z");
    let jumped: Vec<Candle> = flat
        .iter()
        .map(|c| {
            if c.start >= roll {
                candle(c.start, dec!(5100), dec!(5100))
            } else {
                c.clone()
            }
        })
        .collect();
    // (Solo cambia la financiación, que se cobra sobre el precio del contrato nuevo.)
    let chained = buy_and_hold_returns(&jumped, s, &[roll], &setup, from, to).unwrap();
    for ((_, a), (_, b)) in r.iter().zip(&chained) {
        assert!((*a - *b).abs() < dec!(0.0002), "{a} vs {b}");
    }
    // Sin encadenar, mayo ganaría el salto.
    let naive = buy_and_hold_returns(&jumped, s, &[], &setup, from, to).unwrap();
    assert!(naive[1].1 > dec!(0.01));
}

#[test]
fn buy_and_hold_follows_the_price() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let setup = setup();
    let (from, to) = (at("2024-04-01T00:00:00Z"), at("2024-05-01T00:00:00Z"));
    // Sube de 5000 a 5500 dentro de abril.
    let m15 = series(s, day("2024-04-01"), day("2024-04-30"), |k| {
        if k == 0 { dec!(5000) } else { dec!(5500) }
    });
    let r = buy_and_hold_returns(&m15, s, &[], &setup, from, to).unwrap();
    assert_eq!(r.len(), 1);
    // +10 % menos los costos (spread de entrada y salida, ~30 noches).
    assert!(r[0].1 > dec!(0.09) && r[0].1 < dec!(0.1), "{}", r[0].1);
}

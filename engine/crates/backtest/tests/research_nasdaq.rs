//! H1 del programa Nasdaq (#526): ventanas antes del comunicado del FOMC
//! (`docs/analysis/2026-11-programa-nasdaq-preregistro.md` §2 y §3). Series sintéticas sobre el
//! calendario real de XNYS; nunca la base real.

use std::path::PathBuf;

use cfds_backtest::research::{
    CalendarRow, EventWindow, PreFomcConfig, ReleaseStatus, ResearchConfig, Skip, event_schedule,
    managed_returns, pre_fomc_windows,
};
use cfds_backtest::{BacktestConfig, Setup, apply_funding_scenario, setup_from_catalog};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::TrendConfig as IndicatorTrend;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Catalog, ExchangeCalendar, Session};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, NaiveDate, NaiveTime, TimeDelta, Utc};
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

fn calendar() -> ExchangeCalendar {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap()
}

fn research() -> ResearchConfig {
    ResearchConfig::from_toml(cfg!("research.toml")).unwrap()
}

fn h1() -> PreFomcConfig {
    research().pre_fomc
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

fn candle(start: DateTime<Utc>, o: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: us100(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: o.max(c) + dec!(1),
        low: o.min(c) - dec!(1),
        close: c,
        volume: None,
    }
}

/// Velas de 15m de la sesión de contado de cada día con sesión en `[from, to]`; `price(t)` es el
/// cierre de la vela que empieza en `t` y la apertura es el cierre anterior.
fn series(
    session: &Session,
    from: NaiveDate,
    to: NaiveDate,
    price: impl Fn(DateTime<Utc>) -> Decimal,
) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();
    let mut d = from;
    while d <= to {
        if let Some(Some((open, close))) = session.hours(d) {
            let mut s = open;
            while s + TimeDelta::minutes(15) <= close {
                let o = out.last().map_or(price(s), |c| c.close);
                out.push(candle(s, o, price(s)));
                s += TimeDelta::minutes(15);
            }
        }
        d = d.succ_opt().unwrap();
    }
    out
}

fn flat(session: &Session, from: &str, to: &str) -> Vec<Candle> {
    series(session, day(from), day(to), |_| dec!(20000))
}

fn row(key: &str, status: ReleaseStatus, scheduled: Option<&str>, published: &str) -> CalendarRow {
    CalendarRow {
        key: key.to_owned(),
        status,
        scheduled_at: scheduled.map(at),
        published_at: at(published),
    }
}

fn scheduled(key: &str, at_utc: &str) -> CalendarRow {
    row(
        key,
        ReleaseStatus::Scheduled,
        Some(at_utc),
        "2000-01-01T00:00:00Z",
    )
}

fn start_of(m15: &[Candle], i: usize) -> DateTime<Utc> {
    m15[i].start
}

/// Inicio (UTC) de la vela de la entrada y de la de la salida.
fn times(m15: &[Candle], w: &EventWindow) -> (DateTime<Utc>, DateTime<Utc>) {
    (start_of(m15, w.entry()), start_of(m15, w.exit()))
}

// ── configuración ──────────────────────────────────────────────────────────────────────────

#[test]
fn the_nasdaq_config_loads_and_rejects_bad_values() {
    let c = research();
    assert_eq!(c.pre_fomc.timezone, chrono_tz::America::New_York);
    assert_eq!(
        c.pre_fomc.entry_time,
        NaiveTime::from_hms_opt(14, 0, 0).unwrap()
    );
    assert_eq!(c.pre_fomc.exit_minutes_before, 30);
    assert_eq!(
        c.macro_day.entry_time,
        NaiveTime::from_hms_opt(8, 0, 0).unwrap()
    );
    assert_eq!(c.macro_day.out_of_session_spread, dec!(2.5));
    assert_eq!(c.rule_nasdaq.dsr_trials, 356);
    for (from, to) in [
        ("exit_minutes_before = 30", "exit_minutes_before = 20"),
        ("entry_time = \"14:00\"", "entry_time = \"14:05\""),
        (
            "out_of_session_spread_points = \"2.5\"",
            "out_of_session_spread_points = \"0\"",
        ),
    ] {
        let bad = cfg!("research.toml").replace(from, to);
        assert!(ResearchConfig::from_toml(&bad).is_err(), "{to}");
    }
}

// ── ventanas ───────────────────────────────────────────────────────────────────────────────

#[test]
fn the_window_follows_new_york_time_in_winter_and_summer() {
    let cal = calendar();
    let s = cal.session_for(&us100()).unwrap();
    // Invierno (EST, UTC−5): comunicado miércoles 31-01-2024 14:00 ET = 19:00 UTC.
    // Verano (EDT, UTC−4): comunicado miércoles 31-07-2024 14:00 ET = 18:00 UTC.
    let m15 = [
        flat(s, "2024-01-29", "2024-02-01"),
        flat(s, "2024-07-29", "2024-08-01"),
    ]
    .concat();
    let rows = [
        scheduled("2024-01-31", "2024-01-31T19:00:00Z"),
        scheduled("2024-07-31", "2024-07-31T18:00:00Z"),
    ];
    let out = pre_fomc_windows(&m15, s, &rows, &h1());
    assert_eq!(out.len(), 2);
    let w0 = out[0].window.unwrap();
    assert_eq!(
        times(&m15, &w0),
        (at("2024-01-30T19:00:00Z"), at("2024-01-31T18:30:00Z"))
    );
    // La decisión es al cierre de la vela anterior (t + 1).
    assert_eq!(m15[w0.decision_in].end(), at("2024-01-30T19:00:00Z"));
    let w1 = out[1].window.unwrap();
    assert_eq!(
        times(&m15, &w1),
        (at("2024-07-30T18:00:00Z"), at("2024-07-31T17:30:00Z"))
    );
}

#[test]
fn other_release_times_and_days_move_the_exit_and_the_previous_business_day() {
    let cal = calendar();
    let s = cal.session_for(&us100()).unwrap();
    let m15 = [
        flat(s, "2011-03-14", "2011-03-16"),
        flat(s, "2012-04-23", "2012-04-26"),
        flat(s, "2020-11-03", "2020-11-06"),
        flat(s, "2024-01-26", "2024-01-30"),
    ]
    .concat();
    let rows = [
        // 12:30 ET con conferencia (2011–2013): salida 12:00 ET.
        scheduled("2012-04-25", "2012-04-25T16:30:00Z"),
        // Jueves: el día hábil anterior es el miércoles.
        scheduled("2020-11-05", "2020-11-05T19:00:00Z"),
        // Lunes (sintético): el día hábil anterior es el viernes.
        scheduled("2024-01-29", "2024-01-29T19:00:00Z"),
        // 14:15 ET: salida 13:45 ET (el 15-03-2011 ya es horario de verano).
        scheduled("2011-03-15", "2011-03-15T18:15:00Z"),
    ];
    let out = pre_fomc_windows(&m15, s, &rows, &h1());
    let got: Vec<_> = out
        .iter()
        .map(|o| times(&m15, &o.window.expect(&o.row.key)))
        .collect();
    assert_eq!(
        got,
        vec![
            (at("2011-03-14T18:00:00Z"), at("2011-03-15T17:45:00Z")),
            (at("2012-04-24T18:00:00Z"), at("2012-04-25T16:00:00Z")),
            (at("2020-11-04T19:00:00Z"), at("2020-11-05T18:30:00Z")),
            (at("2024-01-26T19:00:00Z"), at("2024-01-29T18:30:00Z")),
        ]
    );
}

#[test]
fn unscheduled_releases_are_ignored_and_do_not_cancel_a_scheduled_one() {
    let cal = calendar();
    let s = cal.session_for(&us100()).unwrap();
    let m15 = flat(s, "2024-01-29", "2024-02-01");
    let rows = [
        scheduled("2024-01-31", "2024-01-31T19:00:00Z"),
        // No programado dentro de la ventana: se supo recién al publicarse.
        row(
            "2024-01-30u",
            ReleaseStatus::Unscheduled,
            Some("2024-01-31T15:00:00Z"),
            "2024-01-31T15:00:00Z",
        ),
    ];
    let out = pre_fomc_windows(&m15, s, &rows, &h1());
    let by_key = |k: &str| out.iter().find(|o| o.row.key == k).unwrap().window;
    assert!(by_key("2024-01-31").is_ok());
    assert_eq!(by_key("2024-01-30u"), Err(Skip::Unscheduled));
}

#[test]
fn a_row_counts_only_once_published_before_the_entry_decision() {
    let cal = calendar();
    let s = cal.session_for(&us100()).unwrap();
    let m15 = flat(s, "2024-01-29", "2024-02-01");
    let decision = "2024-01-30T19:00:00Z"; // cierre de la vela de las 13:45 ET
    let just_in = row(
        "a",
        ReleaseStatus::Scheduled,
        Some("2024-01-31T19:00:00Z"),
        decision,
    );
    let late = row(
        "a",
        ReleaseStatus::Scheduled,
        Some("2024-01-31T19:00:00Z"),
        "2024-01-30T19:00:01Z",
    );
    assert!(
        pre_fomc_windows(&m15, s, &[just_in], &h1())[0]
            .window
            .is_ok()
    );
    assert_eq!(
        pre_fomc_windows(&m15, s, &[late], &h1())[0].window,
        Err(Skip::NotPublished)
    );
}

#[test]
fn the_last_row_published_before_the_decision_wins() {
    let cal = calendar();
    let s = cal.session_for(&us100()).unwrap();
    let m15 = flat(s, "2024-01-29", "2024-02-08");
    let original = row(
        "m",
        ReleaseStatus::Scheduled,
        Some("2024-01-31T19:00:00Z"),
        "2023-06-01T00:00:00Z",
    );
    // Reprogramada una semana después, avisada antes de la entrada original.
    let moved = row(
        "m",
        ReleaseStatus::Rescheduled,
        Some("2024-02-07T19:00:00Z"),
        "2024-01-15T00:00:00Z",
    );
    let out = pre_fomc_windows(&m15, s, &[original.clone(), moved], &h1());
    assert_eq!(out[0].window, Err(Skip::Superseded));
    assert_eq!(
        times(&m15, &out[1].window.unwrap()),
        (at("2024-02-06T19:00:00Z"), at("2024-02-07T18:30:00Z"))
    );
    // Cancelada antes de la entrada: no se opera ninguna.
    let cancelled = row("m", ReleaseStatus::Cancelled, None, "2024-01-15T00:00:00Z");
    let out = pre_fomc_windows(&m15, s, &[original.clone(), cancelled], &h1());
    assert_eq!(out[0].window, Err(Skip::Superseded));
    assert_eq!(out[1].window, Err(Skip::Cancelled));
    // Cancelada DESPUÉS de la entrada: en la decisión no se sabía; se opera.
    let late_cancel = row("m", ReleaseStatus::Cancelled, None, "2024-01-31T00:00:00Z");
    let out = pre_fomc_windows(&m15, s, &[original, late_cancel], &h1());
    assert!(out[0].window.is_ok());
}

#[test]
fn missing_candles_and_early_closes_skip_the_event() {
    let cal = calendar();
    let s = cal.session_for(&us100()).unwrap();
    let full = flat(s, "2024-01-29", "2024-02-01");
    let ev = [scheduled("2024-01-31", "2024-01-31T19:00:00Z")];
    for (gone, why) in [
        ("2024-01-30T18:45:00Z", Skip::MissingEntryCandle), // decisión de entrada
        ("2024-01-30T19:00:00Z", Skip::MissingEntryCandle), // vela de entrada
        ("2024-01-31T18:15:00Z", Skip::MissingExitCandle),  // decisión de salida
        ("2024-01-31T18:30:00Z", Skip::MissingExitCandle),  // vela de salida
    ] {
        let m15: Vec<Candle> = full
            .iter()
            .filter(|c| c.start != at(gone))
            .cloned()
            .collect();
        assert_eq!(
            pre_fomc_windows(&m15, s, &ev, &h1())[0].window,
            Err(why),
            "{gone}"
        );
    }
    // Lunes 02-12-2024 (sintético): el viernes 29-11 cierra a las 13:00 ET, sin vela de las 14:00.
    let m15 = flat(s, "2024-11-27", "2024-12-03");
    let ev = [scheduled("2024-12-02", "2024-12-02T19:00:00Z")];
    assert_eq!(
        pre_fomc_windows(&m15, s, &ev, &h1())[0].window,
        Err(Skip::NoEntryInSession)
    );
}

#[test]
fn windows_do_not_repaint_when_more_candles_arrive() {
    let cal = calendar();
    let s = cal.session_for(&us100()).unwrap();
    let full = flat(s, "2024-01-29", "2024-03-29");
    let ev = [
        scheduled("2024-01-31", "2024-01-31T19:00:00Z"),
        scheduled("2024-03-20", "2024-03-20T18:00:00Z"),
    ];
    let cut = full
        .iter()
        .position(|c| c.start >= at("2024-02-02T00:00:00Z"))
        .unwrap();
    let early = pre_fomc_windows(&full[..cut], s, &ev[..1], &h1());
    let late = pre_fomc_windows(&full, s, &ev, &h1());
    assert_eq!(early[0], late[0]);
}

// ── retornos ───────────────────────────────────────────────────────────────────────────────

#[test]
fn only_the_event_window_is_exposed_and_months_without_events_are_zero() {
    let cal = calendar();
    let s = cal.session_for(&us100()).unwrap();
    let setup = setup();
    // Plano en 20000; salta a 20200 dentro de la ventana (31-01 16:00 UTC) y a 21000 fuera de
    // ella (15-02), sin volver.
    let m15 = series(s, day("2023-12-28"), day("2024-03-28"), |t| {
        if t >= at("2024-02-15T15:00:00Z") {
            dec!(21000)
        } else if t >= at("2024-01-31T16:00:00Z") {
            dec!(20200)
        } else {
            dec!(20000)
        }
    });
    let ev = [scheduled("2024-01-31", "2024-01-31T19:00:00Z")];
    let out = pre_fomc_windows(&m15, s, &ev, &h1());
    let windows: Vec<EventWindow> = out.iter().filter_map(|o| o.window.ok()).collect();
    let schedule = event_schedule(&windows);
    let range = (at("2024-01-01T00:00:00Z"), at("2024-04-01T00:00:00Z"));
    let net = managed_returns(&m15, s, &[], &setup, range, &schedule, Decimal::ONE).unwrap();
    let gross = managed_returns(&m15, s, &[], &setup, range, &schedule, Decimal::ZERO).unwrap();
    assert_eq!(net.len(), 3);
    // Antes de costos: exactamente el salto de la ventana (+1 %); febrero y marzo en cero.
    assert_eq!(gross[0], ((2024, 1), dec!(0.01)));
    assert_eq!(gross[1], ((2024, 2), Decimal::ZERO));
    assert_eq!(gross[2], ((2024, 3), Decimal::ZERO));
    // Con costos: menos (spread, slippage y una noche de financiación), y nada fuera.
    assert!(
        net[0].1 < dec!(0.01) && net[0].1 > dec!(0.0095),
        "{:?}",
        net[0]
    );
    assert_eq!(net[1].1, Decimal::ZERO);
    // El estrés × 1,5 cuesta más.
    let stressed = managed_returns(&m15, s, &[], &setup, range, &schedule, dec!(1.5)).unwrap();
    assert!(stressed[0].1 < net[0].1);
}

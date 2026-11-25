//! H2 del programa Nasdaq (#526): días de CPI y empleo del BLS
//! (`docs/analysis/2026-11-programa-nasdaq-preregistro.md` §2 y §4). Series sintéticas sobre el
//! calendario real de XNYS; nunca la base real.

use std::path::PathBuf;

use cfds_backtest::research::{
    CalendarRow, EventWindow, MacroDayRule, ReleaseStatus, ResearchConfig, Skip, event_schedule,
    macro_day_windows, managed_returns, managed_returns_with_spreads,
};
use cfds_backtest::{BacktestConfig, Setup, apply_funding_scenario, setup_from_catalog};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::TrendConfig as IndicatorTrend;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Catalog, ExchangeCalendar, Session};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeDelta, Utc, Weekday};
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

fn h2() -> MacroDayRule {
    research().macro_day.rule()
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

/// Velas de 15m de todo el día (00:00 a 24:00 UTC) de lunes a viernes en `[from, to]`, como el
/// futuro, que cotiza fuera de la sesión de contado; `price(t)` es el cierre de la vela que
/// empieza en `t` y la apertura es el cierre anterior.
fn series(from: NaiveDate, to: NaiveDate, price: impl Fn(DateTime<Utc>) -> Decimal) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();
    let mut d = from;
    while d <= to {
        if !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
            let mut s = d.and_time(NaiveTime::MIN).and_utc();
            for _ in 0..96 {
                let o = out.last().map_or(price(s), |c| c.close);
                out.push(candle(s, o, price(s)));
                s += TimeDelta::minutes(15);
            }
        }
        d = d.succ_opt().unwrap();
    }
    out
}

fn flat(from: &str, to: &str) -> Vec<Candle> {
    series(day(from), day(to), |_| dec!(20000))
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

/// Inicio (UTC) de la vela de la entrada y de la de la salida.
fn times(m15: &[Candle], w: &EventWindow) -> (DateTime<Utc>, DateTime<Utc>) {
    (m15[w.entry()].start, m15[w.exit()].start)
}

fn session(cal: &ExchangeCalendar) -> &Session {
    cal.session_for(&us100()).unwrap()
}

// ── configuración ──────────────────────────────────────────────────────────────────────────

#[test]
fn the_h2_config_loads_with_the_frozen_values() {
    let c = research().macro_day;
    assert_eq!(c.timezone, chrono_tz::America::New_York);
    assert_eq!(c.entry_time, NaiveTime::from_hms_opt(8, 0, 0).unwrap());
    assert_eq!(c.release_time, NaiveTime::from_hms_opt(8, 30, 0).unwrap());
    assert_eq!(c.out_of_session_spread, dec!(2.5));
    assert_eq!(c.descriptive_out_of_session_spread["US500"], dec!(1.6));
    assert_eq!(c.descriptive_out_of_session_spread["US30"], dec!(5.0));
    for (from, to) in [
        ("release_time = \"08:30\"", "release_time = \"08:31\""),
        ("US30 = \"5.0\"", "US30 = \"0\""),
        ("US30 = \"5.0\"", "US30 = \"x\""),
    ] {
        let bad = cfg!("research.toml").replace(from, to);
        assert!(ResearchConfig::from_toml(&bad).is_err(), "{to}");
    }
}

// ── ventanas ───────────────────────────────────────────────────────────────────────────────

#[test]
fn the_window_follows_new_york_time_in_winter_and_summer() {
    let cal = calendar();
    let s = session(&cal);
    // Invierno (EST): empleo del viernes 05-01-2024 a las 08:30 ET = 13:30 UTC.
    // Verano (EDT): CPI del jueves 11-07-2024 a las 08:30 ET = 12:30 UTC.
    let m15 = [
        flat("2024-01-04", "2024-01-05"),
        flat("2024-07-10", "2024-07-11"),
    ]
    .concat();
    let rows = [
        scheduled("employment_situation/2023-12", "2024-01-05T13:30:00Z"),
        scheduled("cpi/2024-06", "2024-07-11T12:30:00Z"),
    ];
    let out = macro_day_windows(&m15, s, &rows, &h2());
    let w0 = out[0].window.unwrap();
    // Entrada en la apertura de la vela de las 08:00 ET; salida en la de las 15:45 ET.
    assert_eq!(
        times(&m15, &w0),
        (at("2024-01-05T13:00:00Z"), at("2024-01-05T20:45:00Z"))
    );
    // La decisión es al cierre de la vela anterior (t + 1).
    assert_eq!(m15[w0.decision_in].end(), at("2024-01-05T13:00:00Z"));
    let w1 = out[1].window.unwrap();
    assert_eq!(
        times(&m15, &w1),
        (at("2024-07-11T12:00:00Z"), at("2024-07-11T19:45:00Z"))
    );
}

#[test]
fn an_early_close_exits_at_the_last_candle_of_the_short_session() {
    let cal = calendar();
    let s = session(&cal);
    // 03-07-2024: XNYS cierra a las 13:00 ET → salida en la apertura de la vela de las 12:45 ET.
    let m15 = flat("2024-07-02", "2024-07-03");
    let rows = [scheduled(
        "employment_situation/2024-06",
        "2024-07-03T12:30:00Z",
    )];
    let out = macro_day_windows(&m15, s, &rows, &h2());
    assert_eq!(
        times(&m15, &out[0].window.unwrap()),
        (at("2024-07-03T12:00:00Z"), at("2024-07-03T16:45:00Z"))
    );
}

#[test]
fn days_without_a_cash_session_or_at_another_hour_are_not_traded() {
    let cal = calendar();
    let s = session(&cal);
    // Viernes Santo 02-04-2021: hubo empleo, pero XNYS no abrió.
    let m15 = [
        flat("2021-04-01", "2021-04-02"),
        flat("2024-01-04", "2024-01-05"),
    ]
    .concat();
    let rows = [
        scheduled("employment_situation/2021-03", "2021-04-02T12:30:00Z"),
        scheduled("cpi/2023-12", "2024-01-05T15:00:00Z"), // 10:00 ET
    ];
    let out = macro_day_windows(&m15, s, &rows, &h2());
    assert_eq!(out[0].window, Err(Skip::NoCashSession));
    assert_eq!(out[1].window, Err(Skip::OtherTime));
}

#[test]
fn cpi_and_employment_on_the_same_day_are_traded_once() {
    let cal = calendar();
    let s = session(&cal);
    let m15 = flat("2024-01-04", "2024-01-05");
    let rows = [
        scheduled("employment_situation/2023-12", "2024-01-05T13:30:00Z"),
        scheduled("cpi/2023-12", "2024-01-05T13:30:00Z"),
    ];
    let out = macro_day_windows(&m15, s, &rows, &h2());
    let ok = out.iter().filter(|o| o.window.is_ok()).count();
    assert_eq!(ok, 1);
    assert!(out.iter().any(|o| o.window == Err(Skip::SameDay)));
}

#[test]
fn only_the_row_published_before_the_decision_counts() {
    let cal = calendar();
    let s = session(&cal);
    let m15 = flat("2024-01-04", "2024-01-12");
    // Publicada justo al cierre de la vela de la decisión (08:00 ET = 13:00 UTC): vale.
    let on_time = row(
        "cpi/2023-12",
        ReleaseStatus::Scheduled,
        Some("2024-01-11T13:30:00Z"),
        "2024-01-11T13:00:00Z",
    );
    // Publicada un minuto después: no vale.
    let late = row(
        "employment_situation/2023-12",
        ReleaseStatus::Scheduled,
        Some("2024-01-05T13:30:00Z"),
        "2024-01-05T13:01:00Z",
    );
    let out = macro_day_windows(&m15, s, &[late, on_time], &h2());
    assert_eq!(out[0].window, Err(Skip::NotPublished));
    assert!(out[1].window.is_ok());
}

#[test]
fn a_rescheduled_or_cancelled_row_replaces_the_original_from_its_publication() {
    let cal = calendar();
    let s = session(&cal);
    let m15 = flat("2024-01-04", "2024-01-12");
    // Original para el 05-01; reprogramada al 11-01, publicada el 03-01: el 05-01 ya no vale.
    let rows = [
        scheduled("employment_situation/2023-12", "2024-01-05T13:30:00Z"),
        row(
            "employment_situation/2023-12",
            ReleaseStatus::Rescheduled,
            Some("2024-01-11T13:30:00Z"),
            "2024-01-03T00:00:00Z",
        ),
    ];
    let out = macro_day_windows(&m15, s, &rows, &h2());
    assert_eq!(out[0].window, Err(Skip::Superseded));
    assert_eq!(
        times(&m15, &out[1].window.unwrap()),
        (at("2024-01-11T13:00:00Z"), at("2024-01-11T20:45:00Z"))
    );
    // Cancelada (publicada antes): ni la original ni la cancelación se operan.
    let rows = [
        scheduled("cpi/2023-12", "2024-01-11T13:30:00Z"),
        row(
            "cpi/2023-12",
            ReleaseStatus::Cancelled,
            None,
            "2024-01-03T00:00:00Z",
        ),
    ];
    let out = macro_day_windows(&m15, s, &rows, &h2());
    assert_eq!(out[0].window, Err(Skip::Superseded));
    assert_eq!(out[1].window, Err(Skip::Cancelled));
    // Una cancelación publicada después de la decisión no cambia nada (no mira el futuro).
    let rows = [
        scheduled("cpi/2023-12", "2024-01-11T13:30:00Z"),
        row(
            "cpi/2023-12",
            ReleaseStatus::Cancelled,
            None,
            "2024-01-11T14:00:00Z",
        ),
    ];
    let out = macro_day_windows(&m15, s, &rows, &h2());
    assert!(out[0].window.is_ok());
}

#[test]
fn a_missing_or_non_consecutive_candle_skips_the_event() {
    let cal = calendar();
    let s = session(&cal);
    let full = flat("2024-01-04", "2024-01-05");
    let rows = [scheduled(
        "employment_situation/2023-12",
        "2024-01-05T13:30:00Z",
    )];
    for (gap, why) in [
        ("2024-01-05T12:45:00Z", Skip::MissingEntryCandle), // decisión de entrada
        ("2024-01-05T13:00:00Z", Skip::MissingEntryCandle), // vela de entrada
        ("2024-01-05T20:30:00Z", Skip::MissingExitCandle),  // decisión de salida
        ("2024-01-05T20:45:00Z", Skip::MissingExitCandle),  // vela de salida
    ] {
        let m15: Vec<Candle> = full
            .iter()
            .filter(|c| c.start != at(gap))
            .cloned()
            .collect();
        let out = macro_day_windows(&m15, s, &rows, &h2());
        assert_eq!(out[0].window, Err(why), "{gap}");
    }
}

#[test]
fn new_candles_do_not_change_a_past_window() {
    let cal = calendar();
    let s = session(&cal);
    let full = flat("2024-01-04", "2024-01-12");
    let rows = [
        scheduled("employment_situation/2023-12", "2024-01-05T13:30:00Z"),
        scheduled("cpi/2023-12", "2024-01-11T13:30:00Z"),
    ];
    let cut = full
        .iter()
        .position(|c| c.start >= at("2024-01-06T00:00:00Z"))
        .unwrap();
    let early = macro_day_windows(&full[..cut], s, &rows[..1], &h2());
    let late = macro_day_windows(&full, s, &rows, &h2());
    assert_eq!(early[0], late[0]);
}

// ── retornos y spread de la entrada ────────────────────────────────────────────────────────

#[test]
fn the_entry_outside_the_session_pays_the_wider_spread() {
    let cal = calendar();
    let s = session(&cal);
    let setup = setup();
    // Plano en 20000; salta a 20200 dentro de la ventana (05-01, 15:00 UTC) y a 21000 fuera
    // de ella (15-02), sin volver.
    let m15 = series(day("2023-12-28"), day("2024-03-28"), |t| {
        if t >= at("2024-02-15T15:00:00Z") {
            dec!(21000)
        } else if t >= at("2024-01-05T15:00:00Z") {
            dec!(20200)
        } else {
            dec!(20000)
        }
    });
    let rows = [scheduled(
        "employment_situation/2023-12",
        "2024-01-05T13:30:00Z",
    )];
    let out = macro_day_windows(&m15, s, &rows, &h2());
    let windows: Vec<EventWindow> = out.iter().filter_map(|o| o.window.ok()).collect();
    let schedule = event_schedule(&windows);
    let spreads: Vec<(usize, Decimal)> = windows
        .iter()
        .map(|w| (w.entry(), research().macro_day.out_of_session_spread))
        .collect();
    let range = (at("2024-01-01T00:00:00Z"), at("2024-04-01T00:00:00Z"));
    let run = |stress: Decimal, spreads: &[(usize, Decimal)]| {
        managed_returns_with_spreads(&m15, s, &[], &setup, range, &schedule, stress, spreads)
            .unwrap()
    };
    let gross = run(Decimal::ZERO, &spreads);
    // Antes de costos: exactamente el salto de la ventana (+1 %); febrero y marzo en cero.
    assert_eq!(gross[0], ((2024, 1), dec!(0.01)));
    assert_eq!(gross[1].1, Decimal::ZERO);
    assert_eq!(gross[2].1, Decimal::ZERO);
    let wide = run(Decimal::ONE, &spreads);
    let normal = run(Decimal::ONE, &[]);
    // Sin spreads distintos da lo mismo que `managed_returns`.
    assert_eq!(
        normal,
        managed_returns(&m15, s, &[], &setup, range, &schedule, Decimal::ONE).unwrap()
    );
    assert!(wide[0].1 < normal[0].1, "{wide:?} {normal:?}");
    // Sin noches: el costo es solo spread y slippage sobre 1 unidad (exposición 1 con 20000 de
    // cuenta). Entrada: 2,5/2 + 0,1×2,5 + 0,1 = 1,6 puntos; salida: 1,6/2 + 0,1×1,6 + 0,1 = 1,06.
    let expected = (dec!(200) - dec!(1.6) - dec!(1.06)) / dec!(20000);
    assert!(
        (wide[0].1 - expected).abs() < dec!(0.000001),
        "{} vs {expected}",
        wide[0].1
    );
    assert!(run(dec!(1.5), &spreads)[0].1 < wide[0].1);
}

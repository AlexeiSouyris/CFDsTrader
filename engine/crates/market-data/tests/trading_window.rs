//! Horario de entradas y cierre de la semana con el calendario real de `config/` (#64).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use cfds_core::{Candle, Timeframe};
use cfds_market_data::trading_window::flags;
use cfds_market_data::{
    EntryHours, ExchangeCalendar, Session, TradingWindowConfig, entry_allowed,
    is_last_session_of_week, is_week_close_candle,
};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::dec;

fn config_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config")
}

fn calendar() -> ExchangeCalendar {
    ExchangeCalendar::load(&config_dir(), HashMap::new()).unwrap()
}

fn cfg() -> TradingWindowConfig {
    TradingWindowConfig::from_toml(
        &std::fs::read_to_string(config_dir().join("trading_window.toml")).unwrap(),
    )
    .unwrap()
}

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn m15(start: &str) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: utc(start),
        open: dec!(1),
        high: dec!(1),
        low: dec!(1),
        close: dec!(1),
        volume: None,
    }
}

fn allowed(s: &Session, at: &str) -> Option<bool> {
    entry_allowed(s, utc(at), &cfg())
}

#[test]
fn the_repository_config_matches_the_spec() {
    assert_eq!(
        cfg(),
        TradingWindowConfig {
            skip_after_open: TimeDelta::minutes(15),
            no_new_before_week_close: TimeDelta::minutes(30),
            hours: BTreeMap::from([(
                "CMEGLOBEX_METALS".to_string(),
                EntryHours::Pending {
                    reason: "las horas de entrada del oro las elige el dueño (ADR 0013 §2, #418); \
                             hasta entonces el oro no abre operaciones"
                        .into()
                }
            )]),
        }
    );
    assert!(TradingWindowConfig::from_toml("[entries]\nskip_after_open_minutes = 15").is_err());
}

#[test]
fn no_entries_in_the_first_15_minutes() {
    let cal = calendar();
    let nyse = cal.session("XNYS").unwrap();
    // Jueves 19-11-2026, invierno: 09:30–16:00 Nueva York = 14:30–21:00 UTC.
    assert_eq!(allowed(nyse, "2026-11-19T14:30:00Z"), Some(false));
    assert_eq!(allowed(nyse, "2026-11-19T14:44:59Z"), Some(false));
    assert_eq!(allowed(nyse, "2026-11-19T14:45:00Z"), Some(true));
    // Un jueves común se puede entrar hasta el final; en el cierre ya no.
    assert_eq!(allowed(nyse, "2026-11-19T20:45:00Z"), Some(true));
    assert_eq!(allowed(nyse, "2026-11-19T21:00:00Z"), Some(false));
    // Fuera de sesión, tampoco.
    assert_eq!(allowed(nyse, "2026-11-19T13:00:00Z"), Some(false));
}

#[test]
fn no_new_entries_in_the_last_30_minutes_of_the_week() {
    let cal = calendar();
    let nyse = cal.session("XNYS").unwrap();
    // Viernes 20-11-2026: cierre 21:00 UTC → sin entradas desde las 20:30.
    assert_eq!(allowed(nyse, "2026-11-20T20:15:00Z"), Some(true));
    assert_eq!(allowed(nyse, "2026-11-20T20:29:59Z"), Some(true));
    assert_eq!(allowed(nyse, "2026-11-20T20:30:00Z"), Some(false));
    // Xetra el mismo viernes: 09:00–17:30 Berlín = 08:00–16:30 UTC.
    let xetra = cal.session("XETR").unwrap();
    assert_eq!(allowed(xetra, "2026-11-20T08:14:00Z"), Some(false));
    assert_eq!(allowed(xetra, "2026-11-20T08:15:00Z"), Some(true));
    assert_eq!(allowed(xetra, "2026-11-20T15:45:00Z"), Some(true));
    assert_eq!(allowed(xetra, "2026-11-20T16:00:00Z"), Some(false));
}

#[test]
fn an_early_close_on_friday_moves_the_window() {
    let cal = calendar();
    let nyse = cal.session("XNYS").unwrap();
    // Viernes 27-11-2026 (después de Thanksgiving): cierra a las 13:00 = 18:00 UTC.
    assert_eq!(allowed(nyse, "2026-11-27T17:15:00Z"), Some(true));
    assert_eq!(allowed(nyse, "2026-11-27T17:30:00Z"), Some(false));
    assert_eq!(
        is_week_close_candle(nyse, &m15("2026-11-27T17:45:00Z")),
        Some(true)
    );
    assert_eq!(
        is_week_close_candle(nyse, &m15("2026-11-27T20:45:00Z")),
        Some(false)
    );
}

#[test]
fn before_a_friday_holiday_the_thursday_closes_the_week() {
    let cal = calendar();
    let nyse = cal.session("XNYS").unwrap();
    // Viernes Santo 03-04-2026 sin sesión: el jueves 02-04 es el último día hábil.
    // En verano (EDT): 09:30–16:00 = 13:30–20:00 UTC.
    assert_eq!(
        is_last_session_of_week(nyse, date("2026-04-02")),
        Some(true)
    );
    assert_eq!(
        is_last_session_of_week(nyse, date("2026-04-03")),
        Some(false)
    );
    assert_eq!(
        is_last_session_of_week(nyse, date("2026-04-01")),
        Some(false)
    );
    // Con el lunes siguiente feriado (Memorial Day 25-05-2026), el viernes igual cierra la
    // semana aunque la próxima sesión sea un martes.
    assert_eq!(
        is_last_session_of_week(nyse, date("2026-05-22")),
        Some(true)
    );
    assert_eq!(allowed(nyse, "2026-04-02T19:15:00Z"), Some(true));
    assert_eq!(allowed(nyse, "2026-04-02T19:30:00Z"), Some(false));
    assert_eq!(
        is_week_close_candle(nyse, &m15("2026-04-02T19:45:00Z")),
        Some(true)
    );
    // El horario de verano mueve la apertura en UTC una hora antes que en invierno.
    assert_eq!(allowed(nyse, "2026-04-02T13:45:00Z"), Some(true));
    assert_eq!(allowed(nyse, "2026-04-02T13:44:00Z"), Some(false));
}

#[test]
fn weekends_and_holidays_have_no_entries() {
    let cal = calendar();
    let nyse = cal.session("XNYS").unwrap();
    assert_eq!(allowed(nyse, "2026-11-21T15:00:00Z"), Some(false)); // sábado
    assert_eq!(allowed(nyse, "2026-11-26T15:00:00Z"), Some(false)); // Thanksgiving
    assert_eq!(
        is_week_close_candle(nyse, &m15("2026-11-21T20:45:00Z")),
        Some(false)
    );
}

#[test]
fn outside_the_calendar_coverage_nothing_is_assumed() {
    let cal = calendar();
    let nyse = cal.session("XNYS").unwrap();
    assert_eq!(allowed(nyse, "2028-03-01T15:00:00Z"), None);
    // El viernes 31-12-2027 está cubierto, pero saber si es el último de la semana exige
    // mirar 2028.
    assert_eq!(is_last_session_of_week(nyse, date("2027-12-31")), None);
    assert_eq!(allowed(nyse, "2027-12-31T20:45:00Z"), None);
}

#[test]
fn flags_for_a_friday_session() {
    let cal = calendar();
    let nyse = cal.session("XNYS").unwrap();
    // Las 26 velas de 15m del viernes 20-11-2026 (14:30 a 20:45 UTC).
    let start = utc("2026-11-20T14:30:00Z");
    let candles: Vec<Candle> = (0..26)
        .map(|i| Candle {
            start: start + TimeDelta::minutes(15 * i),
            ..m15("2026-11-20T14:30:00Z")
        })
        .collect();
    let (entries, closes) = flags(nyse, &candles, &cfg()).unwrap();
    // Se decide al cierre de cada vela: de 14:45 a 20:15 se puede (23 velas); desde 20:30, no.
    assert_eq!(entries.iter().filter(|&&e| e).count(), 23);
    assert!(entries[..23].iter().all(|&e| e));
    assert!(entries[23..].iter().all(|&e| !e));
    // Solo la última vela cierra la semana.
    assert_eq!(closes.iter().filter(|&&c| c).count(), 1);
    assert!(closes[25]);
    // Un lunes no cierra nada.
    let monday: Vec<Candle> = candles
        .iter()
        .map(|c| Candle {
            start: c.start - TimeDelta::days(4),
            ..c.clone()
        })
        .collect();
    let (entries, closes) = flags(nyse, &monday, &cfg()).unwrap();
    // La última vela termina justo en el cierre: a esa hora ya no se entra (25 de 26).
    assert_eq!(entries.iter().filter(|&&e| e).count(), 25);
    assert!(!entries[25]);
    assert!(closes.iter().all(|&c| !c));
}

// ---------- Oro: sesión de casi 23 h con horas de entrada propias (ADR 0013) ----------

#[test]
fn gold_opens_nothing_while_its_entry_hours_are_pending() {
    let cal = calendar();
    let g = cal.session("CMEGLOBEX_METALS").unwrap();
    // Mercado abierto (miércoles 09:00 CT), pero las horas de entrada están pendientes.
    assert_eq!(g.is_open_at(utc("2026-11-25T15:00:00Z")), Some(true));
    assert_eq!(allowed(g, "2026-11-25T15:00:00Z"), Some(false));
    // Sin ninguna entrada en [entries.hours], tampoco: una sesión nocturna las necesita.
    let mut none = cfg();
    none.hours.clear();
    assert_eq!(
        entry_allowed(g, utc("2026-11-25T15:00:00Z"), &none),
        Some(false)
    );
}

#[test]
fn entry_hours_limit_entries_inside_the_session() {
    let cal = calendar();
    let g = cal.session("CMEGLOBEX_METALS").unwrap();
    let toml = "[entries]\nskip_after_open_minutes = 15\nno_new_before_week_close_minutes = 30\n\
                [entries.hours.CMEGLOBEX_METALS]\ntimezone = \"America/New_York\"\n\
                from = \"10:00\"\nto = \"12:00\"\n";
    let w = TradingWindowConfig::from_toml(toml).unwrap();
    let at = |s: &str| entry_allowed(g, utc(s), &w);
    // Invierno: 10:00–12:00 de Nueva York = 15:00–17:00 UTC.
    assert_eq!(at("2026-11-25T14:59:00Z"), Some(false));
    assert_eq!(at("2026-11-25T15:00:00Z"), Some(true));
    assert_eq!(at("2026-11-25T16:59:00Z"), Some(true));
    assert_eq!(at("2026-11-25T17:00:00Z"), Some(false));
    // Una sesión normal sin horas propias no cambia.
    let nyse = cal.session("XNYS").unwrap();
    assert_eq!(
        entry_allowed(nyse, utc("2026-11-25T18:00:00Z"), &w),
        Some(true)
    );
    // Formatos inválidos.
    for bad in [
        "[entries.hours.X]\ntimezone = \"America/New_York\"\nfrom = \"12:00\"\nto = \"10:00\"\n",
        "[entries.hours.X]\npending = \"\"\n",
        "[entries.hours.X]\npending = \"x\"\nfrom = \"10:00\"\n",
        "[entries.hours.X]\ntimezone = \"Marte/Base\"\nfrom = \"10:00\"\nto = \"12:00\"\n",
    ] {
        let t = format!(
            "[entries]\nskip_after_open_minutes = 15\nno_new_before_week_close_minutes = 30\n{bad}"
        );
        assert!(TradingWindowConfig::from_toml(&t).is_err(), "{bad}");
    }
}

#[test]
fn the_gold_week_closes_with_the_last_candle_before_friday_17_new_york() {
    let cal = calendar();
    let g = cal.session("CMEGLOBEX_METALS").unwrap();
    // Viernes 04-12-2026, cierre 16:00 CT = 22:00 UTC: la última vela de 15m empieza 21:45.
    assert_eq!(
        is_week_close_candle(g, &m15("2026-12-04T21:45:00Z")),
        Some(true)
    );
    assert_eq!(
        is_week_close_candle(g, &m15("2026-12-04T21:30:00Z")),
        Some(false)
    );
    // La última del jueves no cierra la semana.
    assert_eq!(
        is_week_close_candle(g, &m15("2026-12-03T21:45:00Z")),
        Some(false)
    );
    // La primera vela del domingo pertenece al lunes.
    assert_eq!(
        is_week_close_candle(g, &m15("2026-12-06T23:00:00Z")),
        Some(false)
    );
}

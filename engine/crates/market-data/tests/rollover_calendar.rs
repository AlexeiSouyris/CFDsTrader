//! Vencimientos de los futuros por calendario (#269), contra las fechas en que cambió el
//! contrato en los datos reales de Databento (ES `c.0` y FDAX `c.0`, medidas el 23-11-2026).

use std::collections::HashMap;
use std::path::Path;

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{CandleSources, ExchangeCalendar, Session};
use chrono::{DateTime, Datelike, NaiveDate, Utc, Weekday};
use rust_decimal::dec;

fn sources() -> CandleSources {
    CandleSources::from_toml(include_str!("../../../../config/candle_sources.toml")).unwrap()
}

fn session(exchange: &str) -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    ExchangeCalendar::load(&dir, HashMap::new())
        .unwrap()
        .session(exchange)
        .unwrap()
        .clone()
}

fn id(s: &str) -> InstrumentId {
    s.parse().unwrap()
}

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn candle(start: DateTime<Utc>) -> Candle {
    Candle {
        instrument_id: id("US500"),
        timeframe: Timeframe::M15,
        start,
        open: dec!(100),
        high: dec!(101),
        low: dec!(99),
        close: dec!(100),
        volume: None,
    }
}

/// Una vela por día hábil a `hh:mm` UTC y, si `sunday`, otra el domingo a las 23:00 (apertura
/// de CME). Sin sábados.
fn series(from: &str, to: &str, hh: u32, mm: u32, sunday: bool) -> Vec<Candle> {
    let mut out = Vec::new();
    let mut d = day(from);
    while d <= day(to) {
        let at = |h, m| d.and_hms_opt(h, m, 0).unwrap().and_utc();
        match d.weekday() {
            Weekday::Sat => {}
            Weekday::Sun if sunday => out.push(candle(at(23, 0))),
            Weekday::Sun => {}
            _ => out.push(candle(at(hh, mm))),
        }
        d = d.succ_opt().unwrap();
    }
    out
}

#[test]
fn the_expiry_is_the_third_friday_or_the_business_day_before() {
    let nyse = session("XNYS");
    assert_eq!(
        CandleSources::expiry_day(&nyse, 2022, 3),
        Some(day("2022-03-18"))
    );
    // Juneteenth (viernes 19-06-2026): la bolsa no abre, vence el jueves.
    assert_eq!(
        CandleSources::expiry_day(&nyse, 2026, 6),
        Some(day("2026-06-18"))
    );
    let xetra = session("XETR");
    assert_eq!(
        CandleSources::expiry_day(&xetra, 2025, 9),
        Some(day("2025-09-19"))
    );
}

#[test]
fn es_rolls_match_the_real_contract_changes_over_five_years() {
    let m15 = series("2021-11-01", "2026-11-20", 14, 30, true);
    let rolls = sources()
        .roll_instants(&id("US500"), &session("XNYS"), &m15)
        .unwrap();
    let dates: Vec<NaiveDate> = rolls.iter().map(|t| t.date_naive()).collect();
    // Primer día del contrato nuevo de ES.c.0 en `futures_bars_1m` (21 contratos, 20 cambios).
    let real = [
        "2021-12-19",
        "2022-03-20",
        "2022-06-19",
        "2022-09-18",
        "2022-12-18",
        "2023-03-19",
        "2023-06-18",
        "2023-09-17",
        "2023-12-17",
        "2024-03-17",
        "2024-06-23",
        "2024-09-22",
        "2024-12-22",
        "2025-03-23",
        "2025-06-22",
        "2025-09-21",
        "2025-12-21",
        "2026-03-22",
        "2026-06-19",
        "2026-09-20",
    ]
    .map(day);
    assert_eq!(dates, real);
}

#[test]
fn fdax_rolls_on_the_expiry_day_seven_times_in_twenty_months() {
    // En XEUR.EOBI el `instrument_id` cambió 56 veces en este período (se reusa de un día a
    // otro): el calendario da los 7 vencimientos reales, el día en que el precio saltó a la
    // base del contrato siguiente.
    let m15 = series("2025-03-11", "2026-10-30", 0, 15, false);
    let rolls = sources()
        .roll_instants(&id("DE40"), &session("XETR"), &m15)
        .unwrap();
    let expected: Vec<DateTime<Utc>> = [
        "2025-03-21",
        "2025-06-20",
        "2025-09-19",
        "2025-12-19",
        "2026-03-20",
        "2026-06-19",
        "2026-09-18",
    ]
    .map(|d| day(d).and_hms_opt(0, 15, 0).unwrap().and_utc())
    .to_vec();
    assert_eq!(rolls, expected);
}

#[test]
fn no_roll_outside_the_series_and_an_unknown_instrument_is_an_error() {
    let m15 = series("2025-04-01", "2025-05-30", 14, 30, true);
    let s = sources();
    assert!(
        s.roll_instants(&id("US500"), &session("XNYS"), &m15)
            .unwrap()
            .is_empty()
    );
    assert!(
        s.roll_instants(&id("US500"), &session("XNYS"), &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        s.roll_instants(&id("AAPL"), &session("XNYS"), &m15)
            .is_err()
    );
    // Si la serie empieza justo el día del cambio, no hay vela anterior: no es un cambio.
    let from_roll: Vec<Candle> = series("2022-03-20", "2022-03-25", 14, 30, true);
    assert!(
        s.roll_instants(&id("US500"), &session("XNYS"), &from_roll)
            .unwrap()
            .is_empty()
    );
}

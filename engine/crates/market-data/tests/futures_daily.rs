//! Diarias de los futuros (#282) con velas sintéticas: misma ventana que las de IG, solo días
//! con sesión del CFD, sin mezclar contratos en el vencimiento y sin repintado. También el
//! continuo por volumen del oro (`GC.v.0`, #449), que corta donde cambia el contrato.

use std::collections::HashMap;
use std::path::Path;

use cfds_core::{Candle, FuturesCandle, InstrumentId, Timeframe};
use cfds_market_data::{CandleSources, ExchangeCalendar, Session, futures_daily};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use chrono_tz::Tz;
use rust_decimal::{Decimal, dec};

fn session(exchange: &str) -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    ExchangeCalendar::load(&dir, HashMap::new())
        .unwrap()
        .session(exchange)
        .unwrap()
        .clone()
}

fn madrid() -> Tz {
    CandleSources::from_toml(include_str!("../../../../config/candle_sources.toml"))
        .unwrap()
        .daily_tz
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn id(s: &str) -> InstrumentId {
    s.parse().unwrap()
}

/// Velas de 15m seguidas desde `from`, `n` velas; el cierre sube 1 por vela desde `base`.
fn series(from: &str, n: i64, base: Decimal) -> Vec<Candle> {
    (0..n)
        .map(|i| {
            let c = base + Decimal::from(i);
            Candle {
                instrument_id: id("US500"),
                timeframe: Timeframe::M15,
                start: at(from) + TimeDelta::minutes(15 * i),
                open: c - dec!(1),
                high: c + dec!(2),
                low: c - dec!(3),
                close: c,
                volume: Some(dec!(10)),
            }
        })
        .collect()
}

#[test]
fn a_daily_per_session_day_from_madrid_midnight_to_midnight() {
    // Lunes 16 a domingo 22 de noviembre de 2026, 24 h por día (CET = UTC+1).
    let m15 = series("2026-11-15T23:00:00Z", 7 * 96, dec!(6000));
    let d = futures_daily(
        &m15,
        &id("US500.FUT"),
        &session("XNYS"),
        madrid(),
        &[],
        at("2026-11-23T00:00:00Z"),
    );
    let days: Vec<NaiveDate> = d.iter().map(|x| x.day).collect();
    // Solo de lunes a viernes: el sábado y el domingo no tienen sesión.
    assert_eq!(
        days,
        [
            "2026-11-16",
            "2026-11-17",
            "2026-11-18",
            "2026-11-19",
            "2026-11-20"
        ]
        .map(|s| s.parse::<NaiveDate>().unwrap())
    );
    let mon = &d[0];
    assert_eq!(mon.instrument_id, id("US500.FUT"));
    assert_eq!(
        (mon.start, mon.end),
        (at("2026-11-15T23:00:00Z"), at("2026-11-16T23:00:00Z"))
    );
    // Apertura de la primera vela del día, cierre de la última, extremos de todas.
    assert_eq!((mon.open, mon.close), (dec!(5999), dec!(6095)));
    assert_eq!((mon.high, mon.low), (dec!(6097), dec!(5997)));
    assert_eq!(mon.volume, Some(dec!(960)));
}

#[test]
fn a_holiday_has_no_daily_even_if_the_future_traded() {
    // Thanksgiving (jueves 26-11-2026): la bolsa del CFD no abre, aunque el futuro opere.
    let m15 = series("2026-11-23T23:00:00Z", 4 * 96, dec!(6000));
    let d = futures_daily(
        &m15,
        &id("US500.FUT"),
        &session("XNYS"),
        madrid(),
        &[],
        at("2026-11-28T00:00:00Z"),
    );
    let days: Vec<String> = d.iter().map(|x| x.day.to_string()).collect();
    assert_eq!(days, ["2026-11-24", "2026-11-25", "2026-11-27"]);
}

#[test]
fn the_expiry_day_uses_only_the_new_contract() {
    // FDAX vence el viernes 19-09-2025 y el continuo pasa al contrato nuevo a las 00:15 UTC
    // (calendario de #270). La ventana de Madrid empieza el 18 a las 22:00 UTC: las velas de
    // 22:00 a 00:00 son del contrato viejo (100 puntos más abajo) y no entran.
    let mut m15 = series("2025-09-18T22:00:00Z", 9, dec!(23000));
    m15.extend(series("2025-09-19T00:15:00Z", 4 * 21, dec!(23200)));
    let roll = at("2025-09-19T00:15:00Z");
    let d = futures_daily(
        &m15,
        &id("DE40.FUT"),
        &session("XETR"),
        madrid(),
        &[roll],
        at("2025-09-20T00:00:00Z"),
    );
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].day.to_string(), "2025-09-19");
    assert_eq!(d[0].open, dec!(23199), "la primera vela del contrato nuevo");
    assert_eq!(d[0].low, dec!(23197));
    // La ventana es la del día entero, aunque empiece en el vencimiento.
    assert_eq!(d[0].start, at("2025-09-18T22:00:00Z"));
}

#[test]
fn adding_candles_never_repaints_a_closed_day() {
    let m15 = series("2026-11-15T23:00:00Z", 7 * 96, dec!(6000));
    let full = futures_daily(
        &m15,
        &id("US500.FUT"),
        &session("XNYS"),
        madrid(),
        &[],
        at("2026-11-23T00:00:00Z"),
    );
    for n in [10, 96, 97, 200, 300, 500] {
        let until = m15[n - 1].end();
        let part = futures_daily(
            &m15[..n],
            &id("US500.FUT"),
            &session("XNYS"),
            madrid(),
            &[],
            until,
        );
        // Solo días cerrados, y los que hay son iguales a los de la serie completa.
        assert!(part.iter().all(|d| d.end <= until), "{n}");
        assert_eq!(part[..], full[..part.len()], "{n}");
    }
}

fn sources() -> CandleSources {
    CandleSources::from_toml(include_str!("../../../../config/candle_sources.toml")).unwrap()
}

/// `series` con un contrato.
fn with_contract(candles: Vec<Candle>, contract: u64) -> Vec<FuturesCandle> {
    candles
        .into_iter()
        .map(|candle| FuturesCandle {
            candle,
            contract_id: contract,
        })
        .collect()
}

/// Oro: el jueves 30 y el viernes 31 de julio de 2026, con el cambio real de GC.v.0 a las 00:00
/// UTC del 31 (contrato 42011464 → 41271, el precio sube unos 60 puntos).
fn gold_roll() -> Vec<FuturesCandle> {
    let mut m15 = with_contract(series("2026-07-29T22:00:00Z", 104, dec!(4040)), 42011464);
    m15.extend(with_contract(
        series("2026-07-31T00:00:00Z", 84, dec!(4160)),
        41271,
    ));
    m15
}

fn gold_daily(m15: &[FuturesCandle], until: DateTime<Utc>) -> Vec<cfds_core::DailyCandle> {
    let s = sources();
    let session = session("CMEGLOBEX_METALS");
    let rolls = s
        .roll_instants_with_contracts(&id("XAUUSD"), &session, m15)
        .unwrap();
    let candles: Vec<Candle> = m15.iter().map(|c| c.candle.clone()).collect();
    futures_daily(
        &candles,
        &id("XAUUSD.FUT"),
        &session,
        s.daily_tz,
        &rolls,
        until,
    )
}

#[test]
fn the_gold_daily_cuts_where_the_contract_changes() {
    let m15 = gold_roll();
    let d = gold_daily(&m15, at("2026-08-01T00:00:00Z"));
    let days: Vec<String> = d.iter().map(|x| x.day.to_string()).collect();
    assert_eq!(days, ["2026-07-30", "2026-07-31"]);
    // El jueves: solo el contrato viejo (de 22:00 UTC del 29 a 22:00 UTC del 30).
    assert_eq!(d[0].close, dec!(4135));
    // El viernes: la ventana de Madrid empieza el 30 a las 22:00 UTC, pero las velas de 22:00 a
    // 00:00 son del contrato viejo y no entran.
    assert_eq!(d[1].open, dec!(4159), "la primera vela del contrato nuevo");
    assert_eq!(d[1].low, dec!(4157));
    assert_eq!(d[1].start, at("2026-07-30T22:00:00Z"));
    assert_eq!(d[1].close, dec!(4160) + Decimal::from(83));
}

#[test]
fn the_gold_daily_never_repaints_as_candles_arrive() {
    let m15 = gold_roll();
    let full = gold_daily(&m15, at("2026-08-01T00:00:00Z"));
    for n in [1, 50, 96, 97, 104, 105, 112, 150, m15.len()] {
        let until = m15[n - 1].candle.end();
        let part = gold_daily(&m15[..n], until);
        assert!(part.iter().all(|d| d.end <= until), "{n}");
        assert_eq!(part[..], full[..part.len()], "{n}");
    }
}

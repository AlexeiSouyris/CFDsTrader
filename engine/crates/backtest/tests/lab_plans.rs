//! Laboratorio (#727): `plans_from_spec` con cruce de medias y ruptura del rango de apertura,
//! stop en ATR, salidas por tiempo y por sesión, filtros, 1h con velas cerradas y la propiedad
//! de no repintar (cortar la serie no cambia los planes anteriores). Series sintéticas sobre el
//! calendario real de XNYS; nunca la base real.

use std::path::PathBuf;

use cfds_backtest::lab::{
    LabConfig, LabSeries, StrategySpec, ValidSpec, catalog, plans_from_spec, validate_spec,
};
use cfds_backtest::research::{OrbConfig, Plan, orb_plans};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::{crosses, sma, wilder_atr};
use cfds_market_data::{Catalog, DataSplit, ExchangeCalendar, Period, Session, aggregate_h1};
use cfds_risk::RiskConfig;
use cfds_strategy::Side;
use chrono::{DateTime, NaiveDate, NaiveTime, TimeDelta, Utc};
use proptest::prelude::*;
use rust_decimal::{Decimal, dec};
use serde_json::{Value, json};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn us500() -> InstrumentId {
    "US500".parse().unwrap()
}

fn session() -> Session {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    let cal = ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap();
    cal.session_for(&us500()).unwrap().clone()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

/// Velas de 15m seguidas desde `start` con esos cierres (apertura = cierre anterior).
fn candles_from(start: DateTime<Utc>, closes: &[Decimal]) -> Vec<Candle> {
    let mut prev = closes.first().copied().unwrap_or(Decimal::ZERO);
    closes
        .iter()
        .enumerate()
        .map(|(k, &c)| {
            let o = prev;
            prev = c;
            Candle {
                instrument_id: us500(),
                timeframe: Timeframe::M15,
                start: start + TimeDelta::minutes(15 * k as i64),
                open: o,
                high: o.max(c) + dec!(0.5),
                low: o.min(c) - dec!(0.5),
                close: c,
                volume: None,
            }
        })
        .collect()
}

/// Las 26 velas de la sesión de contado del miércoles 10-01-2024 (14:30–21:00 UTC).
fn session_day(closes: impl Fn(usize) -> Decimal) -> Vec<Candle> {
    let v: Vec<Decimal> = (0..26).map(closes).collect();
    candles_from(at("2024-01-10T14:30:00Z"), &v)
}

/// En V: baja 13 velas y sube 13.
fn v_shape(k: usize) -> Decimal {
    Decimal::from(if k < 13 { 1000 - 5 * k } else { 940 + 5 * k })
}

/// En V invertida: sube 13 velas y baja 13.
fn inverted_v(k: usize) -> Decimal {
    Decimal::from(if k < 13 { 1000 + 5 * k } else { 1130 - 5 * k })
}

fn spec(timeframe: &str, entry: Value, exit: Value) -> Value {
    json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": timeframe },
        "direction": "both",
        "entry": [entry],
        "filters": [],
        "stop": { "type": "atr", "params": { "period": 3, "k": "1" } },
        "exit": { "type": "time_target", "params": exit },
        "risk_pct": "0.5",
        "cost_multiplier": "1",
        "period": { "from": "2020-01-01T00:00:00Z", "to": "2025-01-01T00:00:00Z" },
        "mode": "exploratory"
    })
}

fn ma(fast: i64, slow: i64) -> Value {
    json!({ "type": "ma_cross", "params": { "average": "sma", "fast": fast, "slow": slow } })
}

fn valid(doc: &Value) -> ValidSpec {
    let c = catalog(
        &LabConfig::from_toml(cfg!("lab.toml")).unwrap(),
        &RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        [us500()],
    )
    .unwrap();
    let split = DataSplit {
        windows: Vec::new(),
        out_of_sample: Period {
            start: at("2026-01-01T00:00:00Z"),
            end: at("2027-01-01T00:00:00Z"),
        },
    };
    let spec: StrategySpec = serde_json::from_value(doc.clone()).unwrap();
    validate_spec(&spec, &c, &split).unwrap()
}

fn plans(doc: &Value, m15: &[Candle], session: &Session) -> Vec<Plan> {
    let h1 = aggregate_h1(m15).unwrap();
    let entries = vec![true; m15.len()];
    let s = LabSeries {
        m15,
        h1: &h1,
        session,
        entries: &entries,
    };
    plans_from_spec(&valid(doc), &s)
}

#[test]
fn a_cross_gives_a_plan_with_an_atr_stop_and_a_time_exit() {
    let (session, m15) = (session(), session_day(v_shape));
    let doc = spec(
        "15m",
        ma(2, 4),
        json!({ "max_candles": 3, "target_r": "2" }),
    );
    let got = plans(&doc, &m15, &session);
    let cross = crosses(&sma(&m15, 2), &sma(&m15, 4));
    assert_eq!(cross.len(), 1, "{cross:?}");
    let t = cross[0].index;
    let atr = wilder_atr(&m15, 3)[t].unwrap();
    assert_eq!(
        got,
        vec![Plan {
            event: 0,
            signal_index: t,
            side: Side::Long,
            reference: m15[t].close,
            stop: m15[t].close - atr,
            risk_atr: atr,
            exit_index: t + 3,
            target_r: Some(dec!(2)),
            target_cap: None,
        }]
    );
}

#[test]
fn direction_and_hours_only_remove_entries() {
    let (session, m15) = (session(), session_day(inverted_v));
    let doc = spec("15m", ma(2, 4), json!({ "max_candles": 3 }));
    let both = plans(&doc, &m15, &session);
    assert_eq!(both.len(), 1);
    assert_eq!(both[0].side, Side::Short);
    assert_eq!(both[0].stop, both[0].reference + both[0].risk_atr);
    let mut long_only = doc.clone();
    long_only["direction"] = json!("long");
    assert!(plans(&long_only, &m15, &session).is_empty());
    let mut short_only = doc.clone();
    short_only["direction"] = json!("short");
    assert_eq!(plans(&short_only, &m15, &session), both);
    // La decisión (cierre de la vela) tiene que caer en [desde, hasta) UTC.
    let decision = m15[both[0].signal_index].end().time();
    let hm = |t: NaiveTime| t.format("%H:%M").to_string();
    let mut inside = doc.clone();
    inside["filters"] = json!([{ "type": "entry_hours_utc",
        "params": { "from": hm(decision), "to": "23:00" } }]);
    assert_eq!(plans(&inside, &m15, &session), both);
    let mut outside = doc.clone();
    outside["filters"] = json!([{ "type": "entry_hours_utc",
        "params": { "from": "14:00", "to": hm(decision) } }]);
    assert!(plans(&outside, &m15, &session).is_empty());
    // Un tramo que cruza la medianoche (desde > hasta) deja pasar la tarde.
    let mut overnight = doc.clone();
    overnight["filters"] = json!([{ "type": "entry_hours_utc",
        "params": { "from": "15:00", "to": "02:00" } }]);
    assert_eq!(plans(&overnight, &m15, &session), both);
}

#[test]
fn the_session_close_exit_is_the_last_candle_of_the_cash_session() {
    let (session, m15) = (session(), session_day(v_shape));
    let doc = spec(
        "15m",
        ma(2, 4),
        json!({ "max_candles": 100, "session_close": true }),
    );
    let got = plans(&doc, &m15, &session);
    assert_eq!(got.len(), 1);
    assert_eq!(m15[got[0].exit_index].end(), at("2024-01-10T21:00:00Z"));
    // Sin la vela del cierre (los datos terminan antes) la salida no se conoce: no hay plan.
    assert!(plans(&doc, &m15[..25], &session).is_empty());
    // Con las dos salidas, gana la primera.
    let short = spec(
        "15m",
        ma(2, 4),
        json!({ "max_candles": 2, "session_close": true }),
    );
    let got = plans(&short, &m15, &session);
    assert_eq!(got[0].exit_index, got[0].signal_index + 2);
}

#[test]
fn an_entry_outside_the_cash_session_has_no_session_exit() {
    let session = session();
    // Futuros casi 24 h: la V termina de noche (antes de la apertura de las 14:30 UTC).
    let closes: Vec<Decimal> = (0..40).map(v_shape).collect();
    let m15 = candles_from(at("2024-01-10T02:00:00Z"), &closes);
    let by_session = spec("15m", ma(2, 4), json!({ "session_close": true }));
    assert!(plans(&by_session, &m15, &session).is_empty());
    let by_time = spec("15m", ma(2, 4), json!({ "max_candles": 4 }));
    assert_eq!(plans(&by_time, &m15, &session).len(), 1);
}

#[test]
fn the_1h_timeframe_decides_when_the_hour_closes() {
    let session = session();
    let closes: Vec<Decimal> = (0..96).map(|k| v_shape(k / 4)).collect();
    let m15 = candles_from(at("2024-01-10T00:00:00Z"), &closes);
    let h1 = aggregate_h1(&m15).unwrap();
    let doc = spec("1h", ma(2, 4), json!({ "max_candles": 2 }));
    let got = plans(&doc, &m15, &session);
    let cross = crosses(&sma(&h1, 2), &sma(&h1, 4));
    assert_eq!((got.len(), cross.len()), (1, 1));
    let t = cross[0].index;
    // La decisión es la vela de 15m que cierra junto con la hora t; la salida, la que cierra con
    // la hora t + 2. El ATR es el de 1h.
    assert_eq!(m15[got[0].signal_index].end(), h1[t].end());
    assert_eq!(m15[got[0].exit_index].end(), h1[t + 2].end());
    assert_eq!(got[0].risk_atr, wilder_atr(&h1, 3)[t].unwrap());
    assert_eq!(got[0].reference, h1[t].close);
}

#[test]
fn the_opening_range_breakout_is_the_one_of_round_1() {
    let session = session();
    // Rango de las dos primeras velas (995–1005); la sexta cierra arriba del máximo.
    let mut closes = vec![dec!(1000), dec!(1004), dec!(1001), dec!(1003), dec!(1002)];
    closes.extend([dec!(1010), dec!(1012)]);
    closes.extend(std::iter::repeat_n(dec!(1012), 19));
    let m15 = candles_from(at("2024-01-10T14:30:00Z"), &closes);
    let orb = json!({ "type": "opening_range_breakout",
        "params": { "range_candles": 2, "last_breakout_close": "15:00" } });
    // El ATR de 3 velas recién está a partir de la tercera: alcanza para la ruptura.
    let doc = spec("15m", orb, json!({ "session_close": true }));
    let got = plans(&doc, &m15, &session);
    let entries = vec![true; m15.len()];
    let round1 = orb_plans(
        &m15,
        &session,
        &entries,
        &wilder_atr(&m15, 3),
        &OrbConfig {
            range_candles: 2,
            last_breakout_close: NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
        },
    );
    assert_eq!(round1.len(), 1);
    assert_eq!(got.len(), 1);
    assert_eq!(
        (got[0].signal_index, got[0].side),
        (round1[0].signal_index, round1[0].side)
    );
    // El stop y la salida son los de la spec, no los de la ronda 1.
    assert_eq!(got[0].stop, got[0].reference - got[0].risk_atr);
    assert_eq!(got[0].exit_index, round1[0].exit_index);
}

#[test]
fn all_conditions_have_to_agree_and_warmup_gives_nothing() {
    let (session, m15) = (session(), session_day(v_shape));
    let one = spec("15m", ma(2, 4), json!({ "max_candles": 3 }));
    let mut twice = one.clone();
    twice["entry"] = json!([ma(2, 4), ma(2, 4)]);
    assert_eq!(plans(&twice, &m15, &session), plans(&one, &m15, &session));
    // Dos cruces que no caen en la misma vela no dan entrada.
    let mut apart = one.clone();
    apart["entry"] = json!([ma(2, 4), ma(5, 20)]);
    let a = plans(&one, &m15, &session)[0].signal_index;
    let other = crosses(&sma(&m15, 5), &sma(&m15, 20));
    assert!(other.iter().all(|c| c.index != a));
    assert!(plans(&apart, &m15, &session).is_empty());
    // Sin ATR (período más largo que los datos) no hay plan.
    let mut cold = one.clone();
    cold["stop"]["params"]["period"] = json!(40);
    assert!(plans(&cold, &m15, &session).is_empty());
}

// ── Sin repintado: cortar la serie no cambia los planes anteriores ──────────────────────────

/// Velas de 15m casi 24 h (de 00:00 a 22:00 UTC, lunes a viernes) con un camino al azar.
fn random_walk(steps: &[i8], days: usize) -> Vec<Candle> {
    let mut closes = Vec::new();
    let mut out = Vec::new();
    let mut price = dec!(1000);
    let mut k = 0usize;
    let mut day = NaiveDate::from_ymd_opt(2024, 1, 8).unwrap();
    while out.len() < days * 88 {
        let weekday = day.format("%u").to_string();
        if weekday != "6" && weekday != "7" {
            closes.clear();
            for _ in 0..88 {
                price += Decimal::from(steps[k % steps.len()]);
                k += 1;
                closes.push(price);
            }
            let start = day.and_hms_opt(0, 0, 0).unwrap().and_utc();
            out.extend(candles_from(start, &closes));
        }
        day = day.succ_opt().unwrap();
    }
    out
}

fn without_event(p: &[Plan]) -> Vec<Plan> {
    p.iter()
        .cloned()
        .map(|mut x| {
            x.event = 0;
            x
        })
        .collect()
}

#[test]
fn the_random_walk_of_the_property_gives_plans_on_both_sides_of_a_cut() {
    let session = session();
    let steps: Vec<i8> = (0..97).map(|k| ((k * 7) % 13) as i8 - 6).collect();
    let full = random_walk(&steps, 5);
    for timeframe in ["15m", "1h"] {
        let doc = spec(timeframe, ma(2, 5), json!({ "max_candles": 6 }));
        let all = plans(&doc, &full, &session);
        assert!(all.iter().any(|p| p.exit_index < 220), "{timeframe}");
        assert!(all.iter().any(|p| p.exit_index >= 220), "{timeframe}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn cutting_the_series_never_changes_earlier_plans(
        steps in prop::collection::vec(-6i8..=6, 50..200),
        cut in 100usize..440,
        fast in 2i64..6,
        gap in 1i64..8,
        period in 2i64..10,
        max_candles in prop::option::of(1i64..30),
        session_close in any::<bool>(),
        h1 in any::<bool>(),
        orb in any::<bool>(),
    ) {
        let session = session();
        let full = random_walk(&steps, 5);
        let exit = match (max_candles, session_close) {
            (None, false) => json!({ "max_candles": 5 }),
            (Some(n), s) => json!({ "max_candles": n, "session_close": s }),
            (None, true) => json!({ "session_close": true }),
        };
        let entry = if orb && !h1 {
            json!({ "type": "opening_range_breakout",
                "params": { "range_candles": 2, "last_breakout_close": "16:00" } })
        } else {
            ma(fast, fast + gap)
        };
        let mut doc = spec(if h1 { "1h" } else { "15m" }, entry, exit);
        doc["stop"]["params"]["period"] = json!(period);
        let all = plans(&doc, &full, &session);
        let cut_plans = plans(&doc, &full[..cut], &session);
        let expected: Vec<Plan> = all.iter().filter(|p| p.exit_index < cut).cloned().collect();
        prop_assert_eq!(without_event(&cut_plans), without_event(&expected));
    }
}

//! Laboratorio (#726): `validate_spec` con cada error, la spec tipada y el hash normalizado.

use cfds_backtest::lab::{
    Average, EntryCondition, EntryFilter, ErrorCode, ExitRule, LabConfig, StopRule,
    StrategyCatalog, StrategySpec, ValidSpec, catalog, validate_spec,
};
use cfds_core::Timeframe;
use cfds_market_data::{DataSplit, Period};
use cfds_risk::RiskConfig;
use chrono::NaiveTime;
use rust_decimal::dec;
use serde_json::{Value, json};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn lab_cfg() -> LabConfig {
    LabConfig::from_toml(cfg!("lab.toml")).unwrap()
}

fn risk() -> RiskConfig {
    RiskConfig::from_toml(cfg!("risk.toml")).unwrap()
}

fn engine_catalog() -> StrategyCatalog {
    catalog(&lab_cfg(), &risk(), ["US500".parse().unwrap()]).unwrap()
}

fn split() -> DataSplit {
    DataSplit {
        windows: Vec::new(),
        out_of_sample: Period {
            start: "2025-11-01T00:00:00Z".parse().unwrap(),
            end: "2026-11-01T00:00:00Z".parse().unwrap(),
        },
    }
}

fn spec() -> Value {
    json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": "1h" },
        "direction": "both",
        "entry": [
            { "type": "ma_cross", "params": { "average": "ema", "fast": 20, "slow": 50 } }
        ],
        "filters": [
            { "type": "entry_hours_utc", "params": { "from": "13:30", "to": "20:00" } }
        ],
        "stop": { "type": "atr", "params": { "period": 14, "k": "1.50" } },
        "exit": { "type": "time_target", "params": { "max_candles": 6, "target_r": "2.0" } },
        "risk_pct": "0.50",
        "cost_multiplier": "1.0",
        "period": { "from": "2015-01-01T00:00:00Z", "to": "2023-01-01T00:00:00Z" },
        "mode": "exploratory"
    })
}

fn set(doc: &Value, pointer: &str, value: Value) -> Value {
    let mut d = doc.clone();
    if let Some(v) = d.pointer_mut(pointer) {
        *v = value;
    } else {
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        d.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), value);
    }
    d
}

fn validate(doc: &Value) -> Result<ValidSpec, Vec<(ErrorCode, String)>> {
    let spec: StrategySpec = serde_json::from_value(doc.clone()).unwrap();
    validate_spec(&spec, &engine_catalog(), &split())
        .map_err(|e| e.into_iter().map(|x| (x.code, x.path)).collect())
}

fn codes(doc: &Value) -> Vec<(ErrorCode, String)> {
    validate(doc).expect_err("tenía que fallar")
}

fn one(doc: &Value, code: ErrorCode, path: &str) {
    assert_eq!(codes(doc), vec![(code, path.to_owned())], "{doc}");
}

#[test]
fn a_valid_spec_is_typed_and_normalized() {
    let v = validate(&spec()).unwrap();
    assert_eq!(v.timeframe, Timeframe::H1);
    assert_eq!(
        v.entry,
        vec![EntryCondition::MaCross {
            average: Average::Ema,
            fast: 20,
            slow: 50
        }]
    );
    let hm = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap();
    assert_eq!(
        v.filters,
        vec![EntryFilter::EntryHoursUtc {
            from: hm(13, 30),
            to: hm(20, 0)
        }]
    );
    assert_eq!(
        v.stop,
        StopRule::Atr {
            period: 14,
            k: dec!(1.5)
        }
    );
    assert_eq!(
        v.exit,
        ExitRule {
            max_candles: Some(6),
            session_close: false,
            target_r: Some(dec!(2))
        }
    );
    // Normalizada: decimales sin ceros de más.
    let out = serde_json::to_value(&v.spec).unwrap();
    assert_eq!(out["risk_pct"], json!("0.5"));
    assert_eq!(out["cost_multiplier"], json!("1"));
    assert_eq!(out["stop"]["params"]["k"], json!("1.5"));
    assert_eq!(out["exit"]["params"]["target_r"], json!("2"));
    assert_eq!(v.spec_sha256.len(), 64);
}

#[test]
fn the_hash_ignores_key_order_and_trailing_zeros() {
    let a = validate(&spec()).unwrap();
    let text = r#"{"mode":"exploratory","period":{"to":"2023-01-01T00:00:00Z","from":"2015-01-01T00:00:00Z"},
        "cost_multiplier":"1","risk_pct":"0.5","exit":{"params":{"target_r":"2","max_candles":6},"type":"time_target"},
        "stop":{"params":{"k":"1.5","period":14},"type":"atr"},
        "filters":[{"params":{"to":"20:00","from":"13:30"},"type":"entry_hours_utc"}],
        "entry":[{"params":{"slow":50,"fast":20,"average":"ema"},"type":"ma_cross"}],
        "direction":"both","market":{"timeframe":"1h","source":"futures","instrument_id":"US500"},
        "catalog_version":1}"#;
    let b = validate(&serde_json::from_str(text).unwrap()).unwrap();
    assert_eq!(a.spec_sha256, b.spec_sha256);
    assert_eq!(a.family_sha256, b.family_sha256);
    // Otro parámetro: otra spec, la misma familia.
    let c = validate(&set(&spec(), "/entry/0/params/fast", json!(10))).unwrap();
    assert_ne!(a.spec_sha256, c.spec_sha256);
    assert_eq!(a.family_sha256, c.family_sha256);
    let d = validate(&set(&spec(), "/direction", json!("long"))).unwrap();
    assert_eq!(a.family_sha256, d.family_sha256);
    // Otro tipo de bloque (sin el filtro): otra familia.
    let e = validate(&set(&spec(), "/filters", json!([]))).unwrap();
    assert_ne!(a.family_sha256, e.family_sha256);
}

#[test]
fn unknown_misplaced_or_unfit_blocks_are_rejected() {
    one(
        &set(&spec(), "/entry/0/type", json!("rsi")),
        ErrorCode::UnknownBlock,
        "/entry/0/type",
    );
    one(
        &set(&spec(), "/stop", spec()["filters"][0].clone()),
        ErrorCode::WrongSlot,
        "/stop/type",
    );
    // La ruptura del rango de apertura es solo de 15m.
    let orb = json!({
        "type": "opening_range_breakout",
        "params": { "range_candles": 2, "last_breakout_close": "15:00" }
    });
    one(
        &set(&spec(), "/entry/0", orb.clone()),
        ErrorCode::BlockTimeframe,
        "/entry/0/type",
    );
    let ok = validate(&set(
        &set(&spec(), "/entry/0", orb),
        "/market/timeframe",
        json!("15m"),
    ))
    .unwrap();
    assert!(matches!(
        ok.entry[0],
        EntryCondition::OpeningRangeBreakout {
            range_candles: 2,
            ..
        }
    ));
}

#[test]
fn every_param_is_checked_against_the_catalog() {
    one(
        &set(&spec(), "/entry/0/params/extra", json!(1)),
        ErrorCode::UnknownParam,
        "/entry/0/params/extra",
    );
    let mut missing = spec();
    missing["stop"]["params"]
        .as_object_mut()
        .unwrap()
        .remove("period");
    one(&missing, ErrorCode::MissingParam, "/stop/params/period");
    one(
        &set(&spec(), "/entry/0/params/fast", json!("20")),
        ErrorCode::ParamType,
        "/entry/0/params/fast",
    );
    one(
        &set(&spec(), "/stop/params/k", json!("1,5")),
        ErrorCode::ParamType,
        "/stop/params/k",
    );
    one(
        &set(&spec(), "/filters/0/params/from", json!("25:00")),
        ErrorCode::ParamType,
        "/filters/0/params/from",
    );
    one(
        &set(&spec(), "/exit/params/session_close", json!("true")),
        ErrorCode::ParamType,
        "/exit/params/session_close",
    );
    one(
        &set(&spec(), "/entry/0/params/fast", json!(1)),
        ErrorCode::ParamOutOfRange,
        "/entry/0/params/fast",
    );
    one(
        &set(&spec(), "/entry/0/params/average", json!("wma")),
        ErrorCode::ParamOutOfRange,
        "/entry/0/params/average",
    );
    // La distancia del stop sale de config/risk.toml (0,5 a 3 ATR).
    one(
        &set(&spec(), "/stop/params/k", json!("3.5")),
        ErrorCode::ParamOutOfRange,
        "/stop/params/k",
    );
    one(
        &set(&spec(), "/stop/params/k", json!("0.4")),
        ErrorCode::ParamOutOfRange,
        "/stop/params/k",
    );
}

#[test]
fn params_that_do_not_go_together_are_rejected() {
    one(
        &set(&spec(), "/entry/0/params/fast", json!(50)),
        ErrorCode::InvalidCombination,
        "/entry/0/params/fast",
    );
    one(
        &set(&spec(), "/exit/params", json!({ "target_r": "2" })),
        ErrorCode::InvalidCombination,
        "/exit/params",
    );
    one(
        &set(
            &spec(),
            "/exit/params",
            json!({ "session_close": false, "target_r": "2" }),
        ),
        ErrorCode::InvalidCombination,
        "/exit/params",
    );
    assert!(
        validate(&set(
            &spec(),
            "/exit/params",
            json!({ "session_close": true })
        ))
        .is_ok()
    );
    one(
        &set(&spec(), "/preregistration_sha256", json!("a".repeat(64))),
        ErrorCode::InvalidCombination,
        "/preregistration_sha256",
    );
    one(
        &set(&spec(), "/preregistration_sha256", json!("A".repeat(64))),
        ErrorCode::Malformed,
        "/preregistration_sha256",
    );
}

#[test]
fn limits_risk_costs_period_and_mode_are_enforced() {
    let entry = spec()["entry"][0].clone();
    one(
        &set(&spec(), "/entry", json!([entry, entry, entry, entry])),
        ErrorCode::TooManyConditions,
        "/entry",
    );
    let filter = spec()["filters"][0].clone();
    one(
        &set(&spec(), "/filters", json!([filter, filter, filter])),
        ErrorCode::TooManyFilters,
        "/filters",
    );
    one(
        &set(&spec(), "/entry", json!([])),
        ErrorCode::Malformed,
        "/entry",
    );
    // Riesgo: mayor que 0 y como mucho el de config/risk.toml (1 %).
    one(
        &set(&spec(), "/risk_pct", json!("0")),
        ErrorCode::RiskOutOfRange,
        "/risk_pct",
    );
    one(
        &set(&spec(), "/risk_pct", json!("1.01")),
        ErrorCode::RiskOutOfRange,
        "/risk_pct",
    );
    assert!(validate(&set(&spec(), "/risk_pct", json!("1"))).is_ok());
    // Costos: solo se pueden subir.
    one(
        &set(&spec(), "/cost_multiplier", json!("0.99")),
        ErrorCode::CostOutOfRange,
        "/cost_multiplier",
    );
    one(
        &set(&spec(), "/cost_multiplier", json!("5.5")),
        ErrorCode::CostOutOfRange,
        "/cost_multiplier",
    );
    one(
        &set(&spec(), "/period/to", json!("2015-01-01T00:00:00Z")),
        ErrorCode::PeriodInvalid,
        "/period",
    );
    // El fuera de muestra empieza el 01-11-2025: llegar hasta ahí está bien, pasarlo no.
    assert!(validate(&set(&spec(), "/period/to", json!("2025-11-01T00:00:00Z"))).is_ok());
    one(
        &set(&spec(), "/period/to", json!("2025-11-01T00:15:00Z")),
        ErrorCode::PeriodTouchesHoldout,
        "/period/to",
    );
    one(
        &set(&spec(), "/mode", json!("confirmatory")),
        ErrorCode::ModeUnavailable,
        "/mode",
    );
    one(
        &set(&spec(), "/market/instrument_id", json!("DE40")),
        ErrorCode::UnknownMarket,
        "/market/instrument_id",
    );
    one(
        &set(&spec(), "/catalog_version", json!(2)),
        ErrorCode::CatalogVersion,
        "/catalog_version",
    );
}

#[test]
fn all_errors_come_together() {
    let bad = set(
        &set(
            &set(&spec(), "/risk_pct", json!("3")),
            "/entry/0/params/fast",
            json!(1),
        ),
        "/stop/type",
        json!("trailing"),
    );
    let found = codes(&bad);
    assert_eq!(found.len(), 3, "{found:?}");
    for code in [
        ErrorCode::RiskOutOfRange,
        ErrorCode::ParamOutOfRange,
        ErrorCode::UnknownBlock,
    ] {
        assert!(found.iter().any(|(c, _)| *c == code), "{code:?}");
    }
}

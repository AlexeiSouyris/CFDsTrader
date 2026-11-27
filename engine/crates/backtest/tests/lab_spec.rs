//! Laboratorio (#745): forma canónica y hashes de una spec.

use cfds_backtest::lab::{StrategySpec, canonical_json, family_sha256, sha256_hex, spec_sha256};
use serde_json::{Value, json};

fn spec() -> Value {
    json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": "1h" },
        "direction": "both",
        "entry": [
            { "type": "ma_cross", "params": { "average": "ema", "fast": 20, "slow": 50 } },
            { "type": "opening_range_breakout", "params": { "range_candles": 2 } }
        ],
        "filters": [],
        "stop": { "type": "atr", "params": { "period": 14, "k": "1.5" } },
        "exit": { "type": "time_target", "params": { "max_candles": 6 } },
        "risk_pct": "0.5",
        "cost_multiplier": "1",
        "period": { "from": "2015-01-01T00:00:00Z", "to": "2023-01-01T00:00:00Z" },
        "mode": "exploratory"
    })
}

fn parse(v: &Value) -> StrategySpec {
    serde_json::from_value(v.clone()).unwrap()
}

fn set(doc: &Value, pointer: &str, value: Value) -> Value {
    let mut d = doc.clone();
    *d.pointer_mut(pointer).unwrap() = value;
    d
}

#[test]
fn the_canonical_form_sorts_keys_and_has_no_spaces() {
    let v = json!({ "b": [2, { "z": 1, "a": "x y" }], "a": true });
    assert_eq!(
        canonical_json(&v),
        r#"{"a":true,"b":[2,{"a":"x y","z":1}]}"#
    );
    // SHA-256 de "abc" (FIPS 180-2, ejemplo B.1).
    assert_eq!(
        sha256_hex("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn the_spec_hash_does_not_depend_on_key_order() {
    let a = spec_sha256(&parse(&spec())).unwrap();
    let text = r#"{"mode":"exploratory","period":{"to":"2023-01-01T00:00:00Z","from":"2015-01-01T00:00:00Z"},
        "cost_multiplier":"1","risk_pct":"0.5","exit":{"params":{"max_candles":6},"type":"time_target"},
        "stop":{"params":{"k":"1.5","period":14},"type":"atr"},"filters":[],
        "entry":[{"params":{"slow":50,"fast":20,"average":"ema"},"type":"ma_cross"},
                 {"type":"opening_range_breakout","params":{"range_candles":2}}],
        "direction":"both","market":{"timeframe":"1h","source":"futures","instrument_id":"US500"},
        "catalog_version":1}"#;
    let b = spec_sha256(&serde_json::from_str(text).unwrap()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.len(), 64);
    let c = spec_sha256(&parse(&set(&spec(), "/entry/0/params/fast", json!(21)))).unwrap();
    assert_ne!(a, c);
}

#[test]
fn the_family_is_the_instrument_and_the_block_types() {
    let base = family_sha256(&parse(&spec()));
    // Parámetros, dirección, timeframe, riesgo, costos y período: la misma familia.
    for (pointer, value) in [
        ("/entry/0/params/fast", json!(10)),
        ("/direction", json!("long")),
        ("/market/timeframe", json!("15m")),
        ("/risk_pct", json!("1")),
        ("/cost_multiplier", json!("1.5")),
        ("/period/from", json!("2016-01-01T00:00:00Z")),
    ] {
        assert_eq!(family_sha256(&parse(&set(&spec(), pointer, value))), base);
    }
    // El orden de las condiciones de entrada no importa ("y" es conmutativo).
    let mut swapped = spec();
    swapped["entry"].as_array_mut().unwrap().reverse();
    assert_eq!(family_sha256(&parse(&swapped)), base);
    // Otro instrumento u otro tipo de bloque: otra familia.
    for (pointer, value) in [
        ("/market/instrument_id", json!("US100")),
        ("/stop/type", json!("structural")),
        (
            "/filters",
            json!([{ "type": "entry_hours_utc", "params": {} }]),
        ),
    ] {
        assert_ne!(family_sha256(&parse(&set(&spec(), pointer, value))), base);
    }
}

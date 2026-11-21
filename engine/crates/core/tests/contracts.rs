//! Los tipos de Rust y los contratos de `contracts/` tienen que decir exactamente lo mismo:
//! lo que Rust serializa valida contra el JSON Schema, vuelve idéntico al deserializar,
//! y lo que el schema rechaza, Rust también lo rechaza.

use std::path::PathBuf;

use cfds_core::{AssetClass, Candle, Currency, Instrument, Timeframe};
use rust_decimal::dec;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let schema: Value = serde_json::from_str(&text).expect("el contrato no es JSON válido");
    jsonschema::validator_for(&schema).expect("el contrato no es un JSON Schema válido")
}

fn sample_candle(volume: Option<rust_decimal::Decimal>) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: "2026-11-19T15:30:00Z".parse().unwrap(),
        open: dec!(26756.6),
        high: dec!(26760.8),
        low: dec!(26732.3),
        close: dec!(26747.80),
        volume,
    }
}

fn sample_instrument() -> Instrument {
    Instrument {
        id: "US100".parse().unwrap(),
        name: "US Tech 100".into(),
        asset_class: AssetClass::Index,
        currency: Currency::USD,
        exchange_timezone: "America/New_York".into(),
    }
}

/// Serializar → validar contra el schema → deserializar da lo mismo.
fn assert_roundtrip<T>(schema: &str, value: &T)
where
    T: serde::Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_value(value).unwrap();
    let errors: Vec<String> = validator(schema)
        .iter_errors(&json)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{schema} rechazó {json}: {errors:?}");
    let back: T = serde_json::from_value(json).unwrap();
    assert_eq!(&back, value);
}

/// El schema y Rust rechazan el mismo documento.
fn assert_both_reject<T: DeserializeOwned + std::fmt::Debug>(schema: &str, doc: Value) {
    assert!(
        !validator(schema).is_valid(&doc),
        "{schema} debería rechazar {doc}"
    );
    assert!(
        serde_json::from_value::<T>(doc.clone()).is_err(),
        "Rust debería rechazar {doc}"
    );
}

#[test]
fn candle_roundtrips_through_the_contract() {
    assert_roundtrip("candle.v1.schema.json", &sample_candle(Some(dec!(786))));
    assert_roundtrip("candle.v1.schema.json", &sample_candle(None));
}

#[test]
fn candle_prices_are_decimal_strings_and_keep_their_scale() {
    let json = serde_json::to_value(sample_candle(None)).unwrap();
    assert_eq!(json["close"], json!("26747.80"));
    assert_eq!(json["start"], json!("2026-11-19T15:30:00Z"));
    assert_eq!(json["timeframe"], json!("15m"));
    assert_eq!(json["volume"], Value::Null);
}

#[test]
fn instrument_roundtrips_through_the_contract() {
    assert_roundtrip("instrument.v1.schema.json", &sample_instrument());
}

fn candle_doc() -> Value {
    serde_json::to_value(sample_candle(Some(dec!(786)))).unwrap()
}

#[test]
fn contract_and_rust_reject_the_same_bad_candles() {
    let schema = "candle.v1.schema.json";
    let mut cases: Vec<(&str, Value)> = Vec::new();

    let mut d = candle_doc();
    d["close"] = json!(26747.8);
    cases.push(("precio como número JSON (float)", d));

    let mut d = candle_doc();
    d["start"] = json!("2026-11-19T16:30:00+01:00");
    cases.push(("hora con zona que no es Z", d));

    let mut d = candle_doc();
    d["start"] = json!("2026-11-19T15:30:00");
    cases.push(("hora sin zona", d));

    let mut d = candle_doc();
    d["timeframe"] = json!("5m");
    cases.push(("timeframe fuera de la spec", d));

    let mut d = candle_doc();
    d["low"] = json!("-1");
    cases.push(("precio negativo", d));

    let mut d = candle_doc();
    d["extra"] = json!(1);
    cases.push(("campo no definido en el contrato", d));

    let mut d = candle_doc();
    d.as_object_mut().unwrap().remove("volume");
    cases.push(("falta volume (tiene que venir, aunque sea null)", d));

    let mut d = candle_doc();
    d["instrument_id"] = json!("us100");
    cases.push(("id de instrumento en minúsculas", d));

    for (what, doc) in cases {
        eprintln!("caso: {what}");
        assert_both_reject::<Candle>(schema, doc);
    }
}

#[test]
fn contract_and_rust_reject_the_same_bad_instruments() {
    let schema = "instrument.v1.schema.json";
    let base = serde_json::to_value(sample_instrument()).unwrap();

    let mut d = base.clone();
    d["currency"] = json!("usd");
    assert_both_reject::<Instrument>(schema, d);

    let mut d = base.clone();
    d["asset_class"] = json!("crypto");
    assert_both_reject::<Instrument>(schema, d);

    let mut d = base;
    d["epic"] = json!("IX.D.NASDAQ.IFE.IP");
    assert_both_reject::<Instrument>(schema, d);
}

fn sample_sentiment(instrument: Option<&str>) -> cfds_core::SentimentObservation {
    cfds_core::SentimentObservation {
        source: "ig_client".into(),
        series: "long_pct".into(),
        instrument_id: instrument.map(|i| i.parse().unwrap()),
        observed_for: "2026-11-20T21:45:00Z".parse().unwrap(),
        published_at: "2026-11-20T21:45:03Z".parse().unwrap(),
        value: dec!(68.0),
    }
}

#[test]
fn sentiment_observation_roundtrips_through_the_contract() {
    let schema = "sentiment_observation.v1.schema.json";
    assert_roundtrip(schema, &sample_sentiment(Some("US100")));
    // Dato de todo el mercado (sin instrumento) y valor con signo (índices futuros, #25).
    let mut market = sample_sentiment(None);
    market.value = dec!(-0.35);
    assert_roundtrip(schema, &market);
    let json = serde_json::to_value(sample_sentiment(Some("US100"))).unwrap();
    assert_eq!(json["value"], json!("68.0"));
    assert_eq!(json["instrument_id"], json!("US100"));
}

#[test]
fn contract_and_rust_reject_the_same_bad_sentiment_observations() {
    let schema = "sentiment_observation.v1.schema.json";
    let doc = || serde_json::to_value(sample_sentiment(Some("US100"))).unwrap();
    let mut cases: Vec<(&str, Value)> = Vec::new();

    let mut d = doc();
    d["value"] = json!(68.0);
    cases.push(("valor como número JSON (float)", d));

    let mut d = doc();
    d["published_at"] = json!("2026-11-20T22:45:03+01:00");
    cases.push(("hora con zona que no es Z", d));

    let mut d = doc();
    d["source"] = json!("IG Client");
    cases.push(("fuente con mayúsculas y espacios", d));

    let mut d = doc();
    d["source"] = json!("Ig_client");
    cases.push(("fuente que empieza con mayúscula", d));

    let mut d = doc();
    d["series"] = json!("");
    cases.push(("serie vacía", d));

    let mut d = doc();
    d["instrument_id"] = json!("us100");
    cases.push(("id de instrumento en minúsculas", d));

    let mut d = doc();
    d.as_object_mut().unwrap().remove("instrument_id");
    cases.push(("falta instrument_id (tiene que venir, aunque sea null)", d));

    let mut d = doc();
    d["extra"] = json!(1);
    cases.push(("campo no definido en el contrato", d));

    for (what, doc) in cases {
        eprintln!("caso: {what}");
        assert_both_reject::<cfds_core::SentimentObservation>(schema, doc);
    }
}

fn sample_spread() -> cfds_core::CandleSpread {
    cfds_core::CandleSpread {
        instrument_id: "DE40".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: "2026-11-20T09:00:00Z".parse().unwrap(),
        spread: dec!(1.40),
        source: cfds_core::spread::IG_DEMO.into(),
    }
}

#[test]
fn candle_spread_roundtrips_through_the_contract() {
    let schema = "candle_spread.v1.schema.json";
    assert_roundtrip(schema, &sample_spread());
    let mut zero = sample_spread();
    zero.spread = dec!(0);
    assert_roundtrip(schema, &zero);
    let json = serde_json::to_value(sample_spread()).unwrap();
    assert_eq!(json["spread"], json!("1.40"), "decimal exacto como string");
}

#[test]
fn contract_and_rust_reject_the_same_bad_candle_spreads() {
    let schema = "candle_spread.v1.schema.json";
    let doc = || serde_json::to_value(sample_spread()).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("spread", json!("-0.1")),
        ("spread", json!(1.4)),
        ("timeframe", json!("5m")),
        ("start", json!("2026-11-20T10:00:00+01:00")),
        ("source", json!("IG demo")),
        ("instrument_id", json!("de40")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("source");
    cases.push(d);
    for d in cases {
        assert_both_reject::<cfds_core::CandleSpread>(schema, d);
    }
}

fn sample_event() -> cfds_core::EconomicEvent {
    cfds_core::EconomicEvent {
        source: "calendario".into(),
        event: "Decisión de tipos".into(),
        country: Some("EU".into()),
        instrument_id: None,
        impact: cfds_core::Impact::High,
        scheduled_at: "2026-12-10T13:15:00Z".parse().unwrap(),
        published_at: "2026-12-01T08:00:00Z".parse().unwrap(),
    }
}

#[test]
fn economic_event_roundtrips_through_the_contract() {
    let schema = "economic_event.v1.schema.json";
    assert_roundtrip(schema, &sample_event());
    let mut of_instrument = sample_event();
    of_instrument.country = None;
    of_instrument.instrument_id = Some("US100".parse().unwrap());
    of_instrument.impact = cfds_core::Impact::Medium;
    assert_roundtrip(schema, &of_instrument);
    let json = serde_json::to_value(sample_event()).unwrap();
    assert_eq!(json["instrument_id"], Value::Null, "null, no ausente");
    assert_eq!(json["impact"], json!("high"));
}

#[test]
fn contract_and_rust_reject_the_same_bad_economic_events() {
    let schema = "economic_event.v1.schema.json";
    let doc = || serde_json::to_value(sample_event()).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("country", json!(null)), // sin país ni instrumento
        ("country", json!("eu")),
        ("country", json!("EUR")),
        ("impact", json!("critical")),
        ("event", json!("")),
        ("event", json!("x".repeat(201))),
        ("scheduled_at", json!("2026-12-10T14:15:00+01:00")),
        ("source", json!("Calendario")),
        ("instrument_id", json!("us100")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("instrument_id");
    cases.push(d); // tiene que venir, aunque sea null
    for d in cases {
        assert_both_reject::<cfds_core::EconomicEvent>(schema, d);
    }
}

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

fn sample_daily() -> cfds_core::DailyCandle {
    cfds_core::DailyCandle {
        instrument_id: "DE40".parse().unwrap(),
        day: "2026-11-20".parse().unwrap(),
        start: "2026-11-19T23:00:00Z".parse().unwrap(),
        end: "2026-11-20T23:00:00Z".parse().unwrap(),
        open: dec!(24100.5),
        high: dec!(24210.0),
        low: dec!(23950.25),
        close: dec!(24005.75),
        volume: None,
    }
}

#[test]
fn daily_candle_roundtrips_through_the_contract() {
    let schema = "daily_candle.v1.schema.json";
    assert_roundtrip(schema, &sample_daily());
    let mut with_volume = sample_daily();
    with_volume.volume = Some(dec!(1200));
    assert_roundtrip(schema, &with_volume);
    let json = serde_json::to_value(sample_daily()).unwrap();
    assert_eq!(json["day"], json!("2026-11-20"));
    assert_eq!(json["high"], json!("24210.0"), "decimal exacto como string");
}

#[test]
fn contract_and_rust_reject_the_same_bad_daily_candles() {
    let schema = "daily_candle.v1.schema.json";
    let doc = || serde_json::to_value(sample_daily()).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("day", json!("20-11-2026")),
        ("day", json!("2026-11-20T00:00:00Z")),
        ("start", json!("2026-11-20T00:00:00+01:00")),
        ("close", json!(24005.75)),
        ("close", json!("-1")),
        ("instrument_id", json!("de40")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("volume");
    cases.push(d); // tiene que venir, aunque sea null
    for d in cases {
        assert_both_reject::<cfds_core::DailyCandle>(schema, d);
    }
}

fn sample_rate() -> cfds_core::ReferenceRate {
    cfds_core::ReferenceRate {
        series: "ESTR".into(),
        ref_date: "2021-03-01".parse().unwrap(),
        published_at: "2021-03-02T08:00:00Z".parse().unwrap(),
        rate_pct: dec!(-0.567),
    }
}

#[test]
fn reference_rate_roundtrips_through_the_contract() {
    assert_roundtrip("reference_rate.v1.schema.json", &sample_rate());
}

#[test]
fn contract_and_rust_reject_the_same_bad_reference_rates() {
    let schema = "reference_rate.v1.schema.json";
    let doc = || serde_json::to_value(sample_rate()).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("series", json!("estr")),
        ("series", json!("")),
        ("ref_date", json!("01-03-2021")),
        ("published_at", json!("2021-03-02T09:00:00+01:00")),
        ("rate_pct", json!(-0.567)),
        ("rate_pct", json!("-0,567")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("published_at");
    cases.push(d);
    for d in cases {
        assert_both_reject::<cfds_core::ReferenceRate>(schema, d);
    }
}

/// Como lo escribe Python (`Interpretation.to_contract`, #122).
fn news_doc() -> Value {
    json!({
        "interpretation_id": "a".repeat(64),
        "source": "ecb",
        "item_id": "https://www.ecb.europa.eu/press/x.html",
        "published_at": "2026-11-19T12:45:00Z",
        "interpreted_at": "2026-11-19T12:48:00Z",
        "interpreter": {"name": "local-llm", "version": "0.1.0", "model": null, "prompt_sha256": null},
        "summary_es": "El BCE mantiene los tipos.",
        "impacts": [
            {"instrument_id": "DE40", "sentiment": "-0.25", "relevance": "0.8"},
            {"instrument_id": "US100", "sentiment": "1.0000", "relevance": "0"}
        ]
    })
}

#[test]
fn news_interpretation_from_python_roundtrips_through_the_contract() {
    let schema = "news_interpretation.v1.schema.json";
    assert!(validator(schema).is_valid(&news_doc()));
    let n: cfds_core::NewsInterpretation = serde_json::from_value(news_doc()).unwrap();
    assert_eq!(n.impacts[0].sentiment, dec!(-0.25));
    assert_eq!(n.impacts[1].sentiment, dec!(1.0000));
    assert_eq!(
        serde_json::to_value(&n).unwrap(),
        news_doc(),
        "vuelve idéntico"
    );
    assert_roundtrip(schema, &n);
}

#[test]
fn contract_and_rust_reject_the_same_bad_news_interpretations() {
    let schema = "news_interpretation.v1.schema.json";
    let mut cases: Vec<(&str, Value)> = Vec::new();
    for (field, bad) in [
        ("sentiment", json!("1.5")),
        ("sentiment", json!("-1.0001")),
        ("sentiment", json!("0.12345")),
        ("sentiment", json!("+0.5")),
        ("sentiment", json!(".5")),
        ("sentiment", json!(0.5)),
        ("relevance", json!("-0.1")),
        ("relevance", json!("-0")),
        ("relevance", json!("1.01")),
        ("instrument_id", json!("de40")),
    ] {
        let mut d = news_doc();
        d["impacts"][0][field] = bad;
        cases.push((field, d));
    }
    let mut d = news_doc();
    d["interpretation_id"] = json!("abc");
    cases.push(("id que no es SHA-256", d));
    let mut d = news_doc();
    d["source"] = json!("ECB");
    cases.push(("fuente en mayúsculas", d));
    let mut d = news_doc();
    d["interpreter"]["name"] = json!("Local LLM");
    cases.push(("nombre de intérprete con mayúsculas", d));
    let mut d = news_doc();
    d["interpreter"].as_object_mut().unwrap().remove("model");
    cases.push(("falta model (tiene que venir, aunque sea null)", d));
    let mut d = news_doc();
    d["interpreter"]["prompt_sha256"] = json!("123");
    cases.push(("prompt_sha256 que no es SHA-256", d));
    let mut d = news_doc();
    d["summary_es"] = json!("");
    cases.push(("resumen vacío", d));
    let mut d = news_doc();
    d["published_at"] = json!("2026-11-19T13:45:00+01:00");
    cases.push(("hora con zona que no es Z", d));
    let mut d = news_doc();
    d["adjustment"] = json!(1);
    cases.push(("el ajuste no viaja en el evento", d));
    for (what, doc) in cases {
        eprintln!("caso: {what}");
        assert_both_reject::<cfds_core::NewsInterpretation>(schema, doc);
    }
}

#[test]
fn rust_also_rejects_an_interpretation_before_its_publication() {
    let mut d = news_doc();
    d["interpreted_at"] = json!("2026-11-19T12:44:59Z");
    assert!(
        validator("news_interpretation.v1.schema.json").is_valid(&d),
        "el schema no puede comparar dos campos"
    );
    assert!(serde_json::from_value::<cfds_core::NewsInterpretation>(d).is_err());
}

fn sample_indicator(warm: bool) -> cfds_core::DailyIndicator {
    cfds_core::DailyIndicator {
        instrument_id: "DE40".parse().unwrap(),
        day: "2022-03-08".parse().unwrap(),
        published_at: "2022-03-08T23:00:00Z".parse().unwrap(),
        atr_period: 14,
        atr: warm.then_some(dec!(515.2)),
        realized_vol_window: 20,
        realized_vol: warm.then_some(dec!(0.0219)),
    }
}

#[test]
fn daily_indicator_roundtrips_through_the_contract() {
    let schema = "daily_indicator.v1.schema.json";
    assert_roundtrip(schema, &sample_indicator(true));
    assert_roundtrip(schema, &sample_indicator(false));
}

#[test]
fn contract_and_rust_reject_the_same_bad_daily_indicators() {
    let schema = "daily_indicator.v1.schema.json";
    let doc = || serde_json::to_value(sample_indicator(true)).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("atr", json!(515.2)),
        ("atr", json!("-1")),
        ("realized_vol", json!("2,19")),
        ("atr_period", json!(-1)),
        ("realized_vol_window", json!("20")),
        ("day", json!("08-03-2022")),
        ("published_at", json!("2022-03-09T00:00:00+01:00")),
        ("instrument_id", json!("de40")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("atr");
    cases.push(d); // tiene que venir, aunque sea null
    for d in cases {
        assert_both_reject::<cfds_core::DailyIndicator>(schema, d);
    }
}

fn sample_drawdown(warm: bool) -> cfds_core::DailyDrawdown {
    cfds_core::DailyDrawdown {
        instrument_id: "US500".parse().unwrap(),
        day: "2022-03-08".parse().unwrap(),
        published_at: "2022-03-08T23:00:00Z".parse().unwrap(),
        drawdown_window: 252,
        drawdown: warm.then_some(dec!(-0.1234)),
    }
}

#[test]
fn daily_drawdown_roundtrips_through_the_contract() {
    let schema = "daily_drawdown.v1.schema.json";
    assert_roundtrip(schema, &sample_drawdown(true));
    assert_roundtrip(schema, &sample_drawdown(false));
    let mut at_high = sample_drawdown(true);
    at_high.drawdown = Some(dec!(0));
    assert_roundtrip(schema, &at_high);
}

#[test]
fn contract_and_rust_reject_the_same_bad_daily_drawdowns() {
    let schema = "daily_drawdown.v1.schema.json";
    let doc = || serde_json::to_value(sample_drawdown(true)).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("drawdown", json!(-0.1)),
        ("drawdown", json!("-0,1")),
        ("drawdown", json!("--1")),
        ("drawdown_window", json!(-1)),
        ("drawdown_window", json!("252")),
        ("day", json!("08-03-2022")),
        ("published_at", json!("2022-03-09T00:00:00+01:00")),
        ("instrument_id", json!("us500")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("drawdown");
    cases.push(d); // tiene que venir, aunque sea null
    for d in cases {
        assert_both_reject::<cfds_core::DailyDrawdown>(schema, d);
    }
}

fn sample_vol_tercile(t: Option<cfds_core::VolTercile>) -> cfds_core::DailyVolTercile {
    cfds_core::DailyVolTercile {
        instrument_id: "US500".parse().unwrap(),
        day: "2022-03-08".parse().unwrap(),
        published_at: "2022-03-08T23:00:00Z".parse().unwrap(),
        realized_vol_window: 20,
        history_years: 3,
        tercile: t,
    }
}

#[test]
fn daily_vol_tercile_roundtrips_through_the_contract() {
    use cfds_core::VolTercile;
    let schema = "daily_vol_tercile.v1.schema.json";
    for t in [
        None,
        Some(VolTercile::Low),
        Some(VolTercile::Mid),
        Some(VolTercile::High),
    ] {
        assert_roundtrip(schema, &sample_vol_tercile(t));
    }
}

#[test]
fn contract_and_rust_reject_the_same_bad_daily_vol_terciles() {
    let schema = "daily_vol_tercile.v1.schema.json";
    let doc = || serde_json::to_value(sample_vol_tercile(None)).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("tercile", json!("medium")),
        ("tercile", json!("LOW")),
        ("history_years", json!("3")),
        ("realized_vol_window", json!(-1)),
        ("day", json!("08-03-2022")),
        ("published_at", json!("2022-03-09T00:00:00+01:00")),
        ("instrument_id", json!("us500")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("tercile");
    cases.push(d); // tiene que venir, aunque sea null
    for d in cases {
        assert_both_reject::<cfds_core::DailyVolTercile>(schema, d);
    }
}

fn sample_liquidity_level(
    kind: cfds_core::LiquidityLevelKind,
    session: Option<cfds_core::LiquiditySession>,
    state: cfds_core::LiquidityLevelState,
) -> cfds_core::LiquidityLevel {
    use cfds_core::LiquidityLevelState::Intact;
    cfds_core::LiquidityLevel {
        instrument_id: "US100".parse().unwrap(),
        kind,
        session,
        price: dec!(26760.80),
        exists_from: "2026-11-19T21:00:00Z".parse().unwrap(),
        state,
        state_changed_at: (state != Intact).then(|| "2026-11-20T14:45:00Z".parse().unwrap()),
        expired_at: None,
    }
}

#[test]
fn liquidity_level_roundtrips_through_the_contract() {
    use cfds_core::{LiquidityLevelKind as K, LiquidityLevelState as S, LiquiditySession as N};
    let schema = "liquidity_level.v1.schema.json";
    for kind in [K::Eqh, K::Eql, K::Pdh, K::Pdl, K::Pwh, K::Pwl] {
        for state in [S::Intact, S::Swept, S::Broken] {
            assert_roundtrip(schema, &sample_liquidity_level(kind, None, state));
        }
    }
    for kind in [K::SessionHigh, K::SessionLow] {
        for session in [N::Asia, N::London, N::Ny] {
            assert_roundtrip(
                schema,
                &sample_liquidity_level(kind, Some(session), S::Swept),
            );
        }
    }
    let json = serde_json::to_value(sample_liquidity_level(
        K::SessionHigh,
        Some(N::Ny),
        S::Intact,
    ))
    .unwrap();
    assert_eq!(json["price"], json!("26760.80"));
    assert_eq!(json["kind"], json!("session_high"));
    assert_eq!(json["session"], json!("ny"));
    assert_eq!(json["state_changed_at"], Value::Null);
    // `expired_at` es opcional: no se escribe si no venció, y un documento sin él es válido.
    assert!(json.get("expired_at").is_none());
    let mut expired = sample_liquidity_level(K::Eqh, None, S::Intact);
    expired.expired_at = Some("2026-11-25T10:00:00Z".parse().unwrap());
    assert_roundtrip(schema, &expired);
    let mut d = serde_json::to_value(sample_liquidity_level(K::Eqh, None, S::Intact)).unwrap();
    d["expired_at"] = Value::Null;
    assert!(validator(schema).is_valid(&d));
    assert!(serde_json::from_value::<cfds_core::LiquidityLevel>(d).is_ok());
}

#[test]
fn contract_and_rust_reject_the_same_bad_liquidity_levels() {
    use cfds_core::{LiquidityLevelKind as K, LiquidityLevelState as S};
    let schema = "liquidity_level.v1.schema.json";
    let doc = || serde_json::to_value(sample_liquidity_level(K::Pdh, None, S::Broken)).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("kind", json!("equal_highs")),
        ("kind", json!("EQH")),
        ("session", json!("asia")), // pdh no lleva sesión
        ("price", json!(26760.8)),
        ("price", json!("-1")),
        ("exists_from", json!("2026-11-19T22:00:00+01:00")),
        ("state", json!("expired")),
        ("state_changed_at", Value::Null), // roto sin la vela que lo rompió
        ("instrument_id", json!("us100")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d["kind"] = json!("session_low"); // sin el nombre de la sesión
    cases.push(d);
    let mut d = doc();
    d["state"] = json!("intact"); // intacto con fecha de cambio
    cases.push(d);
    let mut d = doc();
    d["expired_at"] = json!("2026-11-25T10:00:00Z"); // un nivel roto no vence
    cases.push(d);
    let mut d = doc();
    d["state"] = json!("intact");
    d["state_changed_at"] = Value::Null;
    d["expired_at"] = json!("2026-11-25"); // sin hora ni Z
    cases.push(d);
    for key in ["session", "state_changed_at"] {
        let mut d = doc();
        d.as_object_mut().unwrap().remove(key);
        cases.push(d); // tiene que venir, aunque sea null
    }
    for d in cases {
        assert_both_reject::<cfds_core::LiquidityLevel>(schema, d);
    }
}

#[test]
fn rust_also_rejects_a_state_change_before_the_level_exists() {
    use cfds_core::{LiquidityLevelKind as K, LiquidityLevelState as S};
    let mut d = serde_json::to_value(sample_liquidity_level(K::Eql, None, S::Swept)).unwrap();
    d["state_changed_at"] = d["exists_from"].clone();
    assert!(validator("liquidity_level.v1.schema.json").is_valid(&d));
    assert!(serde_json::from_value::<cfds_core::LiquidityLevel>(d).is_err());
    let mut d = serde_json::to_value(sample_liquidity_level(K::Eql, None, S::Intact)).unwrap();
    d["expired_at"] = d["exists_from"].clone();
    assert!(validator("liquidity_level.v1.schema.json").is_valid(&d));
    assert!(serde_json::from_value::<cfds_core::LiquidityLevel>(d).is_err());
}

// ---------- estadísticas de liquidez (#231) ----------

fn sample_fvg_event(open: bool) -> cfds_core::FvgEvent {
    use cfds_core::{Direction, FvgEventPoint, FvgReaction, StatsTimeframe, ZoneStatus};
    use std::num::NonZeroU32;
    let point = |at: &str, bars| FvgEventPoint {
        at: at.parse().unwrap(),
        bars: NonZeroU32::new(bars).unwrap(),
    };
    cfds_core::FvgEvent {
        instrument_id: "US100".parse().unwrap(),
        timeframe: StatsTimeframe::M15,
        direction: Direction::Bearish,
        created_at: "2026-11-19T15:45:00Z".parse().unwrap(),
        bottom: dec!(101.5),
        top: dec!(103.25),
        atr: dec!(4.2),
        size_atr: dec!(0.4166666666666666666666666667),
        touch: Some(point("2026-11-19T16:15:00Z", 2)),
        mid: None,
        fill: (!open).then(|| point("2026-11-19T17:00:00Z", 5)),
        expiry: None,
        reactions: vec![
            FvgReaction {
                bars: NonZeroU32::new(4).unwrap(),
                return_atr: Some(dec!(-0.75)),
                at: Some("2026-11-19T17:15:00Z".parse().unwrap()),
            },
            FvgReaction {
                bars: NonZeroU32::new(8).unwrap(),
                return_atr: None,
                at: None,
            },
        ],
        status: if open {
            ZoneStatus::Open
        } else {
            ZoneStatus::Filled
        },
        as_of: "2026-11-19T17:15:00Z".parse().unwrap(),
    }
}

#[test]
fn fvg_event_roundtrips_through_the_contract() {
    for open in [true, false] {
        assert_roundtrip("fvg_event.v1.schema.json", &sample_fvg_event(open));
    }
}

#[test]
fn contract_and_rust_reject_the_same_bad_fvg_events() {
    let schema = "fvg_event.v1.schema.json";
    let doc = || serde_json::to_value(sample_fvg_event(true)).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("timeframe", json!("4h")),
        ("direction", json!("long")),
        ("status", json!("closed")),
        ("top", json!(103.25)),
        ("size_atr", json!("-1")),
        ("touch", json!({ "at": "2026-11-19T16:15:00Z", "bars": 0 })),
        (
            "touch",
            json!({ "at": "2026-11-19T16:15:00+01:00", "bars": 2 }),
        ),
        ("reactions", json!([{ "bars": 4, "return_atr": "1e3" }])),
        ("reactions", json!([{ "bars": 4 }])),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("fill");
    cases.push(d); // tiene que venir, aunque sea null
    for d in cases {
        assert_both_reject::<cfds_core::FvgEvent>(schema, d);
    }
}

fn sample_opening_gap(filled: bool) -> cfds_core::OpeningGap {
    use cfds_core::{Direction, GapSession, ZoneStatus};
    let fill = filled.then(|| GapSession {
        day: "2026-11-17".parse().unwrap(),
        sessions: 1,
        at: Some("2026-11-18T05:00:00Z".parse().unwrap()),
    });
    cfds_core::OpeningGap {
        instrument_id: "US500".parse().unwrap(),
        day: "2026-11-16".parse().unwrap(),
        direction: Direction::Bullish,
        prev_close: dec!(100),
        open: dec!(103),
        atr: dec!(2),
        size_atr: dec!(1.5),
        fill,
        same_session: false,
        expiry: None,
        status: if filled {
            ZoneStatus::Filled
        } else {
            ZoneStatus::Open
        },
        as_of: "2026-11-18T05:00:00Z".parse().unwrap(),
    }
}

#[test]
fn opening_gap_roundtrips_through_the_contract() {
    for filled in [true, false] {
        assert_roundtrip("opening_gap.v1.schema.json", &sample_opening_gap(filled));
    }
}

#[test]
fn contract_and_rust_reject_the_same_bad_opening_gaps() {
    let schema = "opening_gap.v1.schema.json";
    let doc = || serde_json::to_value(sample_opening_gap(true)).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("day", json!("16-11-2026")),
        ("direction", json!("up")),
        ("prev_close", json!(100)),
        ("same_session", json!("no")),
        ("fill", json!({ "day": "2026-11-17", "sessions": -1 })),
        ("fill", json!({ "day": "2026-11-17" })),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("expiry");
    cases.push(d);
    for d in cases {
        assert_both_reject::<cfds_core::OpeningGap>(schema, d);
    }
}

fn sample_liquidity_stats(insufficient: bool) -> cfds_core::LiquidityStats {
    use cfds_core::{
        Direction, DistMetric, DistStat, RateMetric, RateStat, SizeBucket, SizeCuts, StatsSubject,
        StatsTimeframe,
    };
    use std::num::NonZeroU32;
    let some = |d| (!insufficient).then_some(d);
    cfds_core::LiquidityStats {
        instrument_id: "US100".parse().unwrap(),
        subject: StatsSubject::Fvg,
        timeframe: StatsTimeframe::H1,
        direction: Direction::Bullish,
        size_tercile: SizeBucket::Low,
        size_cuts_atr: SizeCuts {
            low_max: dec!(0.3),
            mid_max: dec!(0.7),
        },
        min_cases: NonZeroU32::new(30).unwrap(),
        n: if insufficient { 12 } else { 40 },
        rates: vec![RateStat {
            metric: RateMetric::Touch,
            successes: 7,
            n: if insufficient { 12 } else { 40 },
            insufficient,
            rate: some(dec!(0.175)),
            ci_low: some(dec!(0.0874)),
            ci_high: some(dec!(0.3196)),
        }],
        distributions: vec![
            DistStat {
                metric: DistMetric::BarsToTouch,
                horizon_bars: None,
                n: 7,
                insufficient: true,
                median: None,
                p25: None,
                p75: None,
                iqr: None,
            },
            DistStat {
                metric: DistMetric::Reaction,
                horizon_bars: NonZeroU32::new(4),
                n: 35,
                insufficient: false,
                median: Some(dec!(-0.25)),
                p25: Some(dec!(-1.5)),
                p75: Some(dec!(0.75)),
                iqr: Some(dec!(2.25)),
            },
        ],
        as_of: "2026-11-19T17:00:00Z".parse().unwrap(),
    }
}

#[test]
fn liquidity_stats_roundtrip_through_the_contract() {
    for insufficient in [true, false] {
        assert_roundtrip(
            "liquidity_stats.v1.schema.json",
            &sample_liquidity_stats(insufficient),
        );
    }
}

#[test]
fn contract_and_rust_reject_the_same_bad_liquidity_stats() {
    let schema = "liquidity_stats.v1.schema.json";
    let doc = || serde_json::to_value(sample_liquidity_stats(false)).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("subject", json!("order_block")),
        ("size_tercile", json!("medium")),
        ("min_cases", json!(0)),
        ("n", json!(-1)),
        ("size_cuts_atr", json!({ "low_max": "0.3" })),
        (
            "rates",
            json!([{ "metric": "touch", "successes": 1, "n": 1, "insufficient": false, "rate": 1, "ci_low": null, "ci_high": null }]),
        ),
        (
            "rates",
            json!([{ "metric": "sweep", "successes": 1, "n": 1, "insufficient": false, "rate": null, "ci_low": null, "ci_high": null }]),
        ),
        (
            "distributions",
            json!([{ "metric": "reaction", "horizon_bars": 0, "n": 1, "insufficient": true, "median": null, "p25": null, "p75": null, "iqr": null }]),
        ),
        ("as_of", json!("2026-11-19 17:00:00")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    for d in cases {
        assert_both_reject::<cfds_core::LiquidityStats>(schema, d);
    }
}

fn sample_imbalance(displacement: Option<cfds_core::Direction>) -> cfds_core::Imbalance {
    cfds_core::Imbalance {
        instrument_id: "US100".parse().unwrap(),
        timeframe: cfds_core::StatsTimeframe::M15,
        at: "2026-11-19T17:00:00Z".parse().unwrap(),
        window_bars: std::num::NonZeroU32::new(20).unwrap(),
        displacement,
        bullish_body_sum: dec!(12.5),
        bearish_body_sum: dec!(30),
        range_sum: dec!(70),
        index: dec!(-0.25),
        label: cfds_core::ImbalanceLabel::PriceApproximation,
    }
}

#[test]
fn imbalance_roundtrips_through_the_contract() {
    use cfds_core::Direction;
    for d in [None, Some(Direction::Bullish), Some(Direction::Bearish)] {
        assert_roundtrip("imbalance.v1.schema.json", &sample_imbalance(d));
    }
    let json = serde_json::to_value(sample_imbalance(None)).unwrap();
    assert_eq!(json["label"], json!("aproximación desde el precio"));
}

#[test]
fn contract_and_rust_reject_the_same_bad_imbalances() {
    let schema = "imbalance.v1.schema.json";
    let doc = || serde_json::to_value(sample_imbalance(None)).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("label", json!("order flow")),
        ("displacement", json!("up")),
        ("index", json!(-0.25)),
        ("range_sum", json!("-1")),
        ("window_bars", json!(0)),
        ("timeframe", json!("5m")),
        ("at", json!("2026-11-19T17:00:00")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    for key in ["displacement", "label"] {
        let mut d = doc();
        d.as_object_mut().unwrap().remove(key);
        cases.push(d);
    }
    for d in cases {
        assert_both_reject::<cfds_core::Imbalance>(schema, d);
    }
}

#[test]
fn reaction_and_gap_session_times_are_optional_in_the_contract() {
    let mut d = serde_json::to_value(sample_fvg_event(true)).unwrap();
    d["reactions"][0].as_object_mut().unwrap().remove("at");
    assert!(validator("fvg_event.v1.schema.json").is_valid(&d));
    let e: cfds_core::FvgEvent = serde_json::from_value(d).unwrap();
    assert_eq!(e.reactions[0].at, None);
    let mut g = serde_json::to_value(sample_opening_gap(true)).unwrap();
    g["fill"].as_object_mut().unwrap().remove("at");
    assert!(validator("opening_gap.v1.schema.json").is_valid(&g));
    assert!(serde_json::from_value::<cfds_core::OpeningGap>(g.clone()).is_ok());
    g["fill"]["at"] = json!("2026-11-18 05:00:00");
    assert_both_reject::<cfds_core::OpeningGap>("opening_gap.v1.schema.json", g);
}

fn sample_futures_bar() -> cfds_core::FuturesBar {
    cfds_core::FuturesBar {
        source: "databento".into(),
        dataset: "XEUR.EOBI".into(),
        symbol: "FDAX.v.0".into(),
        instrument_id: "DE40".parse().unwrap(),
        contract_id: 42,
        start: "2026-11-18T14:31:00Z".parse().unwrap(),
        open: dec!(24000.5),
        high: dec!(24010),
        low: dec!(23990.0),
        close: dec!(24005.5),
        volume: 17,
    }
}

#[test]
fn futures_bar_roundtrips_through_the_contract() {
    assert_roundtrip("futures_bar.v1.schema.json", &sample_futures_bar());
    let json = serde_json::to_value(sample_futures_bar()).unwrap();
    assert_eq!(json["start"], json!("2026-11-18T14:31:00Z"));
    assert_eq!(json["low"], json!("23990.0"));
}

#[test]
fn contract_and_rust_reject_the_same_bad_futures_bars() {
    let schema = "futures_bar.v1.schema.json";
    let doc = || serde_json::to_value(sample_futures_bar()).unwrap();
    let mut cases: Vec<Value> = Vec::new();
    for (key, value) in [
        ("start", json!("2026-11-18T14:31:30Z")),
        ("start", json!("2026-11-18T15:31:00+01:00")),
        ("symbol", json!("FDAXZ6")),
        ("symbol", json!("FDAX.x.0")),
        ("dataset", json!("xeur.eobi")),
        ("contract_id", json!(0)),
        ("volume", json!(-1)),
        ("close", json!(24005.5)),
        ("source", json!("Databento")),
        ("instrument_id", json!("de40")),
        ("extra", json!(1)),
    ] {
        let mut d = doc();
        d[key] = value;
        cases.push(d);
    }
    let mut d = doc();
    d.as_object_mut().unwrap().remove("volume");
    cases.push(d);
    for d in cases {
        assert_both_reject::<cfds_core::FuturesBar>(schema, d);
    }
}

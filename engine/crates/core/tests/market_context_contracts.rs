//! `sentiment.v1` y `analog_report.v1` (#294): lo que Rust lee vuelve idéntico y valida contra el
//! schema, y lo que el schema rechaza, Rust también. Números sintéticos.

use std::path::PathBuf;

use cfds_core::{AnalogReport, SentimentIndex};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn roundtrip<T: DeserializeOwned + serde::Serialize>(schema: &str, doc: Value) {
    let errors: Vec<String> = validator(schema)
        .iter_errors(&doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{schema}: {errors:?}");
    let typed: T = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), doc);
}

fn both_reject<T: DeserializeOwned + std::fmt::Debug>(schema: &str, doc: Value) {
    assert!(
        !validator(schema).is_valid(&doc),
        "{schema} debería rechazar {doc}"
    );
    assert!(
        serde_json::from_value::<T>(doc.clone()).is_err(),
        "Rust debería rechazar {doc}"
    );
}

fn sentiment() -> Value {
    json!({
        "instrument_id": "US500.FUT",
        "as_of": "2026-11-20T21:00:00Z",
        "published_at": "2026-11-20T23:00:00Z",
        "value": "-0.35",
        "warmup_complete": true,
        "min_components": 3,
        "components": [
            {
                "name": "cot",
                "status": "valid",
                "weight": "1",
                "observed_for": "2026-11-17T00:00:00Z",
                "published_at": "2026-11-21T20:30:00Z",
                "raw": "-0.12",
                "percentile": "0.3",
                "component": "-0.4"
            },
            {
                "name": "breadth",
                "status": "pending_data",
                "weight": "1",
                "observed_for": null,
                "published_at": null,
                "raw": null,
                "percentile": null,
                "component": null
            }
        ]
    })
}

fn report() -> Value {
    let group = |name: &str, count: u32, insufficient: bool| {
        json!({
            "group": name,
            "count": count,
            "insufficient": insufficient,
            "horizons": [
                {
                    "horizon": 5,
                    "percentiles": [
                        { "p": 10, "return": "-0.031" },
                        { "p": 50, "return": "0.004" },
                        { "p": 90, "return": "0.027" }
                    ],
                    "positive_pct": "55"
                }
            ],
            "hits": { "up": 6, "down": 9, "none": 5 },
            "without_atr": 0
        })
    };
    json!({
        "instrument_id": "US500.FUT",
        "source": "futures",
        "window_end_day": "2026-11-20",
        "as_of": "2026-11-20T22:00:00Z",
        "computed_at": "2026-11-23T12:00:00Z",
        "window": 20,
        "min_neighbors": 20,
        "context": { "vol": "mid", "rates": "estable" },
        "neighbors": [
            { "instrument_id": "US100.FUT", "end_day": "2024-03-01", "distance": "1.234567" }
        ],
        "groups": [group("all", 20, false), group("same_context", 7, true)]
    })
}

#[test]
fn sentiment_roundtrips_and_rejects_the_same() {
    roundtrip::<SentimentIndex>("sentiment.v1.schema.json", sentiment());
    let mut bad = sentiment();
    bad["value"] = json!(-0.35);
    both_reject::<SentimentIndex>("sentiment.v1.schema.json", bad);
    let mut bad = sentiment();
    bad["components"][0]["status"] = json!("ok");
    both_reject::<SentimentIndex>("sentiment.v1.schema.json", bad);
    let mut bad = sentiment();
    bad["extra"] = json!(1);
    both_reject::<SentimentIndex>("sentiment.v1.schema.json", bad);
}

#[test]
fn analog_report_roundtrips_and_rejects_the_same() {
    roundtrip::<AnalogReport>("analog_report.v1.schema.json", report());
    let mut none = report();
    none["context"] = json!({ "vol": null, "rates": null });
    roundtrip::<AnalogReport>("analog_report.v1.schema.json", none);
    for (k, v) in [
        ("source", json!("mezcla")),
        ("prediction", json!("sube")),
        ("window_end_day", json!("20-11-2026")),
        ("as_of", json!("2026-11-20T22:00:00+01:00")),
    ] {
        let mut bad = report();
        bad[k] = v;
        both_reject::<AnalogReport>("analog_report.v1.schema.json", bad);
    }
    let mut bad = report();
    bad["groups"][0]["horizons"][0]["percentiles"][0]["return"] = json!(-0.031);
    both_reject::<AnalogReport>("analog_report.v1.schema.json", bad);
}

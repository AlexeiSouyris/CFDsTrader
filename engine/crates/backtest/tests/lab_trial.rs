//! Laboratorio (#730): `lab_trial.v1`, una variante probada, y el conteo de la familia.

use std::path::PathBuf;

use cfds_backtest::lab::{LabTrial, family_variants, read_trials};
use serde_json::{Value, json};

fn validator() -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/lab_trial.v1.schema.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    jsonschema::validator_for(&doc).expect("schema inválido")
}

fn hash(c: char) -> String {
    c.to_string().repeat(64)
}

fn trial(spec: char, family: char) -> Value {
    json!({
        "spec_sha256": hash(spec),
        "family_sha256": hash(family),
        "instrument_id": "US500",
        "timeframe": "1h",
        "mode": "exploratory",
        "catalog_version": 1,
        "report_id": hash('a'),
        "recorded_at": "2026-11-26T23:00:00Z"
    })
}

fn with(doc: &Value, key: &str, value: Value) -> Value {
    let mut d = doc.clone();
    d[key] = value;
    d
}

#[test]
fn a_trial_roundtrips_and_both_reject_the_same() {
    let t = trial('b', 'c');
    assert!(validator().is_valid(&t));
    let rust: LabTrial = serde_json::from_value(t.clone()).unwrap();
    assert_eq!(serde_json::to_value(&rust).unwrap(), t);
    for (key, value) in [
        ("timeframe", json!("1d")),
        ("recorded_at", json!("2026-11-26T23:00:00+01:00")),
        ("mode", json!("optimize")),
        ("extra", json!(true)),
    ] {
        let bad = with(&t, key, value);
        assert!(!validator().is_valid(&bad), "{bad}");
        assert!(
            serde_json::from_value::<LabTrial>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    // Lo que solo el schema mira (formas de texto) también lo rechaza.
    for (key, value) in [
        ("spec_sha256", json!("B".repeat(64))),
        ("catalog_version", json!(0)),
    ] {
        assert!(!validator().is_valid(&with(&t, key, value)));
    }
}

#[test]
fn the_ledger_is_json_lines_and_counts_each_spec_once() {
    let lines = [
        trial('b', 'c'),
        trial('b', 'c'),
        trial('d', 'c'),
        trial('e', 'f'),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    let trials = read_trials(&format!("{lines}\n\n")).unwrap();
    assert_eq!(trials.len(), 4);
    let c = hash('c');
    // b y d ya probadas; b repetida no suma; g nueva suma una.
    assert_eq!(family_variants(&trials, &c, &[]), 2);
    assert_eq!(family_variants(&trials, &c, &[&hash('b')]), 2);
    assert_eq!(family_variants(&trials, &c, &[&hash('g'), &hash('g')]), 3);
    assert_eq!(family_variants(&[], &c, &[&hash('g')]), 1);
    let err = read_trials(&format!("{lines}\n{{\"spec_sha256\": 1}}\n")).unwrap_err();
    assert!(err.starts_with("línea 5"), "{err}");
}

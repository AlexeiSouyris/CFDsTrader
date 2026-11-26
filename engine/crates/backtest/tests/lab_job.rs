//! Laboratorio (#774): `lab_job_request.v1` y `lab_job_event.v1` contra sus schemas, el hash del
//! pedido (normalizado), el id del trabajo y el chequeo de las variantes.

use std::path::PathBuf;

use cfds_backtest::lab::{
    JobStage, JobState, LabConfig, LabJobEvent, LabJobRequest, StrategyCatalog, StrategySpec,
    ValidSpec, catalog, check_variants, job_id, request_sha256, validate_spec,
};
use cfds_market_data::{DataSplit, Period};
use cfds_risk::RiskConfig;
use rust_decimal::dec;
use serde_json::{Value, json};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn contracts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts")
}

/// Resuelve los `$ref` entre contratos con los archivos del repo, sin red.
struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri.as_str().rsplit('/').next().ok_or("uri sin archivo")?;
        Ok(serde_json::from_str(&std::fs::read_to_string(
            contracts().join(name),
        )?)?)
    }
}

fn validator(name: &str) -> jsonschema::Validator {
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(contracts().join(name)).unwrap()).unwrap();
    jsonschema::options()
        .with_retriever(Local)
        .build(&doc)
        .expect("schema inválido")
}

fn engine_catalog() -> StrategyCatalog {
    let lab = LabConfig::from_toml(cfg!("lab.toml")).unwrap();
    let risk = RiskConfig::from_toml(cfg!("risk.toml")).unwrap();
    catalog(&lab, &risk, ["US500".parse().unwrap()]).unwrap()
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

fn spec(fast: u32, k: &str) -> Value {
    json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": "1h" },
        "direction": "both",
        "entry": [
            { "type": "ma_cross", "params": { "average": "ema", "fast": fast, "slow": 50 } }
        ],
        "filters": [],
        "stop": { "type": "atr", "params": { "period": 14, "k": k } },
        "exit": { "type": "time_target", "params": { "max_candles": 6, "target_r": "2" } },
        "risk_pct": "0.5",
        "cost_multiplier": "1",
        "period": { "from": "2015-01-01T00:00:00Z", "to": "2023-01-01T00:00:00Z" },
        "mode": "exploratory"
    })
}

fn valid(doc: &Value) -> ValidSpec {
    let s: StrategySpec = serde_json::from_value(doc.clone()).unwrap();
    validate_spec(&s, &engine_catalog(), &split()).unwrap()
}

fn event(state: &str, stage: Value, reports: Value, error: Value, percent: u8) -> Value {
    json!({
        "job_id": "a".repeat(64),
        "seq": 7,
        "state": state,
        "stage": stage,
        "percent": percent,
        "variants_done": 1,
        "variants_total": 2,
        "report_ids": reports,
        "error": error,
        "recorded_at": "2026-11-27T01:02:03.456Z"
    })
}

#[test]
fn a_request_roundtrips_and_the_schema_checks_its_shape() {
    let v = validator("lab_job_request.v1.schema.json");
    let doc = json!({ "specs": [spec(20, "1.5"), spec(30, "1.5")], "initial_equity": "100000" });
    let errors: Vec<String> = v.iter_errors(&doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let req: LabJobRequest = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(serde_json::to_value(&req).unwrap(), doc);
    for bad in [
        json!({ "specs": [], "initial_equity": "100000" }),
        json!({ "specs": [spec(20, "1.5")], "initial_equity": "0" }),
        json!({ "specs": [spec(20, "1.5")], "initial_equity": 100000 }),
        json!({ "specs": [spec(20, "1.5")], "initial_equity": "1", "extra": 1 }),
        json!({ "specs": [{ "mode": "exploratory" }], "initial_equity": "1" }),
    ] {
        assert!(!v.is_valid(&bad), "el schema debería rechazar {bad}");
    }
    assert!(serde_json::from_value::<LabJobRequest>(json!({ "specs": [] })).is_err());
}

#[test]
fn events_roundtrip_and_the_schema_ties_fields_to_the_state() {
    let v = validator("lab_job_event.v1.schema.json");
    let h = json!(["b".repeat(64), "c".repeat(64)]);
    let good = [
        event("queued", json!(null), json!([]), json!(null), 0),
        event(
            "running",
            json!("running_variants"),
            json!([]),
            json!(null),
            40,
        ),
        event("done", json!(null), h.clone(), json!(null), 100),
        event(
            "failed",
            json!(null),
            json!([]),
            json!("sin velas de futuros"),
            10,
        ),
        event("cancelled", json!(null), json!([]), json!(null), 0),
    ];
    for doc in &good {
        let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{doc}: {errors:?}");
        let e: LabJobEvent = serde_json::from_value(doc.clone()).unwrap();
        assert_eq!(&serde_json::to_value(&e).unwrap(), doc);
    }
    let bad = [
        event("running", json!(null), json!([]), json!(null), 40),
        event("queued", json!("started"), json!([]), json!(null), 0),
        event("done", json!(null), json!([]), json!(null), 100),
        event("done", json!(null), h.clone(), json!(null), 90),
        event("queued", json!(null), h, json!(null), 0),
        event("failed", json!(null), json!([]), json!(null), 0),
        event("cancelled", json!(null), json!([]), json!("x"), 0),
        event("running", json!("thinking"), json!([]), json!(null), 0),
        event("running", json!("started"), json!([]), json!(null), 101),
    ];
    for doc in &bad {
        assert!(!v.is_valid(doc), "el schema debería rechazar {doc}");
    }
    let e: LabJobEvent = serde_json::from_value(good[1].clone()).unwrap();
    assert_eq!(e.state, JobState::Running);
    assert_eq!(e.stage, Some(JobStage::RunningVariants));
    assert!(!e.state.is_end() && JobState::Cancelled.is_end());
    for s in [JobStage::Started, JobStage::Saving] {
        assert_eq!(JobStage::parse(s.as_str()), Some(s));
    }
}

#[test]
fn the_request_hash_ignores_decimal_spelling_and_the_attempt_changes_the_job() {
    let a = [valid(&spec(20, "1.5")), valid(&spec(30, "1.5"))];
    let b = [valid(&spec(20, "1.50")), valid(&spec(30, "1.500"))];
    let ha = request_sha256(&a, dec!(100000));
    assert_eq!(ha, request_sha256(&b, dec!(100000.00)));
    assert_ne!(ha, request_sha256(&a, dec!(50000)));
    // El orden de las variantes es el de los informes: cuenta.
    assert_ne!(
        ha,
        request_sha256(&[a[1].clone(), a[0].clone()], dec!(100000))
    );
    assert_eq!(ha.len(), 64);
    assert_ne!(job_id(&ha, 1), job_id(&ha, 2));
    assert_eq!(job_id(&ha, 1), job_id(&ha, 1));
    let req = LabJobRequest::normalized(&b, dec!(100000.00));
    assert_eq!(req.initial_equity.to_string(), "100000");
    assert_eq!(req.specs[0], a[0].spec);
}

#[test]
fn variants_are_one_family_one_period_and_capped() {
    let cat = engine_catalog();
    let a = valid(&spec(20, "1.5"));
    let b = valid(&spec(30, "2"));
    assert!(check_variants(&[a.clone(), b.clone()], &cat).is_ok());
    assert!(check_variants(&[], &cat).is_err());
    let mut other_period = spec(30, "2");
    other_period["period"]["from"] = json!("2016-01-01T00:00:00Z");
    let err = check_variants(&[a.clone(), valid(&other_period)], &cat).unwrap_err();
    assert!(err.contains("misma familia y período"), "{err}");
    let mut other_family = spec(30, "2");
    other_family["filters"] = json!([
        { "type": "entry_hours_utc", "params": { "from": "13:30", "to": "20:00" } }
    ]);
    assert!(check_variants(&[a.clone(), valid(&other_family)], &cat).is_err());
    let too_many = vec![a; cat.limits.max_variants_per_run + 1];
    assert!(
        check_variants(&too_many, &cat)
            .unwrap_err()
            .contains("como mucho")
    );
}

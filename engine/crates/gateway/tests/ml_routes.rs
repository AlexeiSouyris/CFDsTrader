//! Ruta de los experimentos de ML (#426), solo lectura. Documentos de ejemplo con la forma de los
//! contratos (`ml_experiment_entry.v1`, que incluye `ml_experiment.v1`), no de mercado.

use std::path::PathBuf;
use std::sync::Mutex;

use cfds_gateway::ml::{HISTORY_LIMIT, MlExperimentSource, is_ml_route, respond_ml};
use serde_json::{Value, json};

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

fn assert_entry(doc: &Value) {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(contracts().join("ml_experiment_entry.v1.schema.json")).unwrap(),
    )
    .unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap();
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
}

fn scores(log_loss: f64, brier: f64, auc: Option<f64>) -> Value {
    json!({ "log_loss": log_loss, "brier": brier, "auc": auc })
}

/// Un experimento en suspenso: ninguna ventana llegó al mínimo.
fn suspended() -> Value {
    json!({
        "experiment_id": "a".repeat(64),
        "created_at": "2026-11-24T03:34:19.970396Z",
        "code_version": "3e8f703-dirty",
        "backtest_run_id": "ml-us500-fut-2011",
        "label_source": "ml_label_runs",
        "config_sha256": "b".repeat(64),
        "features": ["side_long", "score", "instrument_US500"],
        "hyperparameters": {
            "l2_grid": [0.1, 1.0], "inner_tune_fraction": 0.2, "max_iterations": 200,
            "seed": 7, "min_examples": 300, "min_per_class": 30,
            "bootstrap_resamples": 1000, "confidence": 0.95
        },
        "windows": [{
            "train_start": "2011-02-01T00:00:00Z", "validation_start": "2013-02-01T00:00:00Z",
            "validation_end": "2013-05-01T00:00:00Z", "n_train": 0, "n_validation": 0,
            "purged": 0, "embargoed": 0, "skipped": "0 propuestas, el mínimo es 300",
            "l2": null, "metrics": {}
        }],
        "pooled": {},
        "result": { "status": "suspended", "difference": null, "low": null, "high": null, "blocks": null }
    })
}

/// Un experimento en el que M1 no mejora a B1.
fn evaluated() -> Value {
    let mut e = suspended();
    e["experiment_id"] = json!("c".repeat(64));
    e["windows"][0]["skipped"] = Value::Null;
    e["windows"][0]["n_train"] = json!(900);
    e["windows"][0]["n_validation"] = json!(120);
    e["windows"][0]["l2"] = json!(1.0);
    let by_model = json!({
        "B0": scores(0.693, 0.25, None),
        "B1": scores(0.68, 0.243, Some(0.55)),
        "M1": scores(0.684, 0.245, Some(0.56))
    });
    e["windows"][0]["metrics"] = by_model.clone();
    e["pooled"] = by_model;
    e["result"] = json!({
        "status": "b1_stays", "difference": 0.004, "low": -0.002, "high": 0.011, "blocks": 52
    });
    e
}

fn entry(instrument: &str, experiment: Value) -> Value {
    json!({ "instrument_id": instrument, "label_source": "ml_label_runs", "experiment": experiment })
}

/// Fuente de prueba: recuerda con qué se la llamó.
struct Fake {
    entries: Result<Vec<Value>, String>,
    asked: Mutex<Vec<(Option<String>, i64)>>,
}

impl Fake {
    fn new(entries: Result<Vec<Value>, String>) -> Self {
        Self {
            entries,
            asked: Mutex::new(Vec::new()),
        }
    }
}

impl MlExperimentSource for Fake {
    async fn entries(&self, instrument: Option<&str>, limit: i64) -> Result<Vec<Value>, String> {
        self.asked
            .lock()
            .unwrap()
            .push((instrument.map(str::to_owned), limit));
        self.entries.clone()
    }
}

#[test]
fn the_example_entries_follow_the_contracts() {
    assert_entry(&entry("US500.FUT", suspended()));
    assert_entry(&entry("US500.FUT", evaluated()));
    // sin instrumento o con un campo de más no valida
    let mut bad = entry("US500.FUT", suspended());
    bad["extra"] = json!(1);
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(contracts().join("ml_experiment_entry.v1.schema.json")).unwrap(),
    )
    .unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap();
    assert!(!v.is_valid(&bad));
    assert!(!v.is_valid(&json!({ "label_source": "ml_label_runs", "experiment": suspended() })));
}

#[tokio::test]
async fn returns_the_stored_documents_as_they_are() {
    let stored = vec![
        entry("DE40.FUT", evaluated()),
        entry("US500.FUT", suspended()),
    ];
    let src = Fake::new(Ok(stored.clone()));
    let r = respond_ml(&src, "GET", "/api/v1/ml/experiments", None).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.content_type, "application/json");
    let got: Vec<Value> = serde_json::from_str(&r.body).unwrap();
    assert_eq!(got, stored);
    for e in &got {
        assert_entry(e);
    }
    assert_eq!(
        src.asked.lock().unwrap().as_slice(),
        [(None, HISTORY_LIMIT)]
    );
}

#[tokio::test]
async fn filters_by_instrument() {
    let src = Fake::new(Ok(vec![entry("US500.FUT", suspended())]));
    let r = respond_ml(
        &src,
        "GET",
        "/api/v1/ml/experiments",
        Some("instrument=US500.FUT"),
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(
        src.asked.lock().unwrap().as_slice(),
        [(Some("US500.FUT".to_owned()), HISTORY_LIMIT)]
    );
    // sin experimentos: lista vacía, no un error
    let empty = Fake::new(Ok(vec![]));
    let r = respond_ml(
        &empty,
        "GET",
        "/api/v1/ml/experiments",
        Some("instrument=DE40"),
    )
    .await;
    assert_eq!((r.status, r.body.as_str()), (200, "[]"));
}

#[tokio::test]
async fn only_get_and_only_the_instrument_parameter() {
    let src = Fake::new(Ok(vec![]));
    for (method, query, status) in [
        ("POST", None, 405),
        ("DELETE", None, 405),
        ("GET", Some("instrument=us500"), 400),
        ("GET", Some("instrument="), 400),
        ("GET", Some("instrument=US500&instrument=DE40"), 400),
        ("GET", Some("limit=5"), 400),
        ("GET", Some("instrument=US500&x=1"), 400),
        ("GET", Some("instrument=US500;DROP"), 400),
    ] {
        let r = respond_ml(&src, method, "/api/v1/ml/experiments", query).await;
        assert_eq!(r.status, status, "{method} {query:?}");
    }
    // nada de lo rechazado llegó a la base
    assert!(src.asked.lock().unwrap().is_empty());
    assert!(is_ml_route("/api/v1/ml/experiments"));
    assert!(!is_ml_route("/api/v1/ml/experiments/x"));
    assert!(!is_ml_route("/api/v1/ml"));
    let r = respond_ml(&src, "GET", "/api/v1/ml", None).await;
    assert_eq!(r.status, 404);
}

#[tokio::test]
async fn a_database_error_is_a_500_without_details() {
    let src = Fake::new(Err("conexión rechazada: password=secreto".into()));
    let r = respond_ml(&src, "GET", "/api/v1/ml/experiments", None).await;
    assert_eq!(r.status, 500);
    assert!(!r.body.contains("secreto"));
}

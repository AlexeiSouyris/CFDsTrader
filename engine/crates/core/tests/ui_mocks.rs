//! Los datos de ejemplo de la UI (`ui/src/mocks/`) tienen la forma de los contratos: cada mensaje
//! valida contra su JSON Schema de `contracts/` (#231). Así, cuando la vista pase a recibir los
//! mensajes reales del motor, no hay sorpresas.

use std::path::{Path, PathBuf};

use serde_json::Value;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn validator(name: &str) -> Option<jsonschema::Validator> {
    let path = repo().join("contracts").join(name);
    if !path.exists() {
        return None;
    }
    Some(
        jsonschema::validator_for(&read_json(&path))
            .expect("el contrato no es un JSON Schema válido"),
    )
}

fn assert_all_valid(mock: &Value, field: &str, schema: &str) {
    let validator = validator(schema).unwrap_or_else(|| panic!("falta contracts/{schema}"));
    let docs = mock[field]
        .as_array()
        .unwrap_or_else(|| panic!("el ejemplo no trae `{field}`"));
    assert!(!docs.is_empty(), "`{field}` está vacío");
    for (i, doc) in docs.iter().enumerate() {
        let errors: Vec<String> = validator.iter_errors(doc).map(|e| e.to_string()).collect();
        assert!(
            errors.is_empty(),
            "{field}[{i}] no valida contra {schema}: {errors:?}"
        );
    }
}

#[test]
fn ui_liquidity_mock_follows_the_contracts() {
    let mock = read_json(&repo().join("ui/src/mocks/liquidity-us100-15m.json"));
    assert_all_valid(&mock, "candles", "candle.v1.schema.json");
    assert_all_valid(&mock, "levels", "liquidity_level.v1.schema.json");
    assert_all_valid(&mock, "fvgs", "fvg_event.v1.schema.json");
    assert_all_valid(&mock, "stats", "liquidity_stats.v1.schema.json");
    // imbalance.v1 llega con el PR #236: se valida en cuanto el contrato esté en el repo.
    if validator("imbalance.v1.schema.json").is_some() {
        assert_all_valid(&mock, "imbalance", "imbalance.v1.schema.json");
    }
}

#[test]
fn ui_backtest_runs_mock_follows_the_contract() {
    // la pestaña Performance (#286) muestra corridas de ejemplo con la forma de backtest_run.v1
    let mock = read_json(&repo().join("ui/src/mocks/backtest-runs.json"));
    assert_all_valid(&mock, "runs", "backtest_run.v1.schema.json");
}

/// Resuelve los `$ref` entre contratos con los archivos del repo, sin red.
struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri.as_str().rsplit('/').next().ok_or("uri sin archivo")?;
        Ok(read_json(&repo().join("contracts").join(name)))
    }
}

#[test]
fn ui_backtest_run_detail_mock_follows_the_contract() {
    // el detalle de ejemplo de la pestaña Performance (#299), con la forma de
    // backtest_run_detail.v1 (que junta backtest_run, backtest_run_curve, proposal y outcome)
    let mock = read_json(&repo().join("ui/src/mocks/backtest-run-detail.json"));
    let schema = read_json(&repo().join("contracts/backtest_run_detail.v1.schema.json"));
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .expect("el contrato no es un JSON Schema válido");
    let errors: Vec<String> = v
        .iter_errors(&mock["detail"])
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "el detalle de ejemplo no valida: {errors:?}"
    );
    // es el detalle de una de las corridas de ejemplo, idéntica
    let runs = read_json(&repo().join("ui/src/mocks/backtest-runs.json"));
    let id = &mock["detail"]["run"]["run_id"];
    let run = runs["runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| &r["run_id"] == id)
        .expect("el detalle no es de una corrida de ejemplo");
    assert_eq!(run, &mock["detail"]["run"]);
}

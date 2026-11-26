//! Reglas de prueba y validadores de los contratos.
#![allow(dead_code, reason = "cada test usa una parte")]

use std::path::PathBuf;

use cfds_propsim::{PropsimConfig, Rules, RulesDoc};
use serde_json::{Value, json};

pub fn contracts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts")
}

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

pub fn assert_valid(schema: &str, doc: &Value) {
    let text = std::fs::read_to_string(contracts().join(schema)).unwrap();
    let schema_doc: Value = serde_json::from_str(&text).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema_doc)
        .unwrap();
    let errors: Vec<String> = v
        .iter_errors(doc)
        .map(|e| format!("{e} en {}", e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "{schema} rechazó: {errors:?}\n{doc}");
}

pub fn config() -> PropsimConfig {
    PropsimConfig::from_toml(include_str!("../../../../../config/propsim.toml")).unwrap()
}

/// Cuenta de 100 000 USD, una fase con objetivo 10 %, pérdida diaria 5 % del tamaño, caída fija
/// 10 %, sin fondeada, cuota 500. Día en UTC con corte a medianoche.
pub fn rules_json() -> Value {
    json!({
        "id": "prueba", "name": "Reglas de prueba", "source": null,
        "account_size": "100000", "currency": "USD",
        "day": { "timezone": "UTC", "cutoff": "00:00" },
        "phases": [{ "name": "Evaluación", "profit_target_pct": "10", "min_trading_days": null,
            "max_trading_days": null, "consistency_pct": null }],
        "daily_loss": { "limit_pct": "5", "base": "account_size" },
        "max_drawdown": { "limit_pct": "10", "mode": "static", "lock_at_initial": false },
        "funded": null,
        "fees": { "evaluation": "500", "activation": null, "refund_with_first_payout": false }
    })
}

pub fn with(mut doc: Value, pointer: &str, v: Value) -> Value {
    *doc.pointer_mut(pointer)
        .unwrap_or_else(|| panic!("{pointer}")) = v;
    doc
}

pub fn rules(doc: &Value) -> Rules {
    let d: RulesDoc = serde_json::from_value(doc.clone()).unwrap();
    d.validate().unwrap()
}

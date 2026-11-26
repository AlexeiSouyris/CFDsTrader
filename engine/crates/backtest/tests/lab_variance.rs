//! Contrato del simulador de varianza (#801): `variance_sim_request.v1` → `run_variance` →
//! `variance_sim_report.v1`, con los topes y valores por defecto de `[variance]` de
//! `config/lab.toml`.

use std::path::PathBuf;

use cfds_backtest::lab::{LabConfig, VarianceRequest, run_variance};
use rust_decimal::Decimal;
use rust_decimal::dec;
use serde_json::{Value, json};

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    jsonschema::validator_for(&doc).expect("schema inválido")
}

fn assert_valid(schema: &str, doc: &Value) {
    let errors: Vec<String> = validator(schema)
        .iter_errors(doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{schema} rechazó: {errors:?}");
}

const REQUEST: &str = "variance_sim_request.v1.schema.json";
const REPORT: &str = "variance_sim_report.v1.schema.json";

fn lab() -> LabConfig {
    LabConfig::from_toml(include_str!("../../../../config/lab.toml")).unwrap()
}

fn run(req: &Value, report_r: Option<&[Decimal]>) -> Result<Value, String> {
    let parsed: VarianceRequest = serde_json::from_value(req.clone()).map_err(|e| e.to_string())?;
    run_variance(&parsed, &lab().variance, report_r, dec!(1), dec!(0.95)).map_err(|e| e.0)
}

fn model_request() -> Value {
    json!({
        "source": { "kind": "model", "win_rate": "0.55", "avg_win_r": "1.2", "avg_loss_r": "1",
            "observed_trades": 40 },
        "risk_pct": "2", "trades": 200, "paths": 400, "seed": 11
    })
}

#[test]
fn a_model_request_gives_a_valid_report_with_the_fixed_comparison() {
    let req = model_request();
    assert_valid(REQUEST, &req);
    let rep = run(&req, None).unwrap();
    assert_valid(REPORT, &rep);
    assert_eq!(rep["paths"], 400);
    assert_eq!(rep["seed"], 11);
    assert_eq!(rep["ruin_threshold_pct"], "30", "default de config");
    assert_eq!(rep["source"]["observed_trades"], 40);
    assert!(rep["win_rate_draws"].is_object());
    assert!(rep["fixed_win_rate"].is_object());
    assert_eq!(rep["bands"][0]["p50"], "1");
    assert_eq!(
        rep["bands"].as_array().unwrap().last().unwrap()["trade"],
        200
    );
    let notes = rep["notes"].to_string();
    assert!(notes.contains("no es consejo de inversión") || notes.contains("No es consejo"));
    assert!(
        notes.contains("config/risk.toml"),
        "2 % pasa el riesgo del proyecto"
    );
    // Mismo pedido, mismo informe.
    assert_eq!(rep, run(&req, None).unwrap());
}

#[test]
fn without_uncertainty_there_is_no_comparison_and_the_defaults_fill_in() {
    let req = json!({
        "source": { "kind": "model", "win_rate": "0.5", "avg_win_r": "1", "avg_loss_r": "1" },
        "risk_pct": "1", "trades": 10, "paths": 50
    });
    assert_valid(REQUEST, &req);
    let rep = run(&req, None).unwrap();
    assert_valid(REPORT, &rep);
    assert!(rep["win_rate_draws"].is_null() && rep["fixed_win_rate"].is_null());
    assert_eq!(rep["seed"], lab().variance.default_seed);
    assert_eq!(rep["expectancy_r"], "0");
    assert!(rep["notes"].to_string().contains("incertidumbre"));
    assert!(!rep["notes"].to_string().contains("config/risk.toml"));
}

#[test]
fn real_r_values_and_a_saved_report_are_resampled() {
    let req = json!({
        "source": { "kind": "r_values", "r": ["1.5", "-1", "-1", "2", "-0.4"], "block_len": 2 },
        "risk_pct": "0.5", "trades": 100, "paths": 100, "ruin_threshold_pct": "20"
    });
    assert_valid(REQUEST, &req);
    let rep = run(&req, None).unwrap();
    assert_valid(REPORT, &rep);
    assert_eq!(rep["source"]["count"], 5);
    assert_eq!(rep["source"]["block_len"], 2);
    assert_eq!(rep["expectancy_r"], "0.22");
    assert_eq!(rep["ruin_threshold_pct"], "20");

    let id = "a".repeat(64);
    let req = json!({ "source": { "kind": "report", "report_id": id }, "risk_pct": "1",
        "trades": 50, "paths": 60 });
    assert_valid(REQUEST, &req);
    assert!(run(&req, None).unwrap_err().contains("informe"));
    let r = [dec!(1), dec!(-1), dec!(0.5)];
    let rep = run(&req, Some(&r)).unwrap();
    assert_valid(REPORT, &rep);
    assert_eq!(rep["source"]["kind"], "report");
    assert_eq!(rep["source"]["report_id"], id);
    assert_eq!(rep["source"]["block_len"], 1, "default de config");
}

#[test]
fn limits_and_bad_values_are_rejected() {
    let cfg = lab().variance;
    let with = |ptr: &str, v: Value| {
        let mut r = model_request();
        *r.pointer_mut(ptr).unwrap() = v;
        r
    };
    for (req, why) in [
        (with("/paths", json!(cfg.max_paths + 1)), "paths"),
        (with("/trades", json!(cfg.max_trades + 1)), "trades"),
        (with("/trades", json!(0)), "trades"),
        (with("/risk_pct", json!("0")), "risk_pct"),
        (with("/risk_pct", json!("10.5")), "risk_pct"),
        (with("/source/win_rate", json!("1.01")), "win_rate"),
        (with("/source/avg_loss_r", json!("0")), "avg_loss_r"),
    ] {
        let err = run(&req, None).unwrap_err();
        assert!(err.contains(why), "{why}: {err}");
    }
    let empty = json!({ "source": { "kind": "r_values", "r": [] }, "risk_pct": "1",
        "trades": 10 });
    assert!(run(&empty, None).is_err());
    let long_block = json!({ "source": { "kind": "r_values", "r": ["1"], "block_len": 2 },
        "risk_pct": "1", "trades": 10 });
    assert!(run(&long_block, None).unwrap_err().contains("block_len"));
    // Campos que el contrato no tiene o decimales fuera de formato: ni se leen.
    let mut extra = model_request();
    extra["leverage"] = json!("10");
    assert!(run(&extra, None).is_err());
    assert!(run(&with("/risk_pct", json!(1)), None).is_err());
    assert!(run(&with("/risk_pct", json!("1e2")), None).is_err());
}

#[test]
fn the_variance_section_of_the_config_is_validated() {
    let text = include_str!("../../../../config/lab.toml");
    for (from, to) in [
        ("max_paths = 10000", "max_paths = 0"),
        ("default_paths = 10000", "default_paths = 20000"),
        ("max_risk_pct = \"10\"", "max_risk_pct = \"0\""),
        ("drawdown_bin = \"0.05\"", "drawdown_bin = \"2\""),
        ("streak_max_k = 20", "streak_max_k = 0"),
    ] {
        assert!(text.contains(from), "la config real cambió: {from}");
        assert!(
            LabConfig::from_toml(&text.replace(from, to)).is_err(),
            "{to}"
        );
    }
    let without: String = text.split("[variance]").next().unwrap().to_owned();
    assert!(LabConfig::from_toml(&without).is_err());
}

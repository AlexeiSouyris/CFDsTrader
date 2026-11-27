//! Contrato del informe del laboratorio (`lab_report.v1`, #755): ejemplos válidos e inválidos
//! contra su JSON Schema.

use std::path::PathBuf;

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

fn assert_invalid(schema: &str, doc: &Value) {
    assert!(
        !validator(schema).is_valid(doc),
        "{schema} debería rechazar {doc}"
    );
}

fn with(doc: &Value, pointer: &str, value: Value) -> Value {
    let mut d = doc.clone();
    if let Some(v) = d.pointer_mut(pointer) {
        *v = value;
    } else {
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        d.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), value);
    }
    d
}

const REPORT: &str = "lab_report.v1.schema.json";

fn hash(c: char) -> String {
    c.to_string().repeat(64)
}

fn summary() -> Value {
    json!({ "mean": "0.12", "p5": "0.05", "p50": "0.11", "p95": "0.21" })
}

fn quantiles() -> Value {
    json!({ "p25": "0.2", "p50": "0.45", "p75": "0.9", "p90": "1.4" })
}

fn report() -> Value {
    json!({
        "report_id": hash('a'),
        "spec_sha256": hash('b'),
        "family_sha256": hash('c'),
        "spec": { "catalog_version": 1 },
        "mode": "exploratory",
        "votes": false,
        "label": "exploratorio, no vota",
        "instrument_id": "US500.FUT",
        "candle_source": "futures",
        "period": { "from": "2015-01-01T00:00:00Z", "to": "2023-01-01T00:00:00Z" },
        "holdout_start": "2025-11-01T00:00:00Z",
        "reproducibility": {
            "config_sha256": hash('d'),
            "code_version": "b12b0f8",
            "last_candle": "2022-12-30T20:45:00Z",
            "seed": 20261125
        },
        "computed_at": "2026-11-26T23:00:00Z",
        "initial_equity": "100000",
        "cost_multiplier": "1",
        "trades": {
            "count": 120, "long": 70, "short": 50,
            "exits": { "stop": 50, "stop_gap": 2, "target": 30, "planned": 35, "force_close": 3 },
            "rejected": [{ "reason": "DailyLossLimit", "count": 1 }],
            "through_stop": 0
        },
        "expectancy": {
            "mean_r_net": "-0.04",
            "mean_r_net_ci": { "low": "-0.21", "high": "0.13" },
            "mean_r_gross": "0.03",
            "win_rate": "0.41",
            "win_rate_ci": { "low": "0.33", "high": "0.5" },
            "profit_factor": "0.93",
            "confidence": "0.95"
        },
        "r_distribution": {
            "step": "0.5",
            "bins": [
                { "from": "-1.5", "to": "-1", "net": 50, "gross": 48 },
                { "from": "-1", "to": "-0.5", "net": 10, "gross": 12 }
            ]
        },
        "months": [
            {
                "month": "2015-01", "trades": 3, "return": "-0.004", "return_stressed": "-0.005",
                "equity": "0.996", "drawdown": "0.004", "buy_and_hold": "0.01"
            },
            {
                "month": "2015-02", "trades": 0, "return": "0", "return_stressed": "0",
                "equity": "0.996", "drawdown": "0.004", "buy_and_hold": null
            }
        ],
        "sharpe": {
            "monthly_mean": "-0.0004", "sharpe": "-0.05", "sortino": "-0.07",
            "deflated": "0.0001", "project_trials": 361, "family_variants": 3, "trials": 364
        },
        "pbo": {
            "variants": 6, "partitions": 16, "pbo": "0.62", "combinations": 12870,
            "logits": summary(), "oos_loss": "0.7"
        },
        "monte_carlo": {
            "paths": 10000, "block_len": 5, "ruin_threshold": "0.3",
            "max_drawdown": summary(), "longest_losing_streak": summary(),
            "longest_under_water": summary(), "final_return": summary(),
            "risk_of_ruin": "0.02", "observed_max_drawdown": "0.08"
        },
        "walk_forward": [
            { "from": "2015-01-01T00:00:00Z", "to": "2015-04-01T00:00:00Z", "trades": 14, "return": "-0.01" }
        ],
        "halves": { "split_month": "2019-01", "first": "0.001", "second": "-0.002" },
        "cost_stress": { "stress": "1.5", "mean_r_net": "-0.09", "monthly_mean": "-0.001", "sharpe": "-0.12" },
        "luck": {
            "simulations": 1000, "strategy_mean_r": "-0.04",
            "percentile_mean_r": "38.2", "percentile_total": "37.9"
        },
        "buy_and_hold": { "monthly_mean": "0.007", "sharpe": "0.2", "max_drawdown": "0.34", "total_return": "1.1" },
        "excursions": { "mae_r": quantiles(), "mfe_r": quantiles(), "mfe_r_losers": null },
        "edge": {
            "cost_per_trade_r": "0.07", "edge_cost_factor": "2",
            "required_gross_r": "0.14", "measured_gross_r": "0.03"
        },
        "slices": {
            "by_year": [{ "key": "2015", "trades": 14, "mean_r_net": "0.1", "win_rate": "0.5" }],
            "by_hour_utc": [{ "key": "15", "trades": 9, "mean_r_net": "-0.2", "win_rate": "0.33" }],
            "by_side": [{ "key": "long", "trades": 70, "mean_r_net": "0.01", "win_rate": "0.43" }]
        },
        "notes": ["exploratorio, no vota"]
    })
}

#[test]
fn a_full_report_is_valid() {
    assert_valid(REPORT, &report());
}

#[test]
fn a_report_without_trades_uses_nulls_not_invented_numbers() {
    let mut r = report();
    for (pointer, value) in [
        ("/expectancy", Value::Null),
        ("/pbo", Value::Null),
        ("/monte_carlo", Value::Null),
        ("/luck", Value::Null),
        ("/buy_and_hold", Value::Null),
        ("/halves", Value::Null),
        ("/months", json!([])),
        ("/walk_forward", json!([])),
        ("/r_distribution/bins", json!([])),
        ("/sharpe/monthly_mean", Value::Null),
        ("/sharpe/sharpe", Value::Null),
        ("/sharpe/sortino", Value::Null),
        ("/sharpe/deflated", Value::Null),
        ("/cost_stress/mean_r_net", Value::Null),
        ("/excursions/mae_r", Value::Null),
        ("/edge/cost_per_trade_r", Value::Null),
        ("/edge/measured_gross_r", Value::Null),
        ("/reproducibility/code_version", Value::Null),
    ] {
        r = with(&r, pointer, value);
    }
    assert_valid(REPORT, &r);
}

#[test]
fn the_report_rejects_floats_unknown_fields_and_bad_shapes() {
    let r = report();
    for (pointer, value) in [
        ("/votes", json!("no")),
        ("/initial_equity", json!(100000)),
        ("/sharpe/sharpe", json!(-0.05)),
        ("/expectancy/win_rate", json!("-0.1")),
        ("/months/0/month", json!("2015-13")),
        ("/months/0/drawdown", json!("-0.1")),
        ("/pbo/pbo", json!(0.6)),
        ("/report_id", json!("abc")),
        ("/candle_source", json!("ig")),
        ("/mode", json!("optimize")),
        ("/extra", json!(1)),
        ("/trades/exits/extra", json!(1)),
        ("/reproducibility/code_version", json!("not a commit")),
        ("/notes", json!([""])),
    ] {
        assert_invalid(REPORT, &with(&r, pointer, value));
    }
}

//! `prop_sim_request.v1` → `run` → `prop_sim_result.v1` (#802): pedidos y resultados contra
//! sus esquemas, la fuente "informe" y los topes.

mod common;

use cfds_propsim::{PropSimError, PropSimRequest, run};
use chrono::{DateTime, Utc};
use common::{assert_valid, config, rules_json, with};
use rust_decimal::Decimal;
use rust_decimal::dec;
use serde_json::{Value, json};

const REQUEST: &str = "prop_sim_request.v1.schema.json";
const RESULT: &str = "prop_sim_result.v1.schema.json";

fn request(source: Value) -> Value {
    json!({ "rules": rules_json(), "risk_per_trade_pct": "1", "source": source })
}

fn go(req: &Value, report: Option<&[(DateTime<Utc>, Decimal)]>) -> Result<Value, PropSimError> {
    let parsed: PropSimRequest = serde_json::from_value(req.clone()).unwrap();
    run(&parsed, &config(), report)
}

fn trades(rs: &[&str]) -> Value {
    let list: Vec<Value> = rs
        .iter()
        .enumerate()
        .map(|(k, r)| json!({ "closed_at": format!("2026-03-{:02}T15:00:00Z", k + 2), "r": r }))
        .collect();
    json!({ "kind": "trades", "trades": list })
}

#[test]
fn a_trade_list_walks_the_rules_and_matches_the_contract() {
    // 1 % de 100 000 = 1000 por R: +4R, +3R, +3R = 10 000 → pasa el día 3.
    let req = request(trades(&["4", "3", "3", "-1"]));
    assert_valid(REQUEST, &req);
    let res = go(&req, None).unwrap();
    assert_valid(RESULT, &res);
    assert_eq!(res["risk_per_trade_amount"], "1000");
    assert_eq!(res["source"]["trades"], 4);
    assert_eq!(res["source"]["days"], 4);
    assert_eq!(res["observed"]["status"], "passed");
    assert_eq!(res["observed"]["phases"][0]["status"], "passed");
    assert_eq!(res["observed"]["phases"][0]["trading_days"], 3);
    assert_eq!(res["observed"]["phases"][0]["end_day"], "2026-03-04");
    assert_eq!(res["observed"]["phases"][0]["profit"], "10000");
    assert_eq!(res["upper_bound"], false);
    assert_eq!(res["monte_carlo"], Value::Null);
    let notes = res["notes"].to_string();
    assert!(notes.contains("consejo de inversión") && notes.contains("fuera de la app"));
}

#[test]
fn a_failing_list_says_which_rule_and_the_rest_is_not_reached() {
    let rules = with(
        rules_json(),
        "/funded",
        json!({ "payout_min_trading_days": null, "payout_min_profit_pct": null,
            "profit_split_pct": "80", "consistency_pct": null }),
    );
    let req = json!({ "rules": rules, "risk_per_trade_pct": "2.5",
        "source": trades(&["-1", "-1", "-1", "-1"]) });
    assert_valid(REQUEST, &req);
    let res = go(&req, None).unwrap();
    assert_valid(RESULT, &res);
    // 2500 por R: cuatro días de −2500 llegan a 90 000 = el piso fijo.
    assert_eq!(res["observed"]["status"], "failed");
    assert_eq!(res["observed"]["phases"][0]["rule"], "max_drawdown");
    assert_eq!(res["observed"]["funded"]["status"], "not_reached");
}

#[test]
fn intraday_trailing_is_flagged_as_an_upper_bound() {
    let rules = with(
        rules_json(),
        "/max_drawdown/mode",
        json!("trailing_intraday"),
    );
    let req = json!({ "rules": rules, "risk_per_trade_pct": "1", "source": trades(&["1"]) });
    let res = go(&req, None).unwrap();
    assert_valid(RESULT, &res);
    assert_eq!(res["upper_bound"], true);
    assert_eq!(
        res["unsimulable"],
        json!(["max_drawdown.trailing_intraday"])
    );
    assert!(res["notes"].to_string().contains("intradía"));
}

#[test]
fn a_saved_report_and_the_model_are_sources_too() {
    let id = "b".repeat(64);
    let req = request(json!({ "kind": "report", "report_id": id }));
    assert_valid(REQUEST, &req);
    assert!(matches!(go(&req, None), Err(PropSimError::Bad(_))));
    let at: DateTime<Utc> = "2026-02-02T15:00:00Z".parse().unwrap();
    let report = [(at, dec!(2)), (at, dec!(-0.5))];
    let res = go(&req, Some(&report)).unwrap();
    assert_valid(RESULT, &res);
    assert_eq!(res["source"]["kind"], "report");
    assert_eq!(res["source"]["report_id"], id);
    assert_eq!(res["source"]["days"], 1);
    assert_eq!(res["observed"]["phases"][0]["profit"], "1500");

    let req = request(
        json!({ "kind": "model", "win_rate": "0.5", "avg_win_r": "2",
        "avg_loss_r": "1", "observed_trades": null, "trades_per_day": 3 }),
    );
    assert_valid(REQUEST, &req);
    let res = go(&req, None).unwrap();
    assert_valid(RESULT, &res);
    assert_eq!(res["observed"], Value::Null);
    assert_eq!(res["source"]["trades_per_day"], 3);
}

#[test]
fn blank_rules_limits_and_unknown_fields_are_rejected() {
    let blank = serde_json::to_value(&config().templates[0]).unwrap();
    let req = json!({ "rules": blank, "risk_per_trade_pct": "1", "source": trades(&["1"]) });
    assert_valid(REQUEST, &req); // la forma vale; los valores en blanco los rechaza el motor
    match go(&req, None) {
        Err(e @ PropSimError::Rules(_)) => assert!(e.to_json()["rules"].is_array()),
        other => panic!("{other:?}"),
    }
    let too_risky = with(
        request(trades(&["1"])),
        "/risk_per_trade_pct",
        json!("10.5"),
    );
    assert!(matches!(go(&too_risky, None), Err(PropSimError::Bad(_))));
    let empty = request(json!({ "kind": "trades", "trades": [] }));
    assert!(matches!(go(&empty, None), Err(PropSimError::Bad(_))));
    let mut extra = request(trades(&["1"]));
    extra["leverage"] = json!("30");
    assert!(serde_json::from_value::<PropSimRequest>(extra).is_err());
}

//! Monte Carlo de `cfds-propsim` (#803): determinismo, la comparación con el azar, el valor
//! esperado calculado a mano, el tope de intentos y el modelo del simulador de varianza.
#![allow(clippy::float_arithmetic, reason = "probabilidades de los tests")]

mod common;

use cfds_propsim::{PropSimRequest, Tally, economics, run};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use common::{assert_valid, config, rules, rules_json, with};
use rust_decimal::dec;
use serde_json::{Value, json};

const RESULT: &str = "prop_sim_result.v1.schema.json";

fn funded_rules() -> Value {
    let doc = with(
        rules_json(),
        "/funded",
        json!({ "payout_min_trading_days": 2, "payout_min_profit_pct": "1",
            "profit_split_pct": "80", "consistency_pct": null }),
    );
    with(
        doc,
        "/fees",
        json!({ "evaluation": "100", "activation": "50", "refund_with_first_payout": true }),
    )
}

/// Una operación por día, 10 vueltas de los R dados (la media de la lista es la de `rs`).
fn trades(rs: &[&str]) -> Value {
    let t0: DateTime<Utc> = "2026-01-05T15:00:00Z".parse().unwrap();
    let list: Vec<Value> = (0..rs.len() * 10)
        .map(|k| {
            let at = t0 + Duration::days(i64::try_from(k).unwrap());
            json!({ "closed_at": at.to_rfc3339_opts(SecondsFormat::Secs, true),
                "r": rs[k % rs.len()] })
        })
        .collect();
    json!({ "kind": "trades", "trades": list })
}

fn go(rules: Value, risk: &str, source: Value, mc: Value) -> Value {
    let req = json!({ "rules": rules, "risk_per_trade_pct": risk, "source": source,
        "monte_carlo": mc });
    let parsed: PropSimRequest = serde_json::from_value(req).unwrap();
    let res = run(&parsed, &config(), None).unwrap();
    assert_valid(RESULT, &res);
    res
}

fn p(v: &Value) -> f64 {
    v["probability"].as_str().unwrap().parse().unwrap()
}

fn mc() -> Value {
    json!({ "paths": 800, "seed": 5, "horizon_days": 120, "max_attempts": 3 })
}

#[test]
fn the_same_request_gives_the_same_result_and_outcomes_add_up() {
    let a = go(
        funded_rules(),
        "1",
        trades(&["1.5", "-1", "0.5", "-1", "2"]),
        mc(),
    );
    let b = go(
        funded_rules(),
        "1",
        trades(&["1.5", "-1", "0.5", "-1", "2"]),
        mc(),
    );
    assert_eq!(a, b);
    let pa = &a["monte_carlo"]["per_attempt"];
    let total = p(&pa["payout"]) + p(&pa["failed"]) + p(&pa["timeout"]) + p(&pa["unfinished"]);
    assert!((total - 1.0).abs() < 1e-9, "{total}");
    assert!(p(&pa["pass_evaluation"]) >= p(&pa["payout"]));
    assert_eq!(a["monte_carlo"]["phases"].as_array().unwrap().len(), 2);
    assert_eq!(a["monte_carlo"]["block_len_days"], 5);
    assert!(a["observed"].is_object());
}

#[test]
fn a_zero_mean_list_is_no_better_than_chance_and_a_clear_edge_is() {
    // Media 0 exacta: centrarla no cambia nada, así que es idéntica al azar.
    let flat = go(funded_rules(), "1", trades(&["2", "-1", "-1", "0"]), mc());
    let m = &flat["monte_carlo"];
    assert_eq!(m["per_attempt"]["payout"], m["zero_edge"]["payout"]);
    assert_eq!(m["beats_zero_edge"], false);
    let edge = go(funded_rules(), "1", trades(&["2", "1.5", "-1", "1"]), mc());
    let m = &edge["monte_carlo"];
    assert_eq!(m["beats_zero_edge"], true);
    assert!(p(&m["per_attempt"]["payout"]) > p(&m["zero_edge"]["payout"]));
    let ev: f64 = m["economics"]["expected_value"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let ev0: f64 = m["zero_edge"]["expected_value"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(ev > ev0);
}

#[test]
fn expected_value_matches_the_formula_by_hand() {
    let r = rules(&funded_rules());
    let t = Tally {
        paths: 100,
        pass_evaluation: 50,
        payout: 25,
        payouts: vec![dec!(800), dec!(1200)],
        ..Tally::default()
    };
    // q = 0,25 en hasta 2 intentos: P = 1 − 0,75² = 0,4375; intentos = 0,4375 / 0,25 = 1,75;
    // cuotas = 1,75 × (100 + 50 × 0,5) = 218,75; devolución = 100 × 0,4375 = 43,75;
    // valor = 0,4375 × 1000 + 43,75 − 218,75 = 262,5.
    let e = economics(&t, &r, 2);
    assert_eq!(e.success_within, dec!(0.4375));
    assert_eq!(e.expected_attempts, dec!(1.75));
    assert_eq!(e.expected_fees, dec!(218.75));
    assert_eq!(e.expected_payout, Some(dec!(1000)));
    assert_eq!(e.expected_refund, dec!(43.75));
    assert_eq!(e.expected_value, Some(dec!(262.5)));
    // Nadie cobra: se pagan todas las cuotas.
    let none = Tally {
        paths: 100,
        ..Tally::default()
    };
    let e = economics(&none, &r, 3);
    assert_eq!(e.expected_attempts, dec!(3));
    assert_eq!(e.expected_fees, dec!(300));
    assert_eq!(e.expected_value, Some(dec!(-300)));
    // Sin etapa fondeada no hay valor del programa.
    let e = economics(&t, &rules(&rules_json()), 2);
    assert_eq!(e.expected_value, None);
}

#[test]
fn the_model_source_simulates_without_a_real_list() {
    let model = json!({ "kind": "model", "win_rate": "0.5", "avg_win_r": "2",
        "avg_loss_r": "1", "observed_trades": 30, "trades_per_day": 2 });
    let res = go(funded_rules(), "0.5", model, mc());
    assert_eq!(res["observed"], Value::Null);
    let m = &res["monte_carlo"];
    assert_eq!(m["block_len_days"], Value::Null);
    // Con win rate 1 se llega siempre al cobro (+1000 por operación, 2 por día).
    let sure = json!({ "kind": "model", "win_rate": "1", "avg_win_r": "1",
        "avg_loss_r": "1", "trades_per_day": 2 });
    let res = go(funded_rules(), "0.5", sure, mc());
    let m = &res["monte_carlo"];
    assert_eq!(p(&m["per_attempt"]["payout"]), 1.0);
    // El azar del modelo es win rate L / (W + L) = 0,5 con W = L: pierde a veces.
    assert!(p(&m["zero_edge"]["payout"]) < 1.0);
    assert_eq!(m["beats_zero_edge"], true);
}

#[test]
fn unsimulable_rules_still_run_and_limits_are_enforced() {
    let rules = with(
        funded_rules(),
        "/max_drawdown/mode",
        json!("trailing_intraday"),
    );
    let res = go(rules, "1", trades(&["1", "-1", "1"]), mc());
    assert_eq!(res["upper_bound"], true);
    assert!(res["monte_carlo"].is_object());
    for (mc, field) in [
        (json!({ "paths": 10001 }), "paths"),
        (json!({ "horizon_days": 501 }), "horizon_days"),
        (json!({ "max_attempts": 0 }), "max_attempts"),
        (
            json!({ "paths": 10000, "horizon_days": 400 }),
            "horizon_days",
        ),
    ] {
        let req = json!({ "rules": funded_rules(), "risk_per_trade_pct": "1",
            "source": trades(&["1"]), "monte_carlo": mc });
        let parsed: PropSimRequest = serde_json::from_value(req).unwrap();
        let err = format!("{:?}", run(&parsed, &config(), None).unwrap_err());
        assert!(err.contains(field), "{err}");
    }
    let many = json!({ "kind": "model", "win_rate": "0.5", "avg_win_r": "1",
        "avg_loss_r": "1", "trades_per_day": 21 });
    let req = json!({ "rules": funded_rules(), "risk_per_trade_pct": "1", "source": many });
    let parsed: PropSimRequest = serde_json::from_value(req).unwrap();
    assert!(run(&parsed, &config(), None).is_err());
}

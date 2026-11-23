//! Una corrida de backtest dice exactamente lo mismo que `contracts/backtest_run.v1` (#200).

use std::path::PathBuf;

#[path = "common/run_sample.rs"]
mod run_sample;

use cfds_backtest::{Commission, CostConfig, InstrumentCosts};
use cfds_journal::{BacktestRun, RunCosts, SpreadSource};
use run_sample::{at, sample};
use rust_decimal::dec;
use serde_json::{Value, json};

fn validator() -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/backtest_run.v1.schema.json");
    let text = std::fs::read_to_string(&path).unwrap();
    jsonschema::validator_for(&serde_json::from_str(&text).unwrap()).unwrap()
}

#[test]
fn a_run_validates_against_its_contract_and_reads_back_identical() {
    let run = sample("bt-200");
    run.validate().unwrap();
    let json = serde_json::to_value(&run).unwrap();
    let errors: Vec<String> = validator()
        .iter_errors(&json)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{json}\n{errors:?}");
    assert_eq!(
        serde_json::from_value::<BacktestRun>(json.clone()).unwrap(),
        run
    );
    assert_eq!(json["timeframe"], json!("15m"));
    assert_eq!(json["metrics"]["total_return"], json!("-0.0125"));
    assert_eq!(json["metrics"]["max_drawdown"], json!("0.03"));
    assert_eq!(json["metrics"]["sharpe"], json!("-0.42"));
    assert_eq!(json["metrics"]["sortino"], Value::Null);
    assert_eq!(
        json["costs"]["instruments"][0]["commission"],
        json!({"kind": "none"})
    );
    assert_eq!(
        json["news"]["published_from"],
        json!("2025-12-03T00:00:00Z")
    );

    // Sin `spread_source` (corridas anteriores) no se escribe; con él, valida y vuelve igual.
    assert!(json["costs"].get("spread_source").is_none());
    for (source, text) in [
        (SpreadSource::Measured, "measured"),
        (SpreadSource::Config, "config"),
    ] {
        let mut r = run.clone();
        r.costs = r.costs.clone().with_spread_source(source);
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["costs"]["spread_source"], json!(text));
        assert!(validator().is_valid(&j), "{j}");
        assert_eq!(serde_json::from_value::<BacktestRun>(j).unwrap(), r);
    }

    // `kill_switch_at` (#273): ausente si no se activó; si se activó, valida y vuelve igual.
    assert!(json["metrics"].get("kill_switch_at").is_none());
    let mut killed = run.clone();
    killed.metrics = killed
        .metrics
        .clone()
        .with_kill_switch(Some(at("2025-10-23T09:15:00Z")));
    let j = serde_json::to_value(&killed).unwrap();
    assert_eq!(
        j["metrics"]["kill_switch_at"],
        json!("2025-10-23T09:15:00Z")
    );
    assert!(validator().is_valid(&j), "{j}");
    assert_eq!(serde_json::from_value::<BacktestRun>(j).unwrap(), killed);

    // Eventos del kill switch del backtest (#277).
    let mut reset = run.clone();
    reset.metrics = reset.metrics.clone().with_kill_switch_events(
        Some(20),
        vec![
            cfds_journal::RunKillSwitchEvent {
                event: cfds_journal::RunKillSwitchKind::Activated,
                at: at("2025-10-22T09:45:00Z"),
                equity: dec!(10500.5),
            },
            cfds_journal::RunKillSwitchEvent {
                event: cfds_journal::RunKillSwitchKind::Reset,
                at: at("2025-11-19T00:00:00Z"),
                equity: dec!(10480),
            },
        ],
    );
    let j = serde_json::to_value(&reset).unwrap();
    assert_eq!(
        j["metrics"]["kill_switch_events"][1]["event"],
        json!("reset")
    );
    assert_eq!(j["metrics"]["kill_switch_reset_business_days"], json!(20));
    assert!(validator().is_valid(&j), "{j}");
    assert_eq!(serde_json::from_value::<BacktestRun>(j).unwrap(), reset);
    assert!(json["metrics"].get("kill_switch_events").is_none());

    let mut no_news = run.clone();
    no_news.news = None;
    let j = serde_json::to_value(&no_news).unwrap();
    assert!(validator().is_valid(&j));
    assert_eq!(serde_json::from_value::<BacktestRun>(j).unwrap(), no_news);
}

#[test]
fn commissions_of_shares_validate_too() {
    let mut run = sample("bt-acciones");
    for c in [
        Commission::PerShare {
            per_share: dec!(0.02),
            min: dec!(15),
        },
        Commission::Percent {
            pct: dec!(0.1),
            min: dec!(10),
        },
    ] {
        let costs = InstrumentCosts {
            spread: dec!(0),
            tick: dec!(0.01),
            commission: c,
            funding_admin_fee_pct: dec!(2.5),
            funding_day_count: dec!(360),
        };
        run.costs = RunCosts::new(
            &CostConfig {
                slippage_spread_fraction: dec!(0.5),
                slippage_ticks: dec!(1),
            },
            &[("US100".parse().unwrap(), costs)],
        );
        let json = serde_json::to_value(&run).unwrap();
        assert!(validator().is_valid(&json), "{json}");
        assert_eq!(serde_json::from_value::<BacktestRun>(json).unwrap(), run);
    }
}

#[test]
fn the_contract_and_rust_reject_the_same_bad_runs() {
    let base = serde_json::to_value(sample("bt-200")).unwrap();
    // `pointer` es un JSON Pointer (RFC 6901), por ejemplo "/metrics/sharpe".
    let with = |pointer: &str, v: Value| {
        let mut x = base.clone();
        match x.pointer_mut(pointer) {
            Some(node) => *node = v,
            None => x[pointer.trim_start_matches('/')] = v,
        }
        x
    };
    let bad = [
        with("/run_id", json!("bt 200")),
        with("/timeframe", json!("4h")),
        with("/from", json!("2026-11-02T00:00:00+01:00")),
        with("/config_sha256", json!("ABC")),
        with("/initial_equity", json!("-1")),
        with("/initial_equity", json!(10000)),
        with("/instruments", json!([])),
        with("/instruments", json!(["US100", "US100"])),
        with("/metrics/max_drawdown", json!("-0.1")),
        with("/metrics/sharpe", json!(0.5)),
        with("/metrics/trades", json!(-1)),
        with("/costs/instruments/0/commission", json!({"kind": "flat"})),
        with("/news/interpretations", json!(-1)),
        with("/extra", json!(1)),
        with("/costs/spread_source", json!("estimated")),
        with("/metrics/kill_switch_at", json!("2025-10-23 09:15")),
        with("/metrics/kill_switch_reset_business_days", json!(0)),
        with(
            "/metrics/kill_switch_events",
            json!([{"event": "paused", "at": "2025-10-23T09:15:00Z", "equity": "1"}]),
        ),
    ];
    let v = validator();
    for doc in bad {
        assert!(!v.is_valid(&doc), "el contrato aceptó {doc}");
        let rust = serde_json::from_value::<BacktestRun>(doc.clone());
        assert!(
            rust.is_err() || rust.unwrap().validate().is_err(),
            "Rust aceptó {doc}"
        );
    }
}

#[test]
fn rust_also_checks_what_the_schema_cannot() {
    let mut r = sample("bt-200");
    r.finished_at = r.started_at - chrono::TimeDelta::seconds(1);
    assert!(r.validate().is_err());
    let mut r = sample("bt-200");
    r.to = r.from;
    assert!(r.validate().is_err());
    let mut r = sample("bt-200");
    r.instruments = vec!["DE40".parse().unwrap()];
    assert!(r.validate().is_err(), "los costos son de otro instrumento");
}

#[test]
fn the_same_run_ignores_only_when_it_ran() {
    let a = sample("bt-200");
    let mut b = a.clone();
    b.started_at = at("2026-11-24T10:00:00Z");
    b.finished_at = at("2026-11-24T10:01:00Z");
    assert!(a.same_run(&b));
    b.metrics.trades = 4;
    assert!(!a.same_run(&b));
}

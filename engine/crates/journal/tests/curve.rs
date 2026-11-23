//! La curva de una corrida y su detalle (#299): `backtest_run_curve.v1` y
//! `backtest_run_detail.v1`. Sin base; la escritura y la lectura en PostgreSQL están en `pg.rs`.

use std::path::PathBuf;

use cfds_backtest::{BuyAndHold, CloseReason, ExitReason, ExitRecord, TradeRecord};
use cfds_journal::{
    CurveInvalid, CurvePoint, RunBuyAndHold, RunCurve, RunDetail, RunTrade, from_backtest,
};
use cfds_strategy::{Component, Side, TriggerKind};
use chrono::NaiveDate;
use rust_decimal::dec;
use serde_json::Value;

#[path = "common/run_sample.rs"]
mod run_sample;

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
        let text = std::fs::read_to_string(contracts().join(name))?;
        Ok(serde_json::from_str(&text)?)
    }
}

fn validator(name: &str) -> jsonschema::Validator {
    let text = std::fs::read_to_string(contracts().join(name)).unwrap();
    jsonschema::options()
        .with_retriever(Local)
        .build(&serde_json::from_str(&text).unwrap())
        .unwrap()
}

fn assert_valid(name: &str, doc: &Value) {
    let errors: Vec<String> = validator(name)
        .iter_errors(doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "no valida contra {name}: {errors:?}");
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
}

/// Corrida de 10.000: sube a 10.200, cae a 9.996 (−2 % del pico) y cierra en 10.050.
fn curve(run_id: &str) -> RunCurve {
    let bh = BuyAndHold {
        entry_price: dec!(20000),
        exit_price: dec!(20100),
        nights: 3,
        commission: dec!(0),
        funding: dec!(1.5),
        gross_pnl: dec!(100),
        net_pnl: dec!(98.5),
    };
    RunCurve::new(
        run_id,
        dec!(10000),
        &[
            (day(16), dec!(10200)),
            (day(17), dec!(9996)),
            (day(18), dec!(10050)),
        ],
        vec![RunBuyAndHold::from_backtest(
            "US100".parse().unwrap(),
            dec!(1),
            &bh,
            dec!(1),
            dec!(10000),
        )],
    )
    .unwrap()
}

#[test]
fn the_curve_carries_the_engine_drawdown_and_validates() {
    let c = curve("bt-curve");
    assert_eq!(
        c.points.iter().map(|p| p.drawdown).collect::<Vec<_>>(),
        vec![dec!(0), dec!(0.02), dec!(0.0147058823529411764705882353)]
    );
    assert_eq!(c.buy_and_hold[0].total_return, dec!(0.00985));
    let doc = serde_json::to_value(&c).unwrap();
    assert_valid("backtest_run_curve.v1.schema.json", &doc);
    let back: RunCurve = serde_json::from_value(doc).unwrap();
    assert_eq!(back, c);
}

#[test]
fn a_curve_without_buy_and_hold_or_days_is_still_valid() {
    let c = RunCurve::new("bt-empty", dec!(10000), &[], vec![]).unwrap();
    assert_valid(
        "backtest_run_curve.v1.schema.json",
        &serde_json::to_value(&c).unwrap(),
    );
}

#[test]
fn bad_curves_are_rejected_in_rust_and_in_the_contract() {
    let mut c = curve("bt-bad");
    c.points.swap(0, 1);
    assert!(matches!(c.validate(), Err(CurveInvalid::Unordered(_))));

    let mut c = curve("bt-bad");
    c.points[1] = CurvePoint {
        drawdown: dec!(1.5),
        ..c.points[1]
    };
    assert!(matches!(c.validate(), Err(CurveInvalid::Drawdown(_))));
    assert!(matches!(
        RunCurve::new("x", dec!(0), &[], vec![]),
        Err(CurveInvalid::InitialEquity)
    ));

    // el contrato rechaza un drawdown negativo y un campo inventado
    let v = validator("backtest_run_curve.v1.schema.json");
    let mut doc = serde_json::to_value(curve("bt-bad")).unwrap();
    doc["points"][0]["drawdown"] = Value::from("-0.1");
    assert!(!v.is_valid(&doc));
    let mut doc = serde_json::to_value(curve("bt-bad")).unwrap();
    doc["walk_forward"] = Value::Null;
    assert!(!v.is_valid(&doc));
}

fn trade(run_id: &str) -> RunTrade {
    let exit = ExitRecord {
        index: 2,
        time: run_sample::at("2026-11-19T15:15:00Z"),
        mid: dec!(105),
        price: dec!(104.6),
        fraction: dec!(1),
        reason: CloseReason::Exit(ExitReason::Tp1),
    };
    let record = TradeRecord {
        side: Side::Long,
        signal_index: 0,
        signal_at: run_sample::at("2026-11-19T14:45:00Z"),
        score: 6,
        breakdown: vec![
            (Component::Trend, 2),
            (Component::Trigger(TriggerKind::Sweep), 2),
        ],
        triggers: vec![TriggerKind::Sweep],
        reference: dec!(100),
        tp1: dec!(104),
        entry_index: 1,
        entry_time: run_sample::at("2026-11-19T14:45:00Z"),
        entry_mid: dec!(100),
        entry_price: dec!(100.4),
        stop: dec!(98),
        size: dec!(1),
        leverage: dec!(0.2),
        margin: dec!(10),
        exits: vec![exit],
        commission: dec!(0),
        funding: dec!(0),
        mid_pnl: dec!(5),
        gross_pnl: dec!(4.2),
        net_pnl: dec!(4.2),
    };
    let (proposal, outcome) = from_backtest(run_id, &"US100".parse().unwrap(), &record);
    RunTrade {
        proposal,
        outcome: Some(outcome),
    }
}

#[test]
fn the_detail_joins_existing_contracts_and_validates() {
    let run_id = "bt-detail";
    let mut open = trade(run_id);
    open.outcome = None;
    let detail = RunDetail {
        run: run_sample::sample(run_id),
        curve: Some(curve(run_id)),
        trades: vec![trade(run_id), open],
    };
    let doc = serde_json::to_value(&detail).unwrap();
    assert_valid("backtest_run_detail.v1.schema.json", &doc);
    let back: RunDetail = serde_json::from_value(doc).unwrap();
    assert_eq!(back, detail);

    // una corrida anterior a #299: sin curva
    let old = RunDetail {
        curve: None,
        trades: vec![],
        ..detail
    };
    assert_valid(
        "backtest_run_detail.v1.schema.json",
        &serde_json::to_value(&old).unwrap(),
    );
}

//! Etiqueta de ML sin P/L (`contracts/ml_label.v1`, #399) y la propuesta y las variables que se
//! guardan con ella, desde una `cfds_backtest::Proposal`.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_backtest::{ExitReason, Label, Proposal as Counted};
use cfds_core::{DailyVolTercile, InstrumentId};
use cfds_journal::ml_features::{SessionBand, VolTercile};
use cfds_journal::ml_label::{LabelMethod, LabelReason, MlLabel, features, label, proposal};
use cfds_market_data::{ExchangeCalendar, Session};
use cfds_strategy::{Component, Side, TriggerKind};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::{Value, json};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn validator(name: &str) -> jsonschema::Validator {
    let path = root().join("contracts").join(name);
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn fut() -> InstrumentId {
    "US500.FUT".parse().unwrap()
}

fn nyse() -> Session {
    let us500: InstrumentId = "US500".parse().unwrap();
    let cal = ExchangeCalendar::load(
        &root().join("config"),
        HashMap::from([(us500.clone(), "XNYS".to_string())]),
    )
    .unwrap();
    cal.session_for(&us500).unwrap().clone()
}

fn counted(label: Label) -> Counted {
    Counted {
        index: 3,
        // 15:00 UTC del 19-11-2025: NYSE abierta (14:30 a 21:00 UTC), primer tercio.
        signal_at: at("2025-11-19T15:00:00Z"),
        side: Side::Long,
        score: 5,
        breakdown: vec![
            (Component::Trend, 2),
            (Component::Trigger(TriggerKind::CandlePattern), 2),
            (Component::SmaSide, 1),
        ],
        triggers: vec![TriggerKind::CandlePattern],
        reference: dec!(6700.25),
        stop: dec!(6690.25),
        tp1: dec!(6720.25),
        size: dec!(10),
        leverage: dec!(6.70025),
        label,
        resolved_index: 9,
        resolved_at: at("2025-11-19T16:30:00Z"),
    }
}

#[test]
fn labels_validate_against_their_contract_and_read_back_identical() {
    let v = validator("ml_label.v1.schema.json");
    let l = label("ml-us500", &fut(), &counted(Label::Tp1)).unwrap();
    assert_eq!(
        l,
        MlLabel {
            proposal_id: "ml-us500:US500.FUT:2025-11-19T15:00:00Z:long".into(),
            signal_at: at("2025-11-19T15:00:00Z"),
            y: 1,
            exit_reason: LabelReason::Tp1,
            resolved_at: at("2025-11-19T16:30:00Z"),
            method: LabelMethod::MidNoCosts,
        }
    );
    let j = serde_json::to_value(&l).unwrap();
    assert!(v.is_valid(&j), "{j}");
    assert_eq!(j["y"], json!(1));
    assert_eq!(j["exit_reason"], json!("tp1"));
    assert_eq!(j["method"], json!("mid_no_costs"));
    assert_eq!(serde_json::from_value::<MlLabel>(j).unwrap(), l);

    // El contrato no acepta una clase fuera de 0/1 ni otro método.
    let mut bad = serde_json::to_value(&l).unwrap();
    bad["y"] = json!(2);
    assert!(!v.is_valid(&bad));
    let mut bad = serde_json::to_value(&l).unwrap();
    bad["method"] = json!("with_costs");
    assert!(!v.is_valid(&bad));
}

#[test]
fn time_exits_and_forced_closes_are_class_zero_and_unresolved_has_no_label() {
    for (reason, expected) in [
        (ExitReason::Stop, LabelReason::Stop),
        (ExitReason::Time, LabelReason::Time),
        (ExitReason::Forced, LabelReason::Forced),
    ] {
        let l = label("ml-us500", &fut(), &counted(Label::Miss(reason))).unwrap();
        assert_eq!((l.y, l.exit_reason), (0, expected));
    }
    assert_eq!(label("ml-us500", &fut(), &counted(Label::Unresolved)), None);
}

#[test]
fn the_proposal_follows_its_contract_and_shares_the_id() {
    let p = proposal("ml-us500", &fut(), &counted(Label::Tp1));
    let j = serde_json::to_value(&p).unwrap();
    assert!(validator("proposal.v1.schema.json").is_valid(&j), "{j}");
    assert_eq!(j["source"], json!("backtest"));
    assert_eq!(j["instrument_id"], json!("US500.FUT"));
    assert_eq!(j["triggers"], json!(["candle_pattern"]));
    assert_eq!(
        j["breakdown"][1],
        json!({"component": "candle_pattern", "points": 2})
    );
    let l = label("ml-us500", &fut(), &counted(Label::Tp1)).unwrap();
    assert_eq!(p.id, l.proposal_id);
}

#[test]
fn features_need_every_variable_and_use_the_signal_close() {
    let c = counted(Label::Tp1);
    let mut atr = vec![None; 10];
    atr[3] = Some(dec!(4.50));
    let day = |d: &str, t| DailyVolTercile {
        instrument_id: fut(),
        day: d.parse::<NaiveDate>().unwrap(),
        published_at: at(&format!("{d}T23:00:00Z")),
        realized_vol_window: 20,
        history_years: 3,
        tercile: Some(t),
    };
    // El día 19 se publica después de la señal: cuenta el 18.
    let daily = vec![
        day("2025-11-18", VolTercile::Low),
        day("2025-11-19", VolTercile::High),
    ];
    let f = features("ml-us500", &fut(), &c, &atr, &daily, &nyse()).unwrap();
    assert_eq!(
        f.proposal_id,
        "ml-us500:US500.FUT:2025-11-19T15:00:00Z:long"
    );
    assert_eq!(f.as_of, c.signal_at);
    assert_eq!(f.atr14_m15, dec!(4.5));
    assert_eq!(f.realized_vol_tercile, VolTercile::Low);
    assert_eq!(f.session_band, SessionBand::Open);
    // Sin ATR en la vela de la señal, o sin tercio publicado, no hay fila.
    let mut no_atr = atr.clone();
    no_atr[3] = None;
    assert!(features("ml-us500", &fut(), &c, &no_atr, &daily, &nyse()).is_none());
    assert!(features("ml-us500", &fut(), &c, &atr, &daily[1..], &nyse()).is_none());
    let zero = vec![Some(Decimal::ZERO); 10];
    assert!(features("ml-us500", &fut(), &c, &zero, &daily, &nyse()).is_none());
}

#[test]
fn the_label_run_follows_its_contract() {
    use cfds_journal::ml_label::{LabelCandles, MlLabelRun};
    let r = MlLabelRun {
        run_id: "ml-us500-fut-2011".into(),
        instrument_id: fut(),
        candle_source: LabelCandles::Futures,
        from: at("2011-01-02T23:00:00Z"),
        to: at("2026-11-23T00:00:00Z"),
        method: LabelMethod::MidNoCosts,
        config_sha256: cfds_journal::config_sha256(&[("strategy.toml", "x = 1")]),
    };
    let v = validator("ml_label_run.v1.schema.json");
    let j = serde_json::to_value(&r).unwrap();
    assert!(v.is_valid(&j), "{j}");
    assert_eq!(j["candle_source"], json!("futures"));
    assert_eq!(j["from"], json!("2011-01-02T23:00:00Z"));
    assert_eq!(serde_json::from_value::<MlLabelRun>(j).unwrap(), r);
}

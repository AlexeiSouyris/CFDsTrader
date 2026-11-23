//! El diario dice exactamente lo mismo que sus contratos, y una operación del backtest se
//! convierte en su propuesta y su resultado campo por campo (#60).

use std::path::PathBuf;

use cfds_backtest::{CloseReason, ExitReason, ExitRecord, TradeRecord};
use cfds_core::InstrumentId;
use cfds_journal::{
    BreakdownItem, Direction, Exit, Outcome, OutcomeKind, Proposal, Reason, Signal, Source,
    from_backtest, proposal_id,
};
use cfds_strategy::{Component, Side, TriggerKind};
use chrono::{DateTime, Utc};
use rust_decimal::dec;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    jsonschema::validator_for(&serde_json::from_str(&text).unwrap()).unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn id() -> InstrumentId {
    "US100".parse().unwrap()
}

/// Largo: TP1 de la mitad y el resto al breakeven (el escenario a mano de #55).
fn record() -> TradeRecord {
    let exit = |index, time: &str, price, reason| ExitRecord {
        index,
        time: at(time),
        mid: price,
        price,
        fraction: dec!(0.5),
        reason: CloseReason::Exit(reason),
    };
    TradeRecord {
        side: Side::Long,
        signal_index: 0,
        signal_at: at("2026-11-19T14:45:00Z"),
        score: 6,
        breakdown: vec![
            (Component::Trend, 2),
            (Component::SmaSide, 1),
            (Component::Trigger(TriggerKind::Sweep), 2),
            (Component::RecentChoch, 1),
        ],
        triggers: vec![TriggerKind::Sweep],
        reference: dec!(100),
        tp1: dec!(104),
        entry_index: 1,
        entry_time: at("2026-11-19T14:45:00Z"),
        entry_mid: dec!(100),
        entry_price: dec!(100.7),
        stop: dec!(98),
        size: dec!(2),
        leverage: dec!(0.2),
        margin: dec!(10),
        exits: vec![
            exit(2, "2026-11-19T15:15:00Z", dec!(105.4), ExitReason::Tp1),
            exit(
                3,
                "2026-11-19T15:30:00Z",
                dec!(100.0),
                ExitReason::BreakEven,
            ),
        ],
        commission: dec!(0),
        funding: dec!(0.02),
        mid_pnl: dec!(6.8),
        gross_pnl: dec!(4),
        net_pnl: dec!(3.98),
    }
}

#[test]
fn a_backtest_trade_becomes_its_proposal_and_outcome() {
    let (p, o) = from_backtest("bt-2026-11-20", &id(), &record());
    let pid = "bt-2026-11-20:US100:2026-11-19T14:45:00Z:long";
    assert_eq!(
        p,
        Proposal {
            id: pid.into(),
            source: Source::Backtest,
            run_id: "bt-2026-11-20".into(),
            instrument_id: id(),
            side: Direction::Long,
            signal_at: at("2026-11-19T14:45:00Z"),
            reference: dec!(100),
            stop: dec!(98),
            tp1: dec!(104),
            size: dec!(2),
            leverage: dec!(0.2),
            score: 6,
            breakdown: vec![
                BreakdownItem {
                    component: Signal::Trend,
                    points: 2
                },
                BreakdownItem {
                    component: Signal::SmaSide,
                    points: 1
                },
                BreakdownItem {
                    component: Signal::Sweep,
                    points: 2
                },
                BreakdownItem {
                    component: Signal::RecentChoch,
                    points: 1
                },
            ],
            triggers: vec![Signal::Sweep],
            shown: None,
        }
    );
    assert_eq!(
        o,
        Outcome {
            proposal_id: pid.into(),
            kind: OutcomeKind::Simulated,
            entry_at: at("2026-11-19T14:45:00Z"),
            entry_price: dec!(100.7),
            exits: vec![
                Exit {
                    at: at("2026-11-19T15:15:00Z"),
                    price: dec!(105.4),
                    fraction: dec!(0.5),
                    reason: Reason::Tp1
                },
                Exit {
                    at: at("2026-11-19T15:30:00Z"),
                    price: dec!(100.0),
                    fraction: dec!(0.5),
                    reason: Reason::BreakEven
                },
            ],
            commission: dec!(0),
            funding: dec!(0.02),
            gross_pnl: dec!(4),
            net_pnl: dec!(3.98),
            hit_tp1: true,
            hit_stop: false,
            exit_reason: Reason::BreakEven,
        }
    );
}

#[test]
fn stopped_out_and_end_of_data_outcomes() {
    let mut r = record();
    r.exits.truncate(1);
    r.exits[0].reason = CloseReason::Exit(ExitReason::Stop);
    r.exits[0].fraction = dec!(1);
    let (_, o) = from_backtest("bt", &id(), &r);
    assert_eq!(
        (o.hit_tp1, o.hit_stop, o.exit_reason),
        (false, true, Reason::Stop)
    );
    r.exits[0].reason = CloseReason::EndOfData;
    let (_, o) = from_backtest("bt", &id(), &r);
    assert_eq!((o.hit_stop, o.exit_reason), (false, Reason::EndOfData));
}

#[test]
fn the_id_is_idempotent_and_depends_on_every_key() {
    let t = at("2026-11-19T14:45:00Z");
    let base = proposal_id("bt", &id(), t, Side::Long);
    assert_eq!(base, proposal_id("bt", &id(), t, Side::Long));
    assert_ne!(base, proposal_id("bt2", &id(), t, Side::Long));
    assert_ne!(
        base,
        proposal_id("bt", &"DE40".parse().unwrap(), t, Side::Long)
    );
    assert_ne!(
        base,
        proposal_id("bt", &id(), at("2026-11-19T15:00:00Z"), Side::Long)
    );
    assert_ne!(base, proposal_id("bt", &id(), t, Side::Short));
}

fn roundtrip<T: serde::Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    v: &T,
    schema: &str,
) -> Value {
    let json = serde_json::to_value(v).unwrap();
    let errors: Vec<String> = validator(schema)
        .iter_errors(&json)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{json}\n{errors:?}");
    assert_eq!(&serde_json::from_value::<T>(json.clone()).unwrap(), v);
    json
}

#[test]
fn proposals_and_outcomes_validate_against_their_contracts() {
    let (p, o) = from_backtest("bt-2026-11-20", &id(), &record());
    let pj = roundtrip(&p, "proposal.v1.schema.json");
    assert_eq!(pj["reference"], json!("100"));
    assert_eq!(
        pj["breakdown"][2],
        json!({"component": "sweep", "points": 2})
    );
    let oj = roundtrip(&o, "outcome.v1.schema.json");
    assert_eq!(oj["exits"][1]["reason"], json!("break_even"));
    assert_eq!(oj["funding"], json!("0.02"));
}

#[test]
fn losses_and_shorts_that_earn_funding_are_signed() {
    let mut r = record();
    r.side = Side::Short;
    r.funding = dec!(-0.35);
    r.gross_pnl = dec!(-12.5);
    r.net_pnl = dec!(-12.15);
    let (p, o) = from_backtest("bt", &id(), &r);
    assert_eq!(p.side, Direction::Short);
    assert!(p.id.ends_with(":short"));
    let oj = roundtrip(&o, "outcome.v1.schema.json");
    assert_eq!(oj["net_pnl"], json!("-12.15"));
    assert_eq!(oj["funding"], json!("-0.35"));
}

#[test]
fn the_contracts_and_rust_reject_the_same_bad_entries() {
    let (p, o) = from_backtest("bt", &id(), &record());
    let pj = serde_json::to_value(&p).unwrap();
    let oj = serde_json::to_value(&o).unwrap();
    let with = |base: &Value, key: &str, v: Value| {
        let mut x = base.clone();
        x[key] = v;
        x
    };
    let bad_proposals = [
        with(&pj, "size", json!("-2")),
        with(&pj, "size", json!(2)),
        with(&pj, "source", json!("demo")),
        with(&pj, "signal_at", json!("2026-11-19T14:45:00+01:00")),
        with(&pj, "breakdown", json!([{"component": "rsi", "points": 1}])),
        with(&pj, "extra", json!(1)),
    ];
    let pv = validator("proposal.v1.schema.json");
    for bad in bad_proposals {
        assert!(!pv.is_valid(&bad), "el contrato aceptó {bad}");
        assert!(
            serde_json::from_value::<Proposal>(bad.clone()).is_err(),
            "Rust aceptó {bad}"
        );
    }
    let bad_outcomes = [
        with(&oj, "net_pnl", json!("--1")),
        with(&oj, "net_pnl", json!("+1")),
        with(&oj, "commission", json!("-1")),
        with(&oj, "kind", json!("paper")),
        with(&oj, "exit_reason", json!("margin_call")),
    ];
    let ov = validator("outcome.v1.schema.json");
    for bad in bad_outcomes {
        assert!(!ov.is_valid(&bad), "el contrato aceptó {bad}");
        assert!(
            serde_json::from_value::<Outcome>(bad.clone()).is_err(),
            "Rust aceptó {bad}"
        );
    }
}

#[test]
fn every_signal_and_reason_name_is_in_the_contract() {
    let comps = [
        Component::Trend,
        Component::SmaSide,
        Component::Trigger(TriggerKind::CandlePattern),
        Component::Trigger(TriggerKind::DoubleBreak),
        Component::Trigger(TriggerKind::LevelRejection),
        Component::Trigger(TriggerKind::Sweep),
        Component::FvgOrOrderBlock,
        Component::RecentChoch,
        Component::News,
    ];
    let mut r = record();
    r.breakdown = comps.iter().map(|&c| (c, 1)).collect();
    r.triggers = vec![
        TriggerKind::CandlePattern,
        TriggerKind::DoubleBreak,
        TriggerKind::LevelRejection,
        TriggerKind::Sweep,
    ];
    let reasons = [
        CloseReason::Exit(ExitReason::Stop),
        CloseReason::Exit(ExitReason::Tp1),
        CloseReason::Exit(ExitReason::BreakEven),
        CloseReason::Exit(ExitReason::Trailing),
        CloseReason::Exit(ExitReason::Time),
        CloseReason::Exit(ExitReason::Forced),
        CloseReason::EndOfData,
    ];
    r.exits = reasons
        .iter()
        .map(|&reason| ExitRecord {
            reason,
            ..r.exits[0].clone()
        })
        .collect();
    let (p, o) = from_backtest("bt", &id(), &r);
    roundtrip(&p, "proposal.v1.schema.json");
    roundtrip(&o, "outcome.v1.schema.json");
    assert_eq!(
        o.exits.iter().map(|e| e.reason).collect::<Vec<_>>(),
        vec![
            Reason::Stop,
            Reason::Tp1,
            Reason::BreakEven,
            Reason::Trailing,
            Reason::Time,
            Reason::Forced,
            Reason::EndOfData
        ]
    );
    // Nombres distintos para cada señal (no se pisan en el informe de aporte por señal).
    let names: std::collections::HashSet<String> = p
        .breakdown
        .iter()
        .map(|b| serde_json::to_string(&b.component).unwrap())
        .collect();
    assert_eq!(names.len(), comps.len());
}

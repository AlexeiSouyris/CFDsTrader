//! Los mensajes del broker (fase 7, #278) y sus contratos dicen lo mismo: lo que Rust serializa
//! valida contra el JSON Schema y vuelve idéntico, y lo que el schema rechaza, Rust también.

use std::path::PathBuf;

use cfds_core::{
    AccountCurrency, KillSwitch, Order, OrderRejection, OrderReport, OrderSide, OrderStatus,
    PaperAccount, PaperMode, Position, TicketVerdict,
};
use rust_decimal::dec;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn schema_doc(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn validator(name: &str) -> jsonschema::Validator {
    jsonschema::validator_for(&schema_doc(name)).expect("schema inválido")
}

fn assert_roundtrip<T>(schema: &str, value: &T)
where
    T: serde::Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_value(value).unwrap();
    let errors: Vec<String> = validator(schema)
        .iter_errors(&json)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{schema} rechazó {json}: {errors:?}");
    assert_eq!(&serde_json::from_value::<T>(json).unwrap(), value);
}

fn assert_both_reject<T: DeserializeOwned + std::fmt::Debug>(schema: &str, doc: Value) {
    assert!(
        !validator(schema).is_valid(&doc),
        "{schema} debería rechazar {doc}"
    );
    assert!(
        serde_json::from_value::<T>(doc.clone()).is_err(),
        "Rust debería rechazar {doc}"
    );
}

fn with(doc: &Value, key: &str, value: Value) -> Value {
    let mut d = doc.clone();
    d[key] = value;
    d
}

fn order() -> Order {
    Order {
        order_id: "paper-demo:US100:2026-11-20T15:00:00Z:long"
            .parse()
            .unwrap(),
        mode: PaperMode::Paper,
        instrument_id: "US100".parse().unwrap(),
        side: OrderSide::Long,
        stop: dec!(100.5),
        tp1: Some(dec!(110)),
        proposal_id: None,
        created_at: "2026-11-20T15:00:00Z".parse().unwrap(),
    }
}

#[test]
fn order_roundtrips_and_has_no_size_nor_real_mode() {
    let s = "order.v1.schema.json";
    assert_roundtrip(s, &order());
    assert_roundtrip(
        s,
        &Order {
            tp1: None,
            proposal_id: Some("p:1".parse().unwrap()),
            ..order()
        },
    );
    let doc = serde_json::to_value(order()).unwrap();
    for (k, v) in [
        ("mode", json!("live")),
        ("stop", json!(null)),
        ("stop", json!("-1")),
        ("side", json!("buy")),
        ("order_id", json!("con espacio")),
        ("order_id", json!("")),
        ("created_at", json!("2026-11-20T16:00:00+01:00")),
        ("size", json!("1")),
    ] {
        assert_both_reject::<Order>(s, with(&doc, k, v));
    }
    let mut no_stop = doc.clone();
    no_stop.as_object_mut().unwrap().remove("stop");
    assert_both_reject::<Order>(s, no_stop);
}

fn filled() -> OrderReport {
    OrderReport {
        order_id: order().order_id,
        status: OrderStatus::Filled,
        rejection: None,
        position_id: Some("pos:1".parse().unwrap()),
        size: Some(dec!(1.9)),
        fill_price: Some(dec!(105.75)),
        commission: Some(dec!(0)),
        at: "2026-11-20T15:15:00Z".parse().unwrap(),
    }
}

#[test]
fn order_report_roundtrips_filled_and_rejected() {
    let s = "order_report.v1.schema.json";
    assert_roundtrip(s, &filled());
    for r in OrderRejection::ALL {
        let rejected = OrderReport {
            status: OrderStatus::Rejected,
            rejection: Some(r),
            position_id: None,
            size: None,
            fill_price: None,
            commission: None,
            ..filled()
        };
        assert_roundtrip(s, &rejected);
    }
    let doc = serde_json::to_value(filled()).unwrap();
    assert_both_reject::<OrderReport>(s, with(&doc, "status", json!("partial")));
    assert_both_reject::<OrderReport>(s, with(&doc, "rejection", json!("cualquiera")));
    // Reglas que solo pone el schema (el broker nunca arma estos documentos).
    let v = validator(s);
    assert!(!v.is_valid(&with(&doc, "fill_price", json!(null))));
    assert!(!v.is_valid(&with(&doc, "status", json!("rejected"))));
}

fn verdict() -> TicketVerdict {
    TicketVerdict {
        order_id: order().order_id,
        accepted: true,
        rejection: None,
        reference: Some(dec!(105)),
        size: Some(dec!(1.9)),
        risk: Some(dec!(10.5)),
        risk_pct: Some(dec!(1)),
        leverage: Some(dec!(0.2)),
        margin: Some(dec!(10)),
        leverage_capped: false,
        as_of: "2026-11-20T15:00:00Z".parse().unwrap(),
    }
}

#[test]
fn ticket_verdict_roundtrips_and_shares_the_rejection_codes() {
    let s = "ticket_verdict.v1.schema.json";
    assert_roundtrip(s, &verdict());
    let rejected = TicketVerdict {
        accepted: false,
        rejection: Some(OrderRejection::StopTooClose),
        size: None,
        risk: None,
        risk_pct: None,
        leverage: None,
        margin: None,
        ..verdict()
    };
    assert_roundtrip(s, &rejected);
    let doc = serde_json::to_value(verdict()).unwrap();
    assert_both_reject::<TicketVerdict>(s, with(&doc, "risk", json!(10.5)));
    assert!(!validator(s).is_valid(&with(&doc, "rejection", json!("no_price"))));
    // Una sola lista de motivos: la de order_report.v1 y la de ticket_verdict.v1 son iguales,
    // y coinciden con OrderRejection::ALL.
    let a = &schema_doc("order_report.v1.schema.json")["$defs"]["rejection"]["enum"];
    let b = &schema_doc(s)["$defs"]["rejection"]["enum"];
    assert_eq!(a, b);
    assert_eq!(a, &serde_json::to_value(OrderRejection::ALL).unwrap());
}

fn position() -> Position {
    Position {
        position_id: "pos:1".parse().unwrap(),
        order_id: order().order_id,
        instrument_id: "US100".parse().unwrap(),
        side: OrderSide::Short,
        size: dec!(1.9),
        entry: dec!(105.75),
        stop: dec!(110),
        tp1: None,
        opened_at: "2026-11-20T15:15:00Z".parse().unwrap(),
        last: dec!(107),
        unrealized_pl: dec!(-2.375),
        unrealized_r: dec!(-0.29),
        margin: dec!(10.05),
        as_of: "2026-11-20T16:00:00Z".parse().unwrap(),
    }
}

#[test]
fn position_roundtrips_with_signed_pl() {
    let s = "position.v1.schema.json";
    assert_roundtrip(s, &position());
    let doc = serde_json::to_value(position()).unwrap();
    assert_eq!(doc["unrealized_pl"], json!("-2.375"));
    assert_both_reject::<Position>(s, with(&doc, "size", json!("-1")));
    assert_both_reject::<Position>(s, with(&doc, "unrealized_pl", json!(-2.375)));
}

fn account(kill_switch: KillSwitch) -> PaperAccount {
    PaperAccount {
        mode: PaperMode::Paper,
        currency: AccountCurrency::Eur,
        as_of: "2026-11-20T16:00:00Z".parse().unwrap(),
        equity: dec!(1000.5),
        unrealized_pl: dec!(-2.375),
        realized_pl_today: dec!(3),
        margin_used: dec!(10.05),
        margin_free: dec!(990.45),
        leverage: dec!(0.2),
        daily_loss_used_pct: dec!(0),
        open_positions: 1,
        max_positions: 5,
        kill_switch,
    }
}

#[test]
fn paper_account_roundtrips_with_and_without_kill_switch() {
    let s = "paper_account.v1.schema.json";
    assert_roundtrip(s, &account(KillSwitch::Inactive));
    let on = KillSwitch::Active {
        reason: "Pérdida máxima del día".into(),
        since: "2026-11-20T15:30:00Z".parse().unwrap(),
    };
    assert_roundtrip(s, &account(on));
    let doc = serde_json::to_value(account(KillSwitch::Inactive)).unwrap();
    for (k, v) in [
        ("mode", json!("live")),
        ("currency", json!("USD")),
        ("kill_switch", json!({ "active": true })),
        ("kill_switch", json!({ "active": false, "reason": "x" })),
        (
            "kill_switch",
            json!({ "active": true, "reason": "", "since": "2026-11-20T15:30:00Z" }),
        ),
        ("open_positions", json!(-1)),
    ] {
        assert_both_reject::<PaperAccount>(s, with(&doc, k, v));
    }
}

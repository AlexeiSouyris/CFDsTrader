//! Operación segura del gateway (fase 9, #297): la compuerta de arranque, los logs de los
//! eventos del broker sin secretos y las alertas que salen de ellos. Los secretos son falsos.

use std::io::Write;
use std::sync::{Arc, Mutex};

use cfds_broker::{BrokerEvent, Mismatch, Reconciliation};
use cfds_core::{OrderRejection, OrderReport, OrderSide, OrderStatus};
use cfds_gateway::ops::{
    GateState, alert_events, gate_after_reconcile, log_events, report_failure, url_password,
};
use cfds_obs::{AlertConfig, AlertKind, Alerter, Logger, ReqwestPost};
use chrono::{DateTime, Utc};
use serde_json::Value;

const ALERTS: &str = include_str!("../../../../config/alerts.toml");
const FAKE_DB: &str = "postgres://cfds:fake-db-password@127.0.0.1:5432/cfdstrader";
const FAKE_TOKEN: &str = "fake-gateway-token-0123456789";

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn lines(&self) -> Vec<Value> {
        String::from_utf8(self.0.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

fn at() -> DateTime<Utc> {
    "2026-11-23T10:00:00Z".parse().unwrap()
}

/// Un logger con los secretos registrados como en `main`.
fn logger() -> (Arc<Logger>, Captured) {
    let out = Captured::default();
    let log = Arc::new(Logger::new(Box::new(out.clone())));
    log.register_secret(FAKE_DB);
    log.register_secret(url_password(FAKE_DB).unwrap());
    log.register_secret(FAKE_TOKEN);
    (log, out)
}

fn alerter(log: Arc<Logger>) -> Alerter<ReqwestPost> {
    Alerter::new(AlertConfig::from_toml(ALERTS).unwrap(), log, None)
}

fn report(id: &str, status: OrderStatus) -> OrderReport {
    OrderReport {
        order_id: id.parse().unwrap(),
        status,
        rejection: (status == OrderStatus::Rejected).then_some(OrderRejection::StopTooClose),
        position_id: None,
        size: None,
        fill_price: None,
        commission: None,
        at: at(),
    }
}

#[test]
fn orders_open_only_after_a_clean_start_reconciliation() {
    assert_eq!(
        gate_after_reconcile(&Reconciliation::default()),
        GateState::Ready
    );
    let dirty = Reconciliation {
        mismatches: vec![Mismatch::UnknownInBroker("p1".parse().unwrap())],
        ..Default::default()
    };
    assert_eq!(gate_after_reconcile(&dirty), GateState::Starting);
}

#[test]
fn the_database_password_is_found_to_be_hidden() {
    assert_eq!(url_password(FAKE_DB), Some("fake-db-password"));
    assert_eq!(url_password("postgres://cfds@127.0.0.1/db"), None);
    assert_eq!(url_password("postgres://127.0.0.1/db"), None);
    assert_eq!(url_password("sin url"), None);
}

#[tokio::test]
async fn broker_events_are_json_lines_without_secrets() {
    let (log, out) = logger();
    let events = vec![
        BrokerEvent::OrderReceived {
            order_id: "o1".parse().unwrap(),
            instrument_id: "US100".parse().unwrap(),
            side: OrderSide::Long,
            created_at: at(),
        },
        BrokerEvent::RiskDecision {
            order_id: "o1".parse().unwrap(),
            accepted: false,
            rejection: Some(OrderRejection::StopTooClose),
            size: None,
            as_of: at(),
        },
        BrokerEvent::OrderReport {
            report: report("o1", OrderStatus::Rejected),
        },
        // Un motivo que, por error, trae la URL de la base y el token.
        BrokerEvent::KillSwitch {
            reason: format!("sin datos de {FAKE_DB} (token {FAKE_TOKEN})"),
            at: at(),
        },
    ];
    log_events(&log, &events);
    report_failure(
        &log,
        &alerter(log.clone()),
        AlertKind::Disconnect,
        "store_failed",
        &format!("no conecta a {FAKE_DB}"),
        at(),
    )
    .await;
    let lines = out.lines();
    let names: Vec<&str> = lines.iter().map(|l| l["event"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "order_received",
            "risk_decision",
            "order_report",
            "kill_switch",
            "store_failed",
            "alert"
        ]
    );
    for l in &lines {
        assert!(l["ts"].as_str().unwrap().ends_with('Z'));
    }
    assert_eq!(lines[0]["target"], "broker");
    assert_eq!(lines[1]["level"], "warn");
    assert_eq!(lines[1]["rejection"], "stop_too_close");
    assert_eq!(lines[2]["report"]["status"], "rejected");
    let text = serde_json::to_string(&lines).unwrap();
    for secret in ["fake-db-password", FAKE_TOKEN] {
        assert!(!text.contains(secret), "el log tiene un secreto: {secret}");
    }
}

#[tokio::test]
async fn the_kill_switch_and_a_rejection_streak_raise_alerts() {
    let (log, out) = logger();
    let alerts = alerter(log);
    let streak = AlertConfig::from_toml(ALERTS).unwrap().rejected_streak;
    let mut events = vec![BrokerEvent::KillSwitch {
        reason: "drawdown máximo".into(),
        at: at(),
    }];
    for i in 0..streak {
        events.push(BrokerEvent::OrderReport {
            report: report(&format!("o{i}"), OrderStatus::Rejected),
        });
    }
    alert_events(&alerts, &events, at()).await;
    let kinds: Vec<String> = out
        .lines()
        .iter()
        .filter(|l| l["event"] == "alert")
        .map(|l| l["kind"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(kinds, ["kill_switch", "rejected_streak"]);
}

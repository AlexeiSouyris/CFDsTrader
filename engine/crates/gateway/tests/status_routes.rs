//! Estado de la operación (#314): `GET /api/v1/ops/status` y `GET /api/v1/ops/events`, solo
//! lectura. Los secretos son falsos.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cfds_broker::{BrokerEvent, Reconciliation};
use cfds_core::{KillSwitch, OrderReport, OrderStatus, PaperAccount};
use cfds_gateway::ops::{GateState, alert_events, log_events};
use cfds_gateway::status::{
    Feed, FeedWriter, OpsSource, ReconcileKind, ReconcileStatus, StatusConfig, StatusDeps,
    is_status_route, respond_status,
};
use cfds_obs::{AlertConfig, AlertKind, Alerter, Level, Logger, ReqwestPost};
use chrono::{DateTime, Utc};
use rust_decimal::dec;
use serde_json::{Value, json};

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const ALERTS: &str = include_str!("../../../../config/alerts.toml");
const FAKE_TOKEN: &str = "fake-gateway-token-0123456789";

fn now() -> DateTime<Utc> {
    "2026-11-23T10:00:00Z".parse().unwrap()
}

fn account(kill: KillSwitch, used: &str) -> PaperAccount {
    let mut doc = json!({
        "mode": "paper", "currency": "EUR", "as_of": "2026-11-23T10:00:00Z",
        "equity": "9800.00", "unrealized_pl": "-50.00", "realized_pl_today": "-150.00",
        "margin_used": "400.00", "margin_free": "9400.00", "leverage": "1.35",
        "daily_loss_used_pct": used, "open_positions": 1, "max_positions": 5,
        "kill_switch": { "active": false }
    });
    doc["kill_switch"] = serde_json::to_value(kill).unwrap();
    serde_json::from_value(doc).unwrap()
}

struct Fake {
    gate: GateState,
    account: Result<PaperAccount, String>,
    rec: Option<ReconcileStatus>,
}

impl OpsSource for Fake {
    fn gate(&self) -> GateState {
        self.gate
    }
    async fn account(&self, _: DateTime<Utc>) -> Result<PaperAccount, String> {
        self.account.clone()
    }
    fn last_reconcile(&self) -> Option<ReconcileStatus> {
        self.rec.clone()
    }
}

fn ready() -> Fake {
    Fake {
        gate: GateState::Ready,
        account: Ok(account(KillSwitch::Inactive, "40.0")),
        rec: Some(ReconcileStatus::new(
            ReconcileKind::Startup,
            now(),
            &Reconciliation {
                matched: 3,
                ..Reconciliation::default()
            },
        )),
    }
}

fn feed() -> Arc<Feed> {
    Feed::new(StatusConfig::from_toml(GATEWAY).unwrap())
}

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn assert_valid(name: &str, doc: &Value) {
    let errors: Vec<String> = validator(name)
        .iter_errors(doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{name}: {errors:?}\n{doc}");
}

/// Un logger como el de `main`: el token registrado y el feed detrás.
fn logger(feed: &Arc<Feed>) -> (Arc<Logger>, Arc<Mutex<Vec<u8>>>) {
    #[derive(Clone)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let out = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::new(Logger::new(Box::new(FeedWriter::new(
        feed.clone(),
        Box::new(Sink(out.clone())),
    ))));
    log.register_secret(FAKE_TOKEN);
    (log, out)
}

async fn call(paper: Option<&Fake>, feed: &Feed, path: &str, method: &str) -> (u16, Value) {
    let log = Logger::new(Box::new(std::io::sink()));
    log.register_secret(FAKE_TOKEN);
    let scrub = |t: &str| log.scrub(t);
    let deps = StatusDeps {
        feed,
        events: None,
        daily_loss_limit_pct: dec!(3),
        scrub: &scrub,
    };
    let r = respond_status(paper, &deps, method, path, now()).await;
    (
        r.status,
        serde_json::from_str(&r.body).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn ready_status_shows_the_engine_numbers_as_they_are() {
    let f = feed();
    let (status, body) = call(Some(&ready()), &f, "/api/v1/ops/status", "GET").await;
    assert_eq!(status, 200);
    assert_valid("ops_status.v1.schema.json", &body);
    assert_eq!(body["mode"], "paper");
    assert_eq!(body["gate"], "ready");
    assert_eq!(body["accepting_orders"], true);
    let a = &body["account"];
    assert_eq!(a["kill_switch"], json!({ "active": false }));
    // Tal como los calcula el broker y el límite de config/risk.toml: el gateway no hace cuentas.
    assert_eq!(a["daily_loss_used_pct"], "40.0");
    assert_eq!(a["daily_loss_limit_pct"], "3");
    assert_eq!(a["leverage"], "1.35");
    assert_eq!(body["last_reconcile"]["kind"], "startup");
    assert_eq!(body["last_reconcile"]["clean"], true);
    assert_eq!(body["last_reconcile"]["matched"], 3);
}

#[tokio::test]
async fn an_active_kill_switch_stops_orders_and_its_reason_has_no_secrets() {
    let f = feed();
    let fake = Fake {
        account: Ok(account(
            KillSwitch::Active {
                reason: format!("desconexión con token {FAKE_TOKEN}"),
                since: now(),
            },
            "100.0",
        )),
        ..ready()
    };
    let (_, body) = call(Some(&fake), &f, "/api/v1/ops/status", "GET").await;
    assert_valid("ops_status.v1.schema.json", &body);
    assert_eq!(body["gate"], "ready");
    assert_eq!(body["accepting_orders"], false);
    let ks = &body["account"]["kill_switch"];
    assert_eq!(ks["active"], true);
    assert_eq!(ks["since"], "2026-11-23T10:00:00Z");
    assert!(!body.to_string().contains(FAKE_TOKEN));
    assert!(ks["reason"].as_str().unwrap().contains("***"));
}

#[tokio::test]
async fn starting_unavailable_and_broken_account_never_accept_orders() {
    let f = feed();
    let starting = Fake {
        gate: GateState::Starting,
        rec: Some(ReconcileStatus::new(
            ReconcileKind::Startup,
            now(),
            &Reconciliation {
                matched: 1,
                drifts: vec![],
                mismatches: vec![],
            },
        )),
        ..ready()
    };
    let (_, body) = call(Some(&starting), &f, "/api/v1/ops/status", "GET").await;
    assert_valid("ops_status.v1.schema.json", &body);
    assert_eq!(body["gate"], "starting");
    assert_eq!(body["accepting_orders"], false);

    let (_, none) = call(None, &f, "/api/v1/ops/status", "GET").await;
    assert_valid("ops_status.v1.schema.json", &none);
    assert_eq!(none["gate"], "unavailable");
    assert_eq!(none["account"], Value::Null);
    assert_eq!(none["accepting_orders"], false);

    let broken = Fake {
        account: Err(format!("sin base: {FAKE_TOKEN}")),
        ..ready()
    };
    let (status, body) = call(Some(&broken), &f, "/api/v1/ops/status", "GET").await;
    assert_eq!(status, 200);
    assert_valid("ops_status.v1.schema.json", &body);
    assert_eq!(body["account"], Value::Null);
    assert_eq!(body["accepting_orders"], false);
    assert!(!body.to_string().contains(FAKE_TOKEN));
}

#[tokio::test]
async fn events_are_the_redacted_broker_and_alert_lines_newest_first() {
    let f = feed();
    let (log, out) = logger(&f);
    let alerts: Alerter<ReqwestPost> =
        Alerter::new(AlertConfig::from_toml(ALERTS).unwrap(), log.clone(), None);
    // Ruido informativo del gateway: no entra.
    log.log(
        Level::Info,
        "gateway",
        "listening",
        json!({ "bind": "127.0.0.1:8787" }),
    );
    let events = vec![
        BrokerEvent::OrderReport {
            report: OrderReport {
                order_id: "ui:1".parse().unwrap(),
                status: OrderStatus::Filled,
                rejection: None,
                position_id: None,
                size: None,
                fill_price: None,
                commission: None,
                at: now(),
            },
        },
        BrokerEvent::KillSwitch {
            reason: format!("límite diario; token {FAKE_TOKEN}"),
            at: now(),
        },
    ];
    log_events(&log, &events);
    alert_events(&alerts, &events, now()).await;
    // Un error con un secreto en un campo con nombre de secreto.
    log.log(
        Level::Error,
        "gateway",
        "store_failed",
        json!({ "error": "timeout", "token": FAKE_TOKEN }),
    );
    alerts.raise(AlertKind::Disconnect, "sin base", now()).await;

    // La salida de siempre sigue recibiendo todo.
    let written = String::from_utf8(out.lock().unwrap().clone()).unwrap();
    assert!(written.contains("\"listening\""));

    let (status, body) = call(None, &f, "/api/v1/ops/events", "GET").await;
    assert_eq!(status, 200);
    let rows = body.as_array().unwrap();
    let v = validator("ops_event.v1.schema.json");
    for r in rows {
        let errors: Vec<String> = v.iter_errors(r).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{errors:?}\n{r}");
    }
    let names: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r["target"].as_str().unwrap(), r["event"].as_str().unwrap()))
        .collect();
    assert_eq!(
        names,
        [
            ("alert", "alert"),
            ("gateway", "store_failed"),
            ("alert", "alert"),
            ("broker", "kill_switch"),
            ("broker", "order_report"),
        ]
    );
    assert_eq!(rows[2]["fields"]["kind"], "kill_switch");
    assert_eq!(rows[1]["fields"]["token"], "***");
    assert!(!body.to_string().contains(FAKE_TOKEN), "{body}");
}

#[tokio::test]
async fn the_feed_keeps_only_the_last_n() {
    let f = Feed::new(StatusConfig {
        feed_size: 2,
        ..StatusConfig::from_toml(GATEWAY).unwrap()
    });
    let (log, _) = logger(&f);
    for i in 0..5 {
        log.log(Level::Warn, "broker", "risk_decision", json!({ "n": i }));
    }
    // Una línea rota no rompe nada.
    f.push_line("{no es json");
    f.push_line(r#"{"ts":"ayer","level":"warn","target":"broker","event":"x"}"#);
    let latest = f.latest();
    assert_eq!(latest.len(), 2);
    assert_eq!(latest[0].fields["n"], 4);
    assert_eq!(latest[1].fields["n"], 3);
}

#[tokio::test]
async fn only_get_and_only_its_routes() {
    let f = feed();
    for path in ["/api/v1/ops/status", "/api/v1/ops/events"] {
        assert!(is_status_route(path));
        for method in ["POST", "PUT", "DELETE"] {
            let (status, _) = call(Some(&ready()), &f, path, method).await;
            assert_eq!(status, 405, "{method} {path}");
        }
    }
    // No hay ruta para reiniciar el kill switch.
    assert!(!is_status_route("/api/v1/ops/kill_switch"));
    assert!(!is_status_route("/api/v1/ops/reset"));
    let (status, _) = call(Some(&ready()), &f, "/api/v1/ops/reset", "GET").await;
    assert_eq!(status, 404);
}

#[test]
fn feed_size_comes_from_the_config_and_is_bounded() {
    assert_eq!(StatusConfig::from_toml(GATEWAY).unwrap().feed_size, 50);
    let zero = GATEWAY.replace("feed_size = 50", "feed_size = 0");
    assert_ne!(zero, GATEWAY);
    assert!(StatusConfig::from_toml(&zero).is_err());
    let extra = GATEWAY.replace("feed_size = 50", "feed_size = 50\nnope = 1");
    assert!(StatusConfig::from_toml(&extra).is_err());
}

//! Eventos de la operación guardados en la base (#337), con una base falsa en memoria: sin
//! secretos, la base caída no frena ni pierde el feed en memoria, orden, tope y contrato.
//! Los secretos son falsos.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cfds_core::PaperAccount;
use cfds_gateway::ops::GateState;
use cfds_gateway::ops_store::{BoxFuture, EventStore, persistence, run_persister};
use cfds_gateway::status::{
    Feed, FeedWriter, OpsSource, ReconcileStatus, StatusConfig, StatusDeps, respond_status,
};
use cfds_obs::{Level, Logger};
use cfds_storage::OpsEventRecord;
use chrono::{DateTime, Utc};
use rust_decimal::dec;
use serde_json::{Value, json};

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const FAKE_TOKEN: &str = "fake-gateway-token-0123456789";
const FAKE_DB: &str = "postgres://cfds:fake-db-password@127.0.0.1:5432/cfdstrader";

/// Una base en memoria que se puede "caer".
#[derive(Default)]
struct MemStore {
    rows: Mutex<Vec<OpsEventRecord>>,
    down: AtomicBool,
}

impl EventStore for MemStore {
    fn insert<'a>(&'a self, events: &'a [OpsEventRecord]) -> BoxFuture<'a, Result<usize, String>> {
        Box::pin(async move {
            if self.down.load(Ordering::SeqCst) {
                return Err("connection refused".into());
            }
            self.rows.lock().unwrap().extend_from_slice(events);
            Ok(events.len())
        })
    }
    fn latest(&self, limit: u32) -> BoxFuture<'_, Result<Vec<OpsEventRecord>, String>> {
        Box::pin(async move {
            if self.down.load(Ordering::SeqCst) {
                return Err("connection refused".into());
            }
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .rev()
                .take(limit as usize)
                .cloned()
                .collect())
        })
    }
}

fn cfg(feed_size: usize, queue: usize) -> StatusConfig {
    StatusConfig {
        feed_size,
        persist_queue: queue,
        retry: Duration::from_millis(10),
        ..StatusConfig::from_toml(GATEWAY).unwrap()
    }
}

/// Un logger como el de `main`: secretos registrados, feed detrás y la cola hacia la base.
struct Setup {
    log: Arc<Logger>,
    feed: Arc<Feed>,
    store: Arc<MemStore>,
    errors: Arc<AtomicUsize>,
    sink: Arc<cfds_gateway::ops_store::Persistence>,
    task: tokio::task::JoinHandle<()>,
}

fn setup(c: StatusConfig) -> Setup {
    let feed = Feed::new(c);
    let (sink, rx) = persistence(c.persist_queue);
    feed.attach(sink.clone());
    let log = Arc::new(Logger::new(Box::new(FeedWriter::new(
        feed.clone(),
        Box::new(std::io::sink()),
    ))));
    log.register_secret(FAKE_TOKEN);
    log.register_secret(FAKE_DB);
    let store = Arc::new(MemStore::default());
    let errors = Arc::new(AtomicUsize::new(0));
    let e = errors.clone();
    let task = tokio::spawn(run_persister(
        rx,
        store.clone(),
        c.persist_queue,
        c.retry,
        move |_msg| {
            let e = e.clone();
            async move {
                e.fetch_add(1, Ordering::SeqCst);
            }
        },
    ));
    Setup {
        log,
        feed,
        store,
        errors,
        sink,
        task,
    }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(60)).await;
}

fn n_of(r: &OpsEventRecord) -> i64 {
    r.fields["n"].as_i64().unwrap()
}

#[tokio::test]
async fn no_secret_ever_reaches_the_database() {
    let s = setup(cfg(50, 100));
    s.log.log(
        Level::Warn,
        "broker",
        "kill_switch",
        json!({ "reason": format!("caída con {FAKE_TOKEN}"), "url": FAKE_DB }),
    );
    s.log.log(
        Level::Error,
        "gateway",
        "store_failed",
        json!({ "error": "x", "token": FAKE_TOKEN, "password": "hunter2-hunter2" }),
    );
    settle().await;
    let rows = s.store.rows.lock().unwrap().clone();
    assert_eq!(rows.len(), 2);
    let all = serde_json::to_string(
        &rows
            .iter()
            .map(|r| Value::Object(r.fields.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(!all.contains(FAKE_TOKEN), "{all}");
    assert!(!all.contains("fake-db-password"), "{all}");
    assert!(!all.contains("hunter2"), "{all}");
    assert_eq!(rows[1].fields["token"], "***");
    s.task.abort();
}

#[tokio::test]
async fn a_dead_database_never_blocks_and_the_memory_feed_stays() {
    let s = setup(cfg(50, 5));
    s.store.down.store(true, Ordering::SeqCst);
    // Muchas más líneas que la cola: el logger nunca espera a la base.
    let start = Instant::now();
    for n in 0..2000 {
        s.log
            .log(Level::Warn, "broker", "risk_decision", json!({ "n": n }));
    }
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "el log esperó a la base"
    );
    // La memoria tiene los últimos feed_size, en orden.
    let mem = s.feed.latest();
    assert_eq!(mem.len(), 50);
    assert_eq!(mem[0].fields["n"], 1999);
    assert_eq!(mem[49].fields["n"], 1950);
    settle().await;
    assert!(
        s.errors.load(Ordering::SeqCst) >= 1,
        "avisa que la base no responde"
    );
    assert!(s.sink.dropped() > 0, "lo que no entra en la cola se cuenta");
    assert!(s.store.rows.lock().unwrap().is_empty());

    // Vuelve la base: lo pendiente se guarda, en orden y sin pasar el tope.
    s.store.down.store(false, Ordering::SeqCst);
    settle().await;
    let rows = s.store.rows.lock().unwrap().clone();
    assert!(!rows.is_empty() && rows.len() <= 10, "{}", rows.len());
    let ns: Vec<i64> = rows.iter().map(n_of).collect();
    assert!(ns.windows(2).all(|w| w[0] < w[1]), "en orden: {ns:?}");
    s.task.abort();
}

#[tokio::test]
async fn everything_is_saved_in_order_when_the_database_is_up() {
    let s = setup(cfg(10, 1000));
    for n in 0..300 {
        s.log
            .log(Level::Warn, "broker", "order_report", json!({ "n": n }));
    }
    // Ruido informativo del gateway: tampoco va a la base.
    s.log.log(Level::Info, "gateway", "listening", json!({}));
    settle().await;
    let rows = s.store.rows.lock().unwrap().clone();
    assert_eq!(rows.len(), 300);
    assert!(rows.iter().enumerate().all(|(i, r)| n_of(r) == i as i64));
    assert_eq!(s.sink.dropped(), 0);
    s.task.abort();
}

struct NoPaper;

impl OpsSource for NoPaper {
    fn gate(&self) -> GateState {
        GateState::Ready
    }
    async fn account(&self, _: DateTime<Utc>) -> Result<PaperAccount, String> {
        Err("sin broker".into())
    }
    fn last_reconcile(&self) -> Option<ReconcileStatus> {
        None
    }
}

async fn route(feed: &Feed, store: Option<&dyn EventStore>) -> Vec<Value> {
    let log = Logger::new(Box::new(std::io::sink()));
    let scrub = |t: &str| log.scrub(t);
    let deps = StatusDeps {
        feed,
        events: store,
        events_limit: feed.cap(),
        group_scan: 100,
        daily_loss_limit_pct: dec!(3),
        scrub: &scrub,
    };
    let r = respond_status(
        None::<&NoPaper>,
        &deps,
        "GET",
        "/api/v1/ops/events",
        None,
        "2026-11-23T12:00:00Z".parse().unwrap(),
    )
    .await;
    assert_eq!(r.status, 200);
    serde_json::from_str::<Value>(&r.body)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

#[tokio::test]
async fn the_route_reads_the_database_and_falls_back_to_memory() {
    let s = setup(cfg(3, 100));
    for n in 0..6 {
        s.log
            .log(Level::Warn, "broker", "order_report", json!({ "n": n }));
    }
    settle().await;
    // De la base: los últimos feed_size, el más nuevo primero, con la forma del contrato.
    let from_db = route(&s.feed, Some(s.store.as_ref())).await;
    let ns: Vec<i64> = from_db
        .iter()
        .map(|r| r["fields"]["n"].as_i64().unwrap())
        .collect();
    assert_eq!(ns, [5, 4, 3]);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/ops_event.v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    for r in &from_db {
        let errors: Vec<String> = v.iter_errors(r).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{errors:?}\n{r}");
    }
    // Después de un "reinicio" la memoria está vacía, pero la base conserva todo.
    let fresh = Feed::new(cfg(3, 100));
    let after_restart = route(&fresh, Some(s.store.as_ref())).await;
    assert_eq!(after_restart.len(), 3);
    // Base caída: los de memoria.
    s.store.down.store(true, Ordering::SeqCst);
    let fallback = route(&s.feed, Some(s.store.as_ref())).await;
    let ns: Vec<i64> = fallback
        .iter()
        .map(|r| r["fields"]["n"].as_i64().unwrap())
        .collect();
    assert_eq!(ns, [5, 4, 3]);
    // Sin base configurada, también memoria.
    assert_eq!(route(&s.feed, None).await.len(), 3);
    s.task.abort();
}

#[test]
fn the_ops_config_is_validated() {
    let c = StatusConfig::from_toml(GATEWAY).unwrap();
    assert_eq!(c.retention_days, 30);
    assert_eq!(c.persist_queue, 1000);
    assert_eq!(c.retry, Duration::from_secs(30));
    for (old, new) in [
        ("retention_days = 30", "retention_days = 0"),
        ("persist_queue = 1000", "persist_queue = 0"),
        ("retry_secs = 30", "retry_secs = 0"),
        ("retry_secs = 30", "retry_secs = 30\nnope = 1"),
    ] {
        let bad = GATEWAY.replace(old, new);
        assert_ne!(bad, GATEWAY);
        assert!(StatusConfig::from_toml(&bad).is_err(), "{new}");
    }
}

#[tokio::test]
async fn repeated_events_are_stored_each_time_but_read_grouped() {
    // #576: la base guarda cada uno (es de solo agregar); la ruta los agrupa con count y
    // last_seen, aunque vengan intercalados con otros.
    let s = setup(cfg(5, 100));
    s.log
        .log(Level::Warn, "broker", "risk_decision", json!({ "n": 1 }));
    for _ in 0..30 {
        s.log.log(
            Level::Warn,
            "gateway",
            "connection_error",
            json!({ "error": "x" }),
        );
        s.log.log(
            Level::Warn,
            "broker",
            "instrument_skipped",
            json!({ "i": "US100" }),
        );
    }
    settle().await;
    assert_eq!(s.store.rows.lock().unwrap().len(), 61);
    let rows = route(&s.feed, Some(s.store.as_ref())).await;
    let summary: Vec<(&str, u64)> = rows
        .iter()
        .map(|r| (r["event"].as_str().unwrap(), r["count"].as_u64().unwrap()))
        .collect();
    assert_eq!(
        summary,
        [
            ("instrument_skipped", 30),
            ("connection_error", 30),
            ("risk_decision", 1)
        ]
    );
    for r in &rows {
        assert!(r["ts"].as_str().unwrap() <= r["last_seen"].as_str().unwrap());
    }
    // La memoria también agrupa, igual.
    let mem = route(&s.feed, None).await;
    let mem_summary: Vec<(&str, u64)> = mem
        .iter()
        .map(|r| (r["event"].as_str().unwrap(), r["count"].as_u64().unwrap()))
        .collect();
    assert_eq!(mem_summary, summary);
}

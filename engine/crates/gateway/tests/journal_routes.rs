//! Ruta del diario de paper (#615), solo lectura. Entradas de ejemplo con la forma de los contratos
//! (`gateway_journal.v1`, que incluye `proposal.v1` y `outcome.v1`), no de mercado. Sin base real:
//! la fuente es falsa.

use std::path::PathBuf;
use std::sync::Mutex;

use cfds_gateway::journal::{
    DEFAULT_LIMIT, JournalSource, MAX_LIMIT, is_journal_route, respond_journal,
};
use cfds_journal::curve::RunTrade;
use serde_json::{Value, json};

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
        Ok(serde_json::from_str(&std::fs::read_to_string(
            contracts().join(name),
        )?)?)
    }
}

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(contracts().join("gateway_journal.v1.schema.json")).unwrap(),
    )
    .unwrap();
    jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap()
}

fn assert_page(doc: &Value) {
    let errors: Vec<String> = validator()
        .iter_errors(doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

fn proposal(id: &str, signal_at: &str) -> Value {
    json!({
        "id": id, "source": "paper", "run_id": "paper-runner", "instrument_id": "US500",
        "side": "short", "signal_at": signal_at, "reference": "6712.4", "stop": "6730.4",
        "tp1": "6676.4", "size": "0.5", "leverage": "20", "score": 4,
        "breakdown": [{ "component": "trend", "points": 2 }, { "component": "level_rejection", "points": 2 }],
        "triggers": ["level_rejection"]
    })
}

/// Una operación que terminó en el stop: pérdida neta, con costos.
fn closed(id: &str) -> Value {
    json!({
        "proposal_id": id, "kind": "simulated", "entry_at": "2026-11-27T16:45:00Z",
        "entry_price": "6712.1",
        "exits": [{ "at": "2026-11-27T18:00:00Z", "price": "6730.7", "fraction": "1", "reason": "stop" }],
        "commission": "0", "funding": "0", "gross_pnl": "-9.30", "net_pnl": "-9.30",
        "hit_tp1": false, "hit_stop": true, "exit_reason": "stop"
    })
}

fn trade(p: Value, o: Value) -> RunTrade {
    serde_json::from_value(json!({ "proposal": p, "outcome": o })).unwrap()
}

/// Fuente de prueba: recuerda con qué límite se la llamó.
struct Fake {
    trades: Result<Vec<RunTrade>, String>,
    asked: Mutex<Vec<u32>>,
}

impl Fake {
    fn new(trades: Result<Vec<RunTrade>, String>) -> Self {
        Self {
            trades,
            asked: Mutex::new(Vec::new()),
        }
    }
}

impl JournalSource for Fake {
    async fn paper_trades(&self, limit: u32) -> Result<Vec<RunTrade>, String> {
        self.asked.lock().unwrap().push(limit);
        self.trades.clone()
    }
}

#[test]
fn the_contract_rejects_what_it_does_not_define() {
    let v = validator();
    let ok = json!({ "entries": [{ "proposal": proposal("a", "2026-11-27T16:30:00Z"), "outcome": null }], "limit": 100 });
    assert!(v.is_valid(&ok));
    for bad in [
        json!({ "entries": [] }),
        json!({ "entries": [], "limit": 0 }),
        json!({ "entries": [], "limit": 100, "extra": 1 }),
        json!({ "entries": [{ "proposal": proposal("a", "2026-11-27T16:30:00Z") }], "limit": 100 }),
        // P/L como número: el dinero viaja como texto decimal
        json!({ "entries": [{ "proposal": proposal("a", "2026-11-27T16:30:00Z"),
            "outcome": { "net_pnl": -9.3 } }], "limit": 100 }),
    ] {
        assert!(!v.is_valid(&bad), "{bad}");
    }
}

#[tokio::test]
async fn returns_the_paper_journal_as_stored() {
    let a = "paper-runner:US500:2026-11-27T16:30:00Z:short";
    let b = "paper-runner:US500:2026-11-27T19:00:00Z:short";
    let stored = vec![
        trade(proposal(b, "2026-11-27T19:00:00Z"), Value::Null),
        trade(proposal(a, "2026-11-27T16:30:00Z"), closed(a)),
    ];
    let src = Fake::new(Ok(stored.clone()));
    let r = respond_journal(&src, "GET", "/api/v1/journal", None).await;
    assert_eq!((r.status, r.content_type), (200, "application/json"));
    let got: Value = serde_json::from_str(&r.body).unwrap();
    assert_page(&got);
    assert_eq!(got["limit"], json!(DEFAULT_LIMIT));
    // tal cual, en el orden de la fuente, con los decimales como texto
    let entries: Vec<RunTrade> = serde_json::from_value(got["entries"].clone()).unwrap();
    assert_eq!(entries, stored);
    assert_eq!(got["entries"][1]["outcome"]["net_pnl"], json!("-9.30"));
    assert_eq!(got["entries"][0]["outcome"], Value::Null);
    assert_eq!(src.asked.lock().unwrap().as_slice(), [DEFAULT_LIMIT]);
}

#[tokio::test]
async fn an_empty_journal_is_an_empty_list_not_an_error() {
    let src = Fake::new(Ok(vec![]));
    let r = respond_journal(&src, "GET", "/api/v1/journal", Some("limit=20")).await;
    assert_eq!(r.status, 200);
    let got: Value = serde_json::from_str(&r.body).unwrap();
    assert_page(&got);
    assert_eq!(got, json!({ "entries": [], "limit": 20 }));
    assert_eq!(src.asked.lock().unwrap().as_slice(), [20]);
}

#[tokio::test]
async fn only_get_and_only_a_valid_limit() {
    let src = Fake::new(Ok(vec![]));
    let max = format!("limit={MAX_LIMIT}");
    let over = format!("limit={}", MAX_LIMIT + 1);
    for (method, query, status) in [
        ("GET", Some(max.as_str()), 200),
        ("GET", Some("limit=1"), 200),
        ("GET", Some(""), 200),
        ("POST", None, 405),
        ("DELETE", None, 405),
        ("GET", Some(over.as_str()), 400),
        ("GET", Some("limit=0"), 400),
        ("GET", Some("limit="), 400),
        ("GET", Some("limit=-1"), 400),
        ("GET", Some("limit=99999999999"), 400),
        ("GET", Some("limit=5&limit=6"), 400),
        ("GET", Some("before=2026-11-27T00:00:00Z"), 400),
        ("GET", Some("limit=5;DROP"), 400),
    ] {
        let r = respond_journal(&src, method, "/api/v1/journal", query).await;
        assert_eq!(r.status, status, "{method} {query:?}");
    }
    // los pedidos rechazados no llegan a la base
    assert_eq!(src.asked.lock().unwrap().len(), 3);
    assert!(is_journal_route("/api/v1/journal"));
    assert!(!is_journal_route("/api/v1/journal/x"));
    assert!(!is_journal_route("/api/v1/paper/journal"));
    let r = respond_journal(&src, "GET", "/api/v1/journal/x", None).await;
    assert_eq!(r.status, 404);
}

#[tokio::test]
async fn a_database_error_does_not_leak() {
    let src = Fake::new(Err("PostgreSQL: password authentication failed".into()));
    let r = respond_journal(&src, "GET", "/api/v1/journal", None).await;
    assert_eq!((r.status, r.body.as_str()), (500, "error interno"));
}

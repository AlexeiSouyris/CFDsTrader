//! Rutas del laboratorio del gateway (#776): catálogo, validación, encolar (idempotente),
//! estado, cancelar, informe y el SSE de progreso; contratos, anti-CSRF y códigos de error. La
//! cola va a una lista en memoria; las tablas de solo agregar se prueban en `cfds-journal`
//! (`tests/lab.rs`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cfds_backtest::lab::{JobState, LabJobEvent, LabJobRequest, job_id};
use cfds_core::InstrumentId;
use cfds_gateway::Reply;
use cfds_gateway::lab::{
    LabDeps, LabEventsReply, LabRoutes, LabSseClients, LabStore, lab_events_job, respond_lab,
    respond_lab_events,
};
use cfds_gateway::paper::HttpIn;
use cfds_journal::lab::{Appended, Enqueued};
use cfds_market_data::CandleSources;
use cfds_risk::RiskConfig;
use chrono::{DateTime, Utc};
use http_body_util::BodyExt;
use serde_json::{Value, json};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

const TOKEN: &str = "token-local-de-prueba-123";

fn contracts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts")
}

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

fn assert_contract(name: &str, doc: &Value) {
    let schema: Value =
        serde_json::from_str(&std::fs::read_to_string(contracts().join(name)).unwrap()).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap();
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{name}: {errors:?}");
}

fn deps() -> LabDeps {
    let risk = RiskConfig::from_toml(cfg!("risk.toml")).unwrap();
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml")).unwrap();
    LabDeps::from_config(
        cfg!("gateway.toml"),
        cfg!("lab.toml"),
        &risk,
        &sources,
        cfg!("data_split.toml"),
        None,
    )
    .unwrap()
}

#[derive(Default)]
struct Inner {
    last_candle: Option<DateTime<Utc>>,
    /// (hash del pedido, id del trabajo, total de variantes)
    jobs: Vec<(String, String, u32)>,
    events: Vec<LabJobEvent>,
    reports: HashMap<String, Value>,
    candle_calls: usize,
}

#[derive(Clone, Default)]
struct Mem(Arc<Mutex<Inner>>);

impl Mem {
    fn with_candles() -> Self {
        let m = Self::default();
        m.0.lock().unwrap().last_candle = Some("2026-11-20T21:45:00Z".parse().unwrap());
        m
    }

    fn push(&self, job: &str, state: JobState, report_ids: Vec<String>) -> LabJobEvent {
        let mut g = self.0.lock().unwrap();
        let total = g.jobs.iter().find(|j| j.1 == job).map_or(1, |j| j.2);
        let e = LabJobEvent {
            job_id: job.to_owned(),
            seq: i64::try_from(g.events.len()).unwrap() * 3 + 1,
            state,
            stage: (state == JobState::Running)
                .then_some(cfds_backtest::lab::JobStage::RunningVariants),
            percent: if state == JobState::Done { 100 } else { 30 },
            variants_done: 0,
            variants_total: total,
            report_ids,
            error: (state == JobState::Failed).then(|| "x".to_owned()),
            recorded_at: now(),
        };
        g.events.push(e.clone());
        e
    }

    fn last(&self, job: &str) -> Option<LabJobEvent> {
        let g = self.0.lock().unwrap();
        g.events.iter().rev().find(|e| e.job_id == job).cloned()
    }
}

impl LabStore for Mem {
    async fn last_candle(&self, _: &InstrumentId) -> Result<Option<DateTime<Utc>>, String> {
        let mut g = self.0.lock().unwrap();
        g.candle_calls += 1;
        Ok(g.last_candle)
    }

    async fn enqueue(
        &self,
        request: &LabJobRequest,
        request_sha256: &str,
        _family: &str,
        _now: DateTime<Utc>,
    ) -> Result<Enqueued, String> {
        let previous: Vec<String> = {
            let g = self.0.lock().unwrap();
            g.jobs
                .iter()
                .filter(|j| j.0 == request_sha256)
                .map(|j| j.1.clone())
                .collect()
        };
        if let Some(last) = previous.last().and_then(|j| self.last(j))
            && !matches!(last.state, JobState::Failed | JobState::Cancelled)
        {
            return Ok(Enqueued::Existing(last));
        }
        let attempt = u32::try_from(previous.len()).unwrap() + 1;
        let id = job_id(request_sha256, attempt);
        let total = u32::try_from(request.specs.len()).unwrap();
        self.0
            .lock()
            .unwrap()
            .jobs
            .push((request_sha256.to_owned(), id.clone(), total));
        Ok(Enqueued::New(self.push(&id, JobState::Queued, vec![])))
    }

    async fn cancel(&self, job: &str, _now: DateTime<Utc>) -> Result<Option<Appended>, String> {
        Ok(match self.last(job) {
            None => None,
            Some(e) if e.state.is_end() => Some(Appended::AlreadyEnded(e)),
            Some(_) => Some(Appended::Added(self.push(job, JobState::Cancelled, vec![]))),
        })
    }

    async fn last_event(&self, job: &str) -> Result<Option<LabJobEvent>, String> {
        Ok(self.last(job))
    }

    async fn events_after(&self, job: &str, after: i64) -> Result<Vec<LabJobEvent>, String> {
        let g = self.0.lock().unwrap();
        Ok(g.events
            .iter()
            .filter(|e| e.job_id == job && e.seq > after)
            .cloned()
            .collect())
    }

    async fn report(&self, id: &str) -> Result<Option<Value>, String> {
        Ok(self.0.lock().unwrap().reports.get(id).cloned())
    }
}

fn now() -> DateTime<Utc> {
    "2026-11-27T03:00:00Z".parse().unwrap()
}

fn spec(fast: u32) -> Value {
    json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": "1h" },
        "direction": "both",
        "entry": [{ "type": "ma_cross", "params": { "average": "ema", "fast": fast, "slow": 50 } }],
        "filters": [],
        "stop": { "type": "atr", "params": { "period": 14, "k": "2" } },
        "exit": { "type": "time_target", "params": { "max_candles": 8, "target_r": "2" } },
        "risk_pct": "0.5",
        "cost_multiplier": "1",
        "period": { "from": "2020-01-01T00:00:00Z", "to": "2023-01-01T00:00:00Z" },
        "mode": "exploratory"
    })
}

struct Req<'a> {
    method: &'a str,
    path: &'a str,
    token: Option<&'a str>,
    content_type: Option<&'a str>,
    body: String,
}

fn get(path: &str) -> Req<'_> {
    Req {
        method: "GET",
        path,
        token: None,
        content_type: None,
        body: String::new(),
    }
}

fn post<'a>(path: &'a str, body: &Value) -> Req<'a> {
    Req {
        method: "POST",
        path,
        token: Some(TOKEN),
        content_type: Some("application/json"),
        body: body.to_string(),
    }
}

async fn call(store: &Mem, r: Req<'_>) -> Reply {
    let req = HttpIn {
        method: r.method,
        path: r.path,
        token: r.token,
        content_type: r.content_type,
        body: r.body.as_bytes(),
    };
    respond_lab(store, &deps(), Some(TOKEN), req, now()).await
}

fn body(r: &Reply) -> Value {
    serde_json::from_str(&r.body).unwrap_or_else(|_| panic!("no es JSON: {}", r.body))
}

#[test]
fn the_config_is_validated_at_startup() {
    let r = LabRoutes::from_toml(cfg!("gateway.toml")).unwrap();
    assert!(r.sse_poll < r.sse_heartbeat);
    for (from, to) in [
        ("sse_poll_secs = 2", "sse_poll_secs = 0"),
        ("sse_max_clients = 8", "sse_max_clients = 0"),
    ] {
        let text = cfg!("gateway.toml").replace(from, to);
        assert!(LabRoutes::from_toml(&text).is_err(), "{to}");
    }
    assert_eq!(lab_events_job("/api/v1/lab/jobs/abc/events"), Some("abc"));
    assert_eq!(lab_events_job("/api/v1/lab/jobs/abc"), None);
}

#[tokio::test]
async fn the_catalog_comes_from_the_engine_config() {
    let store = Mem::default();
    let r = call(&store, get("/api/v1/lab/catalog")).await;
    assert_eq!(r.status, 200);
    let cat = body(&r);
    assert_contract("strategy_catalog.v1.schema.json", &cat);
    assert_eq!(
        cat["modes"],
        json!(["exploratory"]),
        "el confirmatorio sigue apagado"
    );
    assert!(
        cat["markets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["instrument_id"] == "US500")
    );
    let mut r = get("/api/v1/lab/catalog");
    r.method = "POST";
    assert_eq!(call(&store, r).await.status, 405);
    assert_eq!(call(&store, get("/api/v1/lab/nada")).await.status, 404);
}

#[tokio::test]
async fn validate_returns_the_engine_verdict() {
    let store = Mem::with_candles();
    let r = call(&store, post("/api/v1/lab/validate", &spec(20))).await;
    assert_eq!(r.status, 200, "{}", r.body);
    let v = body(&r);
    assert_contract("spec_validation.v1.schema.json", &v);
    assert_eq!(v["valid"], json!(true));
    // Riesgo mayor que config/risk.toml y un período que llega al fuera de muestra.
    let mut bad = spec(20);
    bad["risk_pct"] = json!("50");
    bad["period"]["to"] = json!("2026-01-01T00:00:00Z");
    let v = body(&call(&store, post("/api/v1/lab/validate", &bad)).await);
    assert_eq!(v["valid"], json!(false));
    let codes: Vec<&str> = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"risk_out_of_range"), "{codes:?}");
    assert!(codes.contains(&"period_touches_holdout"), "{codes:?}");
    // Mal formada: su spec_validation.v1 con `malformed`.
    let v = body(&call(&store, post("/api/v1/lab/validate", &json!({ "x": 1 }))).await);
    assert_eq!(v["errors"][0]["code"], json!("malformed"));
    // Un instrumento fuera del catálogo: rechazado sin mirar velas.
    let calls = store.0.lock().unwrap().candle_calls;
    let mut other = spec(20);
    other["market"]["instrument_id"] = json!("XYZ");
    let v = body(&call(&store, post("/api/v1/lab/validate", &other)).await);
    assert_eq!(v["errors"][0]["code"], json!("unknown_market"));
    assert_eq!(store.0.lock().unwrap().candle_calls, calls);
    // Sin velas guardadas no hay separación: 409.
    let empty = Mem::default();
    assert_eq!(
        call(&empty, post("/api/v1/lab/validate", &spec(20)))
            .await
            .status,
        409
    );
}

#[tokio::test]
async fn writes_need_the_token_json_and_a_small_body() {
    let store = Mem::with_candles();
    for path in ["/api/v1/lab/validate", "/api/v1/lab/jobs"] {
        let mut r = post(path, &spec(20));
        r.token = None;
        assert_eq!(call(&store, r).await.status, 403);
        let mut r = post(path, &spec(20));
        r.token = Some("otro-token-de-prueba-000");
        assert_eq!(call(&store, r).await.status, 403);
        let mut r = post(path, &spec(20));
        r.content_type = Some("text/plain");
        assert_eq!(call(&store, r).await.status, 415);
        let mut r = post(path, &spec(20));
        r.body = "x".repeat(deps().routes.max_body_bytes + 1);
        assert_eq!(call(&store, r).await.status, 413);
    }
    let id = "a".repeat(64);
    let path = format!("/api/v1/lab/jobs/{id}");
    let del = Req {
        method: "DELETE",
        path: &path,
        token: None,
        content_type: None,
        body: String::new(),
    };
    assert_eq!(call(&store, del).await.status, 403);
}

#[tokio::test]
async fn a_job_is_queued_once_followed_and_cancelled() {
    let store = Mem::with_candles();
    let req = json!({ "specs": [spec(20), spec(30)], "initial_equity": "100000" });
    let r = call(&store, post("/api/v1/lab/jobs", &req)).await;
    assert_eq!(r.status, 202, "{}", r.body);
    let queued = body(&r);
    assert_contract("lab_job_event.v1.schema.json", &queued);
    assert_eq!(queued["state"], json!("queued"));
    assert_eq!(queued["variants_total"], json!(2));
    // El mismo pedido (escrito distinto): el mismo trabajo, 200.
    let same = json!({ "specs": [spec(20), spec(30)], "initial_equity": "100000.00" });
    let r = call(&store, post("/api/v1/lab/jobs", &same)).await;
    assert_eq!(r.status, 200);
    assert_eq!(body(&r)["job_id"], queued["job_id"]);

    let id = queued["job_id"].as_str().unwrap();
    let path = format!("/api/v1/lab/jobs/{id}");
    let r = call(&store, get(&path)).await;
    assert_eq!((r.status, body(&r)), (200, queued.clone()));
    let del = |token| Req {
        method: "DELETE",
        path: &path,
        token,
        content_type: None,
        body: String::new(),
    };
    let r = call(&store, del(Some(TOKEN))).await;
    assert_eq!(r.status, 200);
    let cancelled = body(&r);
    assert_contract("lab_job_event.v1.schema.json", &cancelled);
    assert_eq!(cancelled["state"], json!("cancelled"));
    // Cancelar de nuevo devuelve el final, sin otro evento.
    assert_eq!(body(&call(&store, del(Some(TOKEN))).await), cancelled);
    // Después de cancelar, el mismo pedido es un intento nuevo.
    let r = call(&store, post("/api/v1/lab/jobs", &req)).await;
    assert_eq!(r.status, 202);
    assert_ne!(body(&r)["job_id"], queued["job_id"]);

    let unknown = format!("/api/v1/lab/jobs/{}", "f".repeat(64));
    assert_eq!(call(&store, get(&unknown)).await.status, 404);
    assert_eq!(
        call(&store, get("/api/v1/lab/jobs/no-es-un-id"))
            .await
            .status,
        404
    );
    let mut put = get(&path);
    put.method = "PUT";
    assert_eq!(call(&store, put).await.status, 405);
}

#[tokio::test]
async fn bad_requests_are_rejected_before_queueing() {
    let store = Mem::with_candles();
    let jobs = "/api/v1/lab/jobs";
    assert_eq!(
        call(&store, post(jobs, &json!({ "specs": [] })))
            .await
            .status,
        400
    );
    let empty = json!({ "specs": [], "initial_equity": "1000" });
    assert_eq!(call(&store, post(jobs, &empty)).await.status, 422);
    let mut risky = spec(30);
    risky["risk_pct"] = json!("50");
    let r = call(
        &store,
        post(
            jobs,
            &json!({ "specs": [spec(20), risky], "initial_equity": "1000" }),
        ),
    )
    .await;
    assert_eq!(r.status, 422);
    assert!(r.body.contains("variante 2"), "{}", r.body);
    let mut other_period = spec(30);
    other_period["period"]["from"] = json!("2021-01-01T00:00:00Z");
    let r = call(
        &store,
        post(
            jobs,
            &json!({ "specs": [spec(20), other_period], "initial_equity": "1000" }),
        ),
    )
    .await;
    assert_eq!(r.status, 422);
    let max = deps().catalog.limits.max_variants_per_run;
    let many: Vec<Value> = (0..=max)
        .map(|k| spec(5 + u32::try_from(k).unwrap()))
        .collect();
    let r = call(
        &store,
        post(jobs, &json!({ "specs": many, "initial_equity": "1000" })),
    )
    .await;
    assert_eq!(r.status, 422);
    assert!(
        store.0.lock().unwrap().jobs.is_empty(),
        "nada quedó en la cola"
    );
}

#[tokio::test]
async fn reports_are_read_by_id() {
    let store = Mem::default();
    let id = "c".repeat(64);
    store
        .0
        .lock()
        .unwrap()
        .reports
        .insert(id.clone(), json!({ "report_id": id, "votes": false }));
    let r = call(&store, get(&format!("/api/v1/lab/reports/{id}"))).await;
    assert_eq!(r.status, 200);
    assert_eq!(body(&r)["votes"], json!(false));
    let missing = format!("/api/v1/lab/reports/{}", "d".repeat(64));
    assert_eq!(call(&store, get(&missing)).await.status, 404);
    assert_eq!(
        call(&store, get("/api/v1/lab/reports/xyz")).await.status,
        400
    );
}

/// Lee el SSE entero (termina cuando el trabajo termina).
async fn read_all(reply: LabEventsReply) -> String {
    let LabEventsReply::Stream(body) = reply else {
        panic!("no abrió el SSE");
    };
    let bytes = tokio::time::timeout(Duration::from_secs(30), body.collect())
        .await
        .expect("el SSE no terminó")
        .unwrap()
        .to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn the_progress_stream_replays_from_the_last_event_and_ends_with_the_job() {
    let store = Mem::default();
    let deps = deps();
    let clients = Arc::new(LabSseClients::default());
    let job = "e".repeat(64);
    let path = format!("/api/v1/lab/jobs/{job}/events");
    store.push(&job, JobState::Queued, vec![]);
    let running = store.push(&job, JobState::Running, vec![]);
    // El final llega mientras el SSE espera (sin Redis: mira PostgreSQL cada sse_poll).
    let late = store.clone();
    let job2 = job.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        late.push(&job2, JobState::Done, vec!["b".repeat(64)]);
    });
    let text =
        read_all(respond_lab_events(&store, &deps, &clients, "GET", &path, None).await).await;
    assert!(text.starts_with("retry: 3000\n\n"), "{text}");
    let events: Vec<Value> = text
        .split("\n\n")
        .filter_map(|block| block.lines().find_map(|l| l.strip_prefix("data: ")))
        .map(|d| serde_json::from_str(d).unwrap())
        .collect();
    let states: Vec<&str> = events
        .iter()
        .map(|e| e["state"].as_str().unwrap())
        .collect();
    assert_eq!(states, ["queued", "running", "done"]);
    for e in &events {
        assert_contract("lab_job_event.v1.schema.json", e);
        assert!(text.contains(&format!("id: {}\ndata: ", e["seq"])));
    }
    // Reconectar con Last-Event-ID: solo lo posterior.
    let seq = running.seq.to_string();
    let again =
        read_all(respond_lab_events(&store, &deps, &clients, "GET", &path, Some(&seq)).await).await;
    assert!(
        !again.contains("\"queued\"") && again.contains("\"done\""),
        "{again}"
    );
    // Después del final, el navegador reconecta con su seq: 204 (no vuelve a intentar).
    let done_seq = store.last(&job).unwrap().seq.to_string();
    match respond_lab_events(&store, &deps, &clients, "GET", &path, Some(&done_seq)).await {
        LabEventsReply::Error { status, .. } => assert_eq!(status, 204),
        LabEventsReply::Stream(_) => panic!("debería cerrar con 204"),
    }
    // Errores.
    let status = |r: LabEventsReply| match r {
        LabEventsReply::Error { status, .. } => status,
        LabEventsReply::Stream(_) => 200,
    };
    let unknown = format!("/api/v1/lab/jobs/{}/events", "f".repeat(64));
    assert_eq!(
        status(respond_lab_events(&store, &deps, &clients, "GET", &unknown, None).await),
        404
    );
    assert_eq!(
        status(respond_lab_events(&store, &deps, &clients, "POST", &path, None).await),
        405
    );
}

#[tokio::test]
async fn the_progress_stream_has_a_client_cap() {
    let store = Mem::default();
    let deps = deps();
    let clients = Arc::new(LabSseClients::default());
    let job = "9".repeat(64);
    store.push(&job, JobState::Running, vec![]);
    let path = format!("/api/v1/lab/jobs/{job}/events");
    let mut open = Vec::new();
    for _ in 0..deps.routes.sse_max_clients {
        match respond_lab_events(&store, &deps, &clients, "GET", &path, None).await {
            LabEventsReply::Stream(b) => open.push(b),
            LabEventsReply::Error { status, .. } => panic!("{status}"),
        }
    }
    match respond_lab_events(&store, &deps, &clients, "GET", &path, None).await {
        LabEventsReply::Error { status, .. } => assert_eq!(status, 503),
        LabEventsReply::Stream(_) => panic!("pasó el tope"),
    }
    drop(open);
}

#[tokio::test]
#[ignore = "necesita Redis: REDIS_URL"]
async fn with_redis_the_progress_stream_wakes_up_at_once() {
    let url = std::env::var("REDIS_URL").expect("falta REDIS_URL");
    let store = Mem::default();
    let mut deps = deps();
    deps.redis_url = Some(url.clone());
    let clients = Arc::new(LabSseClients::default());
    let job = cfds_backtest::lab::sha256_hex(&cfds_storage::test_db::unique_id("LABSSE"));
    store.push(&job, JobState::Running, vec![]);
    let path = format!("/api/v1/lab/jobs/{job}/events");
    let worker = store.clone();
    let job2 = job.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let done = worker.push(&job2, JobState::Done, vec!["b".repeat(64)]);
        let mut r = cfds_storage::LabStream::connect(&url).await.unwrap();
        let data = serde_json::to_string(&done).unwrap();
        r.publish(&job2, done.seq, &data, Duration::from_secs(60), 100)
            .await
            .unwrap();
    });
    // Sin el aviso de Redis esperaría el heartbeat entero (15 s) antes de volver a mirar.
    let started = std::time::Instant::now();
    let text =
        read_all(respond_lab_events(&store, &deps, &clients, "GET", &path, None).await).await;
    assert!(text.contains("\"done\""), "{text}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

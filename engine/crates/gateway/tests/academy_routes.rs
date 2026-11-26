//! Rutas de la Academia del gateway (#719): contratos, anti-CSRF, códigos de error y que cada
//! intento quede guardado antes de responder. Los eventos van a una lista en memoria; la tabla de
//! solo agregar se prueba en `cfds-academy` (`tests/pg.rs`).

use std::path::PathBuf;
use std::sync::Mutex;

use cfds_academy::{AcademyConfig, AcademyEvent, EventBody};
use cfds_gateway::Reply;
use cfds_gateway::academy::{AcademyRoutes, AcademyStore, respond_academy};
use cfds_gateway::paper::HttpIn;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const ACADEMY: &str = include_str!("../../../../config/academy.toml");
const TOKEN: &str = "token-local-de-prueba-123";

/// La config real sin sus preguntas (#760), para que los tests no dependan del contenido.
fn without_questions(text: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        if line == "[[lessons.questions]]" {
            skipping = true;
        } else if line.starts_with('[') {
            skipping = false;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// La config real sin sus preguntas, con dos de prueba en la lección 0.1: 1.1 queda sin preguntas
/// (examen no listo).
fn academy() -> AcademyConfig {
    let anchor = "unlocks = [\"tool.watchlist\", \"tool.instrument_panel\"]\n";
    let questions = r#"
[[lessons.questions]]
id = "0.1.1"
kind = "choice"
prompt = "¿Quién está del otro lado?"
options = [{ id = "a", text = "Nadie" }, { id = "b", text = "Otra parte" }]
answer = "b"

[[lessons.questions]]
id = "0.1.2"
kind = "number"
prompt = "¿Cuánto es 1 + 1?"
answer = "2"
"#;
    let text = without_questions(ACADEMY).replacen(anchor, &format!("{anchor}{questions}"), 1);
    AcademyConfig::from_toml(&text).unwrap()
}

#[derive(Default)]
struct MemStore {
    events: Mutex<Vec<AcademyEvent>>,
    broken: bool,
}

impl AcademyStore for MemStore {
    async fn append(
        &self,
        member_id: &str,
        body: EventBody,
        catalog_version: &str,
        at: DateTime<Utc>,
    ) -> Result<AcademyEvent, String> {
        if self.broken {
            return Err("sin base".into());
        }
        let mut events = self.events.lock().unwrap();
        let e = AcademyEvent {
            seq: i64::try_from(events.len()).unwrap() + 1,
            member_id: member_id.into(),
            body,
            catalog_version: catalog_version.into(),
            recorded_at: at,
        };
        events.push(e.clone());
        Ok(e)
    }

    async fn events(&self, member_id: &str) -> Result<Vec<AcademyEvent>, String> {
        let events = self.events.lock().unwrap();
        Ok(events
            .iter()
            .filter(|e| e.member_id == member_id)
            .cloned()
            .collect())
    }
}

fn now() -> DateTime<Utc> {
    "2026-11-26T22:00:00Z".parse().unwrap()
}

async fn call(
    store: &MemStore,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: &Value,
) -> Reply {
    let routes = AcademyRoutes::from_toml(GATEWAY).unwrap();
    let body = serde_json::to_vec(body).unwrap();
    let req = HttpIn {
        method,
        path,
        token,
        content_type: Some("application/json; charset=utf-8"),
        body: &body,
    };
    respond_academy(store, &academy(), &routes, Some(TOKEN), req, now()).await
}

async fn post(store: &MemStore, path: &str, body: Value) -> Reply {
    call(store, "POST", path, Some(TOKEN), &body).await
}

async fn get(store: &MemStore, path: &str) -> Value {
    let r = call(store, "GET", path, None, &json!(null)).await;
    assert_eq!(r.status, 200, "{}", r.body);
    serde_json::from_str(&r.body).unwrap()
}

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

fn assert_valid(schema_file: &str, doc: &Value) {
    let text = std::fs::read_to_string(contracts().join(schema_file)).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap();
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{schema_file} rechazó {doc}: {errors:?}");
}

fn answer(choice: &str, value: &str) -> Value {
    json!({
        "lesson_id": "0.1",
        "catalog_version": academy().catalog.catalog_version,
        "answers": [{"question_id": "0.1.1", "choice": choice}, {"question_id": "0.1.2", "value": value}],
    })
}

#[test]
fn the_repo_config_has_the_academy_section() {
    let r = AcademyRoutes::from_toml(GATEWAY).unwrap();
    assert_eq!(r.member_id, "owner");
    assert!(
        AcademyRoutes::from_toml("[academy]\nmember_id = \"no válido\"\nmax_body_bytes = 1")
            .is_err()
    );
    assert!(
        AcademyRoutes::from_toml("[academy]\nmember_id = \"owner\"\nmax_body_bytes = 0").is_err()
    );
    AcademyConfig::from_toml(ACADEMY).unwrap();
}

#[tokio::test]
async fn catalog_and_progress_follow_their_contracts() {
    let store = MemStore::default();
    let catalog = get(&store, "/api/v1/academy/catalog").await;
    assert_valid("academy_catalog.v1.schema.json", &catalog);
    assert!(!catalog.to_string().contains("\"answer\""));
    let progress = get(&store, "/api/v1/academy/progress").await;
    assert_valid("academy_progress.v1.schema.json", &progress);
    assert_eq!(progress["member_id"], json!("owner"));
    assert!(
        progress["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["unlocked"] == json!(false))
    );
}

#[tokio::test]
async fn a_quiz_is_graded_saved_and_unlocks_its_tools() {
    let store = MemStore::default();
    let r = post(&store, "/api/v1/academy/quiz/0.1", answer("a", "2")).await;
    assert_eq!(r.status, 200, "{}", r.body);
    let result: Value = serde_json::from_str(&r.body).unwrap();
    assert_valid("quiz_result.v1.schema.json", &result);
    assert_eq!(
        (result["passed"].clone(), result["score_pct"].clone()),
        (json!(false), json!("50"))
    );
    let r = post(&store, "/api/v1/academy/quiz/0.1", answer("b", "2.0")).await;
    assert_eq!(
        serde_json::from_str::<Value>(&r.body).unwrap()["passed"],
        json!(true)
    );
    // Cada intento queda guardado como evento, y el progreso lo refleja.
    let events = store.events.lock().unwrap().clone();
    assert_eq!(events.len(), 2);
    for e in &events {
        assert_valid(
            "academy_event.v1.schema.json",
            &serde_json::to_value(e).unwrap(),
        );
    }
    let p = get(&store, "/api/v1/academy/progress").await;
    assert_eq!(p["lessons"][0]["attempts"], json!(2));
    assert_eq!(
        p["tools"][0],
        json!({"tool_id": "tool.watchlist", "unlocked": true, "by": "lesson", "lesson_id": "0.1"})
    );
}

#[tokio::test]
async fn manual_unlock_and_completed_lesson_are_events() {
    let store = MemStore::default();
    let r = post(
        &store,
        "/api/v1/academy/events",
        json!({"kind": "tool_override", "tool_id": "*", "action": "unlock"}),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_valid(
        "academy_event.v1.schema.json",
        &serde_json::from_str(&r.body).unwrap(),
    );
    let r = post(
        &store,
        "/api/v1/academy/events",
        json!({"kind": "lesson_completed", "lesson_id": "0.3"}),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    let p = get(&store, "/api/v1/academy/progress").await;
    assert_eq!(p["unlock_all"], json!(true));
    assert!(
        p["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["by"] == json!("override"))
    );
    assert_eq!(p["lessons"][2]["passed"], json!(true));
}

#[tokio::test]
async fn bad_requests_get_their_status_and_write_nothing() {
    let store = MemStore::default();
    let stale = {
        let mut a = answer("b", "2");
        a["catalog_version"] = json!("0".repeat(64));
        a
    };
    let mut mismatch = answer("b", "2");
    mismatch["lesson_id"] = json!("0.2");
    let cases: Vec<(&str, &str, Option<&str>, Value, u16)> = vec![
        ("GET", "/api/v1/academy/nada", None, json!(null), 404),
        (
            "POST",
            "/api/v1/academy/catalog",
            Some(TOKEN),
            json!({}),
            405,
        ),
        ("GET", "/api/v1/academy/events", None, json!(null), 405),
        (
            "POST",
            "/api/v1/academy/events",
            None,
            json!({"kind": "tool_override", "tool_id": "*", "action": "unlock"}),
            403,
        ),
        (
            "POST",
            "/api/v1/academy/events",
            Some("otro-token-de-16-chars"),
            json!({}),
            403,
        ),
        (
            "POST",
            "/api/v1/academy/events",
            Some(TOKEN),
            json!({"kind": "tool_override", "tool_id": "tool.nada", "action": "unlock"}),
            400,
        ),
        (
            "POST",
            "/api/v1/academy/events",
            Some(TOKEN),
            json!({"kind": "lesson_completed", "lesson_id": "0.1"}),
            400,
        ),
        (
            "POST",
            "/api/v1/academy/events",
            Some(TOKEN),
            json!({"kind": "exam_attempt", "lesson_id": "0.1"}),
            400,
        ),
        ("POST", "/api/v1/academy/quiz/0.1", Some(TOKEN), stale, 409),
        (
            "POST",
            "/api/v1/academy/quiz/0.1",
            Some(TOKEN),
            mismatch,
            400,
        ),
        (
            "POST",
            "/api/v1/academy/quiz/9.9",
            Some(TOKEN),
            json!({"lesson_id": "9.9", "catalog_version": "0".repeat(64), "answers": []}),
            404,
        ),
        (
            "POST",
            "/api/v1/academy/quiz/1.1",
            Some(TOKEN),
            json!({"lesson_id": "1.1", "catalog_version": academy().catalog.catalog_version, "answers": []}),
            409,
        ),
        (
            "POST",
            "/api/v1/academy/quiz/0.3",
            Some(TOKEN),
            json!({"lesson_id": "0.3", "catalog_version": academy().catalog.catalog_version, "answers": []}),
            400,
        ),
        (
            "POST",
            "/api/v1/academy/quiz/0.1",
            Some(TOKEN),
            json!({"lesson_id": "0.1"}),
            400,
        ),
    ];
    for (method, path, token, body, status) in cases {
        let r = call(&store, method, path, token, &body).await;
        assert_eq!(r.status, status, "{method} {path} {body}: {}", r.body);
    }
    // Sin application/json y con un body grande.
    let routes = AcademyRoutes::from_toml(GATEWAY).unwrap();
    let big = vec![b' '; routes.max_body_bytes + 1];
    for (ct, body, status) in [
        (Some("text/plain"), &b"{}"[..], 415),
        (Some("application/json"), &big[..], 413),
    ] {
        let req = HttpIn {
            method: "POST",
            path: "/api/v1/academy/events",
            token: Some(TOKEN),
            content_type: ct,
            body,
        };
        assert_eq!(
            respond_academy(&store, &academy(), &routes, Some(TOKEN), req, now())
                .await
                .status,
            status
        );
    }
    // Sin token configurado, la escritura está apagada.
    let req = HttpIn {
        method: "POST",
        path: "/api/v1/academy/events",
        token: Some(TOKEN),
        content_type: Some("application/json"),
        body: b"{}",
    };
    assert_eq!(
        respond_academy(&store, &academy(), &routes, None, req, now())
            .await
            .status,
        403
    );
    assert!(store.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn without_the_database_there_is_no_grade() {
    let store = MemStore {
        broken: true,
        ..MemStore::default()
    };
    let r = post(&store, "/api/v1/academy/quiz/0.1", answer("b", "2")).await;
    assert_eq!(r.status, 500);
    assert!(!r.body.contains("passed"));
}

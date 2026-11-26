//! Rutas del replay de la Academia (#723): contratos, cursor, anti-CSRF y códigos. Las velas son
//! sintéticas (de lunes a viernes, 2011–2016); la config es la del repo.

use std::path::PathBuf;

use cfds_academy::AcademyConfig;
use cfds_academy::replay::{BarRange, BarStore};
use cfds_backtest::BacktestConfig;
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_gateway::Reply;
use cfds_gateway::academy::AcademyRoutes;
use cfds_gateway::academy_replay::{ReplayDeps, ReplaySessions, respond_replay};
use cfds_gateway::paper::HttpIn;
use cfds_market_data::Catalog;
use cfds_risk::RiskConfig;
use chrono::{DateTime, Datelike, Utc, Weekday};
use rust_decimal::Decimal;
use serde_json::{Value, json};

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const ACADEMY: &str = include_str!("../../../../config/academy.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const SPLIT: &str = include_str!("../../../../config/data_split.toml");
const PATTERNS: &str = include_str!("../../../../config/patterns.toml");
const TOKEN: &str = "token-local-de-prueba-123";

struct Synthetic;

impl BarStore for Synthetic {
    async fn range(&self, _: &InstrumentId, _: Timeframe) -> Result<Option<BarRange>, String> {
        Ok(Some((
            "2011-01-03T00:00:00Z".parse().unwrap(),
            "2016-12-30T23:00:00Z".parse().unwrap(),
        )))
    }

    async fn bars(
        &self,
        id: &InstrumentId,
        tf: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, String> {
        let mut out = Vec::new();
        let mut t = from;
        while t < to {
            if !matches!(t.weekday(), Weekday::Sat | Weekday::Sun) {
                let open = Decimal::from(4000 + t.timestamp() / 900 % 37);
                out.push(Candle {
                    instrument_id: id.clone(),
                    timeframe: tf,
                    start: t,
                    open,
                    high: open + Decimal::from(4),
                    low: open - Decimal::from(3),
                    close: open + Decimal::ONE,
                    volume: None,
                });
            }
            t += tf.duration();
        }
        Ok(out)
    }
}

fn deps(academy: &AcademyConfig) -> ReplayDeps {
    ReplayDeps::from_config(
        academy,
        &Catalog::from_toml(CATALOG).unwrap(),
        &RiskConfig::from_toml(RISK).unwrap(),
        &BacktestConfig::from_toml(BACKTEST).unwrap(),
        SPLIT,
        PATTERNS,
    )
    .unwrap()
}

struct World {
    academy: AcademyConfig,
    deps: ReplayDeps,
    sessions: ReplaySessions,
}

impl World {
    fn new() -> Self {
        let academy = AcademyConfig::from_toml(ACADEMY).unwrap();
        Self {
            deps: deps(&academy),
            academy,
            sessions: ReplaySessions::default(),
        }
    }

    async fn call(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Value,
        now: &str,
    ) -> Reply {
        let routes = AcademyRoutes::from_toml(GATEWAY).unwrap();
        let body = serde_json::to_vec(&body).unwrap();
        let req = HttpIn {
            method,
            path,
            token,
            content_type: Some("application/json"),
            body: &body,
        };
        let now = now.parse().unwrap();
        respond_replay(
            &Synthetic,
            &self.sessions,
            &self.deps,
            &self.academy,
            &routes,
            Some(TOKEN),
            req,
            now,
        )
        .await
    }

    async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let r = self
            .call("POST", path, Some(TOKEN), body, "2026-11-26T23:00:00Z")
            .await;
        (
            r.status,
            serde_json::from_str(&r.body).unwrap_or(Value::String(r.body)),
        )
    }
}

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

#[test]
fn the_repo_config_builds_the_replay() {
    let academy = AcademyConfig::from_toml(ACADEMY).unwrap();
    let d = deps(&academy);
    assert_eq!(d.costs.len(), 4);
    assert_eq!(d.rules.max_risk_pct.to_string(), "1");
    // Un instrumento que no está en el catálogo no arranca.
    let mut bad = academy.clone();
    bad.replay.instruments.push("NOPE".parse().unwrap());
    let r = ReplayDeps::from_config(
        &bad,
        &Catalog::from_toml(CATALOG).unwrap(),
        &RiskConfig::from_toml(RISK).unwrap(),
        &BacktestConfig::from_toml(BACKTEST).unwrap(),
        SPLIT,
        PATTERNS,
    );
    assert!(r.is_err());
}

#[tokio::test]
async fn a_whole_session_follows_the_contracts_and_never_shows_the_future() {
    let w = World::new();
    let (status, session) = w
        .post(
            "/api/v1/academy/replay",
            json!({"kind": "trades", "seed": "42"}),
        )
        .await;
    assert_eq!(status, 200, "{session}");
    assert_valid("replay_session.v1.schema.json", &session);
    let text = session.to_string();
    for year in 2011..=2016 {
        assert!(
            !text.contains(&format!("{year}-")),
            "la fecha real no viaja"
        );
    }
    let id = session["session_id"].as_str().unwrap().to_owned();
    let cursor = session["cursor"].as_u64().unwrap();
    assert_eq!(session["bars"].as_array().unwrap().len() as u64, cursor + 1);
    let base = format!("/api/v1/academy/replay/{id}");
    let (status, step) = w.post(&format!("{base}/step"), json!({"bars": 4})).await;
    assert_eq!(status, 200, "{step}");
    assert_valid("replay_step.v1.schema.json", &step);
    assert_eq!(step["bars"][0]["index"].as_u64(), Some(cursor + 1));
    assert_eq!(step["cursor"].as_u64(), Some(cursor + 4));
    let (_, mark) = w
        .post(
            &format!("{base}/marks"),
            json!({"kind": "level", "price": "4010"}),
        )
        .await;
    assert_valid("replay_mark.v1.schema.json", &mark);
    let (status, _) = w
        .post(
            &format!("{base}/marks"),
            json!({"kind": "pivot_low", "price": "4000", "bar_index": cursor + 5}),
        )
        .await;
    assert_eq!(status, 400, "pivote en una vela que todavía no se ve");
    let order =
        json!({"side": "long", "stop": "3990", "risk_pct": "1", "plan": "Compro el rebote"});
    let (status, o) = w.post(&format!("{base}/orders"), order).await;
    assert_eq!(status, 200, "{o}");
    assert_valid("replay_order.v1.schema.json", &o);
    let (status, result) = w.post(&format!("{base}/finish"), json!({})).await;
    assert_eq!(status, 200, "{result}");
    assert_valid("exam_result.v1.schema.json", &result);
    assert_eq!(
        result["passed"],
        json!(false),
        "una sola operación de las 10 del examen"
    );
    assert!(
        result["revealed"]["first_bar_at"]
            .as_str()
            .unwrap()
            .starts_with("201")
    );
    // Terminada, la sesión se cierra.
    let (status, _) = w.post(&format!("{base}/step"), json!({"bars": 1})).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn the_same_seed_gives_the_same_bars() {
    let w = World::new();
    let (_, a) = w
        .post(
            "/api/v1/academy/replay",
            json!({"kind": "practice", "seed": "7"}),
        )
        .await;
    let w2 = World::new();
    let (_, b) = w2
        .post(
            "/api/v1/academy/replay",
            json!({"kind": "practice", "seed": "7"}),
        )
        .await;
    assert_eq!(a["bars"], b["bars"]);
    assert_eq!(a["instrument_id"], b["instrument_id"]);
}

#[tokio::test]
async fn bad_requests_get_their_status() {
    let w = World::new();
    let (_, s) = w
        .post("/api/v1/academy/replay", json!({"kind": "levels"}))
        .await;
    let id = s["session_id"].as_str().unwrap().to_owned();
    let base = format!("/api/v1/academy/replay/{id}");
    let cases: Vec<(&str, String, Option<&str>, Value, u16)> = vec![
        (
            "GET",
            "/api/v1/academy/replay".into(),
            Some(TOKEN),
            json!({}),
            405,
        ),
        (
            "POST",
            "/api/v1/academy/replay".into(),
            None,
            json!({"kind": "practice"}),
            403,
        ),
        (
            "POST",
            "/api/v1/academy/replay".into(),
            Some(TOKEN),
            json!({"kind": "exam"}),
            400,
        ),
        (
            "POST",
            "/api/v1/academy/replay".into(),
            Some(TOKEN),
            json!({"kind": "practice", "seed": "01"}),
            400,
        ),
        (
            "POST",
            "/api/v1/academy/replay".into(),
            Some(TOKEN),
            json!({"kind": "trades", "lesson_id": "0.1"}),
            400,
        ),
        ("POST", format!("{base}/nada"), Some(TOKEN), json!({}), 404),
        (
            "POST",
            "/api/v1/academy/replay/rp-0000000000000000/step".into(),
            Some(TOKEN),
            json!({"bars": 1}),
            404,
        ),
        (
            "POST",
            format!("{base}/step"),
            Some(TOKEN),
            json!({"bars": 0}),
            400,
        ),
        (
            "POST",
            format!("{base}/step"),
            Some(TOKEN),
            json!({"bars": 1, "date": true}),
            400,
        ),
        (
            "POST",
            format!("{base}/orders"),
            Some(TOKEN),
            json!({"side": "long", "risk_pct": "0", "plan": "x"}),
            400,
        ),
        (
            "POST",
            "/api/v1/academy/replay/x".into(),
            Some(TOKEN),
            json!({}),
            404,
        ),
    ];
    for (method, path, token, body, status) in cases {
        let r = w
            .call(method, &path, token, body.clone(), "2026-11-26T23:00:00Z")
            .await;
        assert_eq!(r.status, status, "{method} {path} {body}: {}", r.body);
    }
    // Sin vela siguiente no se puede entrar.
    let (_, _) = w.post(&format!("{base}/step"), json!({"bars": 1000})).await;
    for _ in 0..10 {
        w.post(&format!("{base}/step"), json!({"bars": 1000})).await;
    }
    let (status, _) = w
        .post(
            &format!("{base}/orders"),
            json!({"side": "short", "risk_pct": "1", "plan": "x"}),
        )
        .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn the_oldest_session_is_closed_when_the_memory_is_full() {
    let w = World::new();
    let max = w.academy.replay.max_sessions;
    let mut ids = Vec::new();
    for i in 0..=max {
        let now = format!("2026-11-26T23:00:{:02}Z", i);
        let r = w
            .call(
                "POST",
                "/api/v1/academy/replay",
                Some(TOKEN),
                json!({"kind": "practice"}),
                &now,
            )
            .await;
        let v: Value = serde_json::from_str(&r.body).unwrap();
        ids.push(v["session_id"].as_str().unwrap().to_owned());
    }
    let (first, _) = w
        .post(
            &format!("/api/v1/academy/replay/{}/step", ids[0]),
            json!({"bars": 1}),
        )
        .await;
    let (last, _) = w
        .post(
            &format!("/api/v1/academy/replay/{}/step", ids[max]),
            json!({"bars": 1}),
        )
        .await;
    assert_eq!((first, last), (404, 200));
}

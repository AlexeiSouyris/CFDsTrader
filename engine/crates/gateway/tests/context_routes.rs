//! Rutas del contexto de mercado del Panel (#294): sentimiento y análogos, solo lectura.
//! Números sintéticos.

use std::path::PathBuf;

use cfds_core::{AnalogReport, InstrumentId, SentimentIndex};
use cfds_gateway::context::{ContextConfig, ContextSource, respond_context};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");

fn sentiment(id: &str, value: &str, published: &str) -> SentimentIndex {
    serde_json::from_value(json!({
        "instrument_id": id,
        "as_of": "2026-11-20T21:00:00Z",
        "published_at": published,
        "value": value,
        "warmup_complete": true,
        "min_components": 3,
        "components": [{
            "name": "cot", "status": "valid", "weight": "1",
            "observed_for": "2026-11-17T00:00:00Z", "published_at": "2026-11-20T21:00:00Z",
            "raw": "0.1", "percentile": "0.6", "component": "0.2"
        }]
    }))
    .unwrap()
}

fn report(id: &str) -> AnalogReport {
    let group = |name: &str, count: u32| {
        json!({
            "group": name, "count": count, "insufficient": count < 20,
            "horizons": [{
                "horizon": 5,
                "percentiles": [{ "p": 10, "return": "-0.03" }, { "p": 50, "return": "0.004" }, { "p": 90, "return": "0.027" }],
                "positive_pct": "55"
            }],
            "hits": { "up": 6, "down": 9, "none": 5 }, "without_atr": 0
        })
    };
    serde_json::from_value(json!({
        "instrument_id": id, "source": "futures", "window_end_day": "2026-11-20",
        "as_of": "2026-11-20T22:00:00Z", "computed_at": "2026-11-23T12:00:00Z",
        "window": 20, "min_neighbors": 20,
        "context": { "vol": "mid", "rates": "estable" },
        "neighbors": [],
        "groups": [group("all", 20), group("same_context", 7)]
    }))
    .unwrap()
}

/// Lo que habría en la base: dos instrumentos con sentimiento y análogos.
struct Fake;

impl ContextSource for Fake {
    async fn latest_sentiment(&self, _: DateTime<Utc>) -> Result<Vec<SentimentIndex>, String> {
        Ok(vec![
            sentiment("US100.FUT", "0.2", "2026-11-20T23:00:00Z"),
            sentiment("US500.FUT", "-0.35", "2026-11-20T23:00:00Z"),
        ])
    }
    async fn sentiment_history(
        &self,
        id: &InstrumentId,
        _: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<SentimentIndex>, String> {
        let all = vec![
            sentiment(id.as_str(), "-0.1", "2026-11-18T23:00:00Z"),
            sentiment(id.as_str(), "-0.2", "2026-11-19T23:00:00Z"),
            sentiment(id.as_str(), "-0.35", "2026-11-20T23:00:00Z"),
        ];
        let skip = all.len().saturating_sub(limit as usize);
        Ok(all.into_iter().skip(skip).collect())
    }
    async fn latest_analogs(&self, _: DateTime<Utc>) -> Result<Vec<AnalogReport>, String> {
        Ok(vec![report("US100.FUT"), report("US500.FUT")])
    }
}

fn now() -> DateTime<Utc> {
    "2026-11-23T12:00:00Z".parse().unwrap()
}

async fn get(path: &str, query: Option<&str>) -> (u16, Value) {
    let cfg = ContextConfig::from_toml(GATEWAY).unwrap();
    let r = respond_context(&Fake, &cfg, "GET", path, query, now()).await;
    let body = serde_json::from_str(&r.body).unwrap_or(Value::Null);
    (r.status, body)
}

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn assert_all_valid(schema: &str, docs: &Value) {
    let v = validator(schema);
    for d in docs.as_array().unwrap() {
        let errors: Vec<String> = v.iter_errors(d).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{schema}: {errors:?}");
    }
}

#[tokio::test]
async fn latest_sentiment_of_each_instrument() {
    let (status, body) = get("/api/v1/sentiment", None).await;
    assert_eq!(status, 200);
    assert_all_valid("sentiment.v1.schema.json", &body);
    assert_eq!(body.as_array().unwrap().len(), 2);
    assert_eq!(body[1]["value"], "-0.35");
}

#[tokio::test]
async fn sentiment_history_of_one_instrument_in_order() {
    let (status, body) = get("/api/v1/sentiment", Some("instrument=US500.FUT")).await;
    assert_eq!(status, 200);
    assert_all_valid("sentiment.v1.schema.json", &body);
    let values: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["value"].as_str().unwrap())
        .collect();
    assert_eq!(values, ["-0.1", "-0.2", "-0.35"]);
}

#[tokio::test]
async fn analogs_all_or_one_instrument() {
    let (status, body) = get("/api/v1/analogs", None).await;
    assert_eq!(status, 200);
    assert_all_valid("analog_report.v1.schema.json", &body);
    assert_eq!(body.as_array().unwrap().len(), 2);
    let (_, one) = get("/api/v1/analogs", Some("instrument=US500.FUT")).await;
    assert_eq!(one.as_array().unwrap().len(), 1);
    assert_eq!(one[0]["groups"][1]["insufficient"], true);
    let (_, none) = get("/api/v1/analogs", Some("instrument=DE40.FUT")).await;
    assert_eq!(none, json!([]));
}

#[tokio::test]
async fn only_get_and_valid_instruments() {
    let cfg = ContextConfig::from_toml(GATEWAY).unwrap();
    let post = respond_context(&Fake, &cfg, "POST", "/api/v1/sentiment", None, now()).await;
    assert_eq!(post.status, 405);
    assert_eq!(
        get("/api/v1/sentiment", Some("instrument=us500;drop"))
            .await
            .0,
        400
    );
    let other = respond_context(&Fake, &cfg, "GET", "/api/v1/otra", None, now()).await;
    assert_eq!(other.status, 404);
}

#[test]
fn the_history_size_comes_from_config() {
    let cfg = ContextConfig::from_toml(GATEWAY).unwrap();
    assert!(cfg.sentiment_history >= 1);
    let zero = GATEWAY.replace("sentiment_history = 400", "sentiment_history = 0");
    assert_ne!(zero, GATEWAY);
    assert!(ContextConfig::from_toml(&zero).is_err());
}

/// Las lecturas de PostgreSQL arman documentos válidos de los dos contratos.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_store_reads_sentiment_and_analogs_as_their_contracts() {
    use cfds_core::{AssetClass, Currency, Instrument};
    use cfds_storage::PgStore;

    let url = cfds_storage::test_db::test_database_url();
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id: InstrumentId = format!("ITCX{nanos}.FUT").parse().unwrap();
    store
        .upsert_instrument(&Instrument {
            id: id.clone(),
            name: "Test contexto".into(),
            asset_class: AssetClass::Index,
            currency: Currency::USD,
            exchange_timezone: "America/New_York".into(),
        })
        .await
        .unwrap();
    // El 19 lleva 32 decimales, como los que guardó Python antes de #372: se leen con 28.
    let long = "-0.00004546281142025822876886693333";
    for (day, value) in [("18", "-0.1"), ("19", long), ("20", "-0.35")] {
        sqlx::query(
            "INSERT INTO sentiment_index (instrument_id, as_of, published_at, value,
               warmup_complete, min_components, components)
             VALUES ($1, $2::timestamptz, $2::timestamptz + interval '2 hours', $3::numeric,
               true, 3, $4::jsonb)",
        )
        .bind(id.as_str())
        .bind(format!("2026-11-{day}T21:00:00Z"))
        .bind(value)
        .bind(
            serde_json::to_value(
                &sentiment(id.as_str(), "-0.2", "2026-11-20T23:00:00Z").components,
            )
            .unwrap(),
        )
        .execute(store.pool())
        .await
        .unwrap();
    }
    let doc = serde_json::to_value(report(id.as_str())).unwrap();
    sqlx::query(
        "INSERT INTO analog_reports (instrument_id, source, window_end_day, as_of, computed_at,
           document)
         VALUES ($1, 'futures', '2026-11-20', '2026-11-20T22:00:00Z', '2026-11-23T12:00:00Z', $2)",
    )
    .bind(id.as_str())
    .bind(&doc)
    .execute(store.pool())
    .await
    .unwrap();

    let history = store.sentiment_history(&id, now(), 2).await.unwrap();
    let values: Vec<String> = history.iter().map(|s| s.value.to_string()).collect();
    assert_eq!(
        values,
        ["-0.0000454628114202582287688669", "-0.35"],
        "las últimas 2, de la más vieja a la más nueva"
    );
    let latest = store.latest_sentiment(now()).await.unwrap();
    let mine = latest.iter().find(|s| s.instrument_id == id).unwrap();
    assert_eq!(mine.value.to_string(), "-0.35");
    // Nada publicado después de `at`.
    let before: DateTime<Utc> = "2026-11-19T00:00:00Z".parse().unwrap();
    let early = store.sentiment_history(&id, before, 10).await.unwrap();
    assert_eq!(early.len(), 1);
    let analogs = store.latest_analog_reports(now()).await.unwrap();
    let r = analogs.iter().find(|r| r.instrument_id == id).unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), doc);
    assert!(
        store
            .latest_analog_reports(before)
            .await
            .unwrap()
            .iter()
            .all(|r| r.instrument_id != id)
    );
}

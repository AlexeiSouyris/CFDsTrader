//! `GET /api/v1/news` y `GET /api/v1/calendar` (#600): noticias con sus interpretaciones y el
//! calendario, solo lectura, validados con los contratos. Datos sintéticos.

use std::path::PathBuf;

use cfds_core::{EconomicEvent, NewsInterpretation};
use cfds_gateway::news::{NewsConfig, NewsSource, attach, is_news_route, respond_news};
use cfds_storage::{NewsItemRow, OfficialReleaseRow};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");

fn t(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn cfg() -> NewsConfig {
    NewsConfig::from_toml(GATEWAY).unwrap()
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
        let text = std::fs::read_to_string(contracts().join(name))?;
        Ok(serde_json::from_str(&text)?)
    }
}

fn assert_valid(name: &str, doc: &Value) {
    let text = std::fs::read_to_string(contracts().join(name)).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&serde_json::from_str(&text).unwrap())
        .unwrap();
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(
        errors.is_empty(),
        "no valida contra {name}: {errors:?}\n{doc}"
    );
}

fn item(source: &str, feed: &str, id: &str, published: &str) -> NewsItemRow {
    NewsItemRow {
        source: source.into(),
        feed: feed.into(),
        item_id: id.into(),
        title: format!("Titular {id}"),
        url: format!("https://www.example.org/{id}"),
        published_at: t(published),
        retrieved_at: t(published) + TimeDelta::minutes(3),
    }
}

fn interpretation(
    source: &str,
    id: &str,
    published: &str,
    interpreted: &str,
) -> NewsInterpretation {
    serde_json::from_value(json!({
        "interpretation_id": format!("{:0>64}", id.len()),
        "source": source,
        "item_id": id,
        "published_at": published,
        "interpreted_at": interpreted,
        "interpreter": {
            "name": "ollama", "version": "p2.test", "model": "modelo:1", "prompt_sha256": null
        },
        "summary_es": "Resumen automático.",
        "impacts": [{ "instrument_id": "US500", "sentiment": "-0.15", "relevance": "0.95" }]
    }))
    .unwrap()
}

fn event(name: &str, scheduled: &str, published: &str) -> EconomicEvent {
    serde_json::from_value(json!({
        "source": "forex_factory", "event": name, "country": "US", "instrument_id": null,
        "impact": "high", "scheduled_at": scheduled, "published_at": published
    }))
    .unwrap()
}

fn release(status: &str, scheduled: Option<&str>, published: &str) -> OfficialReleaseRow {
    OfficialReleaseRow {
        source: "fed_fomc".into(),
        series: "fomc_statement".into(),
        reference_period: "2025-12-10".into(),
        status: status.into(),
        scheduled_at: scheduled.map(t),
        published_at: t(published),
        time_rule: "release_page".into(),
        with_press_conference: Some(true),
        source_url: "https://www.federalreserve.gov/x.htm".into(),
    }
}

/// Lo que habría en la base. Aplica los mismos filtros que la consulta.
struct Fake {
    items: Vec<NewsItemRow>,
    interpretations: Vec<NewsInterpretation>,
    events: Vec<EconomicEvent>,
    releases: Vec<OfficialReleaseRow>,
}

fn fake() -> Fake {
    Fake {
        items: vec![
            item("fed", "press_monetary", "a", "2026-11-24T19:00:00Z"),
            item("ecb", "press", "b", "2026-11-24T19:00:00Z"),
            item("fed", "speeches", "c", "2026-11-23T15:00:00Z"),
            item("ecb", "press", "d", "2026-11-20T09:00:00Z"),
            // Publicada después de "ahora": no existe todavía.
            item("fed", "press_all", "futura", "2026-11-26T12:00:00Z"),
        ],
        interpretations: vec![
            interpretation("fed", "a", "2026-11-24T19:00:00Z", "2026-11-24T19:05:00Z"),
            // Hecha después de "ahora": no sale.
            interpretation("ecb", "b", "2026-11-24T19:00:00Z", "2026-11-26T00:00:00Z"),
        ],
        events: vec![
            event("Empleo", "2026-11-27T13:30:00Z", "2026-11-23T10:00:00Z"),
            event(
                "Visto tarde",
                "2026-11-27T15:00:00Z",
                "2026-11-26T10:00:00Z",
            ),
        ],
        releases: vec![
            release(
                "scheduled",
                Some("2026-11-26T19:00:00Z"),
                "2026-01-01T00:00:00Z",
            ),
            release(
                "scheduled",
                Some("2025-12-10T19:00:00Z"),
                "2025-01-01T00:00:00Z",
            ),
        ],
    }
}

const NOW: &str = "2026-11-25T12:00:00Z";

impl NewsSource for Fake {
    async fn news_page(
        &self,
        feed: Option<&str>,
        before: Option<DateTime<Utc>>,
        until: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<NewsItemRow>, String> {
        let mut ok: Vec<NewsItemRow> = self
            .items
            .iter()
            .filter(|i| i.published_at <= until)
            .filter(|i| before.is_none_or(|b| i.published_at < b))
            .filter(|i| feed.is_none_or(|f| i.feed == f))
            .cloned()
            .collect();
        ok.sort_by(|a, b| {
            b.published_at
                .cmp(&a.published_at)
                .then(a.source.cmp(&b.source))
        });
        // Como la consulta: hasta `limit`, más las que comparten hora con la última.
        let Some(cut) = ok
            .get((limit as usize).wrapping_sub(1))
            .map(|i| i.published_at)
        else {
            return Ok(if limit == 0 { Vec::new() } else { ok });
        };
        Ok(ok.into_iter().filter(|i| i.published_at >= cut).collect())
    }
    async fn interpretations_of(
        &self,
        items: &[(String, String)],
        until: DateTime<Utc>,
    ) -> Result<Vec<NewsInterpretation>, String> {
        Ok(self
            .interpretations
            .iter()
            .filter(|i| i.interpreted_at <= until)
            .filter(|i| items.contains(&(i.source.clone(), i.item_id.clone())))
            .cloned()
            .collect())
    }
    async fn economic_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<EconomicEvent>, String> {
        Ok(self
            .events
            .iter()
            .filter(|e| e.scheduled_at >= from && e.scheduled_at <= to && e.published_at <= until)
            .cloned()
            .collect())
    }
    async fn official_releases(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<OfficialReleaseRow>, String> {
        Ok(self
            .releases
            .iter()
            .filter(|r| r.scheduled_at.is_some_and(|s| s >= from && s <= to))
            .filter(|r| r.published_at <= until)
            .cloned()
            .collect())
    }
}

async fn get(store: &Fake, path: &str, query: Option<&str>) -> (u16, Value) {
    let r = respond_news(store, &cfg(), "GET", path, query, t(NOW)).await;
    let body = if r.status == 200 {
        serde_json::from_str(&r.body).unwrap()
    } else {
        Value::String(r.body)
    };
    (r.status, body)
}

fn ids(body: &Value) -> Vec<&str> {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["item"]["item_id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn news_are_newest_first_with_their_interpretations_and_follow_the_contract() {
    let store = fake();
    let (status, body) = get(&store, "/api/v1/news", None).await;
    assert_eq!(status, 200);
    assert_valid("gateway_news.v1.schema.json", &body);
    // La publicada después de ahora no sale.
    assert_eq!(ids(&body), ["b", "a", "c", "d"]);
    assert_eq!(body["as_of"], NOW);
    assert_eq!(body["feed"], Value::Null);
    assert_eq!(body["next_before"], Value::Null);
    let a = &body["items"][1];
    assert_eq!(a["item"]["published_at"], "2026-11-24T19:00:00Z");
    // La interpretación va tal cual la guardó Python (decimales como texto).
    assert_eq!(a["interpretations"][0]["impacts"][0]["sentiment"], "-0.15");
    assert_eq!(a["interpretations"][0]["summary_es"], "Resumen automático.");
    // La de "b" todavía no existía a as_of.
    assert_eq!(body["items"][0]["interpretations"], json!([]));
}

#[tokio::test]
async fn pages_never_skip_news_that_share_a_publication_time() {
    let store = fake();
    // limit=1: la primera comparte hora con otra, así que vienen las dos.
    let (_, first) = get(&store, "/api/v1/news", Some("limit=1")).await;
    assert_valid("gateway_news.v1.schema.json", &first);
    assert_eq!(ids(&first), ["b", "a"]);
    assert_eq!(first["next_before"], "2026-11-24T19:00:00Z");
    let q = "limit=1&before=2026-11-24T19%3A00%3A00Z";
    let (_, second) = get(&store, "/api/v1/news", Some(q)).await;
    assert_eq!(ids(&second), ["c"]);
    assert_eq!(second["next_before"], "2026-11-23T15:00:00Z");
    let (_, last) = get(
        &store,
        "/api/v1/news",
        Some("limit=5&before=2026-11-23T15:00:00Z"),
    )
    .await;
    assert_eq!(ids(&last), ["d"]);
    assert_eq!(last["next_before"], Value::Null);
}

#[tokio::test]
async fn feed_filters_and_bad_params_are_rejected() {
    let store = fake();
    let (status, body) = get(&store, "/api/v1/news", Some("feed=press")).await;
    assert_eq!(status, 200);
    assert_eq!(ids(&body), ["b", "d"]);
    assert_eq!(body["feed"], "press");
    for q in [
        "limit=0",
        "limit=201",
        "limit=x",
        "before=2026-11-24",
        "before=2026-11-24T19:00:00+01:00",
        "feed=Press",
        "feed=a-b",
    ] {
        let (status, _) = get(&store, "/api/v1/news", Some(q)).await;
        assert_eq!(status, 400, "{q}");
    }
    let r = respond_news(&store, &cfg(), "POST", "/api/v1/news", None, t(NOW)).await;
    assert_eq!(r.status, 405);
    let r = respond_news(&store, &cfg(), "GET", "/api/v1/newsx", None, t(NOW)).await;
    assert_eq!(r.status, 404);
    assert!(is_news_route("/api/v1/news") && is_news_route("/api/v1/calendar"));
    assert!(!is_news_route("/api/v1/news/"));
}

#[tokio::test]
async fn calendar_reads_both_tables_only_once_published() {
    let store = fake();
    let (status, body) = get(&store, "/api/v1/calendar", None).await;
    assert_eq!(status, 200);
    assert_valid("gateway_calendar.v1.schema.json", &body);
    // Por defecto: desde un día antes de ahora hasta 14 días después.
    assert_eq!(body["from"], "2026-11-24T12:00:00Z");
    assert_eq!(body["to"], "2026-12-09T12:00:00Z");
    let events = body["economic_events"].as_array().unwrap();
    // "Visto tarde" se publicó después de ahora: no sale.
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "Empleo");
    assert_eq!(events[0]["country"], "US");
    assert_eq!(events[0]["instrument_id"], Value::Null);
    let releases = body["official_releases"].as_array().unwrap();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0]["scheduled_at"], "2026-11-26T19:00:00Z");
    assert_eq!(releases[0]["with_press_conference"], true);

    let q = "from=2025-12-01T00:00:00Z&to=2025-12-31T00:00:00Z";
    let (_, past) = get(&store, "/api/v1/calendar", Some(q)).await;
    assert_valid("gateway_calendar.v1.schema.json", &past);
    assert_eq!(
        past["official_releases"][0]["reference_period"],
        "2025-12-10"
    );
    assert_eq!(past["economic_events"], json!([]));
    // Solo `from`: termina 14 días después de from (o de ahora, si from es anterior).
    let (_, from_only) = get(
        &store,
        "/api/v1/calendar",
        Some("from=2026-12-01T00:00:00Z"),
    )
    .await;
    assert_eq!(from_only["to"], "2026-12-15T00:00:00Z");
}

#[tokio::test]
async fn calendar_rejects_bad_ranges() {
    let store = fake();
    for q in [
        "from=2026-11-30T00:00:00Z&to=2026-11-01T00:00:00Z",
        "from=2020-01-01T00:00:00Z&to=2026-01-01T00:00:00Z",
        "from=ayer",
        "to=2026-11-30",
    ] {
        let (status, _) = get(&store, "/api/v1/calendar", Some(q)).await;
        assert_eq!(status, 400, "{q}");
    }
}

#[test]
fn attach_keeps_each_interpretation_with_its_own_news() {
    let entries = attach(
        vec![
            item("fed", "press", "x", "2026-11-24T19:00:00Z"),
            item("ecb", "press", "x", "2026-11-24T19:00:00Z"),
        ],
        vec![interpretation(
            "ecb",
            "x",
            "2026-11-24T19:00:00Z",
            "2026-11-24T20:00:00Z",
        )],
    );
    assert!(entries[0].interpretations.is_empty());
    assert_eq!(entries[1].interpretations.len(), 1);
}

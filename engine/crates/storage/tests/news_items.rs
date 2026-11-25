//! Lectura de noticias e interpretaciones para la UI desde PostgreSQL **real** (#600). Necesita
//! `DATABASE_URL` de una base de pruebas (`cfds_test…`); está marcado `#[ignore]` y el CI lo corre
//! con `-- --ignored`. Las tablas son de solo agregar y las escribe Python: el test inserta con SQL
//! directo, con una fuente propia por corrida, y no borra nada.

use cfds_storage::PgStore;
use chrono::{DateTime, Utc};

async fn store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let s = PgStore::connect(&url).await.unwrap();
    s.migrate().await.unwrap();
    s
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

async fn put(s: &PgStore, source: &str, feed: &str, id: &str, published: &str) {
    sqlx::query(
        "INSERT INTO news_items (source, feed, item_id, title, url, published_at, retrieved_at)
         VALUES ($1, $2, $3, 'Titular', 'https://www.example.org/x', $4, $4)",
    )
    .bind(source)
    .bind(feed)
    .bind(id)
    .bind(at(published))
    .execute(s.pool())
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn news_pages_keep_ties_and_interpretations_appear_only_once_made() {
    let s = store().await;
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let source = format!("it{nanos}");
    // Un feed propio por corrida: la base de pruebas puede tener noticias de otras corridas.
    let feed = format!("f{nanos}");
    put(&s, &source, &feed, "a", "2090-01-02T10:00:00Z").await;
    put(&s, &source, &feed, "b", "2090-01-02T10:00:00Z").await;
    put(&s, &source, &feed, "c", "2090-01-01T10:00:00Z").await;
    sqlx::query(
        "INSERT INTO news_interpretations
            (interpretation_id, source, item_id, published_at, interpreted_at, interpreter_name,
             interpreter_version, interpreter_model, interpreter_prompt_sha256, summary_es, impacts)
         VALUES (encode(sha256($1::bytea), 'hex'), $2, 'a', $3, $4, 'ollama', 'p2.test', NULL,
                 NULL, 'Resumen.', '[{\"instrument_id\": \"US500\", \"sentiment\": \"-0.15\",
                 \"relevance\": \"0.95\"}]'::jsonb)",
    )
    .bind(source.as_bytes())
    .bind(&source)
    .bind(at("2090-01-02T10:00:00Z"))
    .bind(at("2090-01-02T11:00:00Z"))
    .execute(s.pool())
    .await
    .unwrap();

    let until = at("2090-02-01T00:00:00Z");
    let ids = |v: Vec<cfds_storage::NewsItemRow>| -> Vec<String> {
        v.into_iter().map(|r| r.item_id).collect()
    };
    // limit = 1, pero "a" y "b" comparten hora: vienen las dos (más nueva primero).
    let first = s.news_page(Some(&feed), None, until, 1).await.unwrap();
    assert_eq!(ids(first), ["a", "b"]);
    let next = s
        .news_page(Some(&feed), Some(at("2090-01-02T10:00:00Z")), until, 1)
        .await
        .unwrap();
    assert_eq!(ids(next), ["c"]);
    // Nada aparece antes de publicarse.
    let early = s
        .news_page(Some(&feed), None, at("2090-01-01T09:59:59Z"), 10)
        .await
        .unwrap();
    assert!(early.is_empty());
    assert!(
        s.news_page(Some(&feed), None, until, 0)
            .await
            .unwrap()
            .is_empty()
    );

    let keys = [
        (source.clone(), "a".to_owned()),
        (source.clone(), "c".to_owned()),
    ];
    let before = s
        .news_interpretations_of(&keys, at("2090-01-02T10:59:59Z"))
        .await
        .unwrap();
    assert!(before.is_empty());
    let after = s
        .news_interpretations_of(&keys, at("2090-01-02T11:00:00Z"))
        .await
        .unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].item_id, "a");
    assert_eq!(after[0].impacts[0].sentiment.to_string(), "-0.15");
    assert!(
        s.news_interpretations_of(&[], until)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn official_releases_between_reads_every_series_by_time() {
    let s = store().await;
    let nanos = Utc::now().timestamp_nanos_opt().unwrap();
    let day = 1 + nanos.rem_euclid(28);
    let period = format!("2091-02-{day:02}");
    let scheduled = at(&format!("{period}T19:00:00Z"));
    sqlx::query(
        "INSERT INTO official_releases
            (source, series, reference_period, status, scheduled_at, published_at, time_rule,
             with_press_conference, source_url)
         VALUES ('fed_fomc', 'fomc_statement', $1, 'scheduled', $2, $3, 'release_page', false,
                 'https://www.federalreserve.gov/y.htm')
         ON CONFLICT DO NOTHING",
    )
    .bind(&period)
    .bind(scheduled)
    .bind(at("2091-01-01T00:00:00Z"))
    .execute(s.pool())
    .await
    .unwrap();
    let from = at("2091-02-01T00:00:00Z");
    let to = at("2091-03-01T00:00:00Z");
    let mine = |v: Vec<cfds_storage::OfficialReleaseRow>| {
        v.into_iter()
            .filter(|r| r.reference_period == period)
            .collect::<Vec<_>>()
    };
    assert!(
        mine(
            s.official_releases_between(from, to, at("2090-12-31T23:59:59Z"))
                .await
                .unwrap()
        )
        .is_empty()
    );
    let rows = mine(
        s.official_releases_between(from, to, at("2091-01-01T00:00:00Z"))
            .await
            .unwrap(),
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source_url, "https://www.federalreserve.gov/y.htm");
    assert_eq!(rows[0].scheduled_at, Some(scheduled));
}

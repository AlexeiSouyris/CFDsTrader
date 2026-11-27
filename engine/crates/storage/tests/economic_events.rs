//! Lectura del calendario económico desde PostgreSQL **real** (#243). Necesita `DATABASE_URL`;
//! está marcado `#[ignore]` y el CI lo corre con `-- --ignored`. La tabla es de solo agregar y
//! la escribe Python: el test inserta con SQL directo, como `cfds_intel.calendar`, con una
//! fuente propia por corrida.

use cfds_core::{EconomicEvent, Impact};
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

async fn put(s: &PgStore, source: &str, event: &str, impact: &str, scheduled: &str, seen: &str) {
    sqlx::query(
        "INSERT INTO economic_events
            (source, event, country, instrument_id, impact, scheduled_at, published_at)
         VALUES ($1, $2, 'US', NULL, $3, $4, $5)",
    )
    .bind(source)
    .bind(event)
    .bind(impact)
    .bind(at(scheduled))
    .bind(at(seen))
    .execute(s.pool())
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn events_are_read_only_once_published_and_validated_by_the_contract() {
    let s = store().await;
    // La fuente no admite guiones: un número propio.
    let source = format!("it{}", cfds_storage::test_db::unique_number());
    put(
        &s,
        &source,
        "Empleo",
        "high",
        "2026-11-27T13:30:00Z",
        "2026-11-23T10:00:07Z",
    )
    .await;
    // La fuente movió la hora: otra fila, vista más tarde (ADR 0006 §3).
    put(
        &s,
        &source,
        "Empleo",
        "high",
        "2026-11-27T14:00:00Z",
        "2026-11-25T18:17:03Z",
    )
    .await;
    put(
        &s,
        &source,
        "Menor",
        "low",
        "2026-11-26T09:00:00Z",
        "2026-11-23T10:00:07Z",
    )
    .await;
    // Fuera del período pedido.
    put(
        &s,
        &source,
        "Lejos",
        "high",
        "2026-12-20T13:30:00Z",
        "2026-11-23T10:00:07Z",
    )
    .await;

    let mine = |v: Vec<EconomicEvent>| -> Vec<EconomicEvent> {
        v.into_iter().filter(|e| e.source == source).collect()
    };
    let (from, to) = (at("2026-11-23T00:00:00Z"), at("2026-11-28T00:00:00Z"));
    // El martes solo se conocían los de la primera lectura: el cambio de hora no existía.
    let tuesday = mine(
        s.economic_events(from, to, at("2026-11-24T00:00:00Z"))
            .await
            .unwrap(),
    );
    let expected = |event: &str, impact: Impact, scheduled: &str, seen: &str| EconomicEvent {
        source: source.clone(),
        event: event.into(),
        country: Some("US".into()),
        instrument_id: None,
        impact,
        scheduled_at: at(scheduled),
        published_at: at(seen),
    };
    assert_eq!(
        tuesday,
        vec![
            expected(
                "Menor",
                Impact::Low,
                "2026-11-26T09:00:00Z",
                "2026-11-23T10:00:07Z"
            ),
            expected(
                "Empleo",
                Impact::High,
                "2026-11-27T13:30:00Z",
                "2026-11-23T10:00:07Z"
            ),
        ]
    );
    let friday = mine(
        s.economic_events(from, to, at("2026-11-27T00:00:00Z"))
            .await
            .unwrap(),
    );
    assert_eq!(friday.len(), 3);
    assert_eq!(friday[2].scheduled_at, at("2026-11-27T14:00:00Z"));

    // Las lecturas de la ingesta, también sin mirar el futuro.
    let seen = s
        .economic_event_sightings(at("2026-11-24T00:00:00Z"))
        .await
        .unwrap();
    assert!(seen.contains(&at("2026-11-23T10:00:07Z")));
    assert!(!seen.contains(&at("2026-11-25T18:17:03Z")));
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "distintas y en orden");
}

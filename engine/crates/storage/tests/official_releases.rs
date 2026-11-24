//! Lectura del calendario histórico oficial desde PostgreSQL **real** (ADR 0016, #526). Necesita
//! `DATABASE_URL`; está marcado `#[ignore]` y el CI lo corre con `-- --ignored`. La tabla es de
//! solo agregar y la escribe Python: el test inserta con SQL directo una reunión con una fecha
//! propia por corrida (lejos en el futuro) y no borra nada.

use cfds_storage::PgStore;
use chrono::{DateTime, Days, NaiveDate, Utc};

async fn store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let s = PgStore::connect(&url).await.unwrap();
    s.migrate().await.unwrap();
    s
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn fomc_rows_are_read_only_once_published() {
    let s = store().await;
    let nanos = Utc::now().timestamp_nanos_opt().unwrap();
    let offset = u64::try_from(nanos.rem_euclid(3000)).unwrap();
    let meeting = NaiveDate::from_ymd_opt(2090, 1, 1)
        .unwrap()
        .checked_add_days(Days::new(offset))
        .unwrap();
    let scheduled: DateTime<Utc> = meeting.and_hms_opt(19, 0, 0).unwrap().and_utc();
    let published = scheduled - chrono::TimeDelta::days(300);
    let period = meeting.format("%Y-%m-%d").to_string();
    sqlx::query(
        "INSERT INTO official_releases
            (source, series, reference_period, status, scheduled_at, published_at, time_rule,
             with_press_conference, source_url)
         VALUES ('fed_fomc', 'fomc_statement', $1, 'scheduled', $2, $3, 'release_page', true,
                 'https://www.federalreserve.gov/x.htm')
         ON CONFLICT DO NOTHING",
    )
    .bind(&period)
    .bind(scheduled)
    .bind(published)
    .execute(s.pool())
    .await
    .unwrap();
    let mine = |rows: Vec<cfds_storage::OfficialReleaseRow>| {
        rows.into_iter()
            .filter(|r| r.reference_period == period)
            .collect::<Vec<_>>()
    };
    // Antes de publicarse no existe.
    let before = s
        .official_releases(
            "fed_fomc",
            "fomc_statement",
            published - chrono::TimeDelta::seconds(1),
        )
        .await
        .unwrap();
    assert!(mine(before).is_empty());
    let after = mine(
        s.official_releases("fed_fomc", "fomc_statement", published)
            .await
            .unwrap(),
    );
    assert_eq!(after.len(), 1);
    let r = &after[0];
    assert_eq!(
        (r.status.as_str(), r.scheduled_at, r.published_at),
        ("scheduled", Some(scheduled), published)
    );
    assert_eq!(r.with_press_conference, Some(true));
    // Otra serie no trae la fila.
    let bls = s.official_releases("bls", "cpi", scheduled).await.unwrap();
    assert!(bls.iter().all(|r| r.reference_period != period));
}

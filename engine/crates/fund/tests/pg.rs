//! Historial del fondo en PostgreSQL (#179). Necesitan `DATABASE_URL`; están marcados
//! `#[ignore]` y el CI los corre con `-- --ignored` (job de integración).

use cfds_fund::pg::{FundStoreError, PgFund};
use cfds_fund::{Fund, FundConfig, Record, Request};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use rust_decimal::dec;

const REAL: &str = include_str!("../../../../config/fund.toml");

async fn store() -> PgFund {
    let url = cfds_storage::test_db::test_database_url();
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    PgFund::new(store.pool().clone())
}

/// Movimientos con miembros y fecha únicos por corrida: la tabla no se puede borrar.
fn movements(tag: &str, day: &str) -> Vec<Record> {
    let cfg = FundConfig::from_toml(REAL).unwrap();
    let (_, s) = Fund::default()
        .settle(
            dec!(0),
            &[
                Request::Contribute {
                    member: format!("{tag}-a"),
                    amount: dec!(100),
                },
                Request::Contribute {
                    member: format!("{tag}-b"),
                    amount: dec!(50.25),
                },
            ],
            &cfg,
        )
        .unwrap();
    let at: DateTime<Utc> = format!("{day}T07:00:00Z").parse().unwrap();
    s.records(at, &cfg)
}

/// Un día que no choca con corridas anteriores: según la hora actual, dentro de 3000 años.
fn unique_day() -> (String, String) {
    let n = Utc::now().timestamp_micros();
    let day = chrono::NaiveDate::from_ymd_opt(4000, 1, 1).unwrap()
        + chrono::Days::new((n % 300_000) as u64);
    (
        format!("t{}", n % 1_000_000_000),
        day.format("%Y-%m-%d").to_string(),
    )
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn records_are_idempotent_and_load_in_order() {
    let pg = store().await;
    let (tag, day) = unique_day();
    let recs = movements(&tag, &day);
    assert_eq!(pg.record(&recs).await.unwrap(), 2);
    assert_eq!(
        pg.record(&recs).await.unwrap(),
        0,
        "la segunda vez no duplica"
    );
    let mine: Vec<Record> = pg
        .load()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.member_id.starts_with(&tag))
        .collect();
    assert_eq!(mine, recs);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_same_id_with_other_content_is_a_conflict() {
    let pg = store().await;
    let (tag, day) = unique_day();
    let recs = movements(&tag, &day);
    pg.record(&recs).await.unwrap();
    let mut changed = recs[0].clone();
    changed.amount = dec!(999);
    assert!(matches!(
        pg.record(&[changed]).await,
        Err(FundStoreError::Conflict { .. })
    ));
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_table_is_append_only() {
    let pg = store().await;
    let (tag, day) = unique_day();
    let recs = movements(&tag, &day);
    pg.record(&recs).await.unwrap();
    let url = cfds_storage::test_db::test_database_url();
    let pool = PgStore::connect(&url).await.unwrap().pool().clone();
    let del = sqlx::query("DELETE FROM fund_movements WHERE id = $1")
        .bind(&recs[0].id)
        .execute(&pool)
        .await;
    assert!(del.is_err(), "no se puede borrar");
    let upd = sqlx::query("UPDATE fund_movements SET amount = 1 WHERE id = $1")
        .bind(&recs[0].id)
        .execute(&pool)
        .await;
    assert!(upd.is_err(), "no se puede modificar");
}

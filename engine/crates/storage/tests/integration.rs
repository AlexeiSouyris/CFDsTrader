//! Tests de integración contra PostgreSQL y Redis **reales**.
//!
//! Necesitan `DATABASE_URL` y `REDIS_URL`. Están marcados `#[ignore]` para que `cargo test`
//! funcione sin servicios; el CI los corre en un job propio con `-- --ignored` (no se saltean).
//! Cada test usa sus propios instrumentos para poder correr en paralelo.

use cfds_core::{AssetClass, Candle, Currency, Instrument, InstrumentId, Timeframe};
use cfds_storage::{CandlePublisher, PgStore, Published, StorageError, stream_key};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const NEEDS: &str = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL";

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("falta la variable {name} ({NEEDS})"))
}

async fn store() -> PgStore {
    let s = PgStore::connect(&env("DATABASE_URL")).await.unwrap();
    s.migrate().await.unwrap();
    s
}

fn id(s: &str) -> InstrumentId {
    s.parse().unwrap()
}

/// Registra un instrumento de prueba y borra sus velas de corridas anteriores, para que
/// cada test se pueda repetir sobre la misma base.
async fn with_instrument(store: &PgStore, name: &str) -> InstrumentId {
    let i = Instrument {
        id: id(name),
        name: format!("Test {name}"),
        asset_class: AssetClass::Index,
        currency: Currency::USD,
        exchange_timezone: "America/New_York".into(),
    };
    store.upsert_instrument(&i).await.unwrap();
    let pool = sqlx::PgPool::connect(&env("DATABASE_URL")).await.unwrap();
    sqlx::query("DELETE FROM candles WHERE instrument_id = $1")
        .bind(i.id.as_str())
        .execute(&pool)
        .await
        .unwrap();
    i.id
}

fn t0() -> DateTime<Utc> {
    "2026-11-19T14:30:00Z".parse().unwrap()
}

fn series(instrument: &InstrumentId, n: usize) -> Vec<Candle> {
    (0..n)
        .map(|i| {
            let base = dec!(26700.25) + Decimal::from(i);
            Candle {
                instrument_id: instrument.clone(),
                timeframe: Timeframe::M15,
                start: t0() + TimeDelta::minutes(15 * i as i64),
                open: base,
                high: base + dec!(6),
                low: base - dec!(4),
                close: base + dec!(1.50),
                volume: if i % 2 == 0 { Some(dec!(786)) } else { None },
            }
        })
        .collect()
}

async fn all(store: &PgStore, instrument: &InstrumentId) -> Vec<Candle> {
    store
        .candles(instrument, Timeframe::M15, t0(), t0() + TimeDelta::days(1))
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn migrations_can_run_more_than_once() {
    let s = store().await;
    s.migrate().await.unwrap();
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn candles_roundtrip_exactly() {
    let s = store().await;
    let us = with_instrument(&s, "ITROUNDTRIP").await;
    let candles = series(&us, 10);
    let report = s.insert_candles(&candles).await.unwrap();
    assert_eq!((report.inserted, report.unchanged), (10, 0));
    assert_eq!(all(&s, &us).await, candles); // decimales exactos y horas en UTC
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn reloading_the_same_period_does_not_duplicate() {
    let s = store().await;
    let us = with_instrument(&s, "ITIDEMPOTENT").await;
    let candles = series(&us, 8);
    s.insert_candles(&candles[..5]).await.unwrap();
    let report = s.insert_candles(&candles).await.unwrap();
    assert_eq!((report.inserted, report.unchanged), (3, 5));
    assert_eq!(all(&s, &us).await.len(), 8);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn a_conflicting_candle_is_rejected_and_the_whole_batch_is_rolled_back() {
    let s = store().await;
    let us = with_instrument(&s, "ITCONFLICT").await;
    let candles = series(&us, 4);
    s.insert_candles(&candles[..2]).await.unwrap();

    let mut batch = candles.clone();
    batch[1].close += dec!(0.25); // misma vela, otro precio: no se pisa
    let err = s.insert_candles(&batch).await.unwrap_err();
    assert!(
        matches!(err, StorageError::Conflict { start, .. } if start == candles[1].start),
        "{err}"
    );
    // Las velas 2 y 3 del lote tampoco entraron: todo o nada.
    assert_eq!(all(&s, &us).await, candles[..2].to_vec());
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn the_database_rejects_impossible_candles_even_without_validation() {
    let s = store().await;
    let us = with_instrument(&s, "ITCHECKS").await;
    let mut bad = series(&us, 1);
    bad[0].high = bad[0].low - dec!(1);
    assert!(matches!(
        s.insert_candles(&bad).await,
        Err(StorageError::Db(_))
    ));
    let mut zero = series(&us, 1);
    zero[0].low = dec!(0);
    assert!(matches!(
        s.insert_candles(&zero).await,
        Err(StorageError::Db(_))
    ));
    let unknown = series(&id("ITNOTREGISTERED"), 1);
    assert!(matches!(
        s.insert_candles(&unknown).await,
        Err(StorageError::Db(_))
    ));
}

async fn publisher() -> CandlePublisher {
    CandlePublisher::connect(&env("REDIS_URL")).await.unwrap()
}

async fn read_stream(stream: &str) -> Vec<(String, Candle)> {
    let client = redis::Client::open(env("REDIS_URL")).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let entries: Vec<(String, Vec<(String, String)>)> = redis::cmd("XRANGE")
        .arg(stream)
        .arg("-")
        .arg("+")
        .query_async(&mut conn)
        .await
        .unwrap();
    entries
        .into_iter()
        .map(|(id, fields)| {
            let data = &fields.iter().find(|(k, _)| k == "data").unwrap().1;
            (id, serde_json::from_str(data).unwrap())
        })
        .collect()
}

async fn fresh_stream(instrument: &InstrumentId) -> String {
    let stream = stream_key(instrument, Timeframe::M15);
    let client = redis::Client::open(env("REDIS_URL")).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let _: i64 = redis::cmd("DEL")
        .arg(&stream)
        .query_async(&mut conn)
        .await
        .unwrap();
    stream
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn publishes_closed_candles_with_the_contract_json() {
    let us = id("ITPUBLISH");
    let stream = fresh_stream(&us).await;
    let candles = series(&us, 3);
    let mut p = publisher().await;
    let now = candles[2].end();
    for c in &candles {
        assert!(matches!(
            p.publish(c, now).await.unwrap(),
            Published::New { .. }
        ));
    }
    let read = read_stream(&stream).await;
    assert_eq!(
        read.iter().map(|(_, c)| c.clone()).collect::<Vec<_>>(),
        candles
    );
    assert_eq!(read[0].0, cfds_storage::stream_id(candles[0].start));
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn never_publishes_a_candle_that_is_still_open() {
    let us = id("ITOPEN");
    let stream = fresh_stream(&us).await;
    let c = &series(&us, 1)[0];
    let mut p = publisher().await;
    let err = p
        .publish(c, c.end() - TimeDelta::seconds(1))
        .await
        .unwrap_err();
    assert!(matches!(err, StorageError::NotClosed { .. }), "{err}");
    assert!(read_stream(&stream).await.is_empty());
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn republishing_is_harmless_but_late_old_candles_are_reported() {
    let us = id("ITREPUBLISH");
    let stream = fresh_stream(&us).await;
    let candles = series(&us, 3);
    let now = candles[2].end();
    let mut p = publisher().await;
    p.publish(&candles[0], now).await.unwrap();
    p.publish(&candles[2], now).await.unwrap();

    // Reintento de una vela ya publicada: no se duplica.
    assert!(matches!(
        p.publish(&candles[2], now).await.unwrap(),
        Published::AlreadyPublished { .. }
    ));
    // La vela 1 llega después de la 2: es un error de orden, no se pierde en silencio.
    let err = p.publish(&candles[1], now).await.unwrap_err();
    assert!(matches!(err, StorageError::OutOfOrder { .. }), "{err}");
    assert_eq!(read_stream(&stream).await.len(), 2);
}

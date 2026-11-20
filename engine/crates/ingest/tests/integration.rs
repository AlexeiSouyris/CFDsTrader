//! Circuito completo contra PostgreSQL y Redis **reales** (`DATABASE_URL`, `REDIS_URL`).
//! Marcados `#[ignore]`: el CI los corre en el job de integración con `-- --ignored`.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_core::{AssetClass, Candle, Currency, Instrument, InstrumentId, Timeframe};
use cfds_ingest::{IngestError, IngestRequest, ingest};
use cfds_market_data::{ExchangeCalendar, StaticProvider, ValidationConfig};
use cfds_storage::{CandlePublisher, PgStore, stream_key};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("falta {name}: necesita PostgreSQL y Redis"))
}

fn open_time() -> DateTime<Utc> {
    "2026-11-25T14:30:00Z".parse().unwrap()
}

fn series(id: &InstrumentId, n: usize) -> Vec<Candle> {
    (0..n)
        .map(|i| {
            let base = dec!(26700) + Decimal::from(i);
            Candle {
                instrument_id: id.clone(),
                timeframe: Timeframe::M15,
                start: open_time() + TimeDelta::minutes(15 * i as i64),
                open: base,
                high: base + dec!(6),
                low: base - dec!(4),
                close: base + dec!(1),
                volume: Some(dec!(100)),
            }
        })
        .collect()
}

/// Instrumento de prueba propio de cada test, limpio en la base y en Redis.
async fn setup(name: &str) -> (PgStore, ExchangeCalendar, InstrumentId) {
    let store = PgStore::connect(&env("DATABASE_URL")).await.unwrap();
    store.migrate().await.unwrap();
    let id: InstrumentId = name.parse().unwrap();
    store
        .upsert_instrument(&Instrument {
            id: id.clone(),
            name: format!("Test {name}"),
            asset_class: AssetClass::Index,
            currency: Currency::USD,
            exchange_timezone: "America/New_York".into(),
        })
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&env("DATABASE_URL")).await.unwrap();
    sqlx::query("DELETE FROM candles WHERE instrument_id = $1")
        .bind(id.as_str())
        .execute(&pool)
        .await
        .unwrap();
    let client = redis::Client::open(env("REDIS_URL")).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let _: i64 = redis::cmd("DEL")
        .arg(stream_key(&id, Timeframe::M15))
        .query_async(&mut conn)
        .await
        .unwrap();

    let config = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let cal =
        ExchangeCalendar::load(&config, HashMap::from([(id.clone(), "XNYS".into())])).unwrap();
    (store, cal, id)
}

fn cfg() -> ValidationConfig {
    ValidationConfig::from_toml(include_str!("../../../../config/data_validation.toml")).unwrap()
}

fn req(id: &InstrumentId) -> IngestRequest {
    IngestRequest {
        instrument: id.clone(),
        timeframe: Timeframe::M15,
        from: open_time(),
        to: open_time() + TimeDelta::days(1),
    }
}

async fn stored(store: &PgStore, id: &InstrumentId) -> Vec<Candle> {
    let r = req(id);
    store
        .candles(id, Timeframe::M15, r.from, r.to)
        .await
        .unwrap()
}

async fn stream_len(id: &InstrumentId) -> usize {
    let client = redis::Client::open(env("REDIS_URL")).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    redis::cmd("XLEN")
        .arg(stream_key(id, Timeframe::M15))
        .query_async(&mut conn)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn live_ingest_stores_and_publishes_closed_candles_only_once() {
    let (store, cal, id) = setup("INGLIVE").await;
    let candles = series(&id, 6);
    let now = candles[5].start + TimeDelta::minutes(5); // la última sigue abierta
    let provider = StaticProvider::new(candles.clone());
    let mut publisher = CandlePublisher::connect(&env("REDIS_URL")).await.unwrap();

    let r = ingest(
        &provider,
        &cal,
        &cfg(),
        &store,
        Some(&mut publisher),
        &req(&id),
        now,
    )
    .await
    .unwrap();
    assert_eq!(
        (r.fetched, r.skipped_open, r.inserted, r.published),
        (6, 1, 5, 5)
    );
    assert_eq!(stored(&store, &id).await, candles[..5].to_vec());
    assert_eq!(stream_len(&id).await, 5);

    // Repetir la misma ingesta (por ejemplo, tras un reintento) no duplica nada.
    let r = ingest(
        &provider,
        &cal,
        &cfg(),
        &store,
        Some(&mut publisher),
        &req(&id),
        now,
    )
    .await
    .unwrap();
    assert_eq!(
        (r.inserted, r.unchanged, r.published, r.already_published),
        (0, 5, 0, 5)
    );
    assert_eq!(stream_len(&id).await, 5);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn a_rejected_batch_stores_and_publishes_nothing() {
    let (store, cal, id) = setup("INGREJECT").await;
    let mut candles = series(&id, 6);
    candles[3].high = candles[3].low - dec!(1); // una sola vela rota
    let now = candles[5].end();
    let provider = StaticProvider::new(candles);
    let mut publisher = CandlePublisher::connect(&env("REDIS_URL")).await.unwrap();

    let err = ingest(
        &provider,
        &cal,
        &cfg(),
        &store,
        Some(&mut publisher),
        &req(&id),
        now,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, IngestError::Rejected(_)), "{err}");
    assert!(
        stored(&store, &id).await.is_empty(),
        "ni siquiera las velas buenas del lote"
    );
    assert_eq!(stream_len(&id).await, 0);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn a_historical_backfill_stores_without_publishing() {
    let (store, cal, id) = setup("INGBACKFILL").await;
    let candles = series(&id, 8);
    let now = candles[7].end() + TimeDelta::days(30);
    let provider = StaticProvider::new(candles.clone());

    let r = ingest(&provider, &cal, &cfg(), &store, None, &req(&id), now)
        .await
        .unwrap();
    assert_eq!((r.inserted, r.published), (8, 0));
    assert_eq!(stored(&store, &id).await, candles);
    assert_eq!(stream_len(&id).await, 0);
}

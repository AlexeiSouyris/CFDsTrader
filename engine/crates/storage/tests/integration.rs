//! Tests de integración contra PostgreSQL y Redis **reales**.
//!
//! Necesitan `DATABASE_URL` y `REDIS_URL`. Están marcados `#[ignore]` para que `cargo test`
//! funcione sin servicios; el CI los corre en un job propio con `-- --ignored` (no se saltean).
//! Cada test usa sus propios instrumentos para poder correr en paralelo.

use cfds_core::{
    AssetClass, Candle, CandleSpread, Currency, DailyCandle, Instrument, InstrumentId,
    SentimentObservation, Timeframe,
};
use cfds_storage::{
    AllowanceRecord, CandlePublisher, PgStore, Published, StorageError, stream_key,
};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const NEEDS: &str = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL";

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("falta la variable {name} ({NEEDS})"))
}

async fn store() -> PgStore {
    let s = PgStore::connect(&cfds_storage::test_db::test_database_url())
        .await
        .unwrap();
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
    let pool = sqlx::PgPool::connect(&cfds_storage::test_db::test_database_url())
        .await
        .unwrap();
    sqlx::query("DELETE FROM daily_candles WHERE instrument_id = $1")
        .bind(i.id.as_str())
        .execute(&pool)
        .await
        .unwrap();
    // Primero los spreads: referencian a las velas.
    sqlx::query("DELETE FROM candle_spreads WHERE instrument_id = $1")
        .bind(i.id.as_str())
        .execute(&pool)
        .await
        .unwrap();
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

fn sentiment(instrument: &InstrumentId, at: DateTime<Utc>, value: Decimal) -> SentimentObservation {
    SentimentObservation {
        source: "ig_client".into(),
        series: "long_pct".into(),
        instrument_id: Some(instrument.clone()),
        observed_for: at,
        published_at: at,
        value,
    }
}

/// Hora única por corrida: la tabla es de solo agregar y no se puede limpiar entre corridas.
fn fresh_time() -> DateTime<Utc> {
    let now = Utc::now();
    now - TimeDelta::nanoseconds(i64::from(now.timestamp_subsec_nanos() % 1000))
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn sentiment_is_append_only_and_idempotent() {
    let s = store().await;
    let us = with_instrument(&s, "TSENT").await;
    let at = fresh_time();
    let batch = vec![
        sentiment(&us, at, dec!(68.0)),
        sentiment(&us, at + TimeDelta::minutes(15), dec!(67.5)),
    ];
    let first = s.insert_sentiment(&batch).await.unwrap();
    assert_eq!((first.inserted, first.unchanged), (2, 0));
    // Repetir es inofensivo.
    let again = s.insert_sentiment(&batch).await.unwrap();
    assert_eq!((again.inserted, again.unchanged), (0, 2));

    // La misma clave con otro valor no pisa nada, y el lote entero se descarta.
    let other = sentiment(&us, at + TimeDelta::minutes(30), dec!(66));
    let changed = sentiment(&us, at, dec!(70));
    let err = s
        .insert_sentiment(&[other.clone(), changed])
        .await
        .unwrap_err();
    assert!(
        matches!(err, StorageError::SentimentConflict { .. }),
        "{err}"
    );
    let pool = sqlx::PgPool::connect(&cfds_storage::test_db::test_database_url())
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sentiment_observations WHERE instrument_id = $1 AND observed_for >= $2",
    )
    .bind(us.as_str())
    .bind(at)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        count, 2,
        "la transacción no dejó la fila nueva del lote fallido"
    );

    // La base impide modificar o borrar.
    for sql in [
        "UPDATE sentiment_observations SET value = 1 WHERE instrument_id = $1",
        "DELETE FROM sentiment_observations WHERE instrument_id = $1",
    ] {
        let e = sqlx::query(sql)
            .bind(us.as_str())
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(e.to_string().contains("solo agregar"), "{e}");
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn the_database_rejects_impossible_sentiment() {
    let s = store().await;
    let us = with_instrument(&s, "TSENT2").await;
    let at = fresh_time();
    // % de IG fuera de 0–100.
    assert!(
        s.insert_sentiment(&[sentiment(&us, at, dec!(100.5))])
            .await
            .is_err()
    );
    // Publicado antes del momento al que corresponde (look-ahead).
    let mut early = sentiment(&us, at, dec!(50));
    early.published_at = at - TimeDelta::seconds(1);
    assert!(s.insert_sentiment(&[early]).await.is_err());
    // Un dato de todo el mercado (sin instrumento) es válido, y también idempotente.
    let mut market = sentiment(&us, at, dec!(-0.4));
    market.source = "test_index".into();
    market.instrument_id = None;
    assert_eq!(
        s.insert_sentiment(&[market.clone()])
            .await
            .unwrap()
            .inserted,
        1
    );
    assert_eq!(s.insert_sentiment(&[market]).await.unwrap().unchanged, 1);
}

fn spreads(candles: &[Candle]) -> Vec<CandleSpread> {
    candles
        .iter()
        .enumerate()
        .map(|(i, c)| CandleSpread {
            instrument_id: c.instrument_id.clone(),
            timeframe: c.timeframe,
            start: c.start,
            spread: dec!(1.40) + Decimal::new(i as i64, 1),
            source: "ig_demo".into(),
        })
        .collect()
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn spreads_roundtrip_and_are_never_overwritten() {
    let s = store().await;
    let us = with_instrument(&s, "ITSPREAD").await;
    let candles = series(&us, 6);
    s.insert_candles(&candles).await.unwrap();
    let sp = spreads(&candles);
    let first = s.insert_spreads(&sp[..4]).await.unwrap();
    assert_eq!((first.inserted, first.unchanged), (4, 0));
    let again = s.insert_spreads(&sp).await.unwrap();
    assert_eq!((again.inserted, again.unchanged), (2, 4));
    let read = s
        .spreads(&us, Timeframe::M15, t0(), t0() + TimeDelta::days(1))
        .await
        .unwrap();
    assert_eq!(read, sp, "decimales exactos, en orden");

    // Otro valor para la misma vela: error y no entra nada del lote.
    let mut changed = sp.clone();
    changed[0].spread += dec!(0.1);
    assert!(matches!(
        s.insert_spreads(&changed).await,
        Err(StorageError::Conflict { start, .. }) if start == sp[0].start
    ));
    // El mismo valor de otra fuente tampoco pisa al guardado.
    let mut other_source = sp[..1].to_vec();
    other_source[0].source = "otra_fuente".into();
    assert!(matches!(
        s.insert_spreads(&other_source).await,
        Err(StorageError::Conflict { .. })
    ));
    assert_eq!(
        s.spreads(&us, Timeframe::M15, t0(), t0() + TimeDelta::days(1))
            .await
            .unwrap(),
        sp
    );
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn a_spread_needs_its_candle_and_is_never_negative() {
    let s = store().await;
    let us = with_instrument(&s, "ITSPREADFK").await;
    let candles = series(&us, 2);
    s.insert_candles(&candles[..1]).await.unwrap();
    let sp = spreads(&candles);
    // La vela 1 no está guardada: la base rechaza su spread (y el lote entero).
    assert!(s.insert_spreads(&sp).await.is_err());
    let mut negative = sp[..1].to_vec();
    negative[0].spread = dec!(-0.5);
    assert!(s.insert_spreads(&negative).await.is_err());
    assert!(
        s.spreads(&us, Timeframe::M15, t0(), t0() + TimeDelta::days(1))
            .await
            .unwrap()
            .is_empty()
    );
}

fn days(instrument: &InstrumentId) -> Vec<DailyCandle> {
    // Semana del cambio de horario de octubre de 2026: el domingo 25 dura 25 h.
    [
        ("2026-10-23", "2026-10-22T22:00:00Z", "2026-10-23T22:00:00Z"),
        ("2026-10-25", "2026-10-24T22:00:00Z", "2026-10-25T23:00:00Z"),
        ("2026-10-26", "2026-10-25T23:00:00Z", "2026-10-26T23:00:00Z"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (day, start, end))| DailyCandle {
        instrument_id: instrument.clone(),
        day: day.parse().unwrap(),
        start: start.parse().unwrap(),
        end: end.parse().unwrap(),
        open: dec!(24000.5) + Decimal::from(i),
        high: dec!(24100.25),
        low: dec!(23900),
        close: dec!(24050.75),
        volume: None,
    })
    .collect()
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn daily_candles_roundtrip_and_are_never_overwritten() {
    let s = store().await;
    let us = with_instrument(&s, "ITDAILY").await;
    let d = days(&us);
    let first = s.insert_daily(&d[..2]).await.unwrap();
    assert_eq!((first.inserted, first.unchanged), (2, 0));
    let again = s.insert_daily(&d).await.unwrap();
    assert_eq!((again.inserted, again.unchanged), (1, 2));
    let range = |a: &str, b: &str| (a.parse().unwrap(), b.parse().unwrap());
    let (from, to) = range("2026-10-01", "2026-11-01");
    assert_eq!(
        s.daily_candles(&us, from, to).await.unwrap(),
        d,
        "exacto, con el día de 25 h"
    );

    let mut changed = d.clone();
    changed[0].close += dec!(0.25);
    assert!(matches!(
        s.insert_daily(&changed).await,
        Err(StorageError::DailyConflict { day, .. }) if day == d[0].day
    ));
    assert_eq!(s.daily_candles(&us, from, to).await.unwrap(), d);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn the_database_rejects_impossible_daily_candles() {
    let s = store().await;
    let us = with_instrument(&s, "ITDAILYBAD").await;
    let mut long = days(&us)[..1].to_vec();
    long[0].end += chrono::TimeDelta::hours(2); // 26 h: no es un día
    assert!(s.insert_daily(&long).await.is_err());
    let mut ohlc = days(&us)[..1].to_vec();
    ohlc[0].high = dec!(1);
    assert!(s.insert_daily(&ohlc).await.is_err());
    let (from, to) = ("2026-10-01".parse().unwrap(), "2026-11-01".parse().unwrap());
    assert!(s.daily_candles(&us, from, to).await.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn reference_rates_are_read_only_once_published_in_publication_order() {
    let s = store().await;
    // La tabla es de solo agregar: una serie propia por corrida.
    let nanos = Utc::now().timestamp_nanos_opt().unwrap();
    let series = format!("ITRATE{nanos}");
    let pool = sqlx::PgPool::connect(&cfds_storage::test_db::test_database_url())
        .await
        .unwrap();
    for (day, published, rate) in [
        ("2026-11-13", "2026-11-16T08:00:00Z", "2.437"),
        ("2026-11-12", "2026-11-13T08:00:00Z", "2.443"),
        ("2026-11-12", "2026-11-17T10:00:00Z", "2.5"),
    ] {
        sqlx::query(
            "INSERT INTO reference_rates (series, ref_date, published_at, rate_pct)
             VALUES ($1, $2::date, $3::timestamptz, $4::numeric)",
        )
        .bind(&series)
        .bind(day)
        .bind(published)
        .bind(rate)
        .execute(&pool)
        .await
        .unwrap();
    }
    let until: DateTime<Utc> = "2026-11-17T00:00:00Z".parse().unwrap();
    let got = s.reference_rates(&series, until).await.unwrap();
    let summary: Vec<(String, Decimal)> = got
        .iter()
        .map(|r| (r.ref_date.to_string(), r.rate_pct))
        .collect();
    // La corrección del 17 todavía no estaba publicada.
    assert_eq!(
        summary,
        vec![
            ("2026-11-12".to_string(), dec!(2.443)),
            ("2026-11-13".to_string(), dec!(2.437)),
        ]
    );
    assert!(got.iter().all(|r| r.series == series));
    // Un valor publicado antes de que termine su día no entra.
    let early = sqlx::query(
        "INSERT INTO reference_rates (series, ref_date, published_at, rate_pct)
         VALUES ($1, '2026-11-20', '2026-11-20T12:00:00Z', 1)",
    )
    .bind(&series)
    .execute(&pool)
    .await;
    assert!(early.is_err());
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn ig_allowance_log_is_append_only_and_returns_the_latest() {
    let s = store().await;
    // Fechas lejanas: esta es la última observación aunque otros tests corran antes.
    let at = |h: u32| -> DateTime<Utc> { format!("2100-01-01T{h:02}:00:00Z").parse().unwrap() };
    let records = [
        AllowanceRecord {
            observed_at: at(10),
            points_returned: 2,
            remaining: 9_993,
            total: 10_000,
            resets_at: at(10) + TimeDelta::days(7),
        },
        AllowanceRecord {
            observed_at: at(11),
            points_returned: 1,
            remaining: 9_992,
            total: 10_000,
            resets_at: at(11) + TimeDelta::days(7),
        },
    ];
    s.insert_allowance_log("ig_backfill", &records)
        .await
        .unwrap();
    assert_eq!(s.last_allowance().await.unwrap(), Some(records[1]));
    let pool = s.pool().clone();
    let update = sqlx::query("UPDATE ig_allowance_log SET remaining = 0")
        .execute(&pool)
        .await;
    assert!(update.is_err(), "la tabla es de solo agregar");
    let bad = s.insert_allowance_log("IG Backfill", &records[..1]).await;
    assert!(bad.is_err(), "el nombre del comando se valida en la base");
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn allowance_window_keeps_only_the_observations_not_yet_renewed() {
    let s = store().await;
    // Fechas lejanas, propias de este test y anteriores a las del test de arriba (que necesita
    // tener la última observación). La tabla es de solo agregar: al repetir el test, las filas
    // de corridas anteriores siguen ahí, así que se controla qué entra y qué no.
    let at = |d: u32, h: u32| -> DateTime<Utc> {
        format!("2090-01-{d:02}T{h:02}:00:00Z").parse().unwrap()
    };
    let rec = |observed_at, resets_at, points, remaining| AllowanceRecord {
        observed_at,
        points_returned: points,
        remaining,
        total: 10_000,
        resets_at,
    };
    let old = rec(at(1, 10), at(2, 12), 40, 9_960);
    let current = [
        rec(at(3, 10), at(9, 12), 30, 9_970),
        rec(at(3, 11), at(9, 12), 5, 9_965),
    ];
    s.insert_allowance_log("ig_backfill", &[old]).await.unwrap();
    s.insert_allowance_log("ig_backfill_daily", &current[..1])
        .await
        .unwrap();
    s.insert_allowance_log("ig_live", &current[1..])
        .await
        .unwrap();
    let window = s.allowance_window(at(4, 0)).await.unwrap();
    assert!(window.iter().all(|(_, r)| r.resets_at > at(4, 0)));
    assert!(window.contains(&("ig_backfill_daily".to_owned(), current[0])));
    assert!(window.contains(&("ig_live".to_owned(), current[1])));
    assert!(!window.iter().any(|(_, r)| *r == old));
    let pos = |r| window.iter().position(|(_, w)| *w == r).unwrap();
    assert!(
        pos(current[0]) < pos(current[1]),
        "en el orden en que se guardaron"
    );
    // En la renovación anunciada, esa ventana terminó.
    let later = s.allowance_window(at(9, 12)).await.unwrap();
    assert!(later.iter().all(|(_, r)| r.resets_at > at(9, 12)));
    assert!(!later.iter().any(|(_, r)| current.contains(r)));
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn daily_indicators_are_idempotent_and_refuse_to_repaint() {
    let s = store().await;
    // La tabla es de solo agregar: un instrumento propio por corrida.
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let us = with_instrument(&s, &format!("ITIND{nanos}")).await;
    let row = |day: &str, atr: Option<Decimal>| cfds_core::DailyIndicator {
        instrument_id: us.clone(),
        day: day.parse().unwrap(),
        published_at: format!("{day}T23:00:00Z").parse().unwrap(),
        atr_period: 14,
        atr,
        realized_vol_window: 20,
        realized_vol: atr.map(|_| dec!(0.01)),
    };
    let rows = vec![
        row("2026-11-19", None),
        row("2026-11-20", Some(dec!(250.5))),
    ];
    let first = s.insert_daily_indicators(&rows).await.unwrap();
    let again = s.insert_daily_indicators(&rows).await.unwrap();
    assert_eq!((first.inserted, again.unchanged), (2, 2));

    let (from, to) = ("2026-11-01".parse().unwrap(), "2026-12-01".parse().unwrap());
    assert_eq!(
        s.daily_indicators(&us, 14, 20, from, to).await.unwrap(),
        rows
    );
    assert!(
        s.daily_indicators(&us, 14, 21, from, to)
            .await
            .unwrap()
            .is_empty()
    );

    let repainted = vec![row("2026-11-20", Some(dec!(251)))];
    assert!(matches!(
        s.insert_daily_indicators(&repainted).await,
        Err(StorageError::DailyIndicatorConflict { .. })
    ));
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn daily_drawdowns_are_idempotent_and_refuse_to_repaint() {
    let s = store().await;
    // La tabla es de solo agregar: un instrumento propio por corrida.
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let us = with_instrument(&s, &format!("ITDD{nanos}")).await;
    let row = |day: &str, drawdown: Option<Decimal>| cfds_core::DailyDrawdown {
        instrument_id: us.clone(),
        day: day.parse().unwrap(),
        published_at: format!("{day}T23:00:00Z").parse().unwrap(),
        drawdown_window: 252,
        drawdown,
    };
    let rows = vec![
        row("2026-11-19", None),
        row("2026-11-20", Some(dec!(-0.05))),
    ];
    let first = s.insert_daily_drawdowns(&rows).await.unwrap();
    let again = s.insert_daily_drawdowns(&rows).await.unwrap();
    assert_eq!((first.inserted, again.unchanged), (2, 2));

    let repainted = vec![row("2026-11-20", Some(dec!(-0.06)))];
    assert!(matches!(
        s.insert_daily_drawdowns(&repainted).await,
        Err(StorageError::DailyIndicatorConflict { .. })
    ));
    // La base tampoco deja una caída positiva ni modificar lo guardado.
    assert!(
        s.insert_daily_drawdowns(&[row("2026-11-23", Some(dec!(0.01)))])
            .await
            .is_err()
    );
    let update = sqlx::query("UPDATE daily_drawdowns SET drawdown = 0 WHERE instrument_id = $1")
        .bind(us.as_str())
        .execute(s.pool())
        .await;
    assert!(update.is_err());
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn daily_vol_terciles_are_idempotent_read_back_and_refuse_to_repaint() {
    use cfds_core::{DailyVolTercile, VolTercile};
    let s = store().await;
    // La tabla es de solo agregar: un instrumento propio por corrida.
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let us = with_instrument(&s, &format!("ITVT{nanos}")).await;
    let row = |day: &str, tercile: Option<VolTercile>| DailyVolTercile {
        instrument_id: us.clone(),
        day: day.parse().unwrap(),
        published_at: format!("{day}T23:00:00Z").parse().unwrap(),
        realized_vol_window: 20,
        history_years: 3,
        tercile,
    };
    let rows = vec![
        row("2026-11-19", None),
        row("2026-11-20", Some(VolTercile::High)),
    ];
    let first = s.insert_daily_vol_terciles(&rows).await.unwrap();
    let again = s.insert_daily_vol_terciles(&rows).await.unwrap();
    assert_eq!((first.inserted, again.unchanged), (2, 2));
    let day = |d: &str| d.parse::<chrono::NaiveDate>().unwrap();
    let read = s
        .daily_vol_terciles(&us, 20, 3, day("2026-11-01"), day("2026-12-01"))
        .await
        .unwrap();
    assert_eq!(read, rows);
    // Otros parámetros son otra serie.
    let other = s
        .daily_vol_terciles(&us, 20, 2, day("2026-11-01"), day("2026-12-01"))
        .await
        .unwrap();
    assert!(other.is_empty());

    let repainted = vec![row("2026-11-20", Some(VolTercile::Mid))];
    assert!(matches!(
        s.insert_daily_vol_terciles(&repainted).await,
        Err(StorageError::DailyIndicatorConflict { .. })
    ));
    let update =
        sqlx::query("UPDATE daily_vol_terciles SET tercile = 'low' WHERE instrument_id = $1")
            .bind(us.as_str())
            .execute(s.pool())
            .await;
    assert!(update.is_err());
}

/// Guarda una noticia y su interpretación con SQL directo, como lo hace Python (#186).
#[allow(clippy::too_many_arguments)]
async fn put_interpretation(
    pool: &sqlx::PgPool,
    id: &str,
    item_id: &str,
    version: &str,
    published_at: DateTime<Utc>,
    interpreted_at: DateTime<Utc>,
    impacts: &str,
) {
    sqlx::query(
        "INSERT INTO news_items (source, feed, item_id, title, url, published_at, retrieved_at)
         VALUES ('fed', 'press', $1, 'titular de prueba', 'https://www.federalreserve.gov/x', $2, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(item_id)
    .bind(published_at)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO news_interpretations
            (interpretation_id, source, item_id, published_at, interpreted_at, interpreter_name,
             interpreter_version, interpreter_model, interpreter_prompt_sha256, summary_es, impacts)
         VALUES ($1, 'fed', $2, $3, $4, 'ollama', $5, 'modelo:q4', NULL, 'resumen', $6::jsonb)",
    )
    .bind(id)
    .bind(item_id)
    .bind(published_at)
    .bind(interpreted_at)
    .bind(version)
    .bind(impacts)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "necesita PostgreSQL y Redis: DATABASE_URL y REDIS_URL"]
async fn news_interpretations_are_read_by_interpreter_range_and_cutoff() {
    let s = store().await;
    let pool = s.pool().clone();
    // La tabla es de solo agregar: ids y versión del intérprete propios de esta corrida.
    let run = Utc::now().timestamp_nanos_opt().unwrap();
    let version = format!("t{run}");
    let other = format!("u{run}");
    let id = |k: i64| format!("{:064x}", i128::from(run) * 10 + i128::from(k));
    let item = |k: i64| format!("it-{run}-{k}");
    let t = |x: &str| -> DateTime<Utc> { x.parse().unwrap() };
    // La vela cierra a las 15:00: la interpretación de las 15:00 justas ya cuenta.
    let close = t("2026-11-20T15:00:00Z");
    let impacts = r#"[{"instrument_id": "US500", "sentiment": "-0.75", "relevance": "1"}]"#;
    put_interpretation(
        &pool,
        &id(1),
        &item(1),
        &version,
        t("2026-11-20T14:00:00Z"),
        close,
        impacts,
    )
    .await;
    // Un microsegundo después del cierre: no entra.
    let late = close + TimeDelta::microseconds(1);
    put_interpretation(
        &pool,
        &id(2),
        &item(2),
        &version,
        t("2026-11-20T14:30:00Z"),
        late,
        "[]",
    )
    .await;
    // Justo en el comienzo del tramo: no entra (el tramo anterior ya la leyó).
    let start = t("2026-11-20T12:00:00Z");
    put_interpretation(
        &pool,
        &id(3),
        &item(3),
        &version,
        t("2026-11-20T11:00:00Z"),
        start,
        "[]",
    )
    .await;
    // Otra versión del intérprete: no entra.
    put_interpretation(
        &pool,
        &id(4),
        &item(4),
        &other,
        t("2026-11-20T14:00:00Z"),
        close,
        "[]",
    )
    .await;
    // Publicada antes del corte del modelo (ADR 0008 §5): entra en vivo, no en el backtest.
    let old = t("2025-12-02T23:59:59Z");
    put_interpretation(
        &pool,
        &id(5),
        &item(5),
        &version,
        old,
        t("2026-11-20T13:00:00Z"),
        "[]",
    )
    .await;

    let live = s
        .news_interpretations("ollama", &version, start, close, None)
        .await
        .unwrap();
    let ids: Vec<_> = live.iter().map(|n| n.interpretation_id.clone()).collect();
    assert_eq!(ids, [id(5), id(1)], "por interpreted_at, (start, close]");

    let n = &live[1];
    assert_eq!(n.interpreter.name, "ollama");
    assert_eq!(n.interpreter.version, version);
    assert_eq!(n.interpreter.model.as_deref(), Some("modelo:q4"));
    assert_eq!(n.interpreter.prompt_sha256, None);
    assert_eq!(n.interpreted_at, close);
    assert_eq!(n.impacts.len(), 1);
    assert_eq!(n.impacts[0].instrument_id, id_of("US500"));
    assert_eq!(n.impacts[0].sentiment, dec!(-0.75));
    assert_eq!(n.impacts[0].relevance, dec!(1));

    let cutoff = t("2025-12-03T00:00:00Z");
    let backtest = s
        .news_interpretations("ollama", &version, start, close, Some(cutoff))
        .await
        .unwrap();
    assert_eq!(backtest.len(), 1);
    assert_eq!(backtest[0].interpretation_id, id(1));
}

fn id_of(s: &str) -> InstrumentId {
    s.parse().unwrap()
}

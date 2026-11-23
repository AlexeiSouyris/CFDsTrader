//! Velas de futuros en PostgreSQL **real** (#259). Necesita `DATABASE_URL`; está marcado
//! `#[ignore]` y el CI lo corre con `-- --ignored`. Las tablas son de solo agregar: cada corrida
//! usa un instrumento propio (no hace falta que exista en `instruments`).

use cfds_core::{Candle, FuturesCandle, InstrumentId, Timeframe};
use cfds_storage::{PgStore, StorageError};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};

async fn store() -> PgStore {
    let url = std::env::var("DATABASE_URL").expect("falta DATABASE_URL (necesita PostgreSQL)");
    let s = PgStore::connect(&url).await.unwrap();
    s.migrate().await.unwrap();
    s
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn fc(
    id: &InstrumentId,
    tf: Timeframe,
    start: &str,
    close: Decimal,
    contract: u64,
) -> FuturesCandle {
    FuturesCandle {
        candle: Candle {
            instrument_id: id.clone(),
            timeframe: tf,
            start: at(start),
            open: close - dec!(1),
            high: close + dec!(2),
            low: close - dec!(3),
            close,
            volume: Some(dec!(150)),
        },
        contract_id: contract,
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn futures_candles_are_idempotent_separate_from_ig_and_refuse_to_repaint() {
    let s = store().await;
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id: InstrumentId = format!("ITFC{nanos}").parse().unwrap();
    let batch = vec![
        fc(
            &id,
            Timeframe::M15,
            "2026-11-18T09:00:00Z",
            dec!(24000.5),
            7,
        ),
        fc(&id, Timeframe::M15, "2026-11-18T09:15:00Z", dec!(24001), 7),
        fc(&id, Timeframe::H1, "2026-11-18T09:00:00Z", dec!(24002), 7),
    ];
    let save = |b: Vec<FuturesCandle>| {
        let s = s.clone();
        async move {
            s.insert_futures_candles("databento", "XEUR.EOBI", "FDAX.c.0", &b)
                .await
        }
    };
    let first = save(batch.clone()).await.unwrap();
    let again = save(batch.clone()).await.unwrap();
    assert_eq!((first.inserted, again.unchanged), (3, 3));

    let (from, to) = (at("2026-11-18T00:00:00Z"), at("2026-11-19T00:00:00Z"));
    let m15 = s
        .futures_candles("databento", &id, Timeframe::M15, from, to)
        .await
        .unwrap();
    assert_eq!(m15, vec![batch[0].candle.clone(), batch[1].candle.clone()]);
    let h1 = s
        .futures_candles("databento", &id, Timeframe::H1, from, to)
        .await
        .unwrap();
    assert_eq!(h1, vec![batch[2].candle.clone()]);
    // No están en `candles`: las velas de IG no se tocan.
    assert!(
        s.candles(&id, Timeframe::M15, from, to)
            .await
            .unwrap()
            .is_empty()
    );

    // Repintado (otro cierre u otro contrato): error y no se guarda nada del lote.
    let new_one = fc(&id, Timeframe::M15, "2026-11-18T09:30:00Z", dec!(24003), 7);
    for bad in [
        fc(&id, Timeframe::M15, "2026-11-18T09:00:00Z", dec!(24009), 7),
        fc(
            &id,
            Timeframe::M15,
            "2026-11-18T09:00:00Z",
            dec!(24000.5),
            8,
        ),
    ] {
        assert!(matches!(
            save(vec![new_one.clone(), bad]).await,
            Err(StorageError::Conflict { .. })
        ));
    }
    assert_eq!(
        s.futures_candles("databento", &id, Timeframe::M15, from, to)
            .await
            .unwrap()
            .len(),
        2
    );
    // Desalineada: la base la rechaza.
    let misaligned = fc(&id, Timeframe::M15, "2026-11-18T09:07:00Z", dec!(24000), 7);
    assert!(save(vec![misaligned]).await.is_err());
    // Solo agregar.
    for sql in [
        "UPDATE futures_candles SET close = 1 WHERE instrument_id = $1",
        "DELETE FROM futures_candles WHERE instrument_id = $1",
    ] {
        let r = sqlx::query(sql).bind(id.as_str()).execute(s.pool()).await;
        assert!(r.is_err(), "{sql}");
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_loaded_range_comes_from_the_loads_of_the_instrument() {
    let s = store().await;
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id: InstrumentId = format!("ITFL{nanos}").parse().unwrap();
    let dataset = format!("T{nanos}.TEST");
    assert_eq!(
        s.futures_loaded_range("databento", &id).await.unwrap(),
        None
    );
    sqlx::query(
        "INSERT INTO futures_bars_1m (source, dataset, symbol, contract_id, instrument_id,
             start_at, open, high, low, close, volume)
         VALUES ('databento', $1, 'FDAX.c.0', 7, $2, '2026-11-18T09:00:00Z', 1, 1, 1, 1, 1)",
    )
    .bind(&dataset)
    .bind(id.as_str())
    .execute(s.pool())
    .await
    .unwrap();
    for (from, to) in [
        ("2026-11-01T00:00:00Z", "2026-12-01T00:00:00Z"),
        ("2026-12-01T00:00:00Z", "2026-12-10T00:00:00Z"),
    ] {
        sqlx::query(
            "INSERT INTO futures_bar_loads (source, dataset, symbol, from_at, to_at, bars, cost_usd)
             VALUES ('databento', $1, 'FDAX.c.0', $2, $3, 1, 0)",
        )
        .bind(&dataset)
        .bind(at(from))
        .bind(at(to))
        .execute(s.pool())
        .await
        .unwrap();
    }
    assert_eq!(
        s.futures_loaded_range("databento", &id).await.unwrap(),
        Some((at("2026-11-01T00:00:00Z"), at("2026-12-10T00:00:00Z")))
    );
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn contract_changes_are_the_first_candle_of_each_new_contract() {
    let s = store().await;
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id: InstrumentId = format!("ITCC{nanos}").parse().unwrap();
    let batch: Vec<_> = [
        ("2026-12-17T20:00:00Z", 7),
        ("2026-12-17T20:15:00Z", 7),
        ("2026-12-18T14:30:00Z", 8),
        ("2026-12-18T14:45:00Z", 8),
        ("2027-03-19T14:30:00Z", 9),
    ]
    .into_iter()
    .map(|(t, c)| fc(&id, Timeframe::M15, t, dec!(6000), c))
    .collect();
    s.insert_futures_candles("databento", "GLBX.MDP3", "ES.c.0", &batch)
        .await
        .unwrap();
    let all = s
        .futures_contract_changes(
            "databento",
            &id,
            Timeframe::M15,
            at("2026-12-01T00:00:00Z"),
            at("2027-04-01T00:00:00Z"),
        )
        .await
        .unwrap();
    assert_eq!(
        all,
        vec![at("2026-12-18T14:30:00Z"), at("2027-03-19T14:30:00Z")]
    );
    // Si el período empieza con el contrato nuevo, ese inicio no es un cambio.
    let later = s
        .futures_contract_changes(
            "databento",
            &id,
            Timeframe::M15,
            at("2026-12-18T14:30:00Z"),
            at("2027-04-01T00:00:00Z"),
        )
        .await
        .unwrap();
    assert_eq!(later, vec![at("2027-03-19T14:30:00Z")]);
}

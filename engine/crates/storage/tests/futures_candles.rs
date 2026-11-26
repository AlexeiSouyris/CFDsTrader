//! Velas de futuros en PostgreSQL **real** (#259). Necesita `DATABASE_URL`; está marcado
//! `#[ignore]` y el CI lo corre con `-- --ignored`. Las tablas son de solo agregar: cada corrida
//! usa un instrumento propio (no hace falta que exista en `instruments`).

use cfds_core::{Candle, FuturesCandle, InstrumentId, Timeframe};
use cfds_storage::test_db::{unique_id, unique_number};
use cfds_storage::{PgStore, StorageError};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};

async fn store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
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
    let id: InstrumentId = unique_id("ITFC").parse().unwrap();
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
    // Con el contrato de cada vela (#449).
    assert_eq!(
        s.futures_candles_with_contract("databento", &id, Timeframe::M15, from, to)
            .await
            .unwrap(),
        batch[..2]
    );
    let h1 = s
        .futures_candles("databento", &id, Timeframe::H1, from, to)
        .await
        .unwrap();
    assert_eq!(h1, vec![batch[2].candle.clone()]);
    // El tramo guardado de cada timeframe (replay de la Academia, #723).
    assert_eq!(
        s.futures_candle_range("databento", &id, Timeframe::M15)
            .await
            .unwrap(),
        Some((at("2026-11-18T09:00:00Z"), at("2026-11-18T09:15:00Z")))
    );
    assert_eq!(
        s.futures_candle_range("otra", &id, Timeframe::H1)
            .await
            .unwrap(),
        None
    );
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
async fn the_loaded_range_comes_from_the_loads_of_the_instrument_dataset_and_symbol() {
    let s = store().await;
    let id: InstrumentId = unique_id("ITFL").parse().unwrap();
    // El dataset no admite guiones: un número propio.
    let n = unique_number();
    let dataset = format!("T{n}.TEST");
    let other = format!("O{n}.TEST");
    let range = |dataset: String, symbol: &'static str| {
        let (s, id) = (s.clone(), id.clone());
        async move {
            s.futures_loaded_range("databento", &dataset, symbol, &id)
                .await
                .unwrap()
        }
    };
    assert_eq!(range(dataset.clone(), "FDAX.c.0").await, None);
    // Barras del mismo instrumento en dos datasets: cada uno con sus cargas.
    for ds in [&dataset, &other] {
        sqlx::query(
            "INSERT INTO futures_bars_1m (source, dataset, symbol, contract_id, instrument_id,
                 start_at, open, high, low, close, volume)
             VALUES ('databento', $1, 'FDAX.c.0', 7, $2, '2026-11-18T09:00:00Z', 1, 1, 1, 1, 1)",
        )
        .bind(ds)
        .bind(id.as_str())
        .execute(s.pool())
        .await
        .unwrap();
    }
    for (ds, from, to) in [
        (&dataset, "2026-11-01T00:00:00Z", "2026-12-01T00:00:00Z"),
        (&dataset, "2026-12-01T00:00:00Z", "2026-12-10T00:00:00Z"),
        (&other, "2025-01-01T00:00:00Z", "2027-01-01T00:00:00Z"),
    ] {
        sqlx::query(
            "INSERT INTO futures_bar_loads (source, dataset, symbol, from_at, to_at, bars, cost_usd)
             VALUES ('databento', $1, 'FDAX.c.0', $2, $3, 1, 0)",
        )
        .bind(ds)
        .bind(at(from))
        .bind(at(to))
        .execute(s.pool())
        .await
        .unwrap();
    }
    // Las cargas del otro dataset (más amplias) no se mezclan (#385).
    assert_eq!(
        range(dataset.clone(), "FDAX.c.0").await,
        Some((at("2026-11-01T00:00:00Z"), at("2026-12-10T00:00:00Z")))
    );
    assert_eq!(
        range(other, "FDAX.c.0").await,
        Some((at("2025-01-01T00:00:00Z"), at("2027-01-01T00:00:00Z")))
    );
    // Otro símbolo del mismo dataset: nada bajado.
    assert_eq!(range(dataset, "FESX.c.0").await, None);
}

//! Velas de IBKR en PostgreSQL **real** (#536). Necesita `DATABASE_URL` de una base de pruebas
//! (`cfds_test…`); está marcado `#[ignore]` y el CI lo corre con `-- --ignored`. La tabla es de
//! solo agregar: cada corrida usa un instrumento propio.

use cfds_storage::{IbkrBarCorrection, IbkrBarRow, IbkrSeries, PgStore, StorageError};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};

async fn store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let s = PgStore::connect(&url).await.unwrap();
    s.migrate().await.unwrap();
    s
}

fn row(id: &str, contract: &str, size: &str, start: &str, close: Decimal) -> IbkrBarRow {
    IbkrBarRow {
        source: "ibkr".into(),
        instrument_id: id.into(),
        kind: "future".into(),
        contract: contract.into(),
        bar_size: size.into(),
        what_to_show: "TRADES".into(),
        start: start.parse::<DateTime<Utc>>().unwrap(),
        open: close - dec!(1),
        high: close + dec!(2),
        low: close - dec!(3),
        close,
        volume: Some(dec!(12)),
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn ibkr_bars_are_idempotent_per_contract_and_refuse_to_repaint() {
    let s = store().await;
    let id = cfds_storage::test_db::unique_id("ITIB");
    let batch = vec![
        row(&id, "MNQZ6", "1h", "2026-11-24T09:00:00Z", dec!(26000.25)),
        row(&id, "MNQZ6", "1h", "2026-11-24T10:00:00Z", dec!(26001)),
        // Mismo instante, otro vencimiento: no choca.
        row(&id, "MNQH7", "1h", "2026-11-24T10:00:00Z", dec!(26200)),
        row(&id, "MNQZ6", "1m", "2026-11-24T10:01:00Z", dec!(26001.5)),
    ];
    let first = s.insert_ibkr_bars(&batch).await.unwrap();
    let again = s.insert_ibkr_bars(&batch).await.unwrap();
    assert_eq!((first.inserted, again.unchanged), (4, 4));

    let series = |contract, size| IbkrSeries {
        source: "ibkr",
        instrument_id: &id,
        contract,
        bar_size: size,
        what_to_show: "TRADES",
    };
    let last = s.last_ibkr_bar_start(&series("MNQZ6", "1h")).await.unwrap();
    assert_eq!(last, Some(batch[1].start));
    assert_eq!(
        s.last_ibkr_bar_start(&series("MNQU6", "1h")).await.unwrap(),
        None
    );

    // Repintado: error y no se guarda nada del lote.
    let new_one = row(&id, "MNQZ6", "1h", "2026-11-24T11:00:00Z", dec!(26002));
    let mut no_volume = batch[0].clone();
    no_volume.volume = None;
    for bad in [
        row(&id, "MNQZ6", "1h", "2026-11-24T09:00:00Z", dec!(26009)),
        no_volume,
    ] {
        assert!(matches!(
            s.insert_ibkr_bars(&[new_one.clone(), bad]).await,
            Err(StorageError::Conflict { .. })
        ));
    }
    assert_eq!(
        s.last_ibkr_bar_start(&series("MNQZ6", "1h")).await.unwrap(),
        Some(batch[1].start)
    );
    // Desalineada, diaria o incoherente: la base la rechaza.
    let mut incoherent = row(&id, "MNQZ6", "1h", "2026-11-24T12:00:00Z", dec!(26000));
    incoherent.high = dec!(1);
    for bad in [
        row(&id, "MNQZ6", "15m", "2026-11-24T09:07:00Z", dec!(26000)),
        row(&id, "MNQZ6", "1d", "2026-11-24T00:00:00Z", dec!(26000)),
        incoherent,
    ] {
        assert!(s.insert_ibkr_bars(&[bad]).await.is_err());
    }
    // Solo agregar.
    for sql in [
        "UPDATE ibkr_bars SET close = 1 WHERE instrument_id = $1",
        "DELETE FROM ibkr_bars WHERE instrument_id = $1",
    ] {
        let r = sqlx::query(sql).bind(&id).execute(s.pool()).await;
        assert!(r.is_err(), "{sql}");
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn ibkr_bars_between_and_series_list_one_series_in_order() {
    let s = store().await;
    let id = cfds_storage::test_db::unique_id("ITIW");
    let batch = vec![
        row(&id, "MNQZ6", "1m", "2026-11-24T10:02:00Z", dec!(26002)),
        row(&id, "MNQZ6", "1m", "2026-11-24T10:00:00Z", dec!(26000)),
        row(&id, "MNQZ6", "1m", "2026-11-24T10:15:00Z", dec!(26015)),
        row(&id, "MNQH7", "1m", "2026-11-24T10:01:00Z", dec!(26201)),
        row(&id, "MNQZ6", "15m", "2026-11-24T10:00:00Z", dec!(26002)),
    ];
    s.insert_ibkr_bars(&batch).await.unwrap();
    let series = IbkrSeries {
        source: "ibkr",
        instrument_id: &id,
        contract: "MNQZ6",
        bar_size: "1m",
        what_to_show: "TRADES",
    };
    let got = s
        .ibkr_bars_between(
            &series,
            "2026-11-24T10:00:00Z".parse().unwrap(),
            "2026-11-24T10:15:00Z".parse().unwrap(),
        )
        .await
        .unwrap();
    // Solo ese contrato y tamaño, `[desde, hasta)`, en orden.
    assert_eq!(got, vec![batch[1].clone(), batch[0].clone()]);

    let mine: Vec<_> = s
        .ibkr_series()
        .await
        .unwrap()
        .into_iter()
        .filter(|k| k.instrument_id == id)
        .map(|k| (k.contract, k.bar_size))
        .collect();
    assert_eq!(
        mine,
        vec![
            ("MNQH7".to_owned(), "1m".to_owned()),
            ("MNQZ6".to_owned(), "15m".to_owned()),
            ("MNQZ6".to_owned(), "1m".to_owned()),
        ]
    );
}

fn correction(b: &IbkrBarRow, close: Decimal, volume: Option<Decimal>) -> IbkrBarCorrection {
    IbkrBarCorrection {
        source: b.source.clone(),
        instrument_id: b.instrument_id.clone(),
        contract: b.contract.clone(),
        bar_size: b.bar_size.clone(),
        what_to_show: b.what_to_show.clone(),
        start: b.start,
        open: b.open,
        high: close + dec!(5),
        low: b.low,
        close,
        volume,
        reason: "test #563".into(),
        correction_source: "ibkr_1m_aggregate".into(),
    }
}

/// #563/#575: `ibkr_bar_corrections` es de solo agregar, apunta a una vela guardada y todas las
/// lecturas devuelven la corrección más reciente; `ibkr_bars` no cambia.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn reads_use_the_latest_correction_and_corrections_are_append_only() {
    let s = store().await;
    let id = cfds_storage::test_db::unique_id("ITIC");
    let bars = vec![
        row(&id, "MNQZ6", "15m", "2026-11-25T03:30:00Z", dec!(26044)),
        row(&id, "MNQZ6", "15m", "2026-11-25T03:45:00Z", dec!(26049)),
    ];
    s.insert_ibkr_bars(&bars).await.unwrap();
    let series = IbkrSeries {
        source: "ibkr",
        instrument_id: &id,
        contract: "MNQZ6",
        bar_size: "15m",
        what_to_show: "TRADES",
    };
    let at: DateTime<Utc> = "2026-11-25T05:00:00Z".parse().unwrap();

    // Sin corrección: lo guardado.
    let read = s.ibkr_bars_closed_by(&series, at, 10).await.unwrap();
    assert_eq!(read, bars);

    assert_eq!(
        s.insert_ibkr_bar_corrections(&[correction(&bars[1], dec!(26060), Some(dec!(15)))])
            .await
            .unwrap(),
        1
    );
    // Una segunda corrección (otra transacción, más tarde) gana; sin volumen, se lee NULL.
    s.insert_ibkr_bar_corrections(&[correction(&bars[1], dec!(26061.25), None)])
        .await
        .unwrap();

    let read = s.ibkr_bars_closed_by(&series, at, 10).await.unwrap();
    assert_eq!(read[0], bars[0], "la vela sin corrección no cambia");
    assert_eq!(
        (read[1].close, read[1].high, read[1].volume),
        (dec!(26061.25), dec!(26066.25), None)
    );
    assert_eq!(read[1].open, bars[1].open);
    let between = s
        .ibkr_bars_between(&series, bars[0].start, at)
        .await
        .unwrap();
    assert_eq!(between, read);
    let latest = s
        .latest_closed_ibkr_bar("ibkr", &id, Some("15m"), at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.close, dec!(26061.25));
    let before = s
        .last_ibkr_bar_starting_before(&series, at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.close, dec!(26061.25));

    // `ibkr_bars` sigue igual: volver a cargar la vela original no es conflicto.
    let again = s.insert_ibkr_bars(&bars).await.unwrap();
    assert_eq!(again.unchanged, 2);

    // Una corrección de una vela que no existe no entra (ni las demás del lote).
    let mut ghost = correction(&bars[0], dec!(26050), None);
    ghost.start = "2026-11-25T04:00:00Z".parse().unwrap();
    assert!(
        s.insert_ibkr_bar_corrections(&[correction(&bars[0], dec!(26045), None), ghost])
            .await
            .is_err()
    );
    assert_eq!(
        s.ibkr_bars_closed_by(&series, at, 10).await.unwrap()[0],
        bars[0]
    );

    // De solo agregar: ni UPDATE ni DELETE.
    let pool = sqlx::PgPool::connect(&cfds_storage::test_db::test_database_url())
        .await
        .unwrap();
    for sql in [
        "UPDATE ibkr_bar_corrections SET close = 1 WHERE instrument_id = $1",
        "DELETE FROM ibkr_bar_corrections WHERE instrument_id = $1",
    ] {
        let err = sqlx::query(sql).bind(&id).execute(&pool).await.unwrap_err();
        assert!(err.to_string().contains("solo agregar"), "{sql}: {err}");
    }
    // Un OHLC imposible no entra.
    let mut bad = correction(&bars[0], dec!(26045), None);
    bad.low = dec!(30000);
    assert!(s.insert_ibkr_bar_corrections(&[bad]).await.is_err());
}

/// Fuente de la liquidez (#630): una serie empalmada por contrato, en el orden en que se empezaron
/// a guardar, sin repintar cuando llega la historia de un contrato nuevo.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn ibkr_candles_splice_contracts_without_repainting() {
    use cfds_core::Timeframe;
    use cfds_storage::IbkrFeed;
    let s = store().await;
    let id = cfds_storage::test_db::unique_id("ITIC");
    let feed = IbkrFeed {
        source: "ibkr",
        instrument: &id,
        what_to_show: "TRADES",
    };
    let (from, to) = (
        "2026-11-24T00:00:00Z".parse().unwrap(),
        "2026-11-25T00:00:00Z".parse().unwrap(),
    );
    let starts = |c: &[cfds_core::Candle]| {
        c.iter()
            .map(|c| (c.start.format("%H:%M").to_string(), c.close))
            .collect::<Vec<_>>()
    };
    // El contrato vigente: 10:00 y 10:15, más una de 1h que no entra en 15m.
    s.insert_ibkr_bars(&[
        row(&id, "MNQZ6", "15m", "2026-11-24T10:00:00Z", dec!(100)),
        row(&id, "MNQZ6", "15m", "2026-11-24T10:15:00Z", dec!(101)),
        row(&id, "MNQZ6", "1h", "2026-11-24T10:00:00Z", dec!(999)),
    ])
    .await
    .unwrap();
    let before = s
        .ibkr_candles(&feed, Timeframe::M15, from, to)
        .await
        .unwrap();
    assert_eq!(
        starts(&before),
        vec![("10:00".into(), dec!(100)), ("10:15".into(), dec!(101))]
    );
    assert!(before.iter().all(|c| c.timeframe == Timeframe::M15));
    // Llega el contrato nuevo con historia: antes, encima y después de las del viejo.
    s.insert_ibkr_bars(&[
        row(&id, "MNQH7", "15m", "2026-11-24T09:45:00Z", dec!(200)),
        row(&id, "MNQH7", "15m", "2026-11-24T10:15:00Z", dec!(201)),
        row(&id, "MNQH7", "15m", "2026-11-24T10:30:00Z", dec!(202)),
    ])
    .await
    .unwrap();
    let after = s
        .ibkr_candles(&feed, Timeframe::M15, from, to)
        .await
        .unwrap();
    // Lo ya usado no cambia; el nuevo sigue después de la última del viejo.
    assert_eq!(
        starts(&after),
        vec![
            ("10:00".into(), dec!(100)),
            ("10:15".into(), dec!(101)),
            ("10:30".into(), dec!(202)),
        ]
    );
    assert_eq!(&after[..2], &before[..]);
    // `[desde, hasta)` y el último inicio, de cualquier contrato.
    let one = s
        .ibkr_candles(
            &feed,
            Timeframe::M15,
            "2026-11-24T10:15:00Z".parse().unwrap(),
            "2026-11-24T10:30:00Z".parse().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(starts(&one), vec![("10:15".into(), dec!(101))]);
    let last = s
        .last_candle_start_from(
            cfds_storage::CandleFeed::Ibkr(feed),
            &"US100".parse().unwrap(),
            Timeframe::M15,
            "2026-11-24T10:20:00Z".parse().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(last, Some("2026-11-24T10:15:00Z".parse().unwrap()));
}

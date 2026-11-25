//! Velas de IBKR en PostgreSQL **real** (#536). Necesita `DATABASE_URL` de una base de pruebas
//! (`cfds_test…`); está marcado `#[ignore]` y el CI lo corre con `-- --ignored`. La tabla es de
//! solo agregar: cada corrida usa un instrumento propio.

use cfds_storage::{IbkrBarRow, IbkrSeries, PgStore, StorageError};
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
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id = format!("ITIB{nanos}");
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

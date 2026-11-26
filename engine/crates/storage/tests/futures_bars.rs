//! Lectura de las barras de 1 minuto de futuros desde PostgreSQL **real** (#248). Necesita
//! `DATABASE_URL`; está marcado `#[ignore]` y el CI lo corre con `-- --ignored`. La tabla es de
//! solo agregar y la escribe Python: el test inserta con SQL directo, con un instrumento propio
//! por corrida (la tabla no exige que exista en `instruments`). Filas de otro dataset o símbolo
//! con el mismo instrumento no se mezclan (#376).

use cfds_core::{FuturesBar, InstrumentId};
use cfds_storage::PgStore;
use cfds_storage::test_db::{unique_id, unique_number};
use chrono::{DateTime, Utc};
use rust_decimal::dec;

async fn store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let s = PgStore::connect(&url).await.unwrap();
    s.migrate().await.unwrap();
    s
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn bars_are_read_in_order_for_the_instrument_and_period() {
    let s = store().await;
    let id: InstrumentId = unique_id("ITFB").parse().unwrap();
    // El dataset no admite guiones: un número propio.
    let n = unique_number();
    let dataset = format!("T{n}.TEST");
    let other_dataset = format!("O{n}.TEST");
    for (ds, symbol, start, contract, close) in [
        (
            dataset.as_str(),
            "FDAX.c.0",
            "2026-11-18T14:32:00Z",
            8_i64,
            "24001.5",
        ),
        (
            dataset.as_str(),
            "FDAX.c.0",
            "2026-11-18T14:31:00Z",
            7,
            "24000.25",
        ),
        (
            dataset.as_str(),
            "FDAX.c.0",
            "2026-11-18T15:00:00Z",
            8,
            "24010",
        ), // fuera del período pedido
        // Filas ajenas con el mismo instrumento (#376): otro dataset y otro símbolo.
        (
            other_dataset.as_str(),
            "FDAX.c.0",
            "2026-11-18T14:33:00Z",
            9,
            "100",
        ),
        (
            dataset.as_str(),
            "FESX.c.0",
            "2026-11-18T14:31:00Z",
            9,
            "200",
        ),
    ] {
        sqlx::query(
            "INSERT INTO futures_bars_1m (source, dataset, symbol, contract_id, instrument_id,
                 start_at, open, high, low, close, volume)
             VALUES ('databento', $1, $2, $3, $4, $5, $6::numeric, $6::numeric + 1,
                 $6::numeric - 1, $6::numeric, 12)",
        )
        .bind(ds)
        .bind(symbol)
        .bind(contract)
        .bind(id.as_str())
        .bind(at(start))
        .bind(close)
        .execute(s.pool())
        .await
        .unwrap();
    }
    let bars = s
        .futures_bars(
            "databento",
            &dataset,
            "FDAX.c.0",
            &id,
            at("2026-11-18T14:00:00Z"),
            at("2026-11-18T15:00:00Z"),
        )
        .await
        .unwrap();
    let expected = |start: &str, contract: u64, close| FuturesBar {
        source: "databento".into(),
        dataset: dataset.clone(),
        symbol: "FDAX.c.0".into(),
        instrument_id: id.clone(),
        contract_id: contract,
        start: at(start),
        open: close,
        high: close + dec!(1),
        low: close - dec!(1),
        close,
        volume: 12,
    };
    assert_eq!(
        bars,
        vec![
            expected("2026-11-18T14:31:00Z", 7, dec!(24000.25)),
            expected("2026-11-18T14:32:00Z", 8, dec!(24001.5)),
        ]
    );
    // Otra fuente no se mezcla.
    let other = s
        .futures_bars(
            "otra",
            &dataset,
            "FDAX.c.0",
            &id,
            at("2026-11-18T14:00:00Z"),
            at("2026-11-18T15:00:00Z"),
        )
        .await
        .unwrap();
    assert!(other.is_empty());
    // Cada dataset y símbolo trae solo lo suyo (#376).
    let foreign = |ds: String, symbol: &'static str| {
        let (s, id) = (&s, &id);
        async move {
            s.futures_bars(
                "databento",
                &ds,
                symbol,
                id,
                at("2026-11-18T14:00:00Z"),
                at("2026-11-18T15:00:00Z"),
            )
            .await
            .unwrap()
        }
    };
    let by_dataset = foreign(other_dataset.clone(), "FDAX.c.0").await;
    assert_eq!(by_dataset.len(), 1);
    assert_eq!(by_dataset[0].dataset, other_dataset);
    assert_eq!(by_dataset[0].contract_id, 9);
    let by_symbol = foreign(dataset.clone(), "FESX.c.0").await;
    assert_eq!(by_symbol.len(), 1);
    assert_eq!(by_symbol[0].symbol, "FESX.c.0");
    assert_eq!(by_symbol[0].close, dec!(200));
}

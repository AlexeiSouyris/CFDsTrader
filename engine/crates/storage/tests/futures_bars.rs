//! Lectura de las barras de 1 minuto de futuros desde PostgreSQL **real** (#248). Necesita
//! `DATABASE_URL`; está marcado `#[ignore]` y el CI lo corre con `-- --ignored`. La tabla es de
//! solo agregar y la escribe Python: el test inserta con SQL directo, con un instrumento propio
//! por corrida (la tabla no exige que exista en `instruments`).

use cfds_core::{FuturesBar, InstrumentId};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use rust_decimal::dec;

async fn store() -> PgStore {
    let url = std::env::var("DATABASE_URL").expect("falta DATABASE_URL (necesita PostgreSQL)");
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
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id: InstrumentId = format!("ITFB{nanos}").parse().unwrap();
    let dataset = format!("T{nanos}.TEST");
    for (start, contract, close) in [
        ("2026-11-18T14:32:00Z", 8_i64, "24001.5"),
        ("2026-11-18T14:31:00Z", 7, "24000.25"),
        ("2026-11-18T15:00:00Z", 8, "24010"), // fuera del período pedido
    ] {
        sqlx::query(
            "INSERT INTO futures_bars_1m (source, dataset, symbol, contract_id, instrument_id,
                 start_at, open, high, low, close, volume)
             VALUES ('databento', $1, 'FDAX.c.0', $2, $3, $4, $5::numeric, $5::numeric + 1,
                 $5::numeric - 1, $5::numeric, 12)",
        )
        .bind(&dataset)
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
            &id,
            at("2026-11-18T14:00:00Z"),
            at("2026-11-18T15:00:00Z"),
        )
        .await
        .unwrap();
    assert!(other.is_empty());
}

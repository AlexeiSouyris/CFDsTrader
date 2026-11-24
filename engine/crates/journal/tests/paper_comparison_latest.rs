//! La última comparación paper vs backtest de cada instrumento (#331), contra PostgreSQL real.
//! Necesita `DATABASE_URL`; marcado `#[ignore]` (el CI lo corre con `-- --ignored`). Las tablas
//! son de solo agregar: cada corrida usa su propio instrumento. Documentos de ejemplo.

use cfds_core::{AssetClass, Currency, Instrument};
use cfds_journal::pg::PgJournal;
use cfds_storage::PgStore;
use chrono::Utc;
use serde_json::{Value, json};

fn doc(id: &str, instrument: &str, from: &str, to: &str, created: &str, diff: &str) -> Value {
    json!({
        "comparison_id": id,
        "instrument_id": instrument,
        "from": from,
        "to": to,
        "created_at": created,
        "paper": { "trades": 0, "net_pnl": "0" },
        "backtest": { "trades": 0, "net_pnl": "0" },
        "difference": diff,
        "causes": {
            "fill_price": diff, "size": "0", "commission": "0", "swap": "0",
            "weekend_close": "0", "risk_rejection": "0", "missing_in_paper": "0",
            "extra_in_paper": "0"
        },
        "matched": [], "missing": [], "extra": [], "paper_rejections": {}
    })
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn latest_per_instrument_is_the_most_recent_period() {
    let url = cfds_storage::test_db::test_database_url();
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    let n = Utc::now().timestamp_nanos_opt().unwrap();
    let ids: Vec<String> = ["A", "B"].iter().map(|s| format!("PC{s}{n}")).collect();
    for id in &ids {
        store
            .upsert_instrument(&Instrument {
                id: id.parse().unwrap(),
                name: "prueba".into(),
                asset_class: AssetClass::Index,
                currency: Currency::USD,
                exchange_timezone: "America/New_York".into(),
            })
            .await
            .unwrap();
    }
    let j = PgJournal::new(store.pool().clone());
    let (a, b) = (&ids[0], &ids[1]);
    for d in [
        doc(
            &format!("{a}:old"),
            a,
            "2026-10-01T00:00:00Z",
            "2026-10-31T00:00:00Z",
            "2026-11-01T00:00:00Z",
            "-1",
        ),
        doc(
            &format!("{a}:new"),
            a,
            "2026-11-01T00:00:00Z",
            "2026-11-21T00:00:00Z",
            "2026-11-22T00:00:00Z",
            "-2",
        ),
        // Mismo período, creada más tarde: gana esta.
        doc(
            &format!("{a}:redo"),
            a,
            "2026-11-01T00:00:00Z",
            "2026-11-21T00:00:00Z",
            "2026-11-23T00:00:00Z",
            "-3",
        ),
        doc(
            &format!("{b}:only"),
            b,
            "2026-11-01T00:00:00Z",
            "2026-11-21T00:00:00Z",
            "2026-11-22T00:00:00Z",
            "4",
        ),
    ] {
        j.record_paper_comparison(&d).await.unwrap();
    }
    let latest: Vec<Value> = j
        .latest_paper_comparisons()
        .await
        .unwrap()
        .into_iter()
        .filter(|d| ids.iter().any(|i| d["instrument_id"] == i.as_str()))
        .collect();
    let got: Vec<&str> = latest
        .iter()
        .map(|d| d["comparison_id"].as_str().unwrap())
        .collect();
    assert_eq!(got, [format!("{a}:redo"), format!("{b}:only")]);
}

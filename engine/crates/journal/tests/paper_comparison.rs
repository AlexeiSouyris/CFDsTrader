//! Comparación paper vs backtest (`paper_comparison.v1`, fase 8, #315): en PostgreSQL se guarda
//! una sola vez y nunca se reescribe. Documento de ejemplo, no de mercado.

use cfds_core::{AssetClass, Currency, Instrument};
use cfds_journal::pg::{JournalError, PgJournal, Recorded};
use cfds_storage::PgStore;
use chrono::Utc;
use serde_json::{Value, json};

fn sample(id: &str) -> Value {
    json!({
        "comparison_id": id,
        "instrument_id": "US100",
        "from": "2026-11-02T00:00:00Z",
        "to": "2026-11-21T00:00:00Z",
        "created_at": "2026-11-23T10:00:00Z",
        "paper": { "trades": 1, "net_pnl": "-1.5" },
        "backtest": { "trades": 1, "net_pnl": "-1" },
        "difference": "-0.5",
        "causes": {
            "fill_price": "0", "size": "0", "commission": "0", "swap": "-0.5",
            "weekend_close": "0", "risk_rejection": "0", "missing_in_paper": "0",
            "extra_in_paper": "0"
        },
        "matched": [],
        "missing": [],
        "extra": [],
        "paper_rejections": {}
    })
}

#[test]
fn the_sample_follows_the_contract() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/paper_comparison.v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    assert!(v.is_valid(&sample("paper-vs-backtest:US100:x")));
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn a_comparison_is_stored_once_and_never_rewritten() {
    let url = cfds_storage::test_db::test_database_url();
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    store
        .upsert_instrument(&Instrument {
            id: "US100".parse().unwrap(),
            name: "US Tech 100".into(),
            asset_class: AssetClass::Index,
            currency: Currency::USD,
            exchange_timezone: "America/New_York".into(),
        })
        .await
        .unwrap();
    let j = PgJournal::new(store.pool().clone());
    // Solo agregar: un id propio por corrida del test.
    let id = format!(
        "paper-vs-backtest:US100:{}",
        Utc::now().timestamp_nanos_opt().unwrap()
    );
    let doc = sample(&id);
    assert!(j.paper_comparison(&id).await.unwrap().is_none());
    assert_eq!(
        j.record_paper_comparison(&doc).await.unwrap(),
        Recorded::Inserted
    );
    assert_eq!(j.paper_comparison(&id).await.unwrap(), Some(doc.clone()));
    assert_eq!(
        j.record_paper_comparison(&doc).await.unwrap(),
        Recorded::Unchanged
    );
    let mut other = doc.clone();
    other["difference"] = json!("-0.6");
    assert!(matches!(
        j.record_paper_comparison(&other).await,
        Err(JournalError::Conflict { .. })
    ));
    let mut broken = doc.clone();
    broken["from"] = json!("ayer");
    assert!(matches!(
        j.record_paper_comparison(&broken).await,
        Err(JournalError::InvalidComparison("from"))
    ));
    for sql in [
        "UPDATE paper_comparisons SET period_to = period_to WHERE comparison_id = $1",
        "DELETE FROM paper_comparisons WHERE comparison_id = $1",
    ] {
        assert!(
            sqlx::query(sql)
                .bind(&id)
                .execute(store.pool())
                .await
                .is_err(),
            "{sql}"
        );
    }
}

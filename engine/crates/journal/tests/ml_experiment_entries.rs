//! Los experimentos de ML con el instrumento de su corrida (#426), contra PostgreSQL real.
//! Necesita `DATABASE_URL` de pruebas (`cfds_test…`); marcado `#[ignore]` (el CI lo corre con
//! `-- --ignored`). Las tablas son de solo agregar: cada corrida usa sus propios instrumentos y
//! ids. Los experimentos se insertan como lo hace Python (`cfds_intel.models`). Datos de ejemplo.

use cfds_core::{AssetClass, Currency, Instrument};
use cfds_journal::pg::PgJournal;
use cfds_storage::PgStore;
use cfds_storage::test_db::{unique_id, unique_number};
use serde_json::{Value, json};

/// Un id hexadecimal de 64 caracteres distinto para cada experimento.
fn hex64(n: u64, k: u8) -> String {
    format!("{n:0>62x}{k:02x}")
}

fn document(id: &str, run: &str, source: &str, created: &str) -> Value {
    json!({
        "experiment_id": id,
        "created_at": created,
        "code_version": "3e8f703",
        "backtest_run_id": run,
        "label_source": source,
        "config_sha256": "b".repeat(64),
        "features": ["side_long", "score"],
        "hyperparameters": {
            "l2_grid": [1.0], "inner_tune_fraction": 0.2, "max_iterations": 200, "seed": 7,
            "min_examples": 300, "min_per_class": 30, "bootstrap_resamples": 100,
            "confidence": 0.95
        },
        "windows": [],
        "pooled": {},
        "result": { "status": "suspended", "difference": null, "low": null, "high": null, "blocks": null }
    })
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn entries_carry_the_instrument_of_their_run() {
    let url = cfds_storage::test_db::test_database_url();
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    let pool = store.pool().clone();
    let n = unique_number();
    let label_inst = unique_id("MLA");
    let (bt_a, bt_b) = (unique_id("MLB"), unique_id("MLC"));
    store
        .upsert_instrument(&Instrument {
            id: label_inst.parse().unwrap(),
            name: "prueba".into(),
            asset_class: AssetClass::Index,
            currency: Currency::USD,
            exchange_timezone: "America/New_York".into(),
        })
        .await
        .unwrap();

    let label_run = unique_id("mll");
    sqlx::query(
        "INSERT INTO ml_label_runs (run_id, instrument_id, candle_source, from_at, to_at,
                                    method, config_sha256)
         VALUES ($1, $2, 'futures', '2011-01-01Z', '2026-01-01Z', 'mid_no_costs', $3)",
    )
    .bind(&label_run)
    .bind(&label_inst)
    .bind("b".repeat(64))
    .execute(&pool)
    .await
    .unwrap();
    // una corrida de backtest con dos instrumentos: una entrada por cada uno
    let bt_run = unique_id("mlbt");
    sqlx::query(
        "INSERT INTO backtest_runs (run_id, instruments, timeframe, from_at, to_at, config_sha256,
                                    initial_equity, costs, metrics, started_at, finished_at)
         VALUES ($1, $2, '15m', '2025-01-01Z', '2026-01-01Z', $3, 10000, '{}', '{}',
                 '2026-01-02Z', '2026-01-02Z')",
    )
    .bind(&bt_run)
    .bind(json!([bt_a, bt_b]))
    .bind("b".repeat(64))
    .execute(&pool)
    .await
    .unwrap();

    let insert = |id: String, run: String, source: &'static str, created: &'static str| {
        let pool = pool.clone();
        async move {
            let doc = document(&id, &run, source, created);
            sqlx::query(
                "INSERT INTO ml_experiments (experiment_id, created_at, code_version,
                     backtest_run_id, config_sha256, status, document, label_source)
                 VALUES ($1, $2::timestamptz, '3e8f703', $3, $4, 'suspended', $5, $6)",
            )
            .bind(&id)
            .bind(created)
            .bind(&run)
            .bind("b".repeat(64))
            .bind(&doc)
            .bind(source)
            .execute(&pool)
            .await
            .unwrap();
            doc
        }
    };
    let old = insert(
        hex64(n, 1),
        label_run.clone(),
        "ml_label_runs",
        "2026-11-24T03:17:19Z",
    )
    .await;
    let new = insert(
        hex64(n, 2),
        label_run.clone(),
        "ml_label_runs",
        "2026-11-24T03:34:19Z",
    )
    .await;
    let bt = insert(
        hex64(n, 3),
        bt_run.clone(),
        "backtest_runs",
        "2026-11-24T03:00:00Z",
    )
    .await;

    let j = PgJournal::new(pool.clone());
    let mine = [label_inst.as_str(), bt_a.as_str(), bt_b.as_str()];
    // sin filtro: el último de cada instrumento, ordenado por instrumento
    let latest: Vec<Value> = j
        .ml_experiment_entries(None, 50)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| mine.contains(&e["instrument_id"].as_str().unwrap()))
        .collect();
    assert_eq!(
        latest,
        [
            json!({ "instrument_id": label_inst, "label_source": "ml_label_runs", "experiment": new }),
            json!({ "instrument_id": bt_a, "label_source": "backtest_runs", "experiment": bt }),
            json!({ "instrument_id": bt_b, "label_source": "backtest_runs", "experiment": bt }),
        ]
    );
    // con filtro: todos los de ese instrumento, del más nuevo al más viejo, con límite
    let history = j
        .ml_experiment_entries(Some(&label_inst), 50)
        .await
        .unwrap();
    let docs: Vec<&Value> = history.iter().map(|e| &e["experiment"]).collect();
    assert_eq!(docs, [&new, &old]);
    let one = j.ml_experiment_entries(Some(&label_inst), 1).await.unwrap();
    assert_eq!(one.len(), 1);
    assert!(
        j.ml_experiment_entries(Some("NADA.ACA"), 50)
            .await
            .unwrap()
            .is_empty()
    );
}

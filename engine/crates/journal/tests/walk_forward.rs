//! Reporte de la Fase 5 (`walk_forward_report.v1`, #298): cumple el contrato, se lee igual, y en
//! PostgreSQL se guarda una sola vez y nunca se reescribe. Números de ejemplo, no de mercado.

use cfds_backtest::{
    Aggregate, BuyAndHold, FoldResult, Metrics, ParamFile, PeriodResult, SensitivityPoint,
    Stability,
};
use cfds_core::{AssetClass, Currency, Instrument};
use cfds_journal::pg::{JournalError, PgJournal, Recorded};
use cfds_journal::{CandleSourceName, ReportHeader, WalkForwardReport};
use cfds_market_data::{Period, SplitConfig};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use rust_decimal::dec;

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn validator() -> jsonschema::Validator {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/walk_forward_report.v1.schema.json");
    let text = std::fs::read_to_string(path).unwrap();
    jsonschema::validator_for(&serde_json::from_str(&text).unwrap()).unwrap()
}

fn metrics() -> Metrics {
    Metrics {
        total_return: dec!(-0.0125),
        max_drawdown: dec!(0.03),
        sharpe: Some(dec!(-0.42)),
        sortino: None,
        trades: 3,
        win_rate: Some(dec!(0.3333)),
        profit_factor: Some(dec!(0.8)),
        exposure: Some(dec!(0.12)),
    }
}

fn period(a: &str, b: &str, with_data: bool) -> PeriodResult {
    PeriodResult {
        period: Period {
            start: at(a),
            end: at(b),
        },
        candles: if with_data { 2000 } else { 0 },
        metrics: with_data.then(metrics),
        final_equity: if with_data { dec!(987.5) } else { dec!(1000) },
        buy_and_hold: with_data.then_some(BuyAndHold {
            entry_price: dec!(100.7),
            exit_price: dec!(104.3),
            nights: 20,
            commission: dec!(0),
            funding: dec!(0.35),
            gross_pnl: dec!(3.6),
            net_pnl: dec!(3.25),
        }),
        buy_and_hold_return: with_data.then_some(dec!(0.00325)),
    }
}

fn sample(report_id: &str) -> WalkForwardReport {
    let folds = vec![FoldResult {
        index: 0,
        train: period("2023-01-01T00:00:00Z", "2025-01-01T00:00:00Z", true),
        // Un fold vacío: sin métricas ni comprar y mantener.
        test: period("2025-01-01T00:00:00Z", "2025-04-01T00:00:00Z", false),
    }];
    let agg = Aggregate {
        periods: 1,
        with_data: 0,
        trades: 0,
        mean_return: None,
        median_return: None,
        positive: 0,
        worst_drawdown: None,
        mean_sharpe: None,
        sharpe_defined: 0,
        beat_buy_and_hold: 0,
        with_buy_and_hold: 0,
    };
    let point = |value: &str| SensitivityPoint {
        file: ParamFile::Strategy,
        key: "score.min_score".into(),
        value: value.into(),
        fold: 0,
        metrics: Some(metrics()),
    };
    let stab = Stability {
        file: ParamFile::Strategy,
        key: "score.min_score".into(),
        by_value: vec![
            ("4".into(), Some(dec!(0.01)), None, 5),
            ("5".into(), Some(dec!(-0.0125)), Some(dec!(-0.42)), 3),
        ],
        return_spread: Some(dec!(0.0225)),
        same_sign: false,
    };
    WalkForwardReport::new(
        ReportHeader {
            report_id: report_id.into(),
            instrument_id: "US100".parse().unwrap(),
            candle_source: CandleSourceName::Futures,
            config_sha256: "a".repeat(64),
            split: SplitConfig {
                out_of_sample_months: 12,
                train_months: 24,
                validation_months: 3,
                step_months: 3,
            },
            warmup_days: 30,
            initial_equity: dec!(1000),
            buy_and_hold_size: dec!(1),
            computed_at: at("2026-11-23T10:00:00Z"),
        },
        &folds,
        &agg,
        &[point("4"), point("5")],
        &[stab],
        &period("2025-11-01T00:00:00Z", "2026-11-01T00:00:00Z", true),
    )
}

#[test]
fn a_report_validates_against_its_contract_and_reads_back_identical() {
    let r = sample("wf-1");
    let json = serde_json::to_value(&r).unwrap();
    let v = validator();
    let errors: Vec<String> = v.iter_errors(&json).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(json["folds"][0]["test"]["metrics"], serde_json::Value::Null);
    assert_eq!(json["holdout"]["buy_and_hold"]["return"], "0.00325");
    let back: WalkForwardReport = serde_json::from_value(json).unwrap();
    assert_eq!(back, r);
}

#[test]
fn the_contract_rejects_what_rust_never_writes() {
    let v = validator();
    let good = serde_json::to_value(sample("wf-2")).unwrap();
    let mut bad = Vec::new();
    let mut b = good.clone();
    b["report_id"] = "con espacios".into();
    bad.push(b);
    let mut b = good.clone();
    b["sensitivity"][0]["file"] = "patterns.toml".into();
    bad.push(b);
    let mut b = good.clone();
    b["holdout"]["metrics"]["total_return"] = serde_json::json!(0.1);
    bad.push(b);
    let mut b = good.clone();
    b["extra"] = true.into();
    bad.push(b);
    for b in bad {
        assert!(!v.is_valid(&b), "{b}");
    }
    let mut r = sample("wf-3");
    r.config_sha256 = "x".into();
    assert!(!r.valid_id());
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn a_report_is_stored_once_and_never_rewritten() {
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
    let id = format!("wf-{}", Utc::now().timestamp_nanos_opt().unwrap());
    let r = sample(&id);
    assert!(j.walk_forward(&id).await.unwrap().is_none());
    assert_eq!(j.record_walk_forward(&r).await.unwrap(), Recorded::Inserted);
    assert_eq!(j.walk_forward(&id).await.unwrap(), Some(r.clone()));
    let mut later = r.clone();
    later.computed_at = r.computed_at + chrono::TimeDelta::hours(1);
    assert_eq!(
        j.record_walk_forward(&later).await.unwrap(),
        Recorded::Unchanged
    );
    let mut other = r.clone();
    other.warmup_days = 10;
    assert!(matches!(
        j.record_walk_forward(&other).await,
        Err(JournalError::Conflict { .. })
    ));
    // El último del instrumento: el más nuevo por computed_at.
    let newer_id = format!("{id}-b");
    let mut newer = sample(&newer_id);
    newer.computed_at = r.computed_at + chrono::TimeDelta::days(1);
    j.record_walk_forward(&newer).await.unwrap();
    let latest = j.latest_walk_forward("US100").await.unwrap().unwrap();
    assert!(latest.computed_at >= newer.computed_at);
    assert!(j.latest_walk_forward("ZZ99").await.unwrap().is_none());
    for sql in [
        "UPDATE walk_forward_reports SET candle_source = 'ig' WHERE report_id = $1",
        "DELETE FROM walk_forward_reports WHERE report_id = $1",
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

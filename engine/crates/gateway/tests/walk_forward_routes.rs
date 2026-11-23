//! Rutas del walk-forward (#316): solo lectura, el reporte tal cual lo guardó el motor, `null`
//! si el instrumento no tiene, y pedidos mal armados rechazados. Números de ejemplo.

use std::path::PathBuf;

use cfds_backtest::{Aggregate, BuyAndHold, FoldResult, Metrics, PeriodResult};
use cfds_gateway::walk_forward::{WalkForwardSource, is_walk_forward_route, respond_walk_forward};
use cfds_journal::{CandleSourceName, ReportHeader, WalkForwardReport};
use cfds_market_data::{Period, SplitConfig};
use rust_decimal::dec;
use serde_json::Value;

fn period(a: &str, b: &str) -> PeriodResult {
    PeriodResult {
        period: Period {
            start: a.parse().unwrap(),
            end: b.parse().unwrap(),
        },
        candles: 100,
        metrics: Some(Metrics {
            total_return: dec!(0.012),
            max_drawdown: dec!(0.03),
            sharpe: Some(dec!(0.8)),
            sortino: None,
            trades: 4,
            win_rate: Some(dec!(0.5)),
            profit_factor: Some(dec!(1.2)),
            exposure: Some(dec!(0.1)),
        }),
        final_equity: dec!(1012),
        buy_and_hold: Some(BuyAndHold {
            entry_price: dec!(100),
            exit_price: dec!(101),
            nights: 3,
            commission: dec!(0),
            funding: dec!(0.02),
            gross_pnl: dec!(1),
            net_pnl: dec!(0.98),
        }),
        buy_and_hold_return: Some(dec!(0.00098)),
    }
}

fn report(id: &str, instrument: &str) -> WalkForwardReport {
    let agg = Aggregate {
        periods: 1,
        with_data: 1,
        trades: 4,
        mean_return: Some(dec!(0.012)),
        median_return: Some(dec!(0.012)),
        positive: 1,
        worst_drawdown: Some(dec!(0.03)),
        mean_sharpe: Some(dec!(0.8)),
        sharpe_defined: 1,
        beat_buy_and_hold: 1,
        with_buy_and_hold: 1,
    };
    WalkForwardReport::new(
        ReportHeader {
            report_id: id.into(),
            instrument_id: instrument.parse().unwrap(),
            candle_source: CandleSourceName::Futures,
            config_sha256: "b".repeat(64),
            split: SplitConfig {
                out_of_sample_months: 12,
                train_months: 24,
                validation_months: 3,
                step_months: 3,
            },
            warmup_days: 30,
            initial_equity: dec!(1000),
            buy_and_hold_size: dec!(1),
            computed_at: "2026-11-23T10:00:00Z".parse().unwrap(),
        },
        &[FoldResult {
            index: 0,
            train: period("2023-01-01T00:00:00Z", "2025-01-01T00:00:00Z"),
            test: period("2025-01-01T00:00:00Z", "2025-04-01T00:00:00Z"),
        }],
        &agg,
        &[],
        &[],
        &period("2025-11-01T00:00:00Z", "2026-11-01T00:00:00Z"),
    )
}

struct Fake(Vec<WalkForwardReport>, bool);

impl WalkForwardSource for Fake {
    async fn latest(&self, instrument: &str) -> Result<Option<WalkForwardReport>, String> {
        if self.1 {
            return Err("base caída".into());
        }
        Ok(self
            .0
            .iter()
            .find(|r| r.instrument_id.as_str() == instrument)
            .cloned())
    }
    async fn by_id(&self, id: &str) -> Result<Option<WalkForwardReport>, String> {
        Ok(self.0.iter().find(|r| r.report_id == id).cloned())
    }
}

async fn call(src: &Fake, method: &str, path: &str, query: Option<&str>) -> (u16, Value) {
    let r = respond_walk_forward(src, method, path, query).await;
    (
        r.status,
        serde_json::from_str(&r.body).unwrap_or(Value::Null),
    )
}

fn assert_valid(doc: &Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/walk_forward_report.v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[tokio::test]
async fn the_latest_report_of_an_instrument_comes_as_stored() {
    let src = Fake(vec![report("wf-a", "DE40.FUT")], false);
    let (status, body) = call(
        &src,
        "GET",
        "/api/v1/walk_forward",
        Some("instrument=DE40.FUT"),
    )
    .await;
    assert_eq!(status, 200);
    assert_valid(&body);
    assert_eq!(body["report_id"], "wf-a");
    assert_eq!(
        body["folds"][0]["test"]["buy_and_hold"]["return"],
        "0.00098"
    );
    assert_eq!(body["holdout"]["metrics"]["total_return"], "0.012");
}

#[tokio::test]
async fn an_instrument_without_report_is_null_not_an_error() {
    let src = Fake(vec![report("wf-a", "DE40.FUT")], false);
    let r = respond_walk_forward(
        &src,
        "GET",
        "/api/v1/walk_forward",
        Some("instrument=US500"),
    )
    .await;
    assert_eq!((r.status, r.body.as_str()), (200, "null"));
}

#[tokio::test]
async fn a_report_by_id_or_404() {
    let src = Fake(vec![report("wf-a", "DE40.FUT")], false);
    let (status, body) = call(&src, "GET", "/api/v1/walk_forward/wf-a", None).await;
    assert_eq!(
        (status, body["instrument_id"].as_str()),
        (200, Some("DE40.FUT"))
    );
    let (status, _) = call(&src, "GET", "/api/v1/walk_forward/otro", None).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn bad_requests_and_writes_are_rejected() {
    let src = Fake(vec![report("wf-a", "DE40.FUT")], false);
    for (method, path, query, want) in [
        (
            "POST",
            "/api/v1/walk_forward",
            Some("instrument=DE40.FUT"),
            405,
        ),
        ("DELETE", "/api/v1/walk_forward/wf-a", None, 405),
        ("GET", "/api/v1/walk_forward", None, 400),
        ("GET", "/api/v1/walk_forward", Some("instrument=de40"), 400),
        (
            "GET",
            "/api/v1/walk_forward",
            Some("instrument=DE40&x=1"),
            400,
        ),
        (
            "GET",
            "/api/v1/walk_forward",
            Some("instrument=A&instrument=B"),
            400,
        ),
        ("GET", "/api/v1/walk_forward/con%20espacio", None, 400),
        ("GET", "/api/v1/walk_forward/", None, 400),
    ] {
        let (status, _) = call(&src, method, path, query).await;
        assert_eq!(status, want, "{method} {path} {query:?}");
    }
    assert!(!is_walk_forward_route("/api/v1/walk_forwardx"));
    assert!(!is_walk_forward_route("/api/v1/fund"));
}

#[tokio::test]
async fn a_database_error_is_a_500_without_details() {
    let src = Fake(Vec::new(), true);
    let r =
        respond_walk_forward(&src, "GET", "/api/v1/walk_forward", Some("instrument=DE40")).await;
    assert_eq!((r.status, r.body.as_str()), (500, "error interno"));
}

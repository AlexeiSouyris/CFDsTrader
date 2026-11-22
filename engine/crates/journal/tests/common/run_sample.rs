//! Corrida de ejemplo para los tests de `backtest_run.v1` (#200).

use cfds_backtest::{Commission, CostConfig, InstrumentCosts, Metrics};
use cfds_journal::{BacktestRun, RunCosts, RunMetrics, RunNews, RunTimeframe, config_sha256};
use chrono::{DateTime, Utc};
use rust_decimal::dec;

pub fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

/// Una corrida de ejemplo; la usan también los tests de PostgreSQL.
pub fn sample(run_id: &str) -> BacktestRun {
    let costs = InstrumentCosts {
        spread: dec!(1),
        tick: dec!(0.1),
        commission: Commission::None,
        funding_admin_fee_pct: dec!(2.5),
        funding_day_count: dec!(360),
    };
    let cfg = CostConfig {
        slippage_spread_fraction: dec!(0.5),
        slippage_ticks: dec!(1),
    };
    let metrics = Metrics {
        total_return: dec!(-0.0125),
        max_drawdown: dec!(0.0300),
        sharpe: Some(dec!(-0.42)),
        sortino: None,
        trades: 3,
        win_rate: Some(dec!(0.3333)),
        profit_factor: Some(dec!(0.8)),
        exposure: Some(dec!(0.12)),
    };
    BacktestRun {
        run_id: run_id.to_owned(),
        instruments: vec!["US100".parse().unwrap()],
        timeframe: RunTimeframe::M15,
        from: at("2026-11-02T00:00:00Z"),
        to: at("2026-11-21T00:00:00Z"),
        config_sha256: config_sha256(&[("strategy.toml", "min_score = 5")]),
        initial_equity: dec!(10000),
        costs: RunCosts::new(&cfg, &[("US100".parse().unwrap(), costs)]),
        news: Some(RunNews {
            interpreter_name: "ollama".into(),
            interpreter_version: "p2".into(),
            interpreter_model: Some("modelo@digest".into()),
            published_from: at("2025-12-03T00:00:00Z"),
            interpretations: 0,
        }),
        metrics: RunMetrics::new(dec!(9875), &metrics),
        started_at: at("2026-11-23T10:00:00Z"),
        finished_at: at("2026-11-23T10:00:42.5Z"),
    }
}

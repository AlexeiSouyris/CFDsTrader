//! Exportación para la verificación (#81): contrato, tabla por operación y sin repintado.

use std::path::PathBuf;

use cfds_backtest::{BacktestConfig, chart_export, run, setup_from_catalog};
use cfds_core::{Candle, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::Catalog;
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::Value;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn setup() -> cfds_backtest::Setup {
    setup_from_catalog(
        &Catalog::from_toml(cfg!("instruments.toml")).unwrap(),
        &"DE40".parse().unwrap(),
        Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
            trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
        },
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        dec!(10000),
    )
    .unwrap()
}

/// Caminata aleatoria determinística de velas de 15m del DE40 (sintéticas: no son de IG).
fn random_walk(n: usize, seed: u64) -> Vec<Candle> {
    let mut state = seed;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let t0: DateTime<Utc> = "2026-01-05T00:00:00Z".parse().unwrap();
    let mut close = dec!(20000);
    (0..n)
        .map(|i| {
            let drift = if (i / 150) % 2 == 0 { 1 } else { -1 };
            let open = close;
            close = open + Decimal::new(next(81) - 40 + drift * 4, 1);
            Candle {
                instrument_id: "DE40".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * i as i64),
                open,
                high: open.max(close) + Decimal::new(next(50), 1),
                low: open.min(close) - Decimal::new(next(50), 1),
                close,
                volume: None,
            }
        })
        .collect()
}

fn resample_h1(m15: &[Candle]) -> Vec<Candle> {
    m15.chunks_exact(4)
        .map(|g| Candle {
            instrument_id: g[0].instrument_id.clone(),
            timeframe: Timeframe::H1,
            start: g[0].start,
            open: g[0].open,
            high: g.iter().map(|c| c.high).max().unwrap(),
            low: g.iter().map(|c| c.low).min().unwrap(),
            close: g[3].close,
            volume: None,
        })
        .collect()
}

fn export(m15: &[Candle]) -> cfds_backtest::ChartExport {
    let h1 = resample_h1(m15);
    let s = setup();
    let report = run(m15, &h1, &[], &[], &s).unwrap();
    chart_export(m15, &h1, &s, &report).unwrap()
}

fn validator() -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/chart_export.v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

#[test]
fn the_export_follows_its_contract_and_roundtrips() {
    let e = export(&random_walk(2000, 5));
    assert!(e.trades.len() > 3, "solo {} operaciones", e.trades.len());
    assert!(!e.pivots.is_empty());
    let json = serde_json::to_value(&e).unwrap();
    let errors: Vec<String> = validator()
        .iter_errors(&json)
        .map(|x| x.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        serde_json::from_value::<cfds_backtest::ChartExport>(json).unwrap(),
        e
    );
}

#[test]
fn the_contract_rejects_floats_and_unknown_names() {
    let json = serde_json::to_value(export(&random_walk(2000, 5))).unwrap();
    let v = validator();
    let mut bad = json.clone();
    bad["candles"][100]["close"] = serde_json::json!(20000.5);
    assert!(!v.is_valid(&bad), "precio como número JSON");
    let mut bad = json.clone();
    bad["trades"][0]["exits"][0]["reason"] = serde_json::json!("Stop");
    assert!(!v.is_valid(&bad), "motivo fuera del contrato");
    let mut bad = json;
    bad["candles"][100]["trend_1h"] = serde_json::json!("Up");
    assert!(!v.is_valid(&bad), "tendencia fuera del contrato");
}

#[test]
fn each_trade_carries_the_indicator_values_of_its_signal_candle() {
    let e = export(&random_walk(2000, 5));
    for t in &e.trades {
        let c = &e.candles[t.signal_index];
        assert_eq!(t.at_signal.close, c.close);
        assert_eq!(t.at_signal.sma, c.sma);
        assert_eq!(t.at_signal.atr, c.atr);
        assert_eq!(t.at_signal.trend_1h, c.trend_1h);
        assert!(t.entry_index == t.signal_index + 1, "entra en t+1");
        let side_score = if t.side == "long" {
            c.long_score
        } else {
            c.short_score
        };
        assert_eq!(
            side_score,
            Some(t.score),
            "el puntaje exportado es el de la operación"
        );
        let total: i32 = t.breakdown.iter().map(|b| b.points).sum();
        assert_eq!(total, t.score);
    }
}

#[test]
fn no_exported_value_changes_when_more_candles_arrive() {
    // ROADMAP 3b: "ningún valor histórico cambia al agregar datos".
    let full = random_walk(2000, 9);
    let all = export(&full);
    for n in [400, 1000, 1600] {
        let part = export(&full[..n]);
        assert_eq!(part.candles[..], all.candles[..n], "velas con {n}");
        let confirmed: Vec<_> = all
            .pivots
            .iter()
            .filter(|p| p.confirmed_index < n)
            .cloned()
            .collect();
        assert_eq!(part.pivots, confirmed, "pivotes con {n}");
    }
}

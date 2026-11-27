//! Vista previa de riesgo del laboratorio (#777): el mismo `cfds_risk::check` del broker
//! simulado con una cuenta hipotética, sobre el catálogo y `config/risk.toml` reales. Velas
//! sintéticas (no son precios de mercado).

use std::path::PathBuf;

use cfds_backtest::BacktestConfig;
use cfds_core::{Candle, Timeframe};
use cfds_gateway::lab_risk::{PreviewInputs, PreviewRequest, RiskPreview, closed_at, preview};
use cfds_market_data::Catalog;
use cfds_risk::RiskConfig;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal::dec;
use serde_json::{Value, json};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn contracts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts")
}

struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri.as_str().rsplit('/').next().ok_or("uri sin archivo")?;
        Ok(serde_json::from_str(&std::fs::read_to_string(
            contracts().join(name),
        )?)?)
    }
}

fn schema(sub: Option<&str>) -> jsonschema::Validator {
    let text = std::fs::read_to_string(contracts().join("risk_preview.v1.schema.json")).unwrap();
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    if let Some(def) = sub {
        let defs = doc["$defs"].clone();
        let mut s = defs[def].clone();
        s["$defs"] = defs;
        s["$id"] = doc["$id"].take();
        doc = s;
    }
    jsonschema::options()
        .with_retriever(Local)
        .build(&doc)
        .unwrap()
}

fn assert_contract(p: &RiskPreview) -> Value {
    let doc = serde_json::to_value(p).unwrap();
    let errors: Vec<String> = schema(None)
        .iter_errors(&doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}\n{doc}");
    doc
}

/// `n` velas de 1h cerradas, todas con cierre 5000 y rango `range` (ATR = `range`).
fn candles(n: usize, range: Decimal) -> Vec<Candle> {
    let t0: DateTime<Utc> = "2026-11-20T00:00:00Z".parse().unwrap();
    (0..n)
        .map(|i| Candle {
            instrument_id: "US500".parse().unwrap(),
            timeframe: Timeframe::H1,
            start: t0 + Duration::hours(i64::try_from(i).unwrap()),
            open: dec!(5000),
            high: dec!(5000) + range / dec!(2),
            low: dec!(5000) - range / dec!(2),
            close: dec!(5000),
            volume: None,
        })
        .collect()
}

fn request(v: Value) -> PreviewRequest {
    let base = json!({
        "instrument_id": "US500", "timeframe": "1h", "direction": "long",
        "equity": "100000", "risk_pct": "0.5", "atr_period": 14,
        "stop": { "kind": "atr", "k": "1.5" }
    });
    let mut doc = base;
    for (k, x) in v.as_object().unwrap() {
        doc[k] = x.clone();
    }
    assert!(
        schema(Some("request")).is_valid(&doc),
        "el pedido no cumple: {doc}"
    );
    serde_json::from_value(doc).unwrap()
}

struct World {
    catalog: Catalog,
    risk: RiskConfig,
    costs: cfds_backtest::CostConfig,
}

fn world() -> World {
    World {
        catalog: Catalog::from_toml(cfg!("instruments.toml")).unwrap(),
        risk: RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        costs: BacktestConfig::from_toml(cfg!("backtest.toml"))
            .unwrap()
            .costs,
    }
}

fn run(w: &World, req: &PreviewRequest, candles: &[Candle], fx: Option<Decimal>) -> RiskPreview {
    let spec = &w.catalog.instruments[&req.instrument_id];
    preview(
        req,
        &PreviewInputs {
            candles,
            spec,
            risk: &w.risk,
            costs: &w.costs,
            account_currency: "EUR",
            fx,
        },
    )
}

#[test]
fn accepted_with_the_risk_module_size_and_costs() {
    let w = world();
    let p = run(&w, &request(json!({})), &candles(30, dec!(10)), None);
    let doc = assert_contract(&p);
    assert!(p.accepted, "{doc}");
    assert_eq!(p.reference.as_deref(), Some("5000"));
    assert_eq!(p.atr.as_deref(), Some("10"));
    assert_eq!(p.stop.as_deref(), Some("4985"));
    assert_eq!(p.stop_atr.as_deref(), Some("1.5"));
    assert_eq!(
        p.fx_rate.as_deref(),
        Some("1"),
        "US500 va en EUR, como la cuenta"
    );
    // El riesgo (con el colchón por gaps) nunca pasa el pedido.
    let risk: Decimal = p.risk_pct.as_deref().unwrap().parse().unwrap();
    assert!(risk <= dec!(0.5) && risk > dec!(0.4), "{risk}");
    assert!(p.round_trip_cost.is_some());
    assert!(!p.leverage_capped);
    assert_eq!(p.risk_pct_applied, "0.5");
    // Corto: el stop del otro lado.
    let short = run(
        &w,
        &request(json!({ "direction": "short" })),
        &candles(30, dec!(10)),
        None,
    );
    assert_eq!(short.stop.as_deref(), Some("5015"));
}

#[test]
fn the_requested_risk_never_passes_the_config() {
    let w = world();
    let p = run(
        &w,
        &request(json!({ "risk_pct": "5" })),
        &candles(30, dec!(10)),
        None,
    );
    assert_contract(&p);
    assert_eq!(p.risk_pct_applied, p.limits.max_risk_pct);
    assert!(
        p.notes
            .iter()
            .any(|n| n.contains("pasa el de config/risk.toml"))
    );
    let risk: Decimal = p.risk_pct.as_deref().unwrap().parse().unwrap();
    assert!(risk <= w.risk.risk_per_trade_pct);
}

#[test]
fn each_limit_says_why_it_rejects_or_trims() {
    let w = world();
    let c = candles(30, dec!(10));
    let code = |p: &RiskPreview| {
        assert_contract(p);
        serde_json::to_value(p.rejection).unwrap()
    };
    let close = run(
        &w,
        &request(json!({ "stop": { "kind": "atr", "k": "0.1" } })),
        &c,
        None,
    );
    assert_eq!(code(&close), json!("stop_too_close"));
    let far = run(
        &w,
        &request(json!({ "stop": { "kind": "atr", "k": "9" } })),
        &c,
        None,
    );
    assert_eq!(code(&far), json!("stop_too_far"));
    let wrong = run(
        &w,
        &request(json!({ "stop": { "kind": "price", "price": "5010" } })),
        &c,
        None,
    );
    assert_eq!(code(&wrong), json!("stop_wrong_side"));
    let warmup = run(&w, &request(json!({})), &candles(5, dec!(10)), None);
    assert_eq!(code(&warmup), json!("no_recent_candles"));
    let empty = run(&w, &request(json!({})), &[], None);
    assert_eq!(code(&empty), json!("no_price"));
    assert!(empty.as_of.is_none() && empty.reference.is_none());
    // Un stop muy cerca en un mercado quieto: el tope de apalancamiento recorta el tamaño.
    let tight = run(
        &w,
        &request(json!({ "stop": { "kind": "atr", "k": "0.5" } })),
        &candles(30, dec!(1)),
        None,
    );
    assert!(tight.accepted && tight.leverage_capped, "{tight:?}");
    assert_eq!(
        tight.leverage.as_deref(),
        Some(tight.limits.max_leverage.as_str())
    );
    assert!(tight.notes.iter().any(|n| n.contains("apalancamiento")));
}

#[test]
fn another_currency_needs_a_published_fx_rate() {
    let w = world();
    let req = request(json!({ "instrument_id": "XAUUSD" }));
    let p = run(&w, &req, &candles(30, dec!(10)), None);
    assert_eq!(
        serde_json::to_value(p.rejection).unwrap(),
        json!("missing_fx_rate")
    );
    assert_eq!(p.instrument_currency, "USD");
    assert_contract(&p);
    let with_fx = run(&w, &req, &candles(30, dec!(10)), Some(dec!(1.1)));
    assert_contract(&with_fx);
    assert_eq!(with_fx.fx_rate.as_deref(), Some("1.1"));
    if with_fx.accepted {
        // Materias primas: a la financiación le faltan datos (ADR 0013), sin costo inventado.
        assert!(with_fx.round_trip_cost.is_none());
    }
}

#[test]
fn only_closed_candles_count() {
    let c = candles(3, dec!(10));
    let last_end = c[2].start + Duration::hours(1);
    assert_eq!(closed_at(c.clone(), last_end).len(), 3);
    assert_eq!(closed_at(c, last_end - Duration::minutes(1)).len(), 2);
}

//! La vista de liquidez leída de PostgreSQL **real** (#231). Necesita `DATABASE_URL`; está
//! marcado `#[ignore]` y el CI lo corre con `-- --ignored`. Cada corrida usa un instrumento propio.

use std::num::NonZeroU32;
use std::path::PathBuf;

use cfds_core::{
    AssetClass, Candle, Currency, Instrument, InstrumentId, LiquidityLevel,
    LiquidityLevelKind as K, LiquidityLevelState as S, Timeframe,
};
use cfds_gateway::{GatewayConfig, LiquiditySource, PgSource};
use cfds_storage::PgStore;
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::dec;
use serde_json::Value;

const CONFIG: &str = include_str!("../../../../config/gateway.toml");

fn contracts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts")
}

/// Resuelve los `$ref` entre contratos con los archivos del repo, sin red.
struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri.as_str().rsplit('/').next().ok_or("uri sin archivo")?;
        let text = std::fs::read_to_string(contracts().join(name))?;
        Ok(serde_json::from_str(&text)?)
    }
}

fn assert_valid(doc: &Value) {
    let text = std::fs::read_to_string(contracts().join("liquidity_view.v1.schema.json")).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .expect("liquidity_view.v1 no es un schema válido");
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(
        errors.is_empty(),
        "no valida contra liquidity_view.v1: {errors:?}"
    );
}

async fn source() -> PgSource {
    let url = std::env::var("DATABASE_URL").expect("falta DATABASE_URL (necesita PostgreSQL)");
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    PgSource {
        store,
        window: GatewayConfig::from_toml(CONFIG).unwrap().window,
        reaction_bars: vec![NonZeroU32::new(4).unwrap(), NonZeroU32::new(8).unwrap()],
    }
}

async fn fresh_instrument(s: &PgStore) -> InstrumentId {
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id: InstrumentId = format!("ITGW{nanos}").parse().unwrap();
    s.upsert_instrument(&Instrument {
        id: id.clone(),
        name: "Test gateway".into(),
        asset_class: AssetClass::Index,
        currency: Currency::USD,
        exchange_timezone: "America/New_York".into(),
    })
    .await
    .unwrap();
    id
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn candle(id: &InstrumentId, start: DateTime<Utc>) -> Candle {
    Candle {
        instrument_id: id.clone(),
        timeframe: Timeframe::M15,
        start,
        open: dec!(100.0),
        high: dec!(101.0),
        low: dec!(99.5),
        close: dec!(100.5),
        volume: None,
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn empty_instrument_is_sin_datos() {
    let src = source().await;
    let id = fresh_instrument(&src.store).await;
    let view = src.view(&id, at("2026-11-23T10:00:00Z")).await.unwrap();
    assert_eq!(view.as_of, None);
    assert!(view.candles.is_empty() && view.levels.is_empty() && view.stats.is_empty());
    assert_valid(&serde_json::to_value(&view).unwrap());
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn reads_closed_candles_and_levels_as_known_at_as_of() {
    let src = source().await;
    let id = fresh_instrument(&src.store).await;
    let t0 = at("2026-11-20T14:00:00Z");
    let candles: Vec<Candle> = (0..4)
        .map(|k| candle(&id, t0 + TimeDelta::minutes(15 * k)))
        .collect();
    src.store.insert_candles(&candles).await.unwrap();
    let level = LiquidityLevel {
        instrument_id: id.clone(),
        kind: K::Pdh,
        session: None,
        price: dec!(101.0),
        exists_from: at("2026-11-20T14:15:00Z"),
        state: S::Swept,
        state_changed_at: Some(at("2026-11-20T15:00:00Z")),
        expired_at: None,
    };
    src.store.insert_liquidity_levels(&[level]).await.unwrap();

    // 14:50: la vela de 14:45 (la que barre, cierra a las 15:00) todavía no cerró: as_of = 14:45
    // y el barrido no se ve.
    let view = src.view(&id, at("2026-11-20T14:50:00Z")).await.unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2026-11-20T14:45:00Z"));
    assert_eq!(view.candles.len(), 3);
    assert_eq!(view.levels.len(), 1);
    assert_eq!(view.levels[0].state, S::Intact);
    assert_valid(&serde_json::to_value(&view).unwrap());

    // Con todas las velas cerradas, el nivel llega barrido.
    let view = src.view(&id, at("2026-11-21T00:00:00Z")).await.unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2026-11-20T15:00:00Z"));
    assert_eq!(view.candles.len(), 4);
    assert_eq!(view.levels[0].state, S::Swept);
    assert_valid(&serde_json::to_value(&view).unwrap());
}

/// Los datos de ejemplo de la UI son una respuesta válida del gateway (más su nota).
#[test]
fn ui_mock_is_a_valid_liquidity_view() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../ui/src/mocks/liquidity-us100-15m.json");
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    doc.as_object_mut().unwrap().remove("note");
    assert_valid(&doc);
}

#[test]
fn empty_view_follows_the_contract() {
    let id: InstrumentId = "US100".parse().unwrap();
    let view = cfds_gateway::LiquidityView::empty(id, at("2026-11-23T10:00:00Z"));
    assert_valid(&serde_json::to_value(&view).unwrap());
    // y el schema rechaza un campo inventado
    let mut doc = serde_json::to_value(&view).unwrap();
    doc["extra"] = Value::Bool(true);
    let text = std::fs::read_to_string(contracts().join("liquidity_view.v1.schema.json")).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap();
    assert!(!v.is_valid(&doc));
}

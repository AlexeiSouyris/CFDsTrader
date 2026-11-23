//! La vista de liquidez leída de PostgreSQL **real** (#231). Necesita `DATABASE_URL`; está
//! marcado `#[ignore]` y el CI lo corre con `-- --ignored`. Cada corrida usa un instrumento propio.

use std::num::NonZeroU32;
use std::path::PathBuf;

use cfds_core::{
    AssetClass, Candle, Currency, DailyCandle, Instrument, InstrumentId, LiquidityLevel,
    LiquidityLevelKind as K, LiquidityLevelState as S,
    StatsTimeframe::{D1, M15},
    Timeframe,
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

fn assert_valid_against(schema_file: &str, doc: &Value) {
    let text = std::fs::read_to_string(contracts().join(schema_file)).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap_or_else(|e| panic!("{schema_file} no es un schema válido: {e}"));
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(
        errors.is_empty(),
        "no valida contra {schema_file}: {errors:?}"
    );
}

fn assert_valid(doc: &Value) {
    assert_valid_against("liquidity_view.v1.schema.json", doc);
}

async fn source() -> PgSource {
    let url = std::env::var("DATABASE_URL").expect("falta DATABASE_URL (necesita PostgreSQL)");
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    PgSource {
        store,
        config: GatewayConfig::from_toml(CONFIG).unwrap(),
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
    let view = src
        .view(&id, M15, at("2026-11-23T10:00:00Z"))
        .await
        .unwrap();
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
    let view = src
        .view(&id, M15, at("2026-11-20T14:50:00Z"))
        .await
        .unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2026-11-20T14:45:00Z"));
    assert_eq!(view.candles.len(), 3);
    assert_eq!(view.levels.len(), 1);
    assert_eq!(view.levels[0].state, S::Intact);
    assert_valid(&serde_json::to_value(&view).unwrap());

    // Con todas las velas cerradas, el nivel llega barrido.
    let view = src
        .view(&id, M15, at("2026-11-21T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2026-11-20T15:00:00Z"));
    assert_eq!(view.candles.len(), 4);
    assert_eq!(view.levels[0].state, S::Swept);
    assert_valid(&serde_json::to_value(&view).unwrap());
}

fn daily(id: &InstrumentId, day: &str) -> DailyCandle {
    let d: chrono::NaiveDate = day.parse().unwrap();
    // Día de medianoche a medianoche de Europa central (UTC+1 en noviembre).
    let start = d.and_hms_opt(0, 0, 0).unwrap().and_utc() - TimeDelta::hours(1);
    DailyCandle {
        instrument_id: id.clone(),
        day: d,
        start,
        end: start + TimeDelta::days(1),
        open: dec!(100.0),
        high: dec!(102.0),
        low: dec!(99.0),
        close: dec!(101.0),
        volume: None,
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn daily_view_uses_closed_daily_candles() {
    let src = source().await;
    let id = fresh_instrument(&src.store).await;
    let days = ["2026-11-18", "2026-11-19", "2026-11-20"];
    let rows: Vec<DailyCandle> = days.iter().map(|d| daily(&id, d)).collect();
    src.store.insert_daily(&rows).await.unwrap();
    // El 20 todavía no terminó a las 12:00 UTC: as_of es el fin del 19.
    let view = src.view(&id, D1, at("2026-11-20T12:00:00Z")).await.unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2026-11-19T23:00:00Z"));
    assert_eq!(view.candles.len(), 2);
    let doc = serde_json::to_value(&view).unwrap();
    assert_eq!(doc["timeframe"], "1d");
    assert_eq!(doc["candles"][0]["day"], "2026-11-18");
    assert_valid(&doc);
    // Sin velas de 15m, la vista de 15m está vacía aunque haya diarias.
    let m15 = src
        .view(&id, M15, at("2026-11-21T12:00:00Z"))
        .await
        .unwrap();
    assert_eq!(m15.as_of, None);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_gateway_session_cannot_write() {
    let url = std::env::var("DATABASE_URL").expect("falta DATABASE_URL (necesita PostgreSQL)");
    source().await; // deja las migraciones hechas
    let ro = PgStore::connect(&cfds_gateway::read_only_url(&url))
        .await
        .unwrap();
    let id: InstrumentId = "ITGWRO".parse().unwrap();
    let write = ro
        .upsert_instrument(&Instrument {
            id,
            name: "no se guarda".into(),
            asset_class: AssetClass::Index,
            currency: Currency::USD,
            exchange_timezone: "America/New_York".into(),
        })
        .await;
    assert!(write.is_err(), "la sesión de solo lectura escribió");
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
    let view = cfds_gateway::LiquidityView::empty(id, M15, at("2026-11-23T10:00:00Z"));
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

/// `/api/v1/instruments` con el catálogo y la configuración reales valida contra su contrato.
#[tokio::test]
async fn instruments_reply_follows_the_contract() {
    let catalog =
        cfds_market_data::Catalog::from_toml(include_str!("../../../../config/instruments.toml"))
            .unwrap();
    let instruments: Vec<Instrument> = catalog
        .instruments
        .values()
        .map(|s| s.instrument.clone())
        .collect();
    let cfg = GatewayConfig::from_toml(CONFIG).unwrap();
    assert!(instruments.iter().any(|i| i.id == cfg.default_instrument));
    struct NoDb;
    impl LiquiditySource for NoDb {
        async fn view(
            &self,
            _: &InstrumentId,
            _: cfds_core::StatsTimeframe,
            _: DateTime<Utc>,
        ) -> Result<cfds_gateway::LiquidityView, cfds_storage::StorageError> {
            unreachable!()
        }
    }
    let r = cfds_gateway::respond(
        &NoDb,
        &cfg,
        &instruments,
        "GET",
        "/api/v1/instruments",
        None,
        at("2026-11-23T10:00:00Z"),
    )
    .await;
    assert_eq!(r.status, 200);
    assert_valid_against(
        "gateway_instruments.v1.schema.json",
        &serde_json::from_str(&r.body).unwrap(),
    );
}

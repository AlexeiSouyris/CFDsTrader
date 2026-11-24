//! La vista de liquidez leída de PostgreSQL **real** (#231). Necesita `DATABASE_URL`; está
//! marcado `#[ignore]` y el CI lo corre con `-- --ignored`. Cada corrida usa un instrumento propio.

use std::num::NonZeroU32;
use std::path::PathBuf;

use cfds_core::{
    AssetClass, Candle, Currency, DailyCandle, Instrument, InstrumentId, LiquidityLevel,
    LiquidityLevelKind as K, LiquidityLevelState as S,
    StatsTimeframe::{D1, H1, M15},
    Timeframe,
};
use cfds_gateway::{GatewayConfig, LiquiditySource, PgSource, ViewAnalysis};
use cfds_market_data::{CandleSource, CandleSources};
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

fn sources() -> CandleSources {
    CandleSources::from_toml(include_str!("../../../../config/candle_sources.toml")).unwrap()
}

async fn source() -> PgSource {
    let url = cfds_storage::test_db::test_database_url();
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    PgSource {
        store,
        config: GatewayConfig::from_toml(CONFIG).unwrap(),
        reaction_bars: vec![NonZeroU32::new(4).unwrap(), NonZeroU32::new(8).unwrap()],
        sources: CandleSources::from_toml(include_str!("../../../../config/candle_sources.toml"))
            .unwrap(),
        analysis: Some(view_analysis()),
    }
}

fn view_analysis() -> ViewAnalysis {
    ViewAnalysis::from_toml(
        include_str!("../../../../config/analysis.toml"),
        include_str!("../../../../config/patterns.toml"),
    )
    .unwrap()
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
        .view(&id, CandleSource::Ig, M15, at("2026-11-23T10:00:00Z"))
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
        .view(&id, CandleSource::Ig, M15, at("2026-11-20T14:50:00Z"))
        .await
        .unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2026-11-20T14:45:00Z"));
    assert_eq!(view.candles.len(), 3);
    assert_eq!(view.levels.len(), 1);
    assert_eq!(view.levels[0].state, S::Intact);
    assert_valid(&serde_json::to_value(&view).unwrap());

    // Con todas las velas cerradas, el nivel llega barrido.
    let view = src
        .view(&id, CandleSource::Ig, M15, at("2026-11-21T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2026-11-20T15:00:00Z"));
    assert_eq!(view.candles.len(), 4);
    assert_eq!(view.levels[0].state, S::Swept);
    // 15m de IG: trae las herramientas de análisis (#501), una entrada por vela.
    let analysis = view.analysis.as_ref().expect("analysis en 15m");
    assert_eq!(analysis.ema_daily.len(), 4);
    assert_eq!(analysis.vix.len(), 4);
    assert_valid(&serde_json::to_value(&view).unwrap());
    // En 1h no: los parámetros son de 15m.
    let h1 = src
        .view(&id, CandleSource::Ig, H1, at("2026-11-21T00:00:00Z"))
        .await
        .unwrap();
    assert!(h1.analysis.is_none());
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

/// Como los datos reales (#231): diarias que terminan hace años y nada de 15m. `as_of` sale de la
/// última diaria y la ventana se mide desde ahí, no desde ahora.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn old_daily_data_without_intraday_is_shown_in_1d() {
    let src = source().await;
    let id = fresh_instrument(&src.store).await;
    // Noviembre de 2024, días hábiles (sin cambio de horario en esas fechas).
    let days = [
        "2024-11-18",
        "2024-11-19",
        "2024-11-20",
        "2024-11-21",
        "2024-11-22",
    ];
    let rows: Vec<DailyCandle> = days.iter().map(|d| daily(&id, d)).collect();
    src.store.insert_daily(&rows).await.unwrap();
    // Agregados de 1d calculados hoy (computed_at muy posterior a los datos), como en la base real.
    let mock: Value = serde_json::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../ui/src/mocks/liquidity-us100-15m.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut doc = mock["stats"][0].clone();
    doc["instrument_id"] = Value::String(id.as_str().into());
    doc["timeframe"] = Value::String("1d".into());
    doc["as_of"] = Value::String("2024-11-22T23:00:00Z".into());
    let stats: cfds_core::LiquidityStats = serde_json::from_value(doc).unwrap();
    src.store
        .insert_liquidity_stats(at("2026-11-23T09:00:00Z"), &[stats])
        .await
        .unwrap();

    let now = at("2026-11-23T10:00:00Z");
    let view = src.view(&id, CandleSource::Ig, D1, now).await.unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2024-11-22T23:00:00Z"));
    assert_eq!(view.candles.len(), days.len());
    assert_eq!(
        view.stats.len(),
        1,
        "los agregados de una corrida de hoy se ven"
    );
    assert_valid(&serde_json::to_value(&view).unwrap());
    // 15m y 1h siguen vacíos: no hay velas intradía.
    assert_eq!(
        src.view(&id, CandleSource::Ig, M15, now)
            .await
            .unwrap()
            .as_of,
        None
    );
    assert_eq!(
        src.view(&id, CandleSource::Ig, H1, now)
            .await
            .unwrap()
            .as_of,
        None
    );
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
    let view = src
        .view(&id, CandleSource::Ig, D1, at("2026-11-20T12:00:00Z"))
        .await
        .unwrap();
    assert_eq!(view.as_of.as_deref(), Some("2026-11-19T23:00:00Z"));
    assert_eq!(view.candles.len(), 2);
    let doc = serde_json::to_value(&view).unwrap();
    assert_eq!(doc["timeframe"], "1d");
    assert_eq!(doc["candles"][0]["day"], "2026-11-18");
    assert_valid(&doc);
    // Sin velas de 15m, la vista de 15m está vacía aunque haya diarias.
    let m15 = src
        .view(&id, CandleSource::Ig, M15, at("2026-11-21T12:00:00Z"))
        .await
        .unwrap();
    assert_eq!(m15.as_of, None);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_gateway_session_cannot_write() {
    let url = cfds_storage::test_db::test_database_url();
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
    for source in [CandleSource::Ig, CandleSource::Futures] {
        let s = cfds_gateway::ViewSource::new(source, &sources());
        let view =
            cfds_gateway::LiquidityView::empty(id.clone(), s, M15, at("2026-11-23T10:00:00Z"));
        assert_valid(&serde_json::to_value(&view).unwrap());
    }
    let s = cfds_gateway::ViewSource::new(CandleSource::Ig, &sources());
    let view = cfds_gateway::LiquidityView::empty(id, s, M15, at("2026-11-23T10:00:00Z"));
    // Un documento sin `source` (la primera versión) sigue valiendo.
    let mut old = serde_json::to_value(&view).unwrap();
    old.as_object_mut().unwrap().remove("source");
    assert_valid(&old);
    let mut bad = serde_json::to_value(&view).unwrap();
    bad["source"]["id"] = Value::String("cfd".into());
    let text = std::fs::read_to_string(contracts().join("liquidity_view.v1.schema.json")).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap();
    assert!(!v.is_valid(&bad));
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
            _: CandleSource,
            _: cfds_core::StatsTimeframe,
            _: DateTime<Utc>,
        ) -> Result<cfds_gateway::LiquidityView, cfds_storage::StorageError> {
            unreachable!()
        }
    }
    let r = cfds_gateway::respond(
        &NoDb,
        &cfg,
        &sources(),
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

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_futures_source_reads_futures_candles_and_the_derived_instrument() {
    use cfds_core::FuturesCandle;
    let src = source().await;
    let id = fresh_instrument(&src.store).await;
    let derived = src
        .sources
        .instrument_for(CandleSource::Futures, &id)
        .unwrap();
    // Velas del CFD (IG) y del futuro, con precios distintos, en las mismas horas.
    let t0 = at("2026-11-20T14:00:00Z");
    let ig: Vec<Candle> = (0..4)
        .map(|k| candle(&id, t0 + TimeDelta::minutes(15 * k)))
        .collect();
    src.store.insert_candles(&ig).await.unwrap();
    let fut: Vec<FuturesCandle> = ig
        .iter()
        .map(|c| FuturesCandle {
            candle: Candle {
                open: c.open + dec!(50),
                high: c.high + dec!(50),
                low: c.low + dec!(50),
                close: c.close + dec!(50),
                volume: Some(dec!(10)),
                ..c.clone()
            },
            contract_id: 7,
        })
        .collect();
    src.store
        .insert_futures_candles("databento", "GLBX.MDP3", "ES.c.0", &fut[..3])
        .await
        .unwrap();
    let now = at("2026-11-21T00:00:00Z");
    let ig_view = src.view(&id, CandleSource::Ig, M15, now).await.unwrap();
    let fut_view = src
        .view(&id, CandleSource::Futures, M15, now)
        .await
        .unwrap();
    assert_eq!(ig_view.source.id, "ig");
    assert_eq!(fut_view.source.id, "futures");
    assert_eq!(fut_view.instrument_id, id);
    // Cada fuente con sus velas: el futuro tiene tres, 50 puntos arriba, y su propio as_of.
    assert_eq!(ig_view.candles.len(), 4);
    assert_eq!(fut_view.candles.len(), 3);
    assert_eq!(fut_view.as_of.as_deref(), Some("2026-11-20T14:45:00Z"));
    // Las herramientas de análisis (#501) solo con las velas de IG.
    assert!(ig_view.analysis.is_some());
    assert!(fut_view.analysis.is_none());
    let v = serde_json::to_value(&fut_view).unwrap();
    assert_eq!(v["candles"][0]["close"], "150.5");
    assert_eq!(v["candles"][0]["instrument_id"], derived.as_str());
    assert_valid(&v);
    // Los futuros no tienen diarias: en 1d, "sin datos".
    let d1 = src.view(&id, CandleSource::Futures, D1, now).await.unwrap();
    assert_eq!(d1.as_of, None);
    assert_valid(&serde_json::to_value(&d1).unwrap());
}

/// Una vista de 15m con el bloque `analysis` (#501) que arma `cfds_backtest::analysis` valida
/// contra `liquidity_view.v1` (que lo toma de `chart_export.v1`), y el contrato rechaza un número
/// suelto en vez de un decimal como texto. Sin base: velas armadas acá.
#[test]
fn a_view_with_analysis_follows_the_contract() {
    let id: InstrumentId = "US100".parse().unwrap();
    let t0 = at("2026-11-16T08:00:00Z");
    // Zigzag de 15m: sube 12 velas y baja 12, así hay pivotes confirmados y tramos de Fibonacci.
    let m15: Vec<Candle> = (0..240i64)
        .map(|k| {
            let phase = k % 24;
            let up = if phase < 12 { phase } else { 24 - phase };
            let mid = rust_decimal::Decimal::from(20_000 + 10 * up + k);
            Candle {
                open: mid,
                high: mid + dec!(6),
                low: mid - dec!(6),
                close: mid + dec!(1),
                ..candle(&id, t0 + TimeDelta::minutes(15 * k))
            }
        })
        .collect();
    let days: Vec<DailyCandle> = ["2026-11-12", "2026-11-13", "2026-11-16", "2026-11-17"]
        .iter()
        .map(|d| daily(&id, d))
        .collect();
    let vix = vec![(at("2026-11-16T21:00:00Z"), dec!(21.5))];
    let analysis = view_analysis().block(&m15, &days, &vix);
    assert_eq!(analysis.ema_daily.len(), m15.len());
    assert!(!analysis.fibonacci.is_empty(), "el zigzag tiene tramos");
    assert!(analysis.vix.iter().any(Option::is_some));
    assert!(!analysis.floor_pivots.is_empty());

    let s = cfds_gateway::ViewSource::new(CandleSource::Ig, &sources());
    let mut view = cfds_gateway::LiquidityView::empty(id, s, M15, at("2026-11-19T00:00:00Z"));
    view.as_of = Some("2026-11-18T20:00:00Z".into());
    view.candles = cfds_gateway::Bars::Intraday(m15);
    view.analysis = Some(analysis);
    let doc = serde_json::to_value(&view).unwrap();
    assert!(doc["analysis"]["fibonacci"][0]["golden_pocket_low"].is_string());
    assert_valid(&doc);

    let mut bad = doc.clone();
    bad["analysis"]["ema_daily_period"] = Value::String("365".into());
    let text = std::fs::read_to_string(contracts().join("liquidity_view.v1.schema.json")).unwrap();
    let schema: Value = serde_json::from_str(&text).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap();
    assert!(!v.is_valid(&bad));
    let mut bad = doc;
    bad["analysis"]["fibonacci"][0]["end_price"] = serde_json::json!(20_100.5);
    assert!(!v.is_valid(&bad));
}

/// El bloque `analysis` de los datos de ejemplo de la UI (#501) es el que arma el motor sobre esas
/// velas, sin diarias ni VIX (los datos de ejemplo no los tienen: la EMA diaria, las medias, los
/// pivotes diarios y el VIX quedan vacíos). Para regenerarlo, `CFDS_UI_MOCK_ANALYSIS=<archivo>`
/// escribe el bloque en ese archivo.
#[test]
fn ui_mock_analysis_is_what_the_engine_computes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../ui/src/mocks/liquidity-us100-15m.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let m15: Vec<Candle> = serde_json::from_value(doc["candles"].clone()).unwrap();
    let analysis = serde_json::to_value(view_analysis().block(&m15, &[], &[])).unwrap();
    if let Ok(out) = std::env::var("CFDS_UI_MOCK_ANALYSIS") {
        std::fs::write(out, serde_json::to_string(&analysis).unwrap()).unwrap();
        return;
    }
    assert_eq!(doc["analysis"], analysis);
}

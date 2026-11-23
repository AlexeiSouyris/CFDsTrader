//! El runner como modo del gateway (fase 8, #305): la config del repo y las piezas puras (los
//! filtros de entrada y las noticias del modelo fijado). El paso en sí se prueba en
//! `cfds-runner`.

use std::path::PathBuf;

use cfds_core::{Candle, InstrumentId, NewsInterpretation, Timeframe};
use cfds_gateway::paper_runner::{RunnerDeps, entry_flags, same_model};
use cfds_market_data::Catalog;
use cfds_strategy::BacktestInterpreter;
use chrono::{DateTime, Utc};
use rust_decimal::dec;

const CATALOG: &str = include_str!("../../../../config/instruments.toml");

fn config_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config")
}

fn deps() -> RunnerDeps {
    RunnerDeps::from_repo_config(&Catalog::from_toml(CATALOG).unwrap(), &config_dir()).unwrap()
}

fn candle(start: &str) -> Candle {
    Candle {
        instrument_id: "DE40".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: start.parse().unwrap(),
        open: dec!(100),
        high: dec!(101),
        low: dec!(99),
        close: dec!(100),
        volume: None,
    }
}

#[tokio::test]
async fn the_repo_config_loads_and_the_runner_is_off_by_default() {
    let d = deps();
    assert!(!d.runner.lock().await.config().enabled);
    assert!(
        d.calendar
            .session_for(&"DE40".parse::<InstrumentId>().unwrap())
            .is_some()
    );
}

#[test]
fn entries_need_the_window_the_spread_and_the_news_filter() {
    let d = deps();
    let session = d
        .calendar
        .session_for(&"DE40".parse::<InstrumentId>().unwrap())
        .unwrap();
    // Miércoles al mediodía (sesión abierta) y sábado (cerrado).
    let m15 = [
        candle("2026-11-18T11:00:00Z"),
        candle("2026-11-18T11:15:00Z"),
        candle("2026-11-18T11:30:00Z"),
        candle("2026-11-21T11:00:00Z"),
    ];
    let at = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
    let f = entry_flags(
        session,
        &m15,
        &[true, false, true, true],
        &[true, true, false, true],
        &d.window,
    );
    assert!(f[&at("2026-11-18T11:00:00Z")], "todo permite");
    assert!(!f[&at("2026-11-18T11:15:00Z")], "spread");
    assert!(!f[&at("2026-11-18T11:30:00Z")], "noticia de alto impacto");
    assert!(!f[&at("2026-11-21T11:00:00Z")], "fuera de sesión");
    // Si falta el dato de un filtro, no se entra.
    let short = entry_flags(session, &m15[..1], &[], &[true], &d.window);
    assert!(!short[&at("2026-11-18T11:00:00Z")]);
}

fn interpretation(model: Option<&str>) -> NewsInterpretation {
    let t: DateTime<Utc> = "2026-11-18T10:00:00Z".parse().unwrap();
    NewsInterpretation {
        interpretation_id: "i1".into(),
        source: "fed".into(),
        item_id: "n1".into(),
        published_at: t,
        interpreted_at: t,
        interpreter: cfds_core::Interpreter {
            name: "x".into(),
            version: "1".into(),
            model: model.map(str::to_owned),
            prompt_sha256: None,
        },
        summary_es: String::new(),
        impacts: Vec::new(),
    }
}

#[test]
fn only_interpretations_of_the_pinned_model_count() {
    let interpreter =
        BacktestInterpreter::from_toml(include_str!("../../../../config/news_interpreter.toml"))
            .unwrap();
    let kept = same_model(
        vec![
            interpretation(Some(&interpreter.model)),
            interpretation(Some("otro@sha256:0")),
            interpretation(None),
        ],
        &interpreter,
    );
    assert_eq!(kept.len(), 1);
    assert_eq!(
        kept[0].interpreter.model.as_deref(),
        Some(interpreter.model.as_str())
    );
}

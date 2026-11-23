//! `GET /api/v1/emotions` (#220): estados emocionales tal como están guardados, solo lectura.
//! Datos sintéticos.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use cfds_core::{ActiveEmotion, EmotionBasis, EmotionKind, EmotionSource, EmotionalState};
use cfds_gateway::emotions::{EmotionStore, is_emotions_route, respond_emotions};
use chrono::{DateTime, Utc};
use serde_json::Value;

fn now() -> DateTime<Utc> {
    "2026-11-23T12:00:00Z".parse().unwrap()
}

fn row(
    id: &str,
    source: EmotionSource,
    basis: EmotionBasis,
    states: Vec<ActiveEmotion>,
) -> EmotionalState {
    EmotionalState {
        instrument_id: id.parse().unwrap(),
        source,
        basis,
        as_of: "2026-11-23T11:00:00Z".parse().unwrap(),
        states,
    }
}

fn active(state: EmotionKind, values: &[(&str, &str)]) -> ActiveEmotion {
    ActiveEmotion {
        state,
        values: values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>(),
    }
}

struct Fake {
    rows: Result<Vec<EmotionalState>, String>,
    asked_at: Mutex<Option<DateTime<Utc>>>,
}

impl EmotionStore for Fake {
    async fn latest(&self, at: DateTime<Utc>) -> Result<Vec<EmotionalState>, String> {
        *self.asked_at.lock().unwrap() = Some(at);
        self.rows.clone()
    }
}

fn fake() -> Fake {
    Fake {
        rows: Ok(vec![
            row(
                "US500",
                EmotionSource::Price,
                EmotionBasis::H1,
                vec![active(
                    EmotionKind::Fomo,
                    &[
                        ("stretch", "3.2"),
                        ("atr_multiple", "3"),
                        ("lookback", "20"),
                    ],
                )],
            ),
            row("US500", EmotionSource::Price, EmotionBasis::M15, vec![]),
            row(
                "US500.FUT",
                EmotionSource::Sentiment,
                EmotionBasis::SentimentIndex,
                vec![active(
                    EmotionKind::ExtremeFear,
                    &[("index", "-0.72"), ("threshold", "-0.6")],
                )],
            ),
        ]),
        asked_at: Mutex::new(None),
    }
}

async fn get(store: &Fake, method: &str, query: Option<&str>) -> (u16, Value) {
    let r = respond_emotions(store, method, "/api/v1/emotions", query, now()).await;
    (
        r.status,
        serde_json::from_str(&r.body).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn latest_states_as_stored_and_valid_against_the_contract() {
    let f = fake();
    let (status, body) = get(&f, "GET", None).await;
    assert_eq!(status, 200);
    // Nunca mira el futuro: pide lo conocido hasta ahora.
    assert_eq!(*f.asked_at.lock().unwrap(), Some(now()));
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/emotional_state.v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    for r in rows {
        let errors: Vec<String> = v.iter_errors(r).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{errors:?}\n{r}");
    }
    assert_eq!(rows[0]["states"][0]["state"], "fomo");
    assert_eq!(rows[0]["states"][0]["values"]["stretch"], "3.2");
    assert_eq!(rows[1]["states"], serde_json::json!([]));
    assert_eq!(rows[2]["basis"], "sentiment_index");
}

#[tokio::test]
async fn filters_by_instrument_and_rejects_bad_input() {
    let f = fake();
    let (_, one) = get(&f, "GET", Some("instrument=US500.FUT")).await;
    assert_eq!(one.as_array().unwrap().len(), 1);
    assert_eq!(one[0]["source"], "sentiment");
    let (status, _) = get(&f, "GET", Some("instrument=us500;drop")).await;
    assert_eq!(status, 400);
    for method in ["POST", "PUT", "DELETE"] {
        assert_eq!(get(&f, method, None).await.0, 405);
    }
    assert!(!is_emotions_route("/api/v1/emotions/reset"));
    let broken = Fake {
        rows: Err("sin base".into()),
        asked_at: Mutex::new(None),
    };
    let r = respond_emotions(&broken, "GET", "/api/v1/emotions", None, now()).await;
    assert_eq!(r.status, 500);
    assert!(!r.body.contains("sin base"));
}

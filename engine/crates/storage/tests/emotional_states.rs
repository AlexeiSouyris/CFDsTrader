//! `emotional_states` (#220) contra PostgreSQL **real**: idempotente, sin repintado y la lectura
//! del último conocido. Necesita `DATABASE_URL`; marcado `#[ignore]` (el CI lo corre con
//! `-- --ignored`). La tabla es de solo agregar: cada corrida usa su propio instrumento.

use std::collections::BTreeMap;

use cfds_core::{
    ActiveEmotion, EmotionBasis, EmotionKind, EmotionSource, EmotionalState, InstrumentId,
};
use cfds_storage::{PgStore, StorageError};
use chrono::{DateTime, TimeDelta, Utc};

async fn store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let s = PgStore::connect(&url).await.unwrap();
    s.migrate().await.unwrap();
    s
}

/// Un instrumento propio de esta corrida (la tabla no se puede vaciar).
fn fresh_id() -> InstrumentId {
    let n = Utc::now().timestamp_nanos_opt().unwrap();
    format!("EMO{n}").parse().unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn state(
    id: &InstrumentId,
    basis: EmotionBasis,
    as_of: DateTime<Utc>,
    states: Vec<ActiveEmotion>,
) -> EmotionalState {
    EmotionalState {
        instrument_id: id.clone(),
        source: match basis {
            EmotionBasis::SentimentIndex => EmotionSource::Sentiment,
            _ => EmotionSource::Price,
        },
        basis,
        as_of,
        states,
    }
}

fn fomo(stretch: &str) -> ActiveEmotion {
    ActiveEmotion {
        state: EmotionKind::Fomo,
        values: BTreeMap::from([
            ("stretch".to_owned(), stretch.to_owned()),
            ("atr_multiple".to_owned(), "3".to_owned()),
        ]),
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn idempotent_append_only_and_never_repaints() {
    let s = store().await;
    let id = fresh_id();
    let t0 = at("2026-11-20T14:00:00Z");
    let rows = vec![
        state(&id, EmotionBasis::H1, t0, vec![]),
        state(
            &id,
            EmotionBasis::H1,
            t0 + TimeDelta::hours(1),
            vec![fomo("3.2")],
        ),
        state(&id, EmotionBasis::M15, t0, vec![]),
    ];
    let first = s.insert_emotional_states(&rows).await.unwrap();
    assert_eq!((first.inserted, first.unchanged), (3, 0));
    let again = s.insert_emotional_states(&rows).await.unwrap();
    assert_eq!((again.inserted, again.unchanged), (0, 3));

    // El mismo momento con otros estados: error, y no se guarda nada del lote.
    let new_one = state(&id, EmotionBasis::H1, t0 + TimeDelta::hours(2), vec![]);
    let repaint = state(
        &id,
        EmotionBasis::H1,
        t0 + TimeDelta::hours(1),
        vec![fomo("3.3")],
    );
    let err = s
        .insert_emotional_states(&[new_one, repaint])
        .await
        .unwrap_err();
    assert!(
        matches!(err, StorageError::EmotionalStateConflict { .. }),
        "{err}"
    );
    let all = s
        .emotional_states(&id, t0, t0 + TimeDelta::days(1))
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].states, vec![fomo("3.2")]);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn latest_is_per_basis_and_never_from_the_future() {
    let s = store().await;
    let id = fresh_id();
    let t0 = at("2026-11-20T14:00:00Z");
    s.insert_emotional_states(&[
        state(&id, EmotionBasis::H1, t0, vec![fomo("3.1")]),
        state(&id, EmotionBasis::H1, t0 + TimeDelta::hours(1), vec![]),
        state(&id, EmotionBasis::M15, t0 + TimeDelta::minutes(45), vec![]),
    ])
    .await
    .unwrap();
    let mine = |v: Vec<EmotionalState>| -> Vec<EmotionalState> {
        v.into_iter().filter(|e| e.instrument_id == id).collect()
    };
    let now = mine(
        s.latest_emotional_states(t0 + TimeDelta::days(1))
            .await
            .unwrap(),
    );
    assert_eq!(now.len(), 2);
    let h1 = now.iter().find(|e| e.basis == EmotionBasis::H1).unwrap();
    assert_eq!(h1.as_of, t0 + TimeDelta::hours(1));
    assert!(h1.states.is_empty());
    // En t0 + 30 min solo se conocía el de 1h de t0.
    let then = mine(
        s.latest_emotional_states(t0 + TimeDelta::minutes(30))
            .await
            .unwrap(),
    );
    assert_eq!(then.len(), 1);
    assert_eq!(then[0].states, vec![fomo("3.1")]);
}

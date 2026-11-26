//! `academy_events` en PostgreSQL (#718). Necesitan `DATABASE_URL` de una base de pruebas; están
//! marcados `#[ignore]` y el CI los corre con `-- --ignored` (job de integración).

mod common;

use cfds_academy::pg::PgAcademy;
use cfds_academy::{AcademyConfig, EventBody, OverrideAction, QuizAnswer, grade_quiz, progress};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use common::with_questions;
use serde_json::json;

async fn store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    store
}

/// Un miembro propio por corrida: la tabla no se puede borrar.
fn member() -> String {
    format!("t{}", Utc::now().timestamp_micros())
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn events_are_appended_in_order_and_rebuild_the_progress() {
    let store = store().await;
    let pg = PgAcademy::new(store.pool().clone());
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let v = &cfg.catalog.catalog_version;
    let me = member();
    let at: DateTime<Utc> = "2026-11-26T21:00:00.123456Z".parse().unwrap();
    let a: QuizAnswer = serde_json::from_value(json!({
        "lesson_id": "0.1", "catalog_version": v,
        "answers": [{"question_id": "0.1.1", "choice": "b"}, {"question_id": "0.1.2", "choice": "a"}],
    }))
    .unwrap();
    let quiz = Box::new(grade_quiz(&cfg, &a, at).unwrap());
    let first = pg
        .append(
            &me,
            EventBody::ExamAttempt {
                lesson_id: "0.1".into(),
                quiz,
            },
            v,
            at,
        )
        .await
        .unwrap();
    let unlock = EventBody::ToolOverride {
        tool_id: "*".into(),
        action: OverrideAction::Unlock,
    };
    let second = pg.append(&me, unlock, v, at).await.unwrap();
    assert!(second.seq > first.seq);
    let events = pg.events(&me).await.unwrap();
    assert_eq!(events, vec![first, second]);
    let p = progress(&cfg, &me, &events, at);
    assert!(p.lessons[0].passed && p.unlock_all);
    assert!(
        pg.append(
            "no válido",
            EventBody::LessonCompleted {
                lesson_id: "0.3".into()
            },
            v,
            at
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_table_is_append_only_and_checks_each_kind() {
    let store = store().await;
    let pg = PgAcademy::new(store.pool().clone());
    let me = member();
    let v = "a".repeat(64);
    let at = Utc::now();
    pg.append(
        &me,
        EventBody::LessonCompleted {
            lesson_id: "0.3".into(),
        },
        &v,
        at,
    )
    .await
    .unwrap();
    for sql in [
        "UPDATE academy_events SET lesson_id = '0.1' WHERE member_id = $1",
        "DELETE FROM academy_events WHERE member_id = $1",
        "INSERT INTO academy_events (member_id, kind, lesson_id, catalog_version, recorded_at)
         VALUES ($1, 'exam_attempt', '0.1', repeat('a', 64), now())",
        "INSERT INTO academy_events (member_id, kind, tool_id, catalog_version, recorded_at)
         VALUES ($1, 'tool_override', 'tool.x', repeat('a', 64), now())",
    ] {
        assert!(sqlx_exec(&store, sql, &me).await.is_err(), "{sql}");
    }
    assert_eq!(pg.events(&me).await.unwrap().len(), 1);
}

async fn sqlx_exec(store: &PgStore, sql: &'static str, me: &str) -> Result<(), String> {
    sqlx::query(sql)
        .bind(me.to_owned())
        .execute(store.pool())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

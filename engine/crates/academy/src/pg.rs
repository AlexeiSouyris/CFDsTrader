//! `academy_events` en PostgreSQL (`infra/migrations/0052_academy_events.sql`, #718; la columna
//! `replay` de los intentos de replay, `0054_academy_replay_attempts.sql`, #759): solo agregar (un
//! trigger impide modificar o borrar). El gateway abre la sesión en solo lectura;
//! [`PgAcademy::append`] declara su propia transacción de escritura y toca solo esta tabla.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::Row;
use sqlx::postgres::{PgPool, PgRow};

use crate::grade::ExamResult;
use crate::progress::{EventKind, member_id_ok};
use crate::{AcademyEvent, EventBody, OverrideAction, QuizResult};

#[derive(Debug, thiserror::Error)]
pub enum AcademyStoreError {
    #[error("PostgreSQL: {0}")]
    Db(#[from] sqlx::Error),
    #[error("fila inválida en academy_events: {0}")]
    CorruptRow(String),
    #[error("member_id inválido")]
    BadMember,
}

/// Las columnas de un evento: tipo, lección, herramienta, acción y la nota (`quiz` o `replay`).
type Columns<'a> = (
    EventKind,
    Option<&'a str>,
    Option<&'a str>,
    Option<OverrideAction>,
    Option<&'a QuizResult>,
    Option<&'a ExamResult>,
);

#[derive(Debug, Clone)]
pub struct PgAcademy {
    pool: PgPool,
}

fn name<T: serde::Serialize>(v: T) -> Option<String> {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
}

fn row_to_event(r: &PgRow) -> Result<AcademyEvent, AcademyStoreError> {
    let corrupt = |e: serde_json::Error| AcademyStoreError::CorruptRow(e.to_string());
    let doc = serde_json::json!({
        "seq": r.try_get::<i64, _>("seq")?,
        "member_id": r.try_get::<String, _>("member_id")?,
        "kind": r.try_get::<String, _>("kind")?,
        "lesson_id": r.try_get::<Option<String>, _>("lesson_id")?,
        "tool_id": r.try_get::<Option<String>, _>("tool_id")?,
        "action": r.try_get::<Option<String>, _>("action")?,
        "quiz": r.try_get::<Option<Value>, _>("quiz")?,
        "replay": r.try_get::<Option<Value>, _>("replay")?,
        "catalog_version": r.try_get::<String, _>("catalog_version")?,
        "recorded_at": r.try_get::<DateTime<Utc>, _>("recorded_at")?,
    });
    serde_json::from_value(doc).map_err(corrupt)
}

impl PgAcademy {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Guarda un evento nuevo y lo devuelve con su `seq`.
    pub async fn append(
        &self,
        member_id: &str,
        body: EventBody,
        catalog_version: &str,
        recorded_at: DateTime<Utc>,
    ) -> Result<AcademyEvent, AcademyStoreError> {
        if !member_id_ok(member_id) {
            return Err(AcademyStoreError::BadMember);
        }
        let (kind, lesson, tool, action, quiz, replay): Columns<'_> = match &body {
            EventBody::LessonCompleted { lesson_id } => (
                EventKind::LessonCompleted,
                Some(lesson_id),
                None,
                None,
                None,
                None,
            ),
            EventBody::ExamAttempt { lesson_id, quiz } => (
                EventKind::ExamAttempt,
                Some(lesson_id),
                None,
                None,
                Some(quiz),
                None,
            ),
            EventBody::ReplayAttempt { lesson_id, replay } => (
                EventKind::ExamAttempt,
                Some(lesson_id),
                None,
                None,
                None,
                Some(replay),
            ),
            EventBody::ToolOverride { tool_id, action } => (
                EventKind::ToolOverride,
                None,
                Some(tool_id),
                Some(*action),
                None,
                None,
            ),
        };
        let json = |e: serde_json::Error| AcademyStoreError::CorruptRow(e.to_string());
        let quiz = quiz.map(serde_json::to_value).transpose().map_err(json)?;
        let replay = replay.map(serde_json::to_value).transpose().map_err(json)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION READ WRITE")
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            "INSERT INTO academy_events
               (member_id, kind, lesson_id, tool_id, action, quiz, replay, catalog_version,
                recorded_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             RETURNING seq, member_id, kind, lesson_id, tool_id, action, quiz, replay,
                       catalog_version, recorded_at",
        )
        .bind(member_id)
        .bind(name(kind))
        .bind(lesson)
        .bind(tool)
        .bind(action.and_then(name))
        .bind(quiz)
        .bind(replay)
        .bind(catalog_version)
        .bind(recorded_at)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        row_to_event(&row)
    }

    /// Los eventos de `member_id`, en orden de llegada.
    pub async fn events(&self, member_id: &str) -> Result<Vec<AcademyEvent>, AcademyStoreError> {
        let rows = sqlx::query(
            "SELECT seq, member_id, kind, lesson_id, tool_id, action, quiz, replay,
                    catalog_version, recorded_at
             FROM academy_events WHERE member_id = $1 ORDER BY seq",
        )
        .bind(member_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_event).collect()
    }
}

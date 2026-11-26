//! Academia (docs/design/2026-11-academia.md, #709): catálogo de lecciones y herramientas con
//! candado (`config/academy.toml`), corrección de exámenes y progreso desde eventos de solo
//! agregar. Todo lo calcula el motor; la UI solo muestra (CLAUDE.md §1). Nada de esto toca el
//! riesgo, el paper ni las órdenes: el desbloqueo solo cambia qué se ve en la UI.
//!
//! - [`AcademyConfig`]: la config validada; [`Catalog`] es `academy_catalog.v1`, sin las claves.
//! - [`grade_quiz`]: `quiz_answer.v1` → `quiz_result.v1`.
//! - [`progress`]: eventos (`academy_event.v1`) → `academy_progress.v1`.
//! - [`pg::PgAcademy`]: la tabla `academy_events` (migración 0052), de solo agregar.
//! - [`replay`]: sesiones de replay de los exámenes (semilla, fecha oculta, cursor del motor).
//! - [`grade::grade`]: la calificación de una sesión terminada (`exam_result.v1`), proceso
//!   separado del R.
//! - [`lesson_doc`]: el texto de cada lección (`docs/academia/*.md`): formato, aviso, sin
//!   promesas y cada cifra comparada con su fuente del repo (#760); [`LessonDocs`] lo lee al
//!   arrancar, solo dentro de `docs/academia/`, para servirlo (`academy_lesson_doc.v1`, #798).

pub mod catalog;
pub mod grade;
pub mod lesson_doc;
pub mod pg;
pub mod progress;
pub mod quiz;
pub mod replay;

pub use catalog::{AcademyConfig, Catalog, ExamKind, QuestionKind, Thresholds};
pub use lesson_doc::{LessonDocs, LessonText};
pub use progress::{
    AcademyEvent, EventBody, EventRequest, OverrideAction, Progress, UnlockedBy, progress,
};
pub use quiz::{QuizAnswer, QuizError, QuizResult, grade_quiz};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcademyError {
    #[error("config/academy.toml inválida: {0}")]
    Config(String),
    /// El texto de una lección (`docs/academia/*.md`) no cumple el formato o una cifra no
    /// coincide con su fuente ([`lesson_doc`]).
    #[error("texto de lección inválido: {0}")]
    LessonDoc(String),
}

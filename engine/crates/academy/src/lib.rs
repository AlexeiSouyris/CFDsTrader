//! Academia (docs/design/2026-11-academia.md, #709): catálogo de lecciones y herramientas con
//! candado (`config/academy.toml`), corrección de exámenes y progreso desde eventos de solo
//! agregar. Todo lo calcula el motor; la UI solo muestra (CLAUDE.md §1). Nada de esto toca el
//! riesgo, el paper ni las órdenes: el desbloqueo solo cambia qué se ve en la UI.
//!
//! - [`AcademyConfig`]: la config validada; [`Catalog`] es `academy_catalog.v1`, sin las claves.
//! - [`grade_quiz`]: `quiz_answer.v1` → `quiz_result.v1`.
//! - [`progress`]: eventos (`academy_event.v1`) → `academy_progress.v1`.
//! - [`pg::PgAcademy`]: la tabla `academy_events` (migración 0052), de solo agregar.

pub mod catalog;
pub mod pg;
pub mod progress;
pub mod quiz;

pub use catalog::{AcademyConfig, Catalog, ExamKind, QuestionKind, Thresholds};
pub use progress::{
    AcademyEvent, EventBody, EventRequest, OverrideAction, Progress, UnlockedBy, progress,
};
pub use quiz::{QuizAnswer, QuizError, QuizResult, grade_quiz};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcademyError {
    #[error("config/academy.toml inválida: {0}")]
    Config(String),
}

//! Academia (docs/design/2026-11-academia.md, #709): catálogo de lecciones y herramientas con
//! candado (`config/academy.toml`) y corrección de exámenes. Todo lo calcula el motor; la UI solo
//! muestra (CLAUDE.md §1). Nada de esto toca el riesgo, el paper ni las órdenes: el desbloqueo
//! solo cambia qué se ve en la UI.
//!
//! - [`AcademyConfig`]: la config validada; [`Catalog`] es `academy_catalog.v1`, sin las claves.
//! - [`grade_quiz`]: `quiz_answer.v1` → `quiz_result.v1`.

pub mod catalog;
pub mod quiz;

pub use catalog::{AcademyConfig, Catalog, ExamKind, QuestionKind, Thresholds};
pub use quiz::{QuizAnswer, QuizError, QuizResult, grade_quiz};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AcademyError {
    #[error("config/academy.toml inválida: {0}")]
    Config(String),
}

//! Corrección de un examen de preguntas (`quiz_answer.v1` → `quiz_result.v1`). Función pura:
//! las mismas respuestas con la misma config dan siempre la misma nota.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::AcademyConfig;
use crate::catalog::{ExamKind, Key};

/// Una respuesta: una opción o un número.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuizAnswerItem {
    pub question_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Decimal>,
}

/// `contracts/quiz_answer.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuizAnswer {
    pub lesson_id: String,
    pub catalog_version: String,
    pub answers: Vec<QuizAnswerItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionResult {
    pub question_id: String,
    pub correct: bool,
}

/// `contracts/quiz_result.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuizResult {
    pub lesson_id: String,
    pub catalog_version: String,
    pub score_pct: Decimal,
    pub pass_pct: Decimal,
    pub passed: bool,
    pub correct: u32,
    pub total: u32,
    pub questions: Vec<QuestionResult>,
    pub graded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuizError {
    #[error("no hay una lección {0}")]
    UnknownLesson(String),
    #[error("la lección {0} no tiene examen de preguntas")]
    NotAQuiz(String),
    #[error("el examen de la lección {0} todavía no tiene preguntas")]
    NotReady(String),
    #[error("el catálogo cambió: volvé a cargar las preguntas")]
    StaleCatalog,
    #[error(
        "respuesta inválida para {0}: pregunta de otra lección, repetida o del tipo equivocado"
    )]
    BadAnswer(String),
}

/// Corrige `answer` con las claves de `cfg`. Una pregunta sin responder cuenta como incorrecta;
/// la nota se redondea hacia abajo a 2 decimales.
pub fn grade_quiz(
    cfg: &AcademyConfig,
    answer: &QuizAnswer,
    graded_at: DateTime<Utc>,
) -> Result<QuizResult, QuizError> {
    let lesson = cfg
        .lesson(&answer.lesson_id)
        .ok_or_else(|| QuizError::UnknownLesson(answer.lesson_id.clone()))?;
    if lesson.exam.kind != ExamKind::Quiz {
        return Err(QuizError::NotAQuiz(lesson.id.clone()));
    }
    if !lesson.exam.ready {
        return Err(QuizError::NotReady(lesson.id.clone()));
    }
    if answer.catalog_version != cfg.catalog.catalog_version {
        return Err(QuizError::StaleCatalog);
    }
    let mut seen = BTreeSet::new();
    for a in &answer.answers {
        let q = lesson.exam.questions.iter().find(|q| q.id == a.question_id);
        let fits = match (q.and_then(|q| cfg.key(&q.id)), a) {
            (
                Some(Key::Choice(_)),
                QuizAnswerItem {
                    choice: Some(c),
                    value: None,
                    ..
                },
            ) => q
                .and_then(|q| q.options.as_ref())
                .is_some_and(|o| o.iter().any(|o| o.id == *c)),
            (
                Some(Key::Number { .. }),
                QuizAnswerItem {
                    choice: None,
                    value: Some(_),
                    ..
                },
            ) => true,
            _ => false,
        };
        if !fits || !seen.insert(a.question_id.as_str()) {
            return Err(QuizError::BadAnswer(a.question_id.clone()));
        }
    }
    let questions: Vec<QuestionResult> = lesson
        .exam
        .questions
        .iter()
        .map(|q| {
            let given = answer.answers.iter().find(|a| a.question_id == q.id);
            let correct = match (cfg.key(&q.id), given) {
                (Some(Key::Choice(k)), Some(a)) => a.choice.as_ref() == Some(k),
                (Some(Key::Number { value, tolerance }), Some(a)) => {
                    a.value.is_some_and(|v| (v - value).abs() <= *tolerance)
                }
                _ => false,
            };
            QuestionResult {
                question_id: q.id.clone(),
                correct,
            }
        })
        .collect();
    let total = u32::try_from(questions.len()).unwrap_or(u32::MAX);
    let correct = u32::try_from(questions.iter().filter(|q| q.correct).count()).unwrap_or(0);
    let score_pct = (Decimal::from(correct) * Decimal::ONE_HUNDRED / Decimal::from(total.max(1)))
        .round_dp_with_strategy(2, RoundingStrategy::ToZero)
        .normalize();
    let pass_pct = cfg.catalog.thresholds.quiz_pass_pct;
    Ok(QuizResult {
        lesson_id: lesson.id.clone(),
        catalog_version: cfg.catalog.catalog_version.clone(),
        score_pct,
        pass_pct,
        passed: score_pct >= pass_pct,
        correct,
        total,
        questions,
        graded_at,
    })
}

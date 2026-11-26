//! Eventos de progreso (`academy_event.v1`, solo agregar) y el estado que el motor calcula
//! leyéndolos en orden (`academy_progress.v1`). Función pura: nada del estado se guarda aparte.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::catalog::{ExamKind, dotted};
use crate::{AcademyConfig, QuizResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverrideAction {
    /// Se desbloquea a mano.
    Unlock,
    /// Se quita el desbloqueo a mano: vuelve a lo que digan las lecciones.
    Relock,
}

/// Qué pasó, con los campos que corresponden a cada tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventBody {
    LessonCompleted {
        lesson_id: String,
    },
    ExamAttempt {
        lesson_id: String,
        quiz: Box<QuizResult>,
    },
    /// `tool_id` es una herramienta o `*` (todas).
    ToolOverride {
        tool_id: String,
        action: OverrideAction,
    },
}

/// `contracts/academy_event.v1`. `seq` lo asigna la base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawEvent", into = "RawEvent")]
pub struct AcademyEvent {
    pub seq: i64,
    pub member_id: String,
    pub body: EventBody,
    pub catalog_version: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    LessonCompleted,
    ExamAttempt,
    ToolOverride,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEvent {
    seq: i64,
    member_id: String,
    kind: EventKind,
    #[serde(deserialize_with = "Option::deserialize")]
    lesson_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    tool_id: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    action: Option<OverrideAction>,
    #[serde(deserialize_with = "Option::deserialize")]
    quiz: Option<QuizResult>,
    catalog_version: String,
    recorded_at: DateTime<Utc>,
}

pub fn member_id_ok(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub fn tool_or_all_ok(s: &str) -> bool {
    s == "*"
        || s.strip_prefix("tool.").is_some_and(|n| {
            (1..=48).contains(&n.len())
                && n.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        })
}

impl TryFrom<RawEvent> for AcademyEvent {
    type Error = String;

    fn try_from(r: RawEvent) -> Result<Self, String> {
        let lesson_ok = |l: &str| dotted(l, 2).is_some();
        let body = match (r.kind, r.lesson_id, r.tool_id, r.action, r.quiz) {
            (EventKind::LessonCompleted, Some(lesson_id), None, None, None)
                if lesson_ok(&lesson_id) =>
            {
                EventBody::LessonCompleted { lesson_id }
            }
            (EventKind::ExamAttempt, Some(lesson_id), None, None, Some(quiz))
                if lesson_ok(&lesson_id) =>
            {
                EventBody::ExamAttempt {
                    lesson_id,
                    quiz: Box::new(quiz),
                }
            }
            (EventKind::ToolOverride, None, Some(tool_id), Some(action), None)
                if tool_or_all_ok(&tool_id) =>
            {
                EventBody::ToolOverride { tool_id, action }
            }
            _ => return Err("academy_event.v1: campos que no corresponden a su kind".into()),
        };
        if r.seq < 1 || !member_id_ok(&r.member_id) {
            return Err("academy_event.v1: seq >= 1 y member_id válido".into());
        }
        Ok(Self {
            seq: r.seq,
            member_id: r.member_id,
            body,
            catalog_version: r.catalog_version,
            recorded_at: r.recorded_at,
        })
    }
}

impl From<AcademyEvent> for RawEvent {
    fn from(e: AcademyEvent) -> Self {
        let (kind, lesson_id, tool_id, action, quiz) = match e.body {
            EventBody::LessonCompleted { lesson_id } => (
                EventKind::LessonCompleted,
                Some(lesson_id),
                None,
                None,
                None,
            ),
            EventBody::ExamAttempt { lesson_id, quiz } => (
                EventKind::ExamAttempt,
                Some(lesson_id),
                None,
                None,
                Some(*quiz),
            ),
            EventBody::ToolOverride { tool_id, action } => (
                EventKind::ToolOverride,
                None,
                Some(tool_id),
                Some(action),
                None,
            ),
        };
        Self {
            seq: e.seq,
            member_id: e.member_id,
            kind,
            lesson_id,
            tool_id,
            action,
            quiz,
            catalog_version: e.catalog_version,
            recorded_at: e.recorded_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LessonProgress {
    pub lesson_id: String,
    pub passed: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub passed_at: Option<DateTime<Utc>>,
    pub attempts: u32,
    #[serde(deserialize_with = "Option::deserialize")]
    pub best_score_pct: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnlockedBy {
    Lesson,
    Override,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolProgress {
    pub tool_id: String,
    pub unlocked: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub by: Option<UnlockedBy>,
    pub lesson_id: String,
}

/// `contracts/academy_progress.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub member_id: String,
    pub catalog_version: String,
    pub as_of: DateTime<Utc>,
    pub lessons: Vec<LessonProgress>,
    pub tools: Vec<ToolProgress>,
    pub unlock_all: bool,
}

/// El estado de `member_id` a partir de sus eventos, en el orden de `seq` (los de otros miembros
/// se ignoran). Una lección se aprueba con un intento aprobado, o, si no tiene examen, al
/// completarla. Cada herramienta queda abierta si se aprobó su lección o si el último desbloqueo a
/// mano que la alcanza (el suyo o uno de `*`) es `unlock`.
pub fn progress(
    cfg: &AcademyConfig,
    member_id: &str,
    events: &[AcademyEvent],
    as_of: DateTime<Utc>,
) -> Progress {
    let mut mine: Vec<&AcademyEvent> = events.iter().filter(|e| e.member_id == member_id).collect();
    mine.sort_by_key(|e| e.seq);
    let mut manual: BTreeMap<&str, bool> = BTreeMap::new();
    let mut unlock_all = false;
    for e in &mine {
        if let EventBody::ToolOverride { tool_id, action } = &e.body {
            let on = *action == OverrideAction::Unlock;
            if tool_id == "*" {
                unlock_all = on;
                for t in &cfg.catalog.tools {
                    manual.insert(&t.id, on);
                }
            } else {
                manual.insert(tool_id, on);
            }
        }
    }
    let lessons: Vec<LessonProgress> = cfg
        .lessons()
        .map(|l| {
            let mut p = LessonProgress {
                lesson_id: l.id.clone(),
                passed: false,
                passed_at: None,
                attempts: 0,
                best_score_pct: None,
            };
            for e in &mine {
                let passed_now = match &e.body {
                    EventBody::ExamAttempt { lesson_id, quiz } if *lesson_id == l.id => {
                        p.attempts += 1;
                        p.best_score_pct = p.best_score_pct.max(Some(quiz.score_pct));
                        quiz.passed
                    }
                    EventBody::LessonCompleted { lesson_id } => {
                        *lesson_id == l.id && l.exam.kind == ExamKind::None
                    }
                    _ => false,
                };
                if passed_now && !p.passed {
                    p.passed = true;
                    p.passed_at = Some(e.recorded_at);
                }
            }
            p
        })
        .collect();
    let tools = cfg
        .catalog
        .tools
        .iter()
        .map(|t| {
            let by_lesson = lessons
                .iter()
                .any(|l| l.lesson_id == t.lesson_id && l.passed);
            let by = if by_lesson {
                Some(UnlockedBy::Lesson)
            } else if manual.get(t.id.as_str()).copied().unwrap_or(false) {
                Some(UnlockedBy::Override)
            } else {
                None
            };
            ToolProgress {
                tool_id: t.id.clone(),
                unlocked: by.is_some(),
                by,
                lesson_id: t.lesson_id.clone(),
            }
        })
        .collect();
    Progress {
        member_id: member_id.to_owned(),
        catalog_version: cfg.catalog.catalog_version.clone(),
        as_of,
        lessons,
        tools,
        unlock_all,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RequestKind {
    LessonCompleted,
    ToolOverride,
}

/// `$defs/request` de `academy_event.v1`: el body de `POST /api/v1/academy/events`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRequest {
    kind: RequestKind,
    lesson_id: Option<String>,
    tool_id: Option<String>,
    action: Option<OverrideAction>,
}

impl EventRequest {
    /// El evento que pide, si vale contra el catálogo: completar una lección **sin examen**, o
    /// desbloquear o rebloquear una herramienta del catálogo (o todas con `*`).
    pub fn body(self, cfg: &AcademyConfig) -> Result<EventBody, String> {
        match (self.kind, self.lesson_id, self.tool_id, self.action) {
            (RequestKind::LessonCompleted, Some(lesson_id), None, None) => {
                match cfg.lesson(&lesson_id) {
                    Some(l) if l.exam.kind == ExamKind::None => {
                        Ok(EventBody::LessonCompleted { lesson_id })
                    }
                    Some(_) => Err(format!("la lección {lesson_id} se aprueba con su examen")),
                    None => Err(format!("no hay una lección {lesson_id}")),
                }
            }
            (RequestKind::ToolOverride, None, Some(tool_id), Some(action)) => {
                if tool_id == "*" || cfg.catalog.tools.iter().any(|t| t.id == tool_id) {
                    Ok(EventBody::ToolOverride { tool_id, action })
                } else {
                    Err(format!("no hay una herramienta {tool_id}"))
                }
            }
            _ => Err("lesson_completed lleva lesson_id; tool_override, tool_id y action".into()),
        }
    }
}

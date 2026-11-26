//! Catálogo de la Academia (`config/academy.toml`), validado al arrancar, y su vista pública
//! `academy_catalog.v1` **sin las claves** de las respuestas.

use std::collections::BTreeSet;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::AcademyError;
use crate::replay::ReplayKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExamKind {
    Quiz,
    Replay,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestionKind {
    Choice,
    Number,
}

/// La clave de una pregunta. Nunca sale del motor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Choice(String),
    Number { value: Decimal, tolerance: Decimal },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionOption {
    pub id: String,
    pub text: String,
}

/// Una pregunta como la ve la UI (`academy_catalog.v1`): sin la clave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub id: String,
    pub kind: QuestionKind,
    pub prompt: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub options: Option<Vec<QuestionOption>>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exam {
    pub kind: ExamKind,
    pub ready: bool,
    pub questions: Vec<Question>,
    /// El tipo de sesión de un examen de replay (`levels` o `trades`, #759); `None` en los demás.
    #[serde(default)]
    pub replay_kind: Option<ReplayKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub id: String,
    pub title: String,
    pub objective: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub doc: Option<String>,
    pub exam: Exam,
    pub unlocks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub level: u8,
    pub title: String,
    pub lessons: Vec<Lesson>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub lesson_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    pub quiz_pass_pct: Decimal,
    pub locked_tools_visible: bool,
    pub provisional: bool,
}

/// `contracts/academy_catalog.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub catalog_version: String,
    pub levels: Vec<Level>,
    pub tools: Vec<Tool>,
    pub thresholds: Thresholds,
}

/// La config validada: el catálogo público más las claves, en el mismo orden que las preguntas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcademyConfig {
    pub catalog: Catalog,
    /// `[replay]`: sesiones de replay de los exámenes (#721).
    pub replay: crate::replay::ReplayConfig,
    /// `(id de la pregunta, clave)`.
    keys: Vec<(String, Key)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    thresholds: Thresholds,
    replay: crate::replay::ReplayConfig,
    tools: Vec<RawTool>,
    levels: Vec<RawLevel>,
    lessons: Vec<RawLesson>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTool {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLevel {
    level: u8,
    title: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLesson {
    id: String,
    title: String,
    objective: String,
    doc: Option<String>,
    exam: ExamKind,
    /// Solo con `exam = "replay"`: el tipo de sesión del examen (#759).
    replay_kind: Option<ReplayKind>,
    unlocks: Vec<String>,
    #[serde(default)]
    questions: Vec<RawQuestion>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawQuestion {
    id: String,
    kind: QuestionKind,
    prompt: String,
    options: Option<Vec<QuestionOption>>,
    unit: Option<String>,
    answer: String,
    tolerance: Option<Decimal>,
}

/// Corta la carga con un error de config si no se cumple la condición.
macro_rules! ensure {
    ($cond:expr, $($msg:tt)+) => {
        if !$cond {
            return Err(AcademyError::Config(format!($($msg)+)));
        }
    };
}

fn text_ok(s: &str) -> bool {
    (1..=2000).contains(&s.chars().count()) && s.trim() == s
}

fn tool_id_ok(s: &str) -> bool {
    let name_ok = |n: &str| {
        (1..=48).contains(&n.len())
            && n.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    };
    s.strip_prefix("tool.").is_some_and(name_ok)
}

/// `"N.M"` (o `"N.M.Q"` con `parts = 3`): números de 1 o 2 cifras sin ceros a la izquierda; solo
/// el primero puede ser 0. Devuelve las partes.
pub(crate) fn dotted(s: &str, parts: usize) -> Option<Vec<u8>> {
    let v: Vec<&str> = s.split('.').collect();
    let part = |(i, p): (usize, &&str)| {
        let digits = (1..=2).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit());
        let n: u8 = p
            .parse()
            .ok()
            .filter(|_| digits && !(p.len() == 2 && p.starts_with('0')))?;
        (i == 0 || n > 0).then_some(n)
    };
    (v.len() == parts).then(|| v.iter().enumerate().map(part).collect())?
}

fn doc_ok(d: &str) -> bool {
    let path_ok = |r: &str| {
        (1..=120).contains(&r.len())
            && !r.contains("..")
            && r.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._/-".contains(&b))
    };
    d.strip_prefix("docs/academia/")
        .and_then(|r| r.strip_suffix(".md"))
        .is_some_and(path_ok)
}

fn question(lesson: &str, q: RawQuestion) -> Result<(Question, Key), AcademyError> {
    let id = &q.id;
    ensure!(
        dotted(id, 3).is_some() && id.starts_with(&format!("{lesson}.")) && text_ok(&q.prompt),
        "pregunta {id}: id <lección>.<n> y texto"
    );
    let key = match q.kind {
        QuestionKind::Choice => {
            let opts = q.options.as_deref().unwrap_or_default();
            let ids: BTreeSet<&str> = opts.iter().map(|o| o.id.as_str()).collect();
            let opt_ok = |o: &QuestionOption| {
                o.id.len() == 1 && o.id.bytes().all(|b| b.is_ascii_lowercase()) && text_ok(&o.text)
            };
            ensure!(
                opts.len() >= 2 && ids.len() == opts.len() && opts.iter().all(opt_ok),
                "pregunta {id}: al menos 2 opciones a-z distintas"
            );
            ensure!(
                ids.contains(q.answer.as_str()),
                "pregunta {id}: la clave no está entre las opciones"
            );
            ensure!(
                q.unit.is_none() && q.tolerance.is_none(),
                "pregunta {id}: choice sin unit ni tolerance"
            );
            Key::Choice(q.answer)
        }
        QuestionKind::Number => {
            let value = q.answer.parse::<Decimal>().ok();
            let tolerance = q.tolerance.unwrap_or_default();
            let unit_ok = q
                .unit
                .as_deref()
                .is_none_or(|u| (1..=16).contains(&u.chars().count()));
            ensure!(
                value.is_some() && q.options.is_none() && tolerance >= Decimal::ZERO && unit_ok,
                "pregunta {id}: number con clave decimal, sin opciones, tolerancia >= 0 y unidad de 1 a 16"
            );
            Key::Number {
                value: value.unwrap_or_default(),
                tolerance,
            }
        }
    };
    Ok((
        Question {
            id: q.id,
            kind: q.kind,
            prompt: q.prompt,
            options: q.options,
            unit: q.unit,
        },
        key,
    ))
}

impl AcademyConfig {
    /// Lee y valida `config/academy.toml`. La versión del catálogo es el SHA-256 de `text`.
    pub fn from_toml(text: &str) -> Result<Self, AcademyError> {
        let raw: Raw = toml::from_str(text).map_err(|e| AcademyError::Config(e.to_string()))?;
        let pass = raw.thresholds.quiz_pass_pct;
        ensure!(
            pass > Decimal::ZERO && pass <= Decimal::ONE_HUNDRED,
            "thresholds.quiz_pass_pct: entre 0 (excluido) y 100"
        );
        raw.replay.validate().map_err(AcademyError::Config)?;
        let mut tool_ids = BTreeSet::new();
        for t in &raw.tools {
            ensure!(
                tool_id_ok(&t.id) && text_ok(&t.name) && tool_ids.insert(t.id.as_str()),
                "herramienta {}: id tool.<a-z0-9_> único y nombre",
                t.id
            );
        }
        let mut levels: Vec<Level> = Vec::new();
        for l in raw.levels {
            ensure!(
                levels.last().is_none_or(|p| p.level < l.level)
                    && l.level <= 99
                    && text_ok(&l.title),
                "nivel {}: en orden creciente, sin repetir y con título",
                l.level
            );
            levels.push(Level {
                level: l.level,
                title: l.title,
                lessons: Vec::new(),
            });
        }
        let (mut keys, mut lesson_ids, mut unlocked_by) =
            (Vec::new(), BTreeSet::new(), Vec::<(String, String)>::new());
        for l in raw.lessons {
            let id = l.id.clone();
            let level_no = dotted(&id, 2).map(|p| p[0]);
            let level = levels.iter_mut().find(|v| Some(v.level) == level_no);
            ensure!(
                level.is_some(),
                "lección {id}: id N.M de un nivel que existe"
            );
            ensure!(
                lesson_ids.insert(id.clone())
                    && text_ok(&l.title)
                    && text_ok(&l.objective)
                    && l.doc.as_deref().is_none_or(doc_ok),
                "lección {id}: id único, título, objetivo y doc docs/academia/*.md"
            );
            ensure!(
                l.exam == ExamKind::Quiz || l.questions.is_empty(),
                "lección {id}: solo un examen quiz lleva preguntas"
            );
            let replay_ok = match (l.exam, l.replay_kind) {
                (ExamKind::Replay, Some(k)) => k != ReplayKind::Practice,
                (ExamKind::Replay, None) => false,
                (_, kind) => kind.is_none(),
            };
            ensure!(
                replay_ok,
                "lección {id}: un examen de replay lleva replay_kind levels o trades, y solo él"
            );
            let mut questions = Vec::new();
            for q in l.questions {
                let (public, key) = question(&id, q)?;
                ensure!(
                    keys.iter().all(|(k, _)| *k != public.id),
                    "pregunta {} repetida",
                    public.id
                );
                keys.push((public.id.clone(), key));
                questions.push(public);
            }
            for t in &l.unlocks {
                ensure!(
                    tool_ids.contains(t.as_str()) && unlocked_by.iter().all(|(u, _)| u != t),
                    "lección {id}: {t} no está en [[tools]] o ya la abre otra lección"
                );
                unlocked_by.push((t.clone(), id.clone()));
            }
            // Un examen de replay no tiene preguntas: lo califica el motor con la sesión (#759).
            let ready = l.exam != ExamKind::Quiz || !questions.is_empty();
            let exam = Exam {
                kind: l.exam,
                ready,
                questions,
                replay_kind: l.replay_kind,
            };
            let lesson = Lesson {
                id,
                title: l.title,
                objective: l.objective,
                doc: l.doc,
                exam,
                unlocks: l.unlocks,
            };
            level
                .into_iter()
                .for_each(|v| v.lessons.push(lesson.clone()));
        }
        let mut tools = Vec::new();
        for t in raw.tools {
            let lesson_id = unlocked_by
                .iter()
                .find(|(u, _)| *u == t.id)
                .map(|(_, l)| l.clone());
            ensure!(
                lesson_id.is_some(),
                "herramienta {}: ninguna lección la abre",
                t.id
            );
            tools.push(Tool {
                id: t.id,
                name: t.name,
                lesson_id: lesson_id.unwrap_or_default(),
            });
        }
        let catalog_version = Sha256::digest(text.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let catalog = Catalog {
            catalog_version,
            levels,
            tools,
            thresholds: raw.thresholds,
        };
        Ok(Self {
            catalog,
            replay: raw.replay,
            keys,
        })
    }

    pub fn lessons(&self) -> impl Iterator<Item = &Lesson> {
        self.catalog.levels.iter().flat_map(|l| &l.lessons)
    }

    pub fn lesson(&self, id: &str) -> Option<&Lesson> {
        self.lessons().find(|l| l.id == id)
    }

    pub(crate) fn key(&self, question_id: &str) -> Option<&Key> {
        self.keys
            .iter()
            .find(|(id, _)| id == question_id)
            .map(|(_, k)| k)
    }
}

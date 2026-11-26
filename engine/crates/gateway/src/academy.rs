//! Rutas de la Academia (#719, docs/design/2026-11-academia.md §4 y §5.5). Todo lo calcula
//! `cfds-academy`; el gateway solo lee, escribe el evento y responde.
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/academy/catalog` | `academy_catalog.v1` (sin las claves) |
//! | `GET /api/v1/academy/progress` | `academy_progress.v1` |
//! | `POST /api/v1/academy/events` | `academy_event.v1#/$defs/request` → `academy_event.v1` (guardado) |
//! | `POST /api/v1/academy/quiz/{lesson_id}` | `quiz_answer.v1` → `quiz_result.v1` (el intento se guarda) |
//!
//! Escritura: los mismos chequeos que el broker simulado (token anti-CSRF `x-cfds-token`,
//! `application/json`, tamaño; [`crate::paper::write_guard`]) y solo en `academy_events`, de solo
//! agregar. Nada de esto toca el riesgo, el paper ni las órdenes.

use std::future::Future;

use cfds_academy::pg::PgAcademy;
use cfds_academy::{
    AcademyConfig, AcademyEvent, EventBody, EventRequest, QuizAnswer, QuizError, grade_quiz,
    progress,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::Reply;
use crate::paper::{HttpIn, write_guard};

const PREFIX: &str = "/api/v1/academy/";

/// `[academy]` de `config/gateway.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcademyRoutes {
    /// La persona de la Academia (spec §1.2). Hoy solo el dueño.
    pub member_id: String,
    pub max_body_bytes: usize,
}

impl AcademyRoutes {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            academy: RawAcademy,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawAcademy {
            member_id: String,
            max_body_bytes: usize,
        }
        let r = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .academy;
        if !cfds_academy::progress::member_id_ok(&r.member_id) || r.max_body_bytes == 0 {
            return Err("academy: member_id [A-Za-z0-9._-]{1,64} y max_body_bytes > 0".into());
        }
        Ok(Self {
            member_id: r.member_id,
            max_body_bytes: r.max_body_bytes,
        })
    }
}

/// Dónde se guardan los eventos. En producción, PostgreSQL (`PgAcademy`).
pub trait AcademyStore: Sync {
    fn append(
        &self,
        member_id: &str,
        body: EventBody,
        catalog_version: &str,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<AcademyEvent, String>> + Send;
    fn events(
        &self,
        member_id: &str,
    ) -> impl Future<Output = Result<Vec<AcademyEvent>, String>> + Send;
}

impl AcademyStore for PgAcademy {
    async fn append(
        &self,
        member_id: &str,
        body: EventBody,
        catalog_version: &str,
        at: DateTime<Utc>,
    ) -> Result<AcademyEvent, String> {
        PgAcademy::append(self, member_id, body, catalog_version, at)
            .await
            .map_err(|e| e.to_string())
    }

    async fn events(&self, member_id: &str) -> Result<Vec<AcademyEvent>, String> {
        PgAcademy::events(self, member_id)
            .await
            .map_err(|e| e.to_string())
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_academy_route(path: &str) -> bool {
    path.starts_with(PREFIX)
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

fn json<T: serde::Serialize>(v: &T) -> Reply {
    match serde_json::to_string(v) {
        Ok(body) => Reply {
            status: 200,
            content_type: "application/json",
            body,
        },
        Err(e) => internal(e.to_string()),
    }
}

fn internal(e: String) -> Reply {
    eprintln!("gateway: academia: {e}");
    text(500, "error interno")
}

/// Atiende las rutas de la Academia. `token` es el token local esperado (`None`: escritura
/// apagada).
pub async fn respond_academy<S: AcademyStore>(
    store: &S,
    academy: &AcademyConfig,
    routes: &AcademyRoutes,
    token: Option<&str>,
    req: HttpIn<'_>,
    now: DateTime<Utc>,
) -> Reply {
    let rest = req.path.strip_prefix(PREFIX).unwrap_or_default();
    let quiz_lesson = rest.strip_prefix("quiz/");
    let read = matches!(rest, "catalog" | "progress");
    if !read && rest != "events" && quiz_lesson.is_none() {
        return text(404, "ruta desconocida");
    }
    let wanted = if read { "GET" } else { "POST" };
    if req.method != wanted {
        return text(405, &format!("solo {wanted}"));
    }
    let member = routes.member_id.as_str();
    let version = academy.catalog.catalog_version.as_str();
    if rest == "catalog" {
        return json(&academy.catalog);
    }
    if rest == "progress" {
        return match store.events(member).await {
            Ok(events) => json(&progress(academy, member, &events, now)),
            Err(e) => internal(e),
        };
    }
    if let Some(refused) = write_guard(token, &req, routes.max_body_bytes) {
        return refused;
    }
    if let Some(lesson) = quiz_lesson {
        let answer: QuizAnswer = match serde_json::from_slice(req.body) {
            Ok(a) => a,
            Err(e) => return text(400, &format!("no es un quiz_answer.v1 válido: {e}")),
        };
        if answer.lesson_id != lesson {
            return text(400, "lesson_id del body distinto del de la ruta");
        }
        let result = match grade_quiz(academy, &answer, now) {
            Ok(r) => r,
            Err(e @ QuizError::UnknownLesson(_)) => return text(404, &e.to_string()),
            Err(e @ (QuizError::NotReady(_) | QuizError::StaleCatalog)) => {
                return text(409, &e.to_string());
            }
            Err(e) => return text(400, &e.to_string()),
        };
        // El intento queda guardado antes de responder: si no se puede guardar, no hay nota.
        let attempt = EventBody::ExamAttempt {
            lesson_id: result.lesson_id.clone(),
            quiz: Box::new(result.clone()),
        };
        return match store.append(member, attempt, version, now).await {
            Ok(_) => json(&result),
            Err(e) => internal(e),
        };
    }
    let request: EventRequest = match serde_json::from_slice(req.body) {
        Ok(r) => r,
        Err(e) => return text(400, &format!("no es un pedido de evento válido: {e}")),
    };
    let body = match request.body(academy) {
        Ok(b) => b,
        Err(e) => return text(400, &e),
    };
    match store.append(member, body, version, now).await {
        Ok(event) => json(&event),
        Err(e) => internal(e),
    }
}

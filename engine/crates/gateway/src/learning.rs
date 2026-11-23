//! Rutas de solo lectura de las fases 8 y 8b (#331): la comparación paper vs backtest y el informe
//! de aprendizaje. Los calcula el motor (`cfds-runner`, `cfds_journal::learning`) y los guarda el
//! diario; el gateway solo los lee y los devuelve tal cual.
//!
//! | Ruta | Respuesta |
//! |---|---|
//! | `GET /api/v1/paper_comparisons` | `paper_comparison.v1[]`: la última de cada instrumento |
//! | `GET /api/v1/paper_comparisons/{comparison_id}` | esa `paper_comparison.v1` (404 si no existe) |
//! | `GET /api/v1/learning_report` | el último `learning_report.v1`, o `null` si no hay |
//! | `GET /api/v1/learning_report/votes` | `change_vote_status.v1[]`: el estado de cada propuesta del último informe ahora (vacío si no hay informe) |
//!
//! No escribe nada y no hay rutas para **votar**: votar una propuesta de cambio necesita cuentas y
//! 2FA, una decisión aparte. El estado de cada propuesta lo cuenta el motor con sus votos
//! (`cfds_journal::votes::tally`, reglas de `config/votes.toml`); contar nunca aplica un cambio.

use std::future::Future;

use cfds_journal::pg::PgJournal;
use cfds_journal::votes::{ChangeStatus, VoteRules};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::Reply;

const COMPARISONS: &str = "/api/v1/paper_comparisons";
const COMPARISON_BY_ID: &str = "/api/v1/paper_comparisons/";
const LEARNING: &str = "/api/v1/learning_report";
const VOTES: &str = "/api/v1/learning_report/votes";

/// De dónde salen las comparaciones. En producción, el diario en PostgreSQL (tabla 0032).
pub trait ComparisonSource: Sync {
    fn latest(&self) -> impl Future<Output = Result<Vec<Value>, String>> + Send;
    fn by_id(&self, id: &str) -> impl Future<Output = Result<Option<Value>, String>> + Send;
}

impl ComparisonSource for PgJournal {
    async fn latest(&self) -> Result<Vec<Value>, String> {
        self.latest_paper_comparisons()
            .await
            .map_err(|e| e.to_string())
    }
    async fn by_id(&self, id: &str) -> Result<Option<Value>, String> {
        self.paper_comparison(id).await.map_err(|e| e.to_string())
    }
}

/// De dónde salen el último informe de aprendizaje y el estado de sus propuestas.
pub trait LearningSource: Sync {
    fn latest(&self) -> impl Future<Output = Result<Option<Value>, String>> + Send;
    /// El estado de cada propuesta del informe `report_id` en `now`.
    fn statuses(
        &self,
        report_id: &str,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<ChangeStatus>, String>> + Send;
}

/// En producción: el diario en PostgreSQL (`learning_reports`, `change_proposals` y
/// `change_votes`, migración 0034) con las reglas de voto de `config/votes.toml`.
#[derive(Debug, Clone)]
pub struct PgLearning {
    pub journal: PgJournal,
    pub rules: VoteRules,
}

impl LearningSource for PgLearning {
    async fn latest(&self) -> Result<Option<Value>, String> {
        self.journal
            .latest_learning_report()
            .await
            .map_err(|e| e.to_string())
    }
    async fn statuses(
        &self,
        report_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<ChangeStatus>, String> {
        self.journal
            .change_statuses(report_id, &self.rules, now)
            .await
            .map_err(|e| e.to_string())
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_learning_route(path: &str) -> bool {
    path == COMPARISONS || path == LEARNING || path == VOTES || path.starts_with(COMPARISON_BY_ID)
}

/// Mismo formato que `comparison_id` en `paper_comparison.v1`.
fn valid_id(id: &str) -> bool {
    (1..=200).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
        && id != "."
        && id != ".."
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
        Err(e) => {
            eprintln!("gateway: no se pudo serializar: {e}");
            text(500, "error interno")
        }
    }
}

fn internal(e: String) -> Reply {
    eprintln!("gateway: fase 8: {e}");
    text(500, "error interno")
}

/// Atiende las rutas de este módulo. Solo `GET`.
pub async fn respond_learning<C: ComparisonSource, L: LearningSource>(
    comparisons: &C,
    learning: &L,
    method: &str,
    path: &str,
    now: DateTime<Utc>,
) -> Reply {
    if !is_learning_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: estas rutas son de solo lectura");
    }
    if path == LEARNING {
        return learning.latest().await.map_or_else(internal, |r| json(&r));
    }
    if path == VOTES {
        let report = match learning.latest().await {
            Ok(r) => r,
            Err(e) => return internal(e),
        };
        let Some(report_id) = report
            .as_ref()
            .and_then(|r| r.get("report_id"))
            .and_then(Value::as_str)
        else {
            return json(&Vec::<ChangeStatus>::new());
        };
        return learning
            .statuses(report_id, now)
            .await
            .map_or_else(internal, |v| json(&v));
    }
    if path == COMPARISONS {
        return comparisons
            .latest()
            .await
            .map_or_else(internal, |v| json(&v));
    }
    let id = &path[COMPARISON_BY_ID.len()..];
    if !valid_id(id) {
        return text(400, "comparison_id inválido");
    }
    match comparisons.by_id(id).await {
        Ok(Some(c)) => json(&c),
        Ok(None) => text(404, "no hay una comparación con ese id"),
        Err(e) => internal(e),
    }
}

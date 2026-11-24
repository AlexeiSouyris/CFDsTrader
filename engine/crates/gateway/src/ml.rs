//! Ruta de solo lectura de los experimentos de ML (#426, fase 4b). Los entrena y guarda Python
//! (`cfds_intel.models.experiment`, tabla `ml_experiments`); el gateway solo los lee y los
//! devuelve tal cual, con el instrumento de su corrida (`ml_experiment_entry.v1`).
//!
//! | Ruta | Respuesta |
//! |---|---|
//! | `GET /api/v1/ml/experiments` | `ml_experiment_entry.v1[]`: el último experimento de cada instrumento |
//! | `GET /api/v1/ml/experiments?instrument=<id>` | `ml_experiment_entry.v1[]`: los de ese instrumento, del más nuevo al más viejo (hasta [`HISTORY_LIMIT`]) |
//!
//! El modelo es **solo informativo** (docs/spec/ml-v1.md): no decide operaciones. No escribe nada.

use std::future::Future;

use cfds_journal::pg::PgJournal;
use serde_json::Value;

use crate::Reply;

const EXPERIMENTS: &str = "/api/v1/ml/experiments";

/// Cuántos experimentos de un instrumento devuelve como máximo.
pub const HISTORY_LIMIT: i64 = 50;

/// De dónde salen los experimentos. En producción, el diario en PostgreSQL.
pub trait MlExperimentSource: Sync {
    fn entries(
        &self,
        instrument: Option<&str>,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<Value>, String>> + Send;
}

impl MlExperimentSource for PgJournal {
    async fn entries(&self, instrument: Option<&str>, limit: i64) -> Result<Vec<Value>, String> {
        self.ml_experiment_entries(instrument, limit)
            .await
            .map_err(|e| e.to_string())
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_ml_route(path: &str) -> bool {
    path == EXPERIMENTS
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

/// Mismo formato que `instrument_id` en `ml_experiment_entry.v1`.
fn valid_instrument(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .next()
            .is_some_and(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// `Ok(None)` sin parámetros; `Ok(Some(id))` con `instrument=<id>` válido y nada más.
fn instrument_param(query: Option<&str>) -> Result<Option<&str>, ()> {
    let Some(q) = query.filter(|q| !q.is_empty()) else {
        return Ok(None);
    };
    let mut found = None;
    for pair in q.split('&') {
        match pair.split_once('=') {
            Some(("instrument", v)) if found.is_none() && valid_instrument(v) => found = Some(v),
            // Un parámetro desconocido, repetido o inválido es un pedido mal armado.
            _ => return Err(()),
        }
    }
    Ok(found)
}

/// Atiende la ruta de los experimentos. Solo `GET`.
pub async fn respond_ml<S: MlExperimentSource>(
    source: &S,
    method: &str,
    path: &str,
    query: Option<&str>,
) -> Reply {
    if !is_ml_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: los experimentos de ML son de solo lectura");
    }
    let Ok(instrument) = instrument_param(query) else {
        return text(400, "el único parámetro es ?instrument=<id>");
    };
    match source.entries(instrument, HISTORY_LIMIT).await {
        Ok(entries) => match serde_json::to_string(&entries) {
            Ok(body) => Reply {
                status: 200,
                content_type: "application/json",
                body,
            },
            Err(e) => {
                eprintln!("gateway: no se pudo serializar los experimentos: {e}");
                text(500, "error interno")
            }
        },
        Err(e) => {
            eprintln!("gateway: experimentos de ML: {e}");
            text(500, "error interno")
        }
    }
}

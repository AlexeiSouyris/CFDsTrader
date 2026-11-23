//! Rutas de solo lectura de la evaluación de la Fase 5 (#298, #316). Lo calcula el motor
//! (`cfds_backtest::evaluation` y `sensitivity`) y lo guarda el diario (`walk_forward_reports`);
//! el gateway solo lo lee y lo devuelve tal cual.
//!
//! | Ruta | Respuesta |
//! |---|---|
//! | `GET /api/v1/walk_forward?instrument=<id>` | el último `walk_forward_report.v1` del instrumento, o `null` si no hay |
//! | `GET /api/v1/walk_forward/{report_id}` | ese `walk_forward_report.v1` (404 si no existe) |
//!
//! Es un módulo aparte de las demás rutas (panel de operaciones, fondo, paper): no escribe nada.

use std::future::Future;

use cfds_journal::WalkForwardReport;
use cfds_journal::pg::PgJournal;

use crate::Reply;

const LATEST: &str = "/api/v1/walk_forward";
const BY_ID: &str = "/api/v1/walk_forward/";

/// De dónde salen los reportes. En producción, el diario en PostgreSQL.
pub trait WalkForwardSource: Sync {
    fn latest(
        &self,
        instrument: &str,
    ) -> impl Future<Output = Result<Option<WalkForwardReport>, String>> + Send;
    fn by_id(
        &self,
        report_id: &str,
    ) -> impl Future<Output = Result<Option<WalkForwardReport>, String>> + Send;
}

impl WalkForwardSource for PgJournal {
    async fn latest(&self, instrument: &str) -> Result<Option<WalkForwardReport>, String> {
        self.latest_walk_forward(instrument)
            .await
            .map_err(|e| e.to_string())
    }
    async fn by_id(&self, report_id: &str) -> Result<Option<WalkForwardReport>, String> {
        self.walk_forward(report_id)
            .await
            .map_err(|e| e.to_string())
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_walk_forward_route(path: &str) -> bool {
    path == LATEST || path.starts_with(BY_ID)
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
            eprintln!("gateway: no se pudo serializar el walk-forward: {e}");
            text(500, "error interno")
        }
    }
}

/// Mismo formato que `instrument_id` en los contratos.
fn valid_instrument(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .next()
            .is_some_and(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// Mismo formato que `report_id` en `walk_forward_report.v1`.
fn valid_report_id(s: &str) -> bool {
    (1..=100).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn instrument_param(query: Option<&str>) -> Option<&str> {
    let mut found = None;
    for pair in query?.split('&') {
        match pair.split_once('=') {
            Some(("instrument", v)) if found.is_none() => found = Some(v),
            // Un parámetro desconocido o repetido es un pedido mal armado.
            _ => return None,
        }
    }
    found.filter(|v| valid_instrument(v))
}

/// Atiende las rutas del walk-forward. Solo `GET`.
pub async fn respond_walk_forward<S: WalkForwardSource>(
    source: &S,
    method: &str,
    path: &str,
    query: Option<&str>,
) -> Reply {
    if !is_walk_forward_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: el walk-forward es de solo lectura");
    }
    let found = if path == LATEST {
        let Some(instrument) = instrument_param(query) else {
            return text(400, "falta ?instrument=<id> (y nada más)");
        };
        match source.latest(instrument).await {
            // Sin reporte todavía: `null`, no un error.
            Ok(r) => return json(&r),
            Err(e) => Err(e),
        }
    } else {
        let id = &path[BY_ID.len()..];
        if !valid_report_id(id) || query.is_some() {
            return text(400, "report_id inválido");
        }
        source.by_id(id).await
    };
    match found {
        Ok(Some(r)) => json(&r),
        Ok(None) => text(404, "no hay un reporte con ese id"),
        Err(e) => {
            eprintln!("gateway: walk-forward: {e}");
            text(500, "error interno")
        }
    }
}

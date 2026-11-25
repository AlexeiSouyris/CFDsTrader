//! Ruta de solo lectura del diario de paper para la vista Diario de la UI (#615):
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/journal[?limit=<n>]` | `gateway_journal.v1` |
//!
//! Lee lo que guardó el runner de paper en el diario (spec §15b): las propuestas con
//! `source = paper` y su resultado simulado, de la señal más nueva a la más vieja. Los montos y el
//! resultado los calculó el motor al guardarlos; el gateway los devuelve tal cual, no calcula nada,
//! no envía órdenes ni escribe en la base.

use std::future::Future;

use cfds_journal::Source;
use cfds_journal::curve::RunTrade;
use cfds_journal::pg::PgJournal;
use serde::Serialize;

use crate::Reply;

const JOURNAL: &str = "/api/v1/journal";

/// Entradas sin `?limit`.
pub const DEFAULT_LIMIT: u32 = 100;
/// El máximo que acepta `?limit` (el mismo tope de `gateway_journal.v1`).
pub const MAX_LIMIT: u32 = 500;

/// De dónde sale el diario. En producción, PostgreSQL.
pub trait JournalSource: Sync {
    /// Ver [`PgJournal::source_trades`] con [`Source::Paper`].
    fn paper_trades(
        &self,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<RunTrade>, String>> + Send;
}

impl JournalSource for PgJournal {
    async fn paper_trades(&self, limit: u32) -> Result<Vec<RunTrade>, String> {
        self.source_trades(Source::Paper, i64::from(limit))
            .await
            .map_err(|e| e.to_string())
    }
}

/// `contracts/gateway_journal.v1`.
#[derive(Debug, Serialize)]
pub struct JournalPage {
    pub entries: Vec<RunTrade>,
    pub limit: u32,
}

/// `true` si la ruta es de este módulo.
pub fn is_journal_route(path: &str) -> bool {
    path == JOURNAL
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

/// `Ok(DEFAULT_LIMIT)` sin parámetros; `Ok(n)` con `limit=<n>` en `1..=MAX_LIMIT` y nada más.
fn limit_param(query: Option<&str>) -> Result<u32, ()> {
    let Some(q) = query.filter(|q| !q.is_empty()) else {
        return Ok(DEFAULT_LIMIT);
    };
    match q.split_once('=') {
        Some(("limit", v))
            if !v.is_empty() && v.len() <= 3 && v.bytes().all(|b| b.is_ascii_digit()) =>
        {
            let n: u32 = v.parse().map_err(|_| ())?;
            if (1..=MAX_LIMIT).contains(&n) {
                Ok(n)
            } else {
                Err(())
            }
        }
        // Un parámetro desconocido, repetido o inválido es un pedido mal armado.
        _ => Err(()),
    }
}

/// Atiende la ruta del diario. Solo `GET`.
pub async fn respond_journal<S: JournalSource>(
    source: &S,
    method: &str,
    path: &str,
    query: Option<&str>,
) -> Reply {
    if !is_journal_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: el diario es de solo lectura");
    }
    let Ok(limit) = limit_param(query) else {
        return text(400, "el único parámetro es ?limit=<n>, entre 1 y 500");
    };
    let entries = match source.paper_trades(limit).await {
        Ok(e) => e,
        Err(e) => {
            eprintln!("gateway: diario de paper: {e}");
            return text(500, "error interno");
        }
    };
    match serde_json::to_string(&JournalPage { entries, limit }) {
        Ok(body) => Reply {
            status: 200,
            content_type: "application/json",
            body,
        },
        Err(e) => {
            eprintln!("gateway: no se pudo serializar el diario: {e}");
            text(500, "error interno")
        }
    }
}

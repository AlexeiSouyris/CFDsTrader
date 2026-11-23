//! Rutas de solo lectura del fondo del grupo (fase 6b, #312). Todo lo calcula `cfds-fund` desde
//! el historial de `fund_movements`; el gateway no hace cuentas.
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/fund` | `fund_summary.v1`: NAV, valor total, unidades y la cuenta de cada miembro |
//! | `GET /api/v1/fund/movements` | `fund_movement.v1[]`: el historial, en orden de liquidación |
//!
//! No hay rutas de escritura: registrar aportes y retiros queda fuera del gateway.

use std::future::Future;

use cfds_fund::pg::PgFund;
use cfds_fund::{FundConfig, Record, summary};

use crate::Reply;

const SUMMARY: &str = "/api/v1/fund";
const MOVEMENTS: &str = "/api/v1/fund/movements";

/// De dónde sale el historial. En producción, PostgreSQL (`PgFund`).
pub trait FundSource: Sync {
    fn movements(&self) -> impl Future<Output = Result<Vec<Record>, String>> + Send;
}

impl FundSource for PgFund {
    async fn movements(&self) -> Result<Vec<Record>, String> {
        self.load().await.map_err(|e| e.to_string())
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_fund_route(path: &str) -> bool {
    matches!(path, SUMMARY | MOVEMENTS)
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
            eprintln!("gateway: no se pudo serializar el fondo: {e}");
            text(500, "error interno")
        }
    }
}

/// Atiende las rutas del fondo. Solo `GET`. Un historial inválido es un error interno: no se
/// muestra un fondo a medias.
pub async fn respond_fund<F: FundSource>(
    source: &F,
    cfg: &FundConfig,
    method: &str,
    path: &str,
) -> Reply {
    if !is_fund_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: el fondo es de solo lectura");
    }
    let records = match source.movements().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gateway: fondo: {e}");
            return text(500, "error interno");
        }
    };
    if path == MOVEMENTS {
        return json(&records);
    }
    match summary(&records, cfg) {
        Ok(s) => json(&s),
        Err(e) => {
            eprintln!("gateway: fondo: {e}");
            text(500, "error interno")
        }
    }
}

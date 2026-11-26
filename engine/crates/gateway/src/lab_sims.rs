//! Simuladores del laboratorio (#805): varianza (`docs/design/2026-11-laboratorio.md` §4.2) y
//! cuentas fondeadas (`docs/design/2026-11-academia.md` §6). **Sin efectos**: no guardan nada ni
//! mandan órdenes. El gateway solo lee el informe guardado cuando la fuente es `report` y corre
//! las funciones puras del motor (`cfds_backtest::lab::run_variance`, `cfds_propsim::run`) en un
//! hilo aparte (son cálculos de CPU acotados por los topes de config).
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `POST /api/v1/lab/variance` | `variance_sim_request.v1` → `variance_sim_report.v1` |
//! | `GET /api/v1/lab/propsim/templates` | lista de `prop_rules.v1` (`config/propsim.toml`) |
//! | `POST /api/v1/lab/propsim` | `prop_sim_request.v1` → `prop_sim_result.v1` |

use cfds_backtest::lab::{VarianceRequest, run_variance};
use cfds_propsim::{PropSimError, PropSimRequest};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::Reply;
use crate::lab::{LabDeps, LabStore, internal, is_sha256, json, json_with, text};
use crate::paper::{HttpIn, write_guard};

/// Lo que se lee de la lista de operaciones de un informe guardado (`lab_report.v1#/trade_list`).
enum Trades {
    Missing,
    NoList,
    List(Vec<(DateTime<Utc>, Decimal)>),
}

async fn report_trades<S: LabStore>(store: &S, id: &str) -> Result<Trades, Reply> {
    if !is_sha256(id) {
        return Err(text(400, "report_id: SHA-256 en hexadecimal"));
    }
    let report = match store.report(id).await {
        Ok(Some(r)) => r,
        Ok(None) => return Ok(Trades::Missing),
        Err(e) => return Err(internal(e)),
    };
    let Some(list) = report.get("trade_list").and_then(Value::as_array) else {
        return Ok(Trades::NoList);
    };
    // Una operación sin R medible (`r_net` null) no entra.
    let rows = list
        .iter()
        .filter_map(|t| {
            let at = t
                .get("closed_at")?
                .as_str()?
                .parse::<DateTime<Utc>>()
                .ok()?;
            let r = t.get("r_net")?.as_str()?.parse::<Decimal>().ok()?;
            Some((at, r))
        })
        .collect();
    Ok(Trades::List(rows))
}

/// La lista del informe, o la respuesta de error (404 si no existe, 422 si es anterior a #804).
async fn trades_or_reply<S: LabStore>(
    store: &S,
    id: &str,
) -> Result<Vec<(DateTime<Utc>, Decimal)>, Reply> {
    match report_trades(store, id).await? {
        Trades::Missing => Err(text(404, "no existe ese informe")),
        Trades::NoList => Err(text(
            422,
            "ese informe no guarda la lista de operaciones (es anterior a #804): volvé a correrlo",
        )),
        Trades::List(rows) => Ok(rows),
    }
}

/// `POST /api/v1/lab/variance`.
pub async fn variance<S: LabStore>(
    store: &S,
    deps: &LabDeps,
    token: Option<&str>,
    req: &HttpIn<'_>,
) -> Reply {
    if let Some(refused) = write_guard(token, req, deps.routes.sim_max_body_bytes) {
        return refused;
    }
    let request: VarianceRequest = match serde_json::from_slice(req.body) {
        Ok(r) => r,
        Err(e) => {
            return text(
                400,
                &format!("no es un variance_sim_request.v1 válido: {e}"),
            );
        }
    };
    let report_r = match request.report_id() {
        Some(id) => match trades_or_reply(store, id).await {
            Ok(rows) => Some(rows.into_iter().map(|(_, r)| r).collect::<Vec<_>>()),
            Err(reply) => return reply,
        },
        None => None,
    };
    let cfg = deps.variance.clone();
    let (risk, confidence) = (deps.risk.risk_per_trade_pct, deps.confidence);
    let run = tokio::task::spawn_blocking(move || {
        run_variance(&request, &cfg, report_r.as_deref(), risk, confidence)
    })
    .await;
    match run {
        Ok(Ok(report)) => json(&report),
        Ok(Err(e)) => text(422, &e.0),
        Err(e) => internal(e.to_string()),
    }
}

/// `GET /api/v1/lab/propsim/templates`: las plantillas de `config/propsim.toml` (hoy, solo la
/// genérica en blanco).
pub fn templates(deps: &LabDeps) -> Reply {
    json(&deps.propsim.templates)
}

/// `POST /api/v1/lab/propsim`.
pub async fn propsim<S: LabStore>(
    store: &S,
    deps: &LabDeps,
    token: Option<&str>,
    req: &HttpIn<'_>,
) -> Reply {
    if let Some(refused) = write_guard(token, req, deps.routes.sim_max_body_bytes) {
        return refused;
    }
    let request: PropSimRequest = match serde_json::from_slice(req.body) {
        Ok(r) => r,
        Err(e) => return text(400, &format!("no es un prop_sim_request.v1 válido: {e}")),
    };
    let report = match request.report_id() {
        Some(id) => match trades_or_reply(store, id).await {
            Ok(rows) => Some(rows),
            Err(reply) => return reply,
        },
        None => None,
    };
    let cfg = deps.propsim.clone();
    let run =
        tokio::task::spawn_blocking(move || cfds_propsim::run(&request, &cfg, report.as_deref()))
            .await;
    match run {
        Ok(Ok(result)) => json(&result),
        Ok(Err(e @ (PropSimError::Rules(_) | PropSimError::Bad(_)))) => {
            json_with(422, &e.to_json())
        }
        Err(e) => internal(e.to_string()),
    }
}

//! Operación segura del broker simulado (fase 9, #297): logs estructurados, alertas y la
//! compuerta de arranque y parada.
//!
//! ```text
//! arranque:  Starting ──reconciliación limpia──▶ Ready ──SIGTERM──▶ Stopping
//!            (no acepta órdenes)               (acepta)          (no acepta; guarda el registro)
//! ```
//! Si la reconciliación al arrancar no es limpia, la compuerta se queda en `Starting` (además
//! del kill switch): las órdenes responden 503 hasta que se arregle y se reinicie.

use std::sync::atomic::{AtomicU8, Ordering};

use cfds_broker::{BrokerEvent, Reconciliation};
use cfds_core::OrderStatus;
use cfds_obs::{AlertKind, Alerter, HttpPost, Level, Logger};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

/// En qué etapa está el broker simulado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    /// Arrancando o con la reconciliación al arrancar sin pasar: no acepta órdenes.
    Starting,
    /// Acepta órdenes.
    Ready,
    /// Apagándose: no acepta órdenes nuevas.
    Stopping,
}

/// La compuerta, compartida entre las rutas y la parada.
#[derive(Debug)]
pub struct Gate(AtomicU8);

impl Default for Gate {
    fn default() -> Self {
        Self(AtomicU8::new(0))
    }
}

impl Gate {
    pub fn state(&self) -> GateState {
        match self.0.load(Ordering::SeqCst) {
            1 => GateState::Ready,
            2 => GateState::Stopping,
            _ => GateState::Starting,
        }
    }

    /// Pasa a `state`. Desde `Stopping` no se vuelve atrás.
    pub fn set(&self, state: GateState) {
        let v = match state {
            GateState::Starting => 0,
            GateState::Ready => 1,
            GateState::Stopping => 2,
        };
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |cur| {
                (cur != 2).then_some(v)
            });
    }
}

/// La etapa después de la reconciliación al arrancar: lista solo si fue limpia.
pub fn gate_after_reconcile(rec: &Reconciliation) -> GateState {
    if rec.is_clean() {
        GateState::Ready
    } else {
        GateState::Starting
    }
}

/// Escribe cada evento del broker como una línea JSON (`target = "broker"`).
pub fn log_events(log: &Logger, events: &[BrokerEvent]) {
    for e in events {
        let Ok(Value::Object(mut fields)) = serde_json::to_value(e) else {
            continue;
        };
        let name = fields
            .remove("event")
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "broker_event".into());
        let level = match e {
            BrokerEvent::KillSwitch { .. } => Level::Warn,
            BrokerEvent::RiskDecision {
                accepted: false, ..
            } => Level::Warn,
            _ => Level::Info,
        };
        log.log(level, "broker", &name, Value::Object(fields));
    }
}

/// Las alertas que salen de los eventos: el kill switch y la racha de órdenes rechazadas.
pub async fn alert_events<H: HttpPost>(
    alerts: &Alerter<H>,
    events: &[BrokerEvent],
    now: DateTime<Utc>,
) {
    for e in events {
        match e {
            BrokerEvent::KillSwitch { reason, at } => {
                alerts.raise(AlertKind::KillSwitch, reason, *at).await;
            }
            BrokerEvent::OrderReport { report } => {
                alerts
                    .order_result(report.status == OrderStatus::Rejected, now)
                    .await;
            }
            _ => {}
        }
    }
}

/// La contraseña de una URL `scheme://usuario:clave@host/...`, para registrarla como secreto.
pub fn url_password(url: &str) -> Option<&str> {
    let rest = &url[url.find("://")? + 3..];
    let authority = &rest[..rest.find('/').unwrap_or(rest.len())];
    let userinfo = &authority[..authority.rfind('@')?];
    let pass = &userinfo[userinfo.find(':')? + 1..];
    (!pass.is_empty()).then_some(pass)
}

/// Un error que se loguea (sin secretos: los tapa el `Logger`) y se alerta.
pub async fn report_failure<H: HttpPost>(
    log: &Logger,
    alerts: &Alerter<H>,
    kind: AlertKind,
    event: &str,
    error: &str,
    now: DateTime<Utc>,
) {
    log.log(Level::Error, "gateway", event, json!({ "error": error }));
    alerts.raise(kind, &format!("{event}: {error}"), now).await;
}

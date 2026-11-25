//! Estado de la operación para la UI (#314), **solo lectura**:
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/ops/status` | `ops_status.v1`: compuerta, kill switch, pérdida diaria y apalancamiento, última reconciliación |
//! | `GET /api/v1/ops/events[?limit=<n>]` | `ops_event.v1[]`: los últimos eventos **agrupados**, el más nuevo primero (de la base; si no responde, de memoria); `[ops] events_limit` por defecto, como mucho `[ops] feed_size` |
//!
//! - Los números de riesgo son los de `paper_account.v1` (los calcula el broker con el riesgo);
//!   el límite de pérdida diaria es el de `config/risk.toml`. Acá no se calcula nada.
//! - Los eventos son las líneas del `Logger` de `cfds-obs` **ya redactadas** (sin secretos): el
//!   [`FeedWriter`] se pone detrás del `Logger`, escribe cada línea en la salida de siempre y
//!   guarda en memoria las del broker, las alertas y todo lo `warn` o `error`. Además las pasa,
//!   sin esperar nunca, a la tarea que las guarda en `ops_events` (#337, ver [`crate::ops_store`]).
//! - **Agrupados (#576):** un evento igual a otro (mismo nivel, origen, nombre y campos) no
//!   ocupa otra fila: suma a `count` y mueve `last_seen`; `ts` queda en la primera vez. En
//!   memoria se agrupa al llegar; de la base, sobre las últimas `[ops] group_scan` filas (la
//!   tabla sigue guardando cada uno: es de solo agregar).
//! - No hay rutas de escritura: el kill switch no se reinicia desde acá.

use std::collections::VecDeque;
use std::future::Future;
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use cfds_broker::Reconciliation;
use cfds_core::candle::utc_z;
use cfds_core::{KillSwitch, PaperAccount, decimal_str};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::Reply;
use crate::ops::GateState;
use crate::ops_store::{EventStore, Persistence, from_record};
use crate::paper::{PaperService, PgPaper};

const STATUS: &str = "/api/v1/ops/status";
const EVENTS: &str = "/api/v1/ops/events";

/// `[ops]` de `config/gateway.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusConfig {
    /// Cuántos eventos (grupos) guarda en memoria; también el máximo de `?limit=`.
    pub feed_size: usize,
    /// Cuántos grupos devuelve `GET /api/v1/ops/events` sin `?limit=` (#576).
    pub events_limit: usize,
    /// Cuántas filas de la base se leen para agruparlas (#576).
    pub group_scan: usize,
    /// Días de eventos que conserva la base (los borra el job de retención).
    pub retention_days: u32,
    /// Eventos que esperan para guardarse en la base (cola y pendientes con la base caída).
    pub persist_queue: usize,
    /// Cada cuánto se reintenta guardar si la base no responde.
    pub retry: Duration,
}

impl StatusConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            ops: RawOps,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawOps {
            feed_size: usize,
            events_limit: usize,
            group_scan: usize,
            retention_days: u32,
            persist_queue: usize,
            retry_secs: u64,
        }
        let r = toml::from_str::<Raw>(text).map_err(|e| e.to_string())?.ops;
        if !(1..=1000).contains(&r.feed_size) {
            return Err("ops.feed_size tiene que estar entre 1 y 1000".into());
        }
        if !(1..=r.feed_size).contains(&r.events_limit) {
            return Err("ops.events_limit tiene que estar entre 1 y ops.feed_size".into());
        }
        if !(r.feed_size..=100_000).contains(&r.group_scan) {
            return Err("ops.group_scan tiene que estar entre ops.feed_size y 100000".into());
        }
        if !(1..=3650).contains(&r.retention_days) {
            return Err("ops.retention_days tiene que estar entre 1 y 3650".into());
        }
        if !(1..=100_000).contains(&r.persist_queue) {
            return Err("ops.persist_queue tiene que estar entre 1 y 100000".into());
        }
        if !(1..=3600).contains(&r.retry_secs) {
            return Err("ops.retry_secs tiene que estar entre 1 y 3600".into());
        }
        Ok(Self {
            feed_size: r.feed_size,
            events_limit: r.events_limit,
            group_scan: r.group_scan,
            retention_days: r.retention_days,
            persist_queue: r.persist_queue,
            retry: Duration::from_secs(r.retry_secs),
        })
    }
}

/// `ops_event.v1`. Un grupo de eventos iguales: `ts` es la primera vez, `last_seen` la última y
/// `count` cuántas veces (#576). Un evento solo tiene `count = 1` y `last_seen = ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpsEvent {
    pub ts: String,
    pub level: String,
    pub target: String,
    pub event: String,
    pub fields: Map<String, Value>,
    pub count: u64,
    pub last_seen: String,
}

impl OpsEvent {
    /// Un evento solo (todavía sin agrupar).
    pub fn single(
        ts: String,
        level: String,
        target: String,
        event: String,
        fields: Map<String, Value>,
    ) -> Self {
        Self {
            last_seen: ts.clone(),
            ts,
            level,
            target,
            event,
            fields,
            count: 1,
        }
    }

    /// `true` si es el mismo evento: mismo nivel, origen, nombre y campos (la hora no cuenta).
    pub fn same_as(&self, other: &Self) -> bool {
        self.level == other.level
            && self.target == other.target
            && self.event == other.event
            && self.fields == other.fields
    }
}

/// Agrupa eventos que llegan **del más nuevo al más viejo**: cada grupo queda donde apareció su
/// ocurrencia más nueva (`last_seen`), suma los `count` y su `ts` es el de la más vieja. Devuelve
/// como mucho `limit` grupos, el más nuevo primero; los que no entran no se cuentan.
pub fn group_newest_first(
    events: impl IntoIterator<Item = OpsEvent>,
    limit: usize,
) -> Vec<OpsEvent> {
    let mut out: Vec<OpsEvent> = Vec::new();
    for e in events {
        if let Some(g) = out.iter_mut().find(|g| g.same_as(&e)) {
            g.count += e.count;
            g.ts = e.ts;
        } else if out.len() < limit {
            out.push(e);
        }
    }
    out
}

/// Los últimos eventos, en memoria.
#[derive(Debug)]
pub struct Feed {
    cap: usize,
    items: Mutex<VecDeque<OpsEvent>>,
    /// La cola hacia la base, si se guardan (#337).
    sink: OnceLock<Arc<Persistence>>,
}

fn utc_text(s: &str) -> bool {
    s.ends_with('Z') && s.parse::<DateTime<Utc>>().is_ok()
}

impl Feed {
    pub fn new(cfg: StatusConfig) -> Arc<Self> {
        Arc::new(Self {
            cap: cfg.feed_size,
            items: Mutex::new(VecDeque::with_capacity(cfg.feed_size)),
            sink: OnceLock::new(),
        })
    }

    /// Desde ahora, cada evento guardado en memoria también se ofrece a la base (sin esperar).
    pub fn attach(&self, sink: Arc<Persistence>) {
        let _ = self.sink.set(sink);
    }

    /// Cuántos eventos guarda en memoria y devuelve la ruta.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Una línea del `Logger` (ya redactada). Guarda las del broker, las alertas y lo `warn` o
    /// `error`; ignora el resto y cualquier línea que no tenga la forma esperada.
    pub fn push_line(&self, line: &str) {
        let Ok(Value::Object(mut doc)) = serde_json::from_str::<Value>(line) else {
            return;
        };
        let mut take = |k: &str| match doc.remove(k) {
            Some(Value::String(s)) => Some(s),
            _ => None,
        };
        let (Some(ts), Some(level), Some(target), Some(event)) =
            (take("ts"), take("level"), take("target"), take("event"))
        else {
            return;
        };
        let ok_level = matches!(level.as_str(), "info" | "warn" | "error");
        let ok_target = (1..=32).contains(&target.len())
            && target.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
        let ok_event = (1..=64).contains(&event.chars().count());
        if !(ok_level && ok_target && ok_event && utc_text(&ts)) {
            return;
        }
        let wanted = matches!(target.as_str(), "broker" | "alert") || level != "info";
        if !wanted {
            return;
        }
        let e = OpsEvent::single(ts, level, target, event, doc);
        // A la base va cada uno (es de solo agregar); en memoria, agrupados.
        if let Some(sink) = self.sink.get() {
            sink.offer(e.clone());
        }
        let Ok(mut items) = self.items.lock() else {
            return;
        };
        if let Some(i) = items.iter().position(|g| g.same_as(&e)) {
            if let Some(mut g) = items.remove(i) {
                g.count += 1;
                g.last_seen = e.last_seen;
                items.push_back(g);
            }
            return;
        }
        if items.len() == self.cap {
            items.pop_front();
        }
        items.push_back(e);
    }

    /// Los grupos guardados, el más nuevo (por `last_seen`) primero.
    pub fn latest(&self) -> Vec<OpsEvent> {
        self.items
            .lock()
            .map(|items| items.iter().rev().cloned().collect())
            .unwrap_or_default()
    }
}

/// Va detrás del `Logger`: escribe todo en `inner` (stderr) y pasa cada línea completa al feed.
pub struct FeedWriter {
    feed: Arc<Feed>,
    inner: Box<dyn Write + Send>,
    buf: Vec<u8>,
}

impl FeedWriter {
    pub fn new(feed: Arc<Feed>, inner: Box<dyn Write + Send>) -> Self {
        Self {
            feed,
            inner,
            buf: Vec::new(),
        }
    }
}

impl Write for FeedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        // El log sigue saliendo aunque el feed no pueda leer la línea.
        self.inner.write_all(data)?;
        self.buf.extend_from_slice(data);
        while let Some(i) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=i).collect();
            if let Ok(text) = std::str::from_utf8(&line) {
                self.feed.push_line(text.trim_end());
            }
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconcileKind {
    Startup,
    Periodic,
}

/// La última reconciliación, como va en `ops_status.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReconcileStatus {
    pub kind: ReconcileKind,
    #[serde(with = "utc_z")]
    pub at: DateTime<Utc>,
    pub clean: bool,
    pub matched: usize,
    pub mismatches: usize,
    pub drifts: usize,
}

impl ReconcileStatus {
    pub fn new(kind: ReconcileKind, at: DateTime<Utc>, rec: &Reconciliation) -> Self {
        Self {
            kind,
            at,
            clean: rec.is_clean(),
            matched: rec.matched,
            mismatches: rec.mismatches.len(),
            drifts: rec.drifts.len(),
        }
    }
}

/// `account` de `ops_status.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountStatus {
    pub kill_switch: KillSwitch,
    #[serde(with = "decimal_str")]
    pub daily_loss_used_pct: Decimal,
    #[serde(with = "decimal_str")]
    pub daily_loss_limit_pct: Decimal,
    #[serde(with = "decimal_str")]
    pub leverage: Decimal,
    pub open_positions: u32,
    pub max_positions: u32,
}

/// `ops_status.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpsStatus {
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
    pub mode: &'static str,
    pub gate: &'static str,
    pub accepting_orders: bool,
    pub account: Option<AccountStatus>,
    pub last_reconcile: Option<ReconcileStatus>,
}

/// El nombre de la compuerta en el contrato.
pub fn gate_name(g: GateState) -> &'static str {
    match g {
        GateState::Starting => "starting",
        GateState::Ready => "ready",
        GateState::Stopping => "stopping",
    }
}

/// Lo que el estado necesita del broker. En producción, [`PgPaper`].
pub trait OpsSource: Sync {
    fn gate(&self) -> GateState;
    fn account(
        &self,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<PaperAccount, String>> + Send;
    fn last_reconcile(&self) -> Option<ReconcileStatus>;
}

impl OpsSource for PgPaper {
    fn gate(&self) -> GateState {
        PaperService::gate(self)
    }
    async fn account(&self, now: DateTime<Utc>) -> Result<PaperAccount, String> {
        PaperService::account(self, now).await
    }
    fn last_reconcile(&self) -> Option<ReconcileStatus> {
        PgPaper::last_reconcile(self)
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_status_route(path: &str) -> bool {
    matches!(path, STATUS | EVENTS)
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

fn json<T: Serialize>(v: &T) -> Reply {
    match serde_json::to_string(v) {
        Ok(body) => Reply {
            status: 200,
            content_type: "application/json",
            body,
        },
        Err(e) => {
            eprintln!("gateway: no se pudo serializar el estado: {e}");
            text(500, "error interno")
        }
    }
}

/// Lo que necesita la ruta además del broker.
pub struct StatusDeps<'a> {
    pub feed: &'a Feed,
    /// Los eventos guardados (#337). Sin base, o si falla, se leen los de memoria.
    pub events: Option<&'a dyn EventStore>,
    /// `[ops] events_limit`: cuántos grupos devuelve la ruta sin `?limit=` (#576).
    pub events_limit: usize,
    /// `[ops] group_scan`: cuántas filas de la base se agrupan (#576).
    pub group_scan: usize,
    /// `[limits] max_daily_loss_pct` de `config/risk.toml`.
    pub daily_loss_limit_pct: Decimal,
    /// Tapa secretos en un texto (el motivo del kill switch). En producción, `Logger::scrub`.
    pub scrub: &'a (dyn Fn(&str) -> String + Sync),
}

/// Arma `ops_status.v1`. Si el broker no está o la cuenta no responde, `account` es `null`.
pub async fn status<S: OpsSource>(
    paper: Option<&S>,
    deps: &StatusDeps<'_>,
    now: DateTime<Utc>,
) -> OpsStatus {
    let Some(paper) = paper else {
        return OpsStatus {
            as_of: now,
            mode: "paper",
            gate: "unavailable",
            accepting_orders: false,
            account: None,
            last_reconcile: None,
        };
    };
    let gate = paper.gate();
    let account = match paper.account(now).await {
        Ok(a) => Some(AccountStatus {
            kill_switch: match a.kill_switch {
                KillSwitch::Inactive => KillSwitch::Inactive,
                KillSwitch::Active { reason, since } => KillSwitch::Active {
                    reason: (deps.scrub)(&reason),
                    since,
                },
            },
            daily_loss_used_pct: a.daily_loss_used_pct,
            daily_loss_limit_pct: deps.daily_loss_limit_pct,
            leverage: a.leverage,
            open_positions: a.open_positions,
            max_positions: a.max_positions,
        }),
        Err(e) => {
            eprintln!("gateway: estado: {}", (deps.scrub)(&e));
            None
        }
    };
    let kill = account
        .as_ref()
        .is_none_or(|a| matches!(a.kill_switch, KillSwitch::Active { .. }));
    OpsStatus {
        as_of: now,
        mode: "paper",
        gate: gate_name(gate),
        accepting_orders: gate == GateState::Ready && !kill,
        account,
        last_reconcile: paper.last_reconcile(),
    }
}

/// Los últimos `limit` grupos de eventos, el más nuevo primero: de la base (agrupando sus
/// últimas `scan` filas) y, si no hay o no responde, de memoria.
pub async fn events(deps: &StatusDeps<'_>, limit: usize, scan: usize) -> Vec<OpsEvent> {
    let from_memory = || deps.feed.latest().into_iter().take(limit).collect();
    match deps.events {
        Some(store) => match store.latest(u32::try_from(scan).unwrap_or(u32::MAX)).await {
            Ok(rows) => group_newest_first(rows.into_iter().map(from_record), limit),
            Err(e) => {
                eprintln!("gateway: eventos de la base: {}", (deps.scrub)(&e));
                from_memory()
            }
        },
        None => from_memory(),
    }
}

/// `?limit=` de `GET /api/v1/ops/events`: sin él, `default`; con él, entre 1 y `max`.
pub fn events_limit(query: Option<&str>, default: usize, max: usize) -> Result<usize, String> {
    match crate::param(query, "limit") {
        None => Ok(default),
        Some(v) => v
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=max).contains(n))
            .ok_or_else(|| format!("limit tiene que ser un entero entre 1 y {max}")),
    }
}

/// Atiende las rutas del estado. Solo `GET`.
pub async fn respond_status<S: OpsSource>(
    paper: Option<&S>,
    deps: &StatusDeps<'_>,
    method: &str,
    path: &str,
    query: Option<&str>,
    now: DateTime<Utc>,
) -> Reply {
    if !is_status_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: el estado es de solo lectura");
    }
    if path == EVENTS {
        return match events_limit(query, deps.events_limit, deps.feed.cap()) {
            Ok(limit) => json(&events(deps, limit, deps.group_scan).await),
            Err(e) => text(400, &e),
        };
    }
    json(&status(paper, deps, now).await)
}

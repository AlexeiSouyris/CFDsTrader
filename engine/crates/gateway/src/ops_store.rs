//! Guardado de los eventos de la operación en PostgreSQL (#337, sigue a #314).
//!
//! ```text
//! Logger ─▶ FeedWriter ─▶ Feed (memoria, últimos feed_size)
//!                            └─ try_send (nunca espera) ─▶ cola [ops] persist_queue
//!                                                            └─ tarea en segundo plano ─▶ ops_events
//! ```
//! - Los eventos ya vienen **redactados** por el `Logger`: a la base llega lo mismo que a la
//!   memoria, sin secretos.
//! - **Nunca frena al broker:** el `FeedWriter` solo hace `try_send`. Si la cola está llena, el
//!   evento queda en memoria y no se guarda (se cuenta en [`Persistence::dropped`]).
//! - **Base caída:** la tarea guarda lo pendiente (hasta `persist_queue` eventos; si no entran, se
//!   descartan los más viejos), avisa con `on_error` (en producción, la alerta de desconexión que
//!   ya existe) y reintenta cada `[ops] retry_secs`. El feed en memoria sigue igual.
//! - La retención (`[ops] retention_days`) es un job aparte: `examples/ops_events_retention.rs`.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cfds_storage::{OpsEventRecord, PgStore};
use chrono::{DateTime, SecondsFormat, Utc};
use tokio::sync::mpsc;

use crate::status::OpsEvent;

/// Cuántos eventos se guardan por transacción, como mucho.
const BATCH: usize = 100;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Dónde se guardan y de dónde se leen los eventos. En producción, PostgreSQL.
pub trait EventStore: Send + Sync {
    fn insert<'a>(&'a self, events: &'a [OpsEventRecord]) -> BoxFuture<'a, Result<usize, String>>;
    fn latest(&self, limit: u32) -> BoxFuture<'_, Result<Vec<OpsEventRecord>, String>>;
}

impl EventStore for PgStore {
    fn insert<'a>(&'a self, events: &'a [OpsEventRecord]) -> BoxFuture<'a, Result<usize, String>> {
        Box::pin(async move {
            self.insert_ops_events(events)
                .await
                .map_err(|e| e.to_string())
        })
    }
    fn latest(&self, limit: u32) -> BoxFuture<'_, Result<Vec<OpsEventRecord>, String>> {
        Box::pin(async move {
            self.latest_ops_events(limit)
                .await
                .map_err(|e| e.to_string())
        })
    }
}

/// Un evento del feed como fila de la base. `None` si la hora no se lee (no debería pasar: el
/// feed ya la validó).
pub fn to_record(e: &OpsEvent) -> Option<OpsEventRecord> {
    Some(OpsEventRecord {
        ts: e.ts.parse::<DateTime<Utc>>().ok()?,
        level: e.level.clone(),
        target: e.target.clone(),
        event: e.event.clone(),
        fields: e.fields.clone(),
    })
}

/// Una fila de la base como `ops_event.v1`.
pub fn from_record(r: OpsEventRecord) -> OpsEvent {
    OpsEvent {
        ts: r.ts.to_rfc3339_opts(SecondsFormat::Secs, true),
        level: r.level,
        target: r.target,
        event: r.event,
        fields: r.fields,
    }
}

/// El lado del feed: la cola hacia la tarea de guardado.
#[derive(Debug)]
pub struct Persistence {
    tx: mpsc::Sender<OpsEvent>,
    dropped: AtomicU64,
}

impl Persistence {
    /// Encola sin esperar nunca. Con la cola llena (o la tarea terminada), cuenta el evento como
    /// no guardado y sigue.
    pub fn offer(&self, e: OpsEvent) {
        if self.tx.try_send(e).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Eventos que no entraron en la cola y no se guardaron (siguen en memoria).
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// La cola y su receptor, que se pasa a [`run_persister`].
pub fn persistence(queue: usize) -> (Arc<Persistence>, mpsc::Receiver<OpsEvent>) {
    let (tx, rx) = mpsc::channel(queue.max(1));
    (
        Arc::new(Persistence {
            tx,
            dropped: AtomicU64::new(0),
        }),
        rx,
    )
}

/// La tarea en segundo plano: guarda en lotes, en orden. Si la base falla, conserva lo pendiente
/// (hasta `queue` eventos), llama a `on_error` y reintenta después de `retry`. Termina cuando se
/// cierra la cola y no queda nada pendiente, o cuando se cierra la cola con la base caída.
pub async fn run_persister<S, F, Fut>(
    mut rx: mpsc::Receiver<OpsEvent>,
    store: Arc<S>,
    queue: usize,
    retry: Duration,
    on_error: F,
) where
    S: EventStore + ?Sized,
    F: Fn(String) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut pending: VecDeque<OpsEventRecord> = VecDeque::new();
    let mut open = true;
    loop {
        if pending.is_empty() {
            if !open {
                return;
            }
            match rx.recv().await {
                Some(e) => pending.extend(to_record(&e)),
                None => return,
            }
        }
        // Lo que ya está en la cola, sin esperar.
        while pending.len() < BATCH.max(queue) {
            match rx.try_recv() {
                Ok(e) => pending.extend(to_record(&e)),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    open = false;
                    break;
                }
            }
        }
        while pending.len() > queue {
            pending.pop_front();
        }
        let n = pending.len().min(BATCH);
        let batch: Vec<OpsEventRecord> = pending.iter().take(n).cloned().collect();
        match store.insert(&batch).await {
            Ok(_) => {
                pending.drain(..n);
            }
            Err(e) => {
                on_error(format!(
                    "no se pudieron guardar los eventos de la operación: {e}"
                ))
                .await;
                if !open {
                    return;
                }
                tokio::time::sleep(retry).await;
            }
        }
    }
}

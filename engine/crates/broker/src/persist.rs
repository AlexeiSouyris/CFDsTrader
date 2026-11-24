//! El broker simulado con su registro en PostgreSQL (fase 7, #278).
//!
//! ```text
//! enviar:     submit → guardar orden + informe (una transacción; idempotente por order_id)
//!             pendiente de su vela del fill (#471) → guardarla como pendiente (#479)
//! decidir:    llegó la vela del fill → orden + informe al registro y la pendiente `settled`
//!             (una transacción)
//! arrancar:   leer el registro → restore_and_reconcile (kill switch si algo no coincide)
//!             → retomar las pendientes guardadas sin final (restore_pending_from_store)
//! cada N min: leer el registro → reconstruir aparte → comparar con el broker en marcha
//!             (informes y posiciones) → kill switch si algo no coincide
//! ```
//! N sale de `[reconcile] every_minutes` de `config/broker.toml`.
//!
//! Las dos reconstrucciones aplican los cambios guardados del kill switch (#441,
//! [`kill_switch_marks`]) en su punto del registro, y arrancan desde el último punto de control
//! que verifica (#463, [`crate::checkpoint`]); después de cada reconciliación periódica limpia se
//! guarda uno nuevo si ya hay algo para sellar ([`save_checkpoint`]).

use std::collections::BTreeMap;

use cfds_core::{InstrumentId, Order, OrderReport, OrderStatus, Position};
use cfds_storage::{CheckpointRow, PgStore, StorageError};
use chrono::{DateTime, Utc};

use crate::Broker;
use crate::checkpoint::{BrokerCheckpoint, SealedCheckpoint, seal, verified_chain};
use crate::paper::{BrokerConfig, MarketSetup, PaperBroker, RestorePending};
use crate::reconcile::{
    KillSwitchChange, KillSwitchMark, Mismatch, Reconciliation, restore_from_checkpoint,
    restore_with_kill_switch,
};

/// Los puntos de control guardados cuya cadena de hashes verifica (#463), en orden, con su hash.
pub async fn stored_checkpoints(
    store: &PgStore,
) -> Result<Vec<(BrokerCheckpoint, String)>, StorageError> {
    let rows: Vec<SealedCheckpoint> = store
        .paper_checkpoints()
        .await?
        .into_iter()
        .map(|r| SealedCheckpoint {
            after_orders: usize::try_from(r.after_orders).unwrap_or(usize::MAX),
            state: r.state,
            prev_hash: r.prev_hash,
            hash: r.hash,
        })
        .collect();
    Ok(verified_chain(&rows))
}

/// Reconstruye desde el último punto de control que verifica con este registro, esta config y
/// estas velas; si ninguno, desde la primera orden (#463). Devuelve también desde cuántas
/// órdenes arrancó (`None`: desde la primera).
pub fn restore_best(
    cfg: BrokerConfig,
    markets: BTreeMap<InstrumentId, MarketSetup>,
    checkpoints: &[(BrokerCheckpoint, String)],
    journal: &[(Order, OrderReport)],
    marks: &[KillSwitchMark],
    broker_positions: &[Position],
    now: DateTime<Utc>,
) -> (PaperBroker, Reconciliation, Option<usize>) {
    for (ck, _) in checkpoints.iter().rev() {
        if let Ok((paper, rec)) = restore_from_checkpoint(
            cfg.clone(),
            markets.clone(),
            ck,
            journal,
            marks,
            broker_positions,
            now,
        ) {
            return (paper, rec, Some(ck.after_orders));
        }
    }
    let (paper, rec) =
        restore_with_kill_switch(cfg, markets, journal, marks, broker_positions, now);
    (paper, rec, None)
}

/// Guarda el punto de control `ck` (planeado sobre el broker reconstruido desde lo guardado,
/// después de una reconciliación limpia: [`PaperBroker::plan_checkpoint`]) encadenado al último
/// (`chain`), y corta también el broker en marcha en ese punto. Si no coincide, el broker en
/// marcha sigue entero: la próxima reconstrucción usa sus velas completas, igual de correcta.
pub async fn save_checkpoint(
    store: &PgStore,
    running: &mut PaperBroker,
    ck: &BrokerCheckpoint,
    chain: &[(BrokerCheckpoint, String)],
) -> Result<(), StorageError> {
    let sealed = seal(ck, chain.last().map(|(_, h)| h.as_str()))
        .map_err(|e| StorageError::CorruptRow(format!("punto de control: {e}")))?;
    store
        .insert_paper_checkpoint(&CheckpointRow {
            after_orders: u64::try_from(sealed.after_orders).unwrap_or(u64::MAX),
            state: sealed.state,
            prev_hash: sealed.prev_hash,
            hash: sealed.hash,
        })
        .await?;
    // Si no coincide, el broker en marcha queda entero (más lento, pero igual de correcto).
    let _ = running.apply_checkpoint(ck);
    Ok(())
}

/// Envía la orden y guarda la orden con su informe. Si el guardado falla, el informe no se
/// pierde: se devuelve el error y la orden se puede reenviar (el `order_id` es idempotente).
pub async fn submit_and_record(
    broker: &mut PaperBroker,
    store: &PgStore,
    order: &Order,
) -> Result<OrderReport, StorageError> {
    let was_pending = broker
        .pending()
        .iter()
        .any(|o| o.order_id == order.order_id);
    let report = broker.submit(order);
    if report.status != OrderStatus::Pending {
        store.insert_paper_order(order, &report).await?;
    } else if !was_pending {
        // Una pendiente (#471) no está en el registro: se guarda como pendiente (#479) para que
        // sobreviva a un reinicio, y entra al registro cuando se decide. Si no se pudo guardar,
        // no queda pendiente en memoria (un reinicio la perdería): error, y se puede reenviar.
        if let Err(e) = store.insert_paper_pending(order).await {
            broker.forget_pending(&order.order_id);
            return Err(e);
        }
    }
    Ok(report)
}

/// Lo que pasó con las órdenes pendientes guardadas al retomarlas ([`restore_pending_from_store`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RestoredPending {
    /// Vuelven a estar pendientes, en orden de llegada.
    pub restored: Vec<String>,
    /// No se pudieron retomar (su instrumento no se opera en este arranque): quedan guardadas
    /// como `dropped` con el motivo y no entran al registro.
    pub dropped: Vec<String>,
    /// Ya estaban en el registro o ya pendientes: no se tocan.
    pub skipped: Vec<String>,
}

/// Arranque (#479): retoma las órdenes pendientes guardadas sin final, en orden de llegada,
/// sobre el broker ya reconstruido desde el registro (con su kill switch). No se evalúan otra
/// vez: se deciden con [`settle_and_record`] cuando llega su vela del fill, sobre el estado de
/// ese momento, igual que en la sesión que las recibió y que en la reconstrucción. Las que no se
/// pueden retomar se guardan como descartadas (`at` = la hora del arranque).
pub async fn restore_pending_from_store(
    broker: &mut PaperBroker,
    store: &PgStore,
    at: DateTime<Utc>,
) -> Result<RestoredPending, StorageError> {
    let mut out = RestoredPending::default();
    for order in store.paper_pending().await? {
        let id = order.order_id.to_string();
        let instrument = order.instrument_id.to_string();
        match broker.restore_pending(order) {
            Ok(()) => out.restored.push(id),
            Err(RestorePending::UnknownInstrument) => {
                store
                    .drop_paper_pending(
                        &id,
                        &format!("el instrumento {instrument} no se opera en este arranque"),
                        at,
                    )
                    .await?;
                out.dropped.push(id);
            }
            Err(RestorePending::AlreadyDecided | RestorePending::AlreadyPending) => {
                out.skipped.push(id);
            }
        }
    }
    Ok(out)
}

/// Decide las órdenes pendientes cuya vela del fill ya llegó (`PaperBroker::settle_pending`,
/// #471) y las guarda en el registro, en orden. Si un guardado falla, sigue con las demás y
/// devuelve el primer error: lo que no se guardó queda en el registro del broker (la parada
/// segura lo vuelve a intentar y la reconciliación periódica lo marca).
pub async fn settle_and_record(
    broker: &mut PaperBroker,
    store: &PgStore,
) -> Result<Vec<OrderReport>, StorageError> {
    let mut first_err = None;
    let mut out = Vec::new();
    for (order, report) in broker.settle_pending() {
        if let Err(e) = store.insert_paper_order(&order, &report).await {
            first_err.get_or_insert(e);
        }
        out.push(report);
    }
    first_err.map_or(Ok(out), Err)
}

/// Los cambios guardados del kill switch (#441), en orden.
pub async fn kill_switch_marks(store: &PgStore) -> Result<Vec<KillSwitchMark>, StorageError> {
    Ok(store
        .kill_switch_changes()
        .await?
        .into_iter()
        .map(|r| KillSwitchMark {
            after_orders: usize::try_from(r.after_orders).unwrap_or(usize::MAX),
            change: if r.activated {
                KillSwitchChange::Activated {
                    reason: r.reason,
                    at: r.at,
                }
            } else {
                KillSwitchChange::Released
            },
        })
        .collect())
}

/// `true` si el último cambio guardado es una activación: el kill switch sigue activo.
pub fn kill_switch_stored_active(marks: &[KillSwitchMark]) -> bool {
    matches!(
        marks.last(),
        Some(KillSwitchMark {
            change: KillSwitchChange::Activated { .. },
            ..
        })
    )
}

/// Guarda la activación del kill switch del broker si está activo y todavía no estaba guardado
/// (`saved` = el último cambio guardado es una activación). Devuelve si quedó guardado.
pub async fn save_kill_switch(
    broker: &mut PaperBroker,
    store: &PgStore,
    saved: bool,
) -> Result<bool, StorageError> {
    let Some((reason, at)) = broker.kill_switch().map(|(r, at)| (r.to_owned(), at)) else {
        return Ok(saved);
    };
    if saved {
        return Ok(true);
    }
    let after = u64::try_from(broker.journal().len()).unwrap_or(u64::MAX);
    store
        .insert_kill_switch_activation(after, &reason, at)
        .await?;
    Ok(true)
}

/// Arranque: reconstruye el broker desde el registro guardado (con los cambios guardados del
/// kill switch) y lo reconcilia con las posiciones del broker (en paper, las que se guardaron o
/// informó el simulador anterior).
pub async fn restore_from_store(
    store: &PgStore,
    cfg: BrokerConfig,
    markets: BTreeMap<InstrumentId, MarketSetup>,
    broker_positions: &[Position],
    now: DateTime<Utc>,
) -> Result<(PaperBroker, Reconciliation), StorageError> {
    let journal = store.paper_journal().await?;
    let marks = kill_switch_marks(store).await?;
    let checkpoints = stored_checkpoints(store).await?;
    let (paper, rec, _) = restore_best(
        cfg,
        markets,
        &checkpoints,
        &journal,
        &marks,
        broker_positions,
        now,
    );
    Ok((paper, rec))
}

/// Cuándo toca la próxima reconciliación periódica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReconcileSchedule {
    last: Option<DateTime<Utc>>,
}

impl ReconcileSchedule {
    /// `true` si nunca se reconcilió o ya pasaron `every` desde la última vez.
    pub fn due(&self, now: DateTime<Utc>, every: chrono::TimeDelta) -> bool {
        self.last.is_none_or(|last| now - last >= every)
    }
}

/// Reconciliación periódica: si toca (`[reconcile] every_minutes`), reconstruye aparte el broker
/// desde el registro guardado y lo compara con el que está en marcha: el registro tiene que ser
/// el mismo (órdenes e informes) y las posiciones también. Si algo no coincide, activa el kill
/// switch del broker en marcha. `Ok(None)` si todavía no tocaba.
pub async fn reconcile_periodically(
    store: &PgStore,
    running: &mut PaperBroker,
    markets: BTreeMap<InstrumentId, MarketSetup>,
    schedule: &mut ReconcileSchedule,
    now: DateTime<Utc>,
) -> Result<Option<Reconciliation>, StorageError> {
    let cfg = running.config().clone();
    if !schedule.due(now, cfg.reconcile_every) {
        return Ok(None);
    }
    let stored = store.paper_journal().await?;
    let marks = kill_switch_marks(store).await?;
    let checkpoints = stored_checkpoints(store).await?;
    let running_positions = running.positions(now);
    let (rebuilt, mut rec, _) = restore_best(
        cfg,
        markets,
        &checkpoints,
        &stored,
        &marks,
        &running_positions,
        now,
    );
    // Lo que el broker en marcha procesó y no está guardado, o al revés, también es diferencia.
    for (o, _) in running.journal() {
        if !stored.iter().any(|(s, _)| s.order_id == o.order_id) {
            rec.mismatches
                .push(Mismatch::UnsavedOrder(o.order_id.clone()));
        }
    }
    for (s, _) in &stored {
        if !running
            .journal()
            .iter()
            .any(|(o, _)| o.order_id == s.order_id)
        {
            rec.mismatches
                .push(Mismatch::UnknownStoredOrder(s.order_id.clone()));
        }
    }
    schedule.last = Some(now);
    // El punto de control sale de la reconstrucción desde lo guardado, no del broker en marcha.
    let plan = rebuilt.plan_checkpoint(now, marks.len());
    drop(rebuilt);
    if rec.is_clean() {
        if let Some(ck) = plan {
            save_checkpoint(store, running, &ck, &checkpoints).await?;
        }
    } else {
        running.activate_kill_switch(rec.reason(), now);
    }
    Ok(Some(rec))
}

/// Lo que la parada segura necesita del registro guardado. En producción, [`PgStore`]; en los
/// tests, uno en memoria.
pub trait JournalStore: Sync {
    /// Guarda la orden y su informe (idempotente por `order_id`).
    fn save(
        &self,
        order: &Order,
        report: &OrderReport,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send;
    /// Los `order_id` que ya están guardados.
    fn saved_ids(&self) -> impl std::future::Future<Output = Result<Vec<String>, String>> + Send;
    /// Los `order_id` de las órdenes pendientes guardadas sin final (#479).
    fn saved_pending_ids(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<String>, String>> + Send;
}

impl JournalStore for PgStore {
    async fn save(&self, order: &Order, report: &OrderReport) -> Result<(), String> {
        self.insert_paper_order(order, report)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn saved_ids(&self) -> Result<Vec<String>, String> {
        self.paper_journal()
            .await
            .map(|j| j.into_iter().map(|(o, _)| o.order_id.to_string()).collect())
            .map_err(|e| e.to_string())
    }

    async fn saved_pending_ids(&self) -> Result<Vec<String>, String> {
        self.paper_pending()
            .await
            .map(|p| p.into_iter().map(|o| o.order_id.to_string()).collect())
            .map_err(|e| e.to_string())
    }
}

/// Resultado de [`flush_journal`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Flush {
    /// Órdenes que el broker procesó y no estaban guardadas (se guardaron ahora).
    pub saved_now: Vec<String>,
    /// Órdenes que siguen sin guardar después del intento (huérfanas). Vacío = parada limpia.
    pub orphans: Vec<String>,
    /// Órdenes pendientes de su vela del fill (#471) que **no** están guardadas como
    /// pendientes (#479): se pierden al apagar. Las guardadas se retoman al arrancar.
    pub pending_dropped: Vec<String>,
    /// Órdenes pendientes guardadas (#479): sobreviven al apagado y se retoman al arrancar.
    pub pending_kept: Vec<String>,
}

impl Flush {
    pub fn is_clean(&self) -> bool {
        self.orphans.is_empty()
    }
}

/// Parada segura (fase 9, #297): guarda toda orden que el broker procesó y no llegó al registro
/// (por ejemplo, si un guardado falló) y verifica contra lo guardado que no quede ninguna suelta.
pub async fn flush_journal<S: JournalStore>(
    broker: &PaperBroker,
    store: &S,
) -> Result<Flush, String> {
    let saved = store.saved_ids().await?;
    let saved_pending = store.saved_pending_ids().await?;
    let mut flush = Flush::default();
    for o in broker.pending() {
        let id = o.order_id.to_string();
        if saved_pending.contains(&id) {
            flush.pending_kept.push(id);
        } else {
            flush.pending_dropped.push(id);
        }
    }
    for (o, r) in broker.journal() {
        let id = o.order_id.to_string();
        if !saved.contains(&id) && store.save(o, r).await.is_ok() {
            flush.saved_now.push(id);
        }
    }
    // Verificación final contra lo guardado de verdad.
    let saved = store.saved_ids().await?;
    for (o, _) in broker.journal() {
        let id = o.order_id.to_string();
        if !saved.contains(&id) {
            flush.saved_now.retain(|s| s != &id);
            flush.orphans.push(id);
        }
    }
    Ok(flush)
}

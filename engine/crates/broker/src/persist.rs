//! El broker simulado con su registro en PostgreSQL (fase 7, #278).
//!
//! ```text
//! enviar:     submit → guardar orden + informe (una transacción; idempotente por order_id)
//! arrancar:   leer el registro → restore_and_reconcile (kill switch si algo no coincide)
//! cada N min: leer el registro → reconstruir aparte → comparar con el broker en marcha
//!             (informes y posiciones) → kill switch si algo no coincide
//! ```
//! N sale de `[reconcile] every_minutes` de `config/broker.toml`.

use std::collections::BTreeMap;

use cfds_core::{InstrumentId, Order, OrderReport, Position};
use cfds_storage::{PgStore, StorageError};
use chrono::{DateTime, Utc};

use crate::Broker;
use crate::paper::{BrokerConfig, MarketSetup, PaperBroker};
use crate::reconcile::{Mismatch, Reconciliation, restore_and_reconcile};

/// Envía la orden y guarda la orden con su informe. Si el guardado falla, el informe no se
/// pierde: se devuelve el error y la orden se puede reenviar (el `order_id` es idempotente).
pub async fn submit_and_record(
    broker: &mut PaperBroker,
    store: &PgStore,
    order: &Order,
) -> Result<OrderReport, StorageError> {
    let report = broker.submit(order);
    store.insert_paper_order(order, &report).await?;
    Ok(report)
}

/// Arranque: reconstruye el broker desde el registro guardado y lo reconcilia con las
/// posiciones del broker (en paper, las que se guardaron o informó el simulador anterior).
pub async fn restore_from_store(
    store: &PgStore,
    cfg: BrokerConfig,
    markets: BTreeMap<InstrumentId, MarketSetup>,
    broker_positions: &[Position],
    now: DateTime<Utc>,
) -> Result<(PaperBroker, Reconciliation), StorageError> {
    let journal = store.paper_journal().await?;
    Ok(restore_and_reconcile(
        cfg,
        markets,
        &journal,
        broker_positions,
        now,
    ))
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
    let running_positions = running.positions(now);
    let (_, mut rec) = restore_and_reconcile(cfg, markets, &stored, &running_positions, now);
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
    if !rec.is_clean() {
        running.activate_kill_switch(rec.reason(), now);
    }
    Ok(Some(rec))
}

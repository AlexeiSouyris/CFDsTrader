//! Broker (fase 7, #278): una interfaz con dos implementaciones.
//!
//! - [`PaperBroker`]: el broker **simulado** contra las velas guardadas. Es el único que existe.
//! - [`LiveBroker`]: el real, **imposible de construir**. El dinero real está bloqueado por
//!   CLAUDE.md §5 y la fase 11 (ADR aprobado por dos personas). Ver [`live`].
//!
//! Toda orden pasa por el módulo de riesgo (`cfds_risk::check`): no hay otro camino para obtener
//! un tamaño. Los costos (spread, slippage, comisión) y las salidas (stop, TP1, trailing, tiempo)
//! son los de `cfds-backtest`: no se duplican.

pub mod checkpoint;
pub mod live;
pub mod paper;
pub mod persist;
pub mod reconcile;

use cfds_core::{Order, OrderReport, PaperAccount, Position, TicketVerdict};
use chrono::{DateTime, Utc};

pub use checkpoint::{
    BrokerCheckpoint, CHECKPOINT_VERSION, MarketCut, SealedCheckpoint, journal_hash, seal,
    setup_hash, verified_chain,
};
pub use live::{LiveAuthorization, LiveBroker, live_trading_status};
pub use paper::{
    BrokerConfig, BrokerEvent, FundingSetup, MarketSetup, PaperBroker, PaperExit, PaperTrade,
    rejection_code,
};
pub use persist::{
    Flush, JournalStore, ReconcileSchedule, flush_journal, kill_switch_marks,
    kill_switch_stored_active, reconcile_periodically, restore_best, restore_from_store,
    save_checkpoint, save_kill_switch, settle_and_record, stored_checkpoints, submit_and_record,
};
pub use reconcile::{
    Drift, KillSwitchChange, KillSwitchMark, Mismatch, Reconciliation, reconcile,
    restore_and_reconcile, restore_from_checkpoint, restore_with_kill_switch,
};

/// Lo que cualquier broker hace. Las órdenes son idempotentes por `order_id`.
pub trait Broker {
    /// La respuesta del riesgo a `order` al cierre de `now`, **sin enviarla**.
    fn preview(&self, order: &Order, now: DateTime<Utc>) -> TicketVerdict;
    /// Envía la orden. Mandar dos veces el mismo `order_id` devuelve el informe de la primera
    /// y nunca abre dos posiciones.
    fn submit(&mut self, order: &Order) -> OrderReport;
    /// Posiciones abiertas, valuadas al cierre de la última vela cerrada en `now`.
    fn positions(&mut self, now: DateTime<Utc>) -> Vec<Position>;
    /// Estado de la cuenta en `now`.
    fn account(&mut self, now: DateTime<Utc>) -> PaperAccount;
    /// Activa el kill switch (no se desactiva desde el broker: spec §1.2).
    fn activate_kill_switch(&mut self, reason: &str, at: DateTime<Utc>);
}

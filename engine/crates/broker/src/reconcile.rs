//! Reconciliación de posiciones al arrancar (fase 7, #278).
//!
//! ```text
//! al arrancar:
//!   1. se reconstruye el broker simulado reenviando el registro de órdenes guardado, en orden
//!      (es determinista: mismas velas y mismas órdenes → mismos informes);
//!   2. cada informe nuevo tiene que ser igual al guardado (si no, las velas o la config
//!      cambiaron por debajo: es repintado) → `Drift`;
//!   3. las posiciones del libro propio se comparan con las que informa el broker
//!      (en paper, el mismo simulador; con un broker real, su API) → `Mismatch`;
//!   4. cualquier diferencia: **kill switch** con el motivo, y no se opera hasta revisar.
//! ```
//!
//! El kill switch también se guarda (#441): cada activación queda en el registro con cuántas
//! órdenes había (`KillSwitchMark`), y al reconstruir se vuelve a activar en ese mismo punto.
//! Así sobrevive a un reinicio, y las órdenes que rechazó por estar activo se reconstruyen
//! iguales. Solo una liberación guardada (votación de la spec §1.2) lo apaga.

use std::collections::BTreeMap;

use cfds_core::{ClientId, InstrumentId, Order, OrderReport, Position};
use chrono::{DateTime, Utc};

use crate::Broker;
use crate::paper::{BrokerConfig, MarketSetup, PaperBroker};

/// Una diferencia entre el libro propio y el broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// El libro propio la tiene y el broker no.
    MissingInBroker(ClientId),
    /// El broker la tiene y el libro propio no.
    UnknownInBroker(ClientId),
    /// Las dos la tienen, pero no coinciden instrumento, dirección, tamaño o stop.
    Different(ClientId),
    /// El broker en marcha procesó esta orden y no está en el registro guardado.
    UnsavedOrder(ClientId),
    /// El registro guardado tiene esta orden y el broker en marcha no.
    UnknownStoredOrder(ClientId),
}

/// Un informe reconstruido que no coincide con el guardado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drift {
    pub order_id: ClientId,
    pub stored: OrderReport,
    pub replayed: OrderReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reconciliation {
    pub matched: usize,
    pub mismatches: Vec<Mismatch>,
    pub drifts: Vec<Drift>,
}

impl Reconciliation {
    pub fn is_clean(&self) -> bool {
        self.mismatches.is_empty() && self.drifts.is_empty()
    }

    /// Motivo legible para el kill switch.
    pub fn reason(&self) -> String {
        format!(
            "Reconciliación al arrancar: {} posiciones distintas y {} órdenes que no se \
             reconstruyen igual. Revisar antes de operar.",
            self.mismatches.len(),
            self.drifts.len()
        )
    }
}

type Key<'a> = (
    &'a InstrumentId,
    cfds_core::OrderSide,
    rust_decimal::Decimal,
    rust_decimal::Decimal,
);

fn key(p: &Position) -> Key<'_> {
    (
        &p.instrument_id,
        p.side,
        p.size.normalize(),
        p.stop.normalize(),
    )
}

/// Compara las posiciones del libro propio con las del broker, por `position_id`.
pub fn reconcile(own: &[Position], broker: &[Position]) -> Reconciliation {
    let theirs: BTreeMap<&ClientId, &Position> =
        broker.iter().map(|p| (&p.position_id, p)).collect();
    let mut out = Reconciliation::default();
    for p in own {
        match theirs.get(&p.position_id) {
            None => out
                .mismatches
                .push(Mismatch::MissingInBroker(p.position_id.clone())),
            Some(b) if key(b) != key(p) => out
                .mismatches
                .push(Mismatch::Different(p.position_id.clone())),
            Some(_) => out.matched += 1,
        }
    }
    let mine: BTreeMap<&ClientId, &Position> = own.iter().map(|p| (&p.position_id, p)).collect();
    for b in broker {
        if !mine.contains_key(&b.position_id) {
            out.mismatches
                .push(Mismatch::UnknownInBroker(b.position_id.clone()));
        }
    }
    out
}

/// Un cambio guardado del kill switch (#441), en orden: después de `after_orders` órdenes del
/// registro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillSwitchMark {
    /// Cuántas órdenes del registro había cuando pasó (se aplica antes de la orden siguiente).
    pub after_orders: usize,
    pub change: KillSwitchChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KillSwitchChange {
    /// Se activó, con este motivo y en este momento.
    Activated { reason: String, at: DateTime<Utc> },
    /// Se liberó después de la votación de la spec §1.2. El gateway nunca la escribe.
    Released,
}

fn apply(paper: &mut PaperBroker, mark: &KillSwitchMark) {
    match &mark.change {
        KillSwitchChange::Activated { reason, at } => paper.activate_kill_switch(reason, *at),
        KillSwitchChange::Released => paper.replay_kill_switch_release(),
    }
}

/// Reconstruye el broker simulado desde el registro guardado (`PaperBroker::journal`) y lo
/// reconcilia con `broker_positions` en `now`. Si algo no coincide, el broker reconstruido queda
/// con el **kill switch activo**.
pub fn restore_and_reconcile(
    cfg: BrokerConfig,
    markets: BTreeMap<InstrumentId, MarketSetup>,
    journal: &[(Order, OrderReport)],
    broker_positions: &[Position],
    now: DateTime<Utc>,
) -> (PaperBroker, Reconciliation) {
    restore_with_kill_switch(cfg, markets, journal, &[], broker_positions, now)
}

/// Como [`restore_and_reconcile`], pero aplicando además los cambios guardados del kill switch
/// (`marks`, en orden) en el punto del registro donde pasaron (#441). Si el último es una
/// activación, el broker reconstruido queda con el kill switch activo.
pub fn restore_with_kill_switch(
    cfg: BrokerConfig,
    markets: BTreeMap<InstrumentId, MarketSetup>,
    journal: &[(Order, OrderReport)],
    marks: &[KillSwitchMark],
    broker_positions: &[Position],
    now: DateTime<Utc>,
) -> (PaperBroker, Reconciliation) {
    let mut paper = PaperBroker::new(cfg, markets);
    let mut drifts = Vec::new();
    let mut pending = marks.iter().peekable();
    for (i, (order, stored)) in journal.iter().enumerate() {
        while let Some(m) = pending.next_if(|m| m.after_orders <= i) {
            apply(&mut paper, m);
        }
        let replayed = paper.submit(order);
        if &replayed != stored {
            drifts.push(Drift {
                order_id: order.order_id.clone(),
                stored: stored.clone(),
                replayed,
            });
        }
    }
    for m in pending {
        apply(&mut paper, m);
    }
    let own = paper.positions(now);
    let mut rec = reconcile(&own, broker_positions);
    rec.drifts = drifts;
    if !rec.is_clean() {
        paper.activate_kill_switch(rec.reason(), now);
    }
    (paper, rec)
}

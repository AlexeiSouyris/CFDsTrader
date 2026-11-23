//! Broker real: **no se puede activar** (CLAUDE.md §5, fase 11).
//!
//! Pasar a dinero real requiere un ADR aprobado por **dos personas** (CLAUDE.md, "Excepciones que
//! no cambian") y, mientras el dueño construya solo, queda bloqueado. Por eso:
//!
//! - [`LiveAuthorization`] no tiene constructor público: fuera de este módulo no se puede crear.
//! - Este módulo tampoco la crea en ningún lado.
//! - [`LiveBroker`] contiene un tipo vacío: no existe ningún valor de `LiveBroker`, y su
//!   implementación de [`Broker`] no tiene código que pueda correr.
//! - [`cfds_core::Mode`] solo tiene `Paper`.
//!
//! Cambiar esto es cambiar este archivo, y eso pasa por un PR con el ADR de la fase 11.
//!
//! ```compile_fail
//! // Desde afuera no se puede construir una autorización: el campo es privado.
//! let auth = cfds_broker::LiveAuthorization { _approved_by_two_people_adr: () };
//! ```

use cfds_core::{Order, OrderReport, PaperAccount, Position, TicketVerdict};
use chrono::{DateTime, Utc};

use crate::Broker;

/// Prueba de que existe un ADR aprobado por dos personas para operar con dinero real. **No hay
/// forma de crearla.**
#[derive(Debug)]
pub struct LiveAuthorization {
    _approved_by_two_people_adr: (),
}

/// Un tipo sin valores: nadie puede tener uno.
#[derive(Debug)]
enum Never {}

/// El broker real. No existe ningún valor de este tipo.
#[derive(Debug)]
pub struct LiveBroker {
    never: Never,
}

impl LiveBroker {
    /// Conectar al broker real exige una [`LiveAuthorization`], que no se puede crear. Aun si
    /// existiera, hoy responde siempre que está bloqueado.
    pub fn connect(_auth: LiveAuthorization) -> Result<LiveBroker, &'static str> {
        Err(live_trading_status())
    }
}

/// Por qué no hay órdenes reales.
pub const fn live_trading_status() -> &'static str {
    "dinero real bloqueado: requiere ADR aprobado por dos personas (CLAUDE.md §5, fase 11)"
}

impl Broker for LiveBroker {
    fn preview(&self, _order: &Order, _now: DateTime<Utc>) -> TicketVerdict {
        match self.never {}
    }
    fn submit(&mut self, _order: &Order) -> OrderReport {
        match self.never {}
    }
    fn positions(&mut self, _now: DateTime<Utc>) -> Vec<Position> {
        match self.never {}
    }
    fn account(&mut self, _now: DateTime<Utc>) -> PaperAccount {
        match self.never {}
    }
}

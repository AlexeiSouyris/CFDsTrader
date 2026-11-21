//! Tipos compartidos del motor de CFDsTrader.
//!
//! Los tipos que cruzan entre lenguajes tienen su contrato en `contracts/`
//! (ver `tests/contracts.rs`).

pub mod candle;
pub mod decimal_str;
pub mod economic_event;
pub mod instrument;
pub mod money;
pub mod sentiment;
pub mod spread;

pub use candle::{Candle, Timeframe};
pub use economic_event::{EconomicEvent, Impact};
pub use instrument::{AssetClass, Instrument, InstrumentId};
pub use money::{Currency, Money, MoneyError};
pub use sentiment::SentimentObservation;
pub use spread::CandleSpread;

/// Modo de operación del sistema. Por defecto, y por regla (CLAUDE.md §5),
/// solo existe la simulación: no hay forma de enviar órdenes reales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Simulación con precios reales y dinero ficticio.
    #[default]
    Paper,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_is_paper() {
        assert_eq!(Mode::default(), Mode::Paper);
    }
}

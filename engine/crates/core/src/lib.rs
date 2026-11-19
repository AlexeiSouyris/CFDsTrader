//! Tipos compartidos del motor de CFDsTrader.
//!
//! Los tipos reales (`Instrument`, `Candle`, `Money`) llegan con el Issue #4,
//! junto con su contrato en `contracts/`.

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

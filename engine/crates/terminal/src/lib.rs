//! Ver la bolsa desde la terminal (#541): tabla de precios con variación del día y mini gráfico de
//! velas de IBKR paper, **solo lectura y solo presentación**.
//!
//! - Los datos salen de la sesión de solo lectura de `cfds-ibkr` (guardas de puerto, cuenta paper y
//!   kill switch), con un `client_id` propio de `config/terminal.toml`.
//! - No calcula indicadores ni señales: muestra velas cerradas tal como llegan y, como única cuenta,
//!   la diferencia entre el último precio y el cierre de la diaria anterior ([`quote`]).
//! - Todo lo que decide qué mostrar es puro y se prueba sin red ni terminal ([`view`], [`chart`],
//!   [`schedule`]); el binario `bolsa` solo conecta, pide y pinta.

pub mod chart;
pub mod config;
pub mod quote;
pub mod schedule;
pub mod view;

pub use config::{TerminalConfig, TerminalConfigError};

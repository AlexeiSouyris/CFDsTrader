//! Observabilidad del motor (fase 9, #297): logs estructurados y alertas.
//!
//! - [`log`]: una línea JSON por evento, con la hora en UTC. **Nunca** escribe secretos: los
//!   campos con nombre de secreto se tapan, la contraseña de una URL de base de datos se tapa, y
//!   cualquier valor secreto registrado se reemplaza donde aparezca.
//! - [`alert`]: una interfaz de alertas con dos destinos, el log y Telegram (apagado salvo que
//!   estén `TELEGRAM_BOT_TOKEN` y `TELEGRAM_CHAT_ID`), y las reglas que deciden cuándo alertar.
//!
//! No usa `tracing`: ningún crate del repo lo usa directamente, y una línea JSON propia alcanza
//! (y deja la redacción de secretos en un solo lugar, probado).

pub mod alert;
pub mod log;

pub use alert::{
    Alert, AlertConfig, AlertKind, AlertSink, Alerter, HttpPost, LogSink, ReqwestPost, TelegramSink,
};
pub use log::{Level, Logger, redact_url};

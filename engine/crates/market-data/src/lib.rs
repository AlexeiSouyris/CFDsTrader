//! Datos de mercado: validación de velas (#5) y calendario de sesiones y feriados (#9).
//! Ninguna vela entra al sistema sin pasar por [`validation::validate_series`].

pub mod calendar;
pub mod config;
pub mod validation;

pub use calendar::{CalendarError, ExchangeCalendar, Session};
pub use config::{ConfigError, ValidationConfig};
pub use validation::{Issue, IssueKind, SessionCalendar, validate_series};

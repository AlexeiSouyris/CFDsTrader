//! Datos de mercado. Por ahora, la validación de velas (Issue #5): ninguna vela
//! entra al sistema sin pasar por [`validation::validate_series`].

pub mod config;
pub mod validation;

pub use config::{ConfigError, ValidationConfig};
pub use validation::{Issue, IssueKind, SessionCalendar, validate_series};

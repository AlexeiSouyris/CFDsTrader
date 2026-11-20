//! Datos de mercado: validación de velas (#5), calendario de sesiones y feriados (#9)
//! y separación de datos sin fugas (#16).
//! Ninguna vela entra al sistema sin pasar por [`validation::validate_series`].

pub mod calendar;
pub mod config;
pub mod split;
pub mod validation;

pub use calendar::{CalendarError, ExchangeCalendar, Session};
pub use config::{ConfigError, ValidationConfig};
pub use split::{DataSplit, Period, SplitConfig, SplitError, Window, plan};
pub use validation::{Issue, IssueKind, SessionCalendar, validate_series};

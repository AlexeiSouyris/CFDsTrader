//! Datos de mercado:
//! - validación de velas (#5);
//! - calendario de sesiones y feriados (#9);
//! - separación de datos sin fugas (#16);
//! - catálogo de instrumentos con sus costos (#7);
//! - trait de proveedores de datos (#28).
//!
//! Ninguna vela entra al sistema sin pasar por [`validation::validate_series`].

pub mod calendar;
pub mod config;
pub mod instruments;
pub mod provider;
pub mod split;
pub mod trading_window;
pub mod validation;

pub use calendar::{CalendarError, ExchangeCalendar, Session};
pub use config::{ConfigError, ValidationConfig};
pub use instruments::{BrokerTerms, Catalog, CatalogError, InstrumentSpec, PendingField, Sourced};
pub use provider::{MarketDataProvider, ProviderError, StaticProvider};
pub use split::{DataSplit, Period, SplitConfig, SplitError, Window, plan};
pub use trading_window::{
    TradingWindowConfig, entry_allowed, is_last_session_of_week, is_week_close_candle,
};
pub use validation::{Issue, IssueKind, SessionCalendar, validate_series};

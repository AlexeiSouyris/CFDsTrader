//! Datos de mercado:
//! - validación de velas (#5);
//! - calendario de sesiones y feriados (#9);
//! - separación de datos sin fugas (#16);
//! - catálogo de instrumentos con sus costos (#7);
//! - trait de proveedores de datos (#28);
//! - barras de 1 minuto de futuros: agrupación y verificación contra IG (#248).
//!
//! Ninguna vela entra al sistema sin pasar por [`validation::validate_series`].

pub mod aggregate;
pub mod calendar;
pub mod candle_source;
pub mod config;
pub mod daily;
pub mod futures;
pub mod instruments;
pub mod news_filter;
pub mod provider;
pub mod split;
pub mod spread_filter;
pub mod trading_window;
pub mod validation;

pub use aggregate::{AggregateError, H1Comparison, aggregate_h1, compare_h1};
pub use calendar::{CalendarError, ExchangeCalendar, Session};
pub use candle_source::{CandleSource, CandleSources};
pub use config::{ConfigError, ValidationConfig};
pub use daily::{DailyIssue, DailyIssueKind, daily_bounds, local_day_bounds, validate_daily};
pub use futures::{
    ContractBasis, DailyComparison, DayPair, FuturesCandles, compare_daily, futures_candles,
};
pub use instruments::{BrokerTerms, Catalog, CatalogError, InstrumentSpec, PendingField, Sourced};
pub use news_filter::{NewsFilter, NewsFilterConfig, calendar_coverage, news_flags};
pub use provider::{MarketDataProvider, ProviderError, StaticProvider};
pub use split::{DataSplit, Period, SplitConfig, SplitError, Window, plan};
pub use spread_filter::{SpreadFilterConfig, spread_flags};
pub use trading_window::{
    TradingWindowConfig, entry_allowed, is_last_session_of_week, is_week_close_candle,
};
pub use validation::{
    CheckedSeries, Issue, IssueKind, SessionCalendar, check_series, validate_series,
};

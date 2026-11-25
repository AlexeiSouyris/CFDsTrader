//! Datos de mercado:
//! - validación de velas (#5);
//! - calendario de sesiones y feriados (#9);
//! - separación de datos sin fugas (#16);
//! - catálogo de instrumentos con sus costos (#7);
//! - trait de proveedores de datos (#28);
//! - barras de 1 minuto de futuros: agrupación y verificación contra IG (#248);
//! - día de sesión de una vela intradía y variación contra la sesión anterior (#551);
//! - sorpresa del calendario: dato real oficial − pronóstico previo (ADR 0018, #698).
//!
//! Ninguna vela entra al sistema sin pasar por [`validation::validate_series`].

pub mod aggregate;
pub mod calendar;
pub mod candle_source;
pub mod config;
pub mod daily;
pub mod futures;
pub mod futures_feeds;
pub mod instruments;
pub mod news_filter;
pub mod provider;
pub mod session_day;
pub mod split;
pub mod spread_filter;
pub mod surprise;
pub mod trading_window;
pub mod validation;

pub use aggregate::{AggregateError, H1Comparison, H1Stats, aggregate_h1, compare_h1};
pub use calendar::{CalendarError, ExchangeCalendar, Session};
pub use candle_source::{
    BasisMode, BasisSpec, CandleSource, CandleSources, FuturesBasisConfig, RateSegment, RollDay,
    contract_changes,
};
pub use config::{ConfigError, ValidationConfig};
pub use daily::{DailyIssue, DailyIssueKind, daily_bounds, local_day_bounds, validate_daily};
pub use futures::{
    ContractBasis, DailyComparison, DayPair, FuturesCandles, compare_daily, futures_candles,
    futures_daily,
};
pub use futures_feeds::{FuturesFeed, FuturesFeeds};
pub use instruments::{
    BrokerTerms, Catalog, CatalogError, FundingModel, InstrumentSpec, PendingField, Sourced,
};
pub use news_filter::{NewsFilter, NewsFilterConfig, calendar_coverage, news_flags};
pub use provider::{MarketDataProvider, ProviderError, StaticProvider};
pub use session_day::{SessionOffsets, session_date, session_start};
pub use split::{DataSplit, Period, SplitConfig, SplitError, Window, plan};
pub use spread_filter::{SpreadFilterConfig, spread_flags};
pub use trading_window::{
    EntryHours, TradingWindowConfig, entry_allowed, is_last_session_of_week, is_week_close_candle,
};
pub use validation::{
    CheckedSeries, Issue, IssueKind, JumpOutcome, SessionCalendar, check_series, decision_flags,
    jump_outcome, validate_series,
};

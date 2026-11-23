//! Tipos compartidos del motor de CFDsTrader.
//!
//! Los tipos que cruzan entre lenguajes tienen su contrato en `contracts/`
//! (ver `tests/contracts.rs`).

pub mod broker;
pub mod candle;
pub mod daily;
pub mod daily_drawdown;
pub mod daily_indicator;
pub mod daily_vol_tercile;
pub mod decimal_str;
pub mod economic_event;
pub mod futures_bar;
pub mod imbalance;
pub mod instrument;
pub mod liquidity_level;
pub mod liquidity_stats;
pub mod market_context;
pub mod money;
pub mod news;
pub mod rate;
pub mod sentiment;
pub mod spread;

pub use broker::{
    AccountCurrency, ClientId, KillSwitch, Order, OrderRejection, OrderReport, OrderSide,
    OrderStatus, PaperAccount, PaperMode, Position, TicketVerdict,
};
pub use candle::{Candle, Timeframe};
pub use daily::DailyCandle;
pub use daily_drawdown::DailyDrawdown;
pub use daily_indicator::DailyIndicator;
pub use daily_vol_tercile::{DailyVolTercile, VolTercile};
pub use economic_event::{EconomicEvent, Impact};
pub use futures_bar::{FuturesBar, FuturesCandle};
pub use imbalance::{Imbalance, ImbalanceLabel};
pub use instrument::{AssetClass, Instrument, InstrumentId};
pub use liquidity_level::{
    LiquidityLevel, LiquidityLevelKind, LiquidityLevelState, LiquiditySession,
};
pub use liquidity_stats::{
    Direction, DistMetric, DistStat, FvgEvent, FvgEventPoint, FvgReaction, GapSession,
    LiquidityStats, OpeningGap, RateMetric, RateStat, SizeBucket, SizeCuts, StatsSubject,
    StatsTimeframe, ZoneStatus,
};
pub use market_context::{
    AnalogContext, AnalogGroup, AnalogGroupKind, AnalogHits, AnalogHorizon, AnalogNeighbor,
    AnalogPercentile, AnalogReport, AnalogSource, ComponentStatus, RatesTrend, SentimentComponent,
    SentimentIndex, VolBand,
};
pub use money::{Currency, Money, MoneyError};
pub use news::{Interpreter, NewsImpact, NewsInterpretation};
pub use rate::ReferenceRate;
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

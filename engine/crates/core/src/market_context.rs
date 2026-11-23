//! Contexto de mercado que calcula Python y Rust solo lee (#294): el índice de sentimiento
//! (`sentiment.v1`, #213) y los análogos históricos (`analog_report.v1`, #148). Solo
//! informativos: no suman al puntaje, y los análogos son una distribución de resultados pasados,
//! nunca un pronóstico.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;
use crate::liquidity_stats::utc_z_option;

/// Estado de un componente del índice de sentimiento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentStatus {
    Valid,
    WarmingUp,
    Expired,
    Missing,
    PendingData,
}

/// Un componente de `sentiment.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SentimentComponent {
    pub name: String,
    pub status: ComponentStatus,
    #[serde(with = "crate::decimal_str::signed")]
    pub weight: Decimal,
    #[serde(with = "utc_z_option")]
    pub observed_for: Option<DateTime<Utc>>,
    #[serde(with = "utc_z_option")]
    pub published_at: Option<DateTime<Utc>>,
    #[serde(with = "crate::decimal_str::signed_option")]
    pub raw: Option<Decimal>,
    #[serde(with = "crate::decimal_str::signed_option")]
    pub percentile: Option<Decimal>,
    #[serde(with = "crate::decimal_str::signed_option")]
    pub component: Option<Decimal>,
}

/// `sentiment.v1`: el índice de sentimiento de un instrumento, de −1 (miedo) a +1 (euforia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SentimentIndex {
    pub instrument_id: InstrumentId,
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub published_at: DateTime<Utc>,
    #[serde(with = "crate::decimal_str::signed")]
    pub value: Decimal,
    pub warmup_complete: bool,
    pub min_components: u32,
    pub components: Vec<SentimentComponent>,
}

/// De qué diarias salen los análogos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalogSource {
    Ig,
    Futures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolBand {
    Low,
    Mid,
    High,
}

/// Tendencia del tipo de referencia (los valores del contrato, en español).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RatesTrend {
    Sube,
    Baja,
    Estable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalogContext {
    pub vol: Option<VolBand>,
    pub rates: Option<RatesTrend>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalogNeighbor {
    pub instrument_id: InstrumentId,
    pub end_day: NaiveDate,
    #[serde(with = "crate::decimal_str")]
    pub distance: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalogPercentile {
    pub p: u8,
    #[serde(rename = "return", with = "crate::decimal_str::signed")]
    pub ret: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalogHorizon {
    pub horizon: u32,
    pub percentiles: Vec<AnalogPercentile>,
    #[serde(with = "crate::decimal_str")]
    pub positive_pct: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalogHits {
    pub up: u32,
    pub down: u32,
    pub none: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalogGroupKind {
    All,
    SameContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalogGroup {
    pub group: AnalogGroupKind,
    pub count: u32,
    /// `true` con menos vecinos que `min_neighbors`: "datos insuficientes".
    pub insufficient: bool,
    pub horizons: Vec<AnalogHorizon>,
    pub hits: AnalogHits,
    pub without_atr: u32,
}

/// `analog_report.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalogReport {
    pub instrument_id: InstrumentId,
    pub source: AnalogSource,
    pub window_end_day: NaiveDate,
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub computed_at: DateTime<Utc>,
    pub window: u32,
    pub min_neighbors: u32,
    pub context: AnalogContext,
    pub neighbors: Vec<AnalogNeighbor>,
    pub groups: Vec<AnalogGroup>,
}

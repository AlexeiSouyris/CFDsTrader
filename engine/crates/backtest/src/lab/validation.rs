//! `spec_validation.v1` (el resultado de validar una spec) y la spec ya validada, con sus
//! bloques tipados (los arma [`super::validate::validate_spec`]).

use cfds_core::Timeframe;
use chrono::NaiveTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::spec::StrategySpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Malformed,
    CatalogVersion,
    UnknownMarket,
    TimeframeUnavailable,
    UnknownBlock,
    WrongSlot,
    BlockTimeframe,
    UnknownParam,
    MissingParam,
    ParamType,
    ParamOutOfRange,
    InvalidCombination,
    TooManyConditions,
    TooManyFilters,
    RiskOutOfRange,
    CostOutOfRange,
    PeriodInvalid,
    PeriodTouchesHoldout,
    ModeUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecError {
    pub code: ErrorCode,
    /// JSON Pointer al campo (vacío = la spec entera).
    pub path: String,
    pub message: String,
}

impl SpecError {
    pub fn new(code: ErrorCode, path: &str, message: impl Into<String>) -> Self {
        let (path, message) = (path.to_owned(), message.into());
        Self {
            code,
            path,
            message,
        }
    }
}

/// `spec_validation.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecValidation {
    pub valid: bool,
    pub catalog_version: u32,
    pub spec_sha256: Option<String>,
    pub family_sha256: Option<String>,
    pub errors: Vec<SpecError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Average {
    Sma,
    Ema,
}

/// Una condición de entrada tipada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryCondition {
    MaCross {
        average: Average,
        fast: usize,
        slow: usize,
    },
    OpeningRangeBreakout {
        range_candles: u32,
        /// Hora local de la bolsa.
        last_breakout_close: NaiveTime,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryFilter {
    /// Decisión en `[from, to)` UTC; si `from > to`, cruza la medianoche.
    EntryHoursUtc { from: NaiveTime, to: NaiveTime },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopRule {
    Atr { period: usize, k: Decimal },
}

/// Salida por tiempo (al menos una de las dos) con objetivo opcional en R.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitRule {
    pub max_candles: Option<usize>,
    pub session_close: bool,
    pub target_r: Option<Decimal>,
}

/// Una spec válida: la spec normalizada (decimales sin ceros de más), sus hashes y sus bloques
/// tipados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidSpec {
    pub spec: StrategySpec,
    pub spec_sha256: String,
    pub family_sha256: String,
    pub timeframe: Timeframe,
    pub entry: Vec<EntryCondition>,
    pub filters: Vec<EntryFilter>,
    pub stop: StopRule,
    pub exit: ExitRule,
}

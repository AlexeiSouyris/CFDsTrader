//! `strategy_catalog.v1`: qué bloques y parámetros corre el motor, con sus rangos (los arma
//! [`super::blocks::catalog`] desde el código y la config).

use cfds_core::{InstrumentId, Timeframe};
use chrono::NaiveTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::spec::{Mode, Source};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Entry,
    Filter,
    Stop,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_entry_conditions: usize,
    pub max_filters: usize,
    #[serde(with = "cfds_core::decimal_str")]
    pub max_risk_pct: Decimal,
    #[serde(with = "cfds_core::decimal_str")]
    pub min_cost_multiplier: Decimal,
    #[serde(with = "cfds_core::decimal_str")]
    pub max_cost_multiplier: Decimal,
    pub max_variants_per_run: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketDef {
    pub instrument_id: InstrumentId,
    pub source: Source,
    pub timeframes: Vec<Timeframe>,
}

/// Un parámetro de un bloque: nombre, etiqueta, si es obligatorio y su tipo con el rango.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamSchema {
    pub name: String,
    pub label: String,
    pub required: bool,
    #[serde(flatten)]
    pub kind: ParamKind,
}

/// Tipo (`kind`), rango inclusivo y valor propuesto (`default`, solo para la UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParamKind {
    Integer {
        min: i64,
        max: i64,
        default: Option<i64>,
    },
    Decimal {
        #[serde(with = "cfds_core::decimal_str")]
        min: Decimal,
        #[serde(with = "cfds_core::decimal_str")]
        max: Decimal,
        #[serde(with = "cfds_core::decimal_str::option")]
        default: Option<Decimal>,
    },
    Time {
        default: Option<String>,
    },
    Enum {
        options: Vec<String>,
        default: Option<String>,
    },
    Boolean {
        default: Option<bool>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockSchema {
    pub slot: Slot,
    #[serde(rename = "type")]
    pub kind: String,
    pub label: String,
    pub description: String,
    pub timeframes: Vec<Timeframe>,
    pub params: Vec<ParamSchema>,
}

/// `strategy_catalog.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrategyCatalog {
    pub catalog_version: u32,
    pub limits: Limits,
    pub markets: Vec<MarketDef>,
    pub modes: Vec<Mode>,
    pub blocks: Vec<BlockSchema>,
}

impl StrategyCatalog {
    pub fn block(&self, kind: &str) -> Option<&BlockSchema> {
        self.blocks.iter().find(|b| b.kind == kind)
    }
}

/// Hora `HH:MM` del contrato.
pub fn parse_time(s: &str) -> Option<NaiveTime> {
    (s.len() == 5).then(|| NaiveTime::parse_from_str(s, "%H:%M").ok())?
}

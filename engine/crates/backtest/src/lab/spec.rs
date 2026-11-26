//! `strategy_spec.v1` (la estrategia declarativa) y su hash canónico.

use std::collections::BTreeMap;

use cfds_core::{InstrumentId, Timeframe};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Futures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Long,
    Short,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Exploratory,
    Confirmatory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Market {
    pub instrument_id: InstrumentId,
    pub source: Source,
    pub timeframe: Timeframe,
}

/// El valor de un parámetro: entero, booleano o texto (decimal, hora `HH:MM` u opción).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Integer(i64),
    Boolean(bool),
    Text(String),
}

/// Un bloque: su tipo y sus parámetros por nombre (los define el catálogo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    #[serde(rename = "type")]
    pub kind: String,
    pub params: BTreeMap<String, ParamValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecPeriod {
    #[serde(with = "cfds_core::candle::utc_z")]
    pub from: DateTime<Utc>,
    #[serde(with = "cfds_core::candle::utc_z")]
    pub to: DateTime<Utc>,
}

/// `strategy_spec.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrategySpec {
    pub catalog_version: u32,
    pub market: Market,
    pub direction: Direction,
    pub entry: Vec<Block>,
    pub filters: Vec<Block>,
    pub stop: Block,
    pub exit: Block,
    #[serde(with = "cfds_core::decimal_str")]
    pub risk_pct: Decimal,
    #[serde(with = "cfds_core::decimal_str")]
    pub cost_multiplier: Decimal,
    pub period: SpecPeriod,
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preregistration_sha256: Option<String>,
}

/// JSON compacto con las claves de cada objeto ordenadas (no depende del orden de entrada).
pub fn canonical_json(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let items: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{}:{}", Value::String(k.clone()), canonical_json(&m[k])))
                .collect();
            format!("{{{}}}", items.join(","))
        }
        Value::Array(xs) => {
            let items: Vec<String> = xs.iter().map(canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        other => other.to_string(),
    }
}

/// SHA-256 en hexadecimal de un texto.
pub fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Hash de la forma canónica de una spec **ya normalizada** (decimales sin ceros de más, ver
/// `validate`): claves ordenadas, sin espacios.
pub fn spec_sha256(spec: &StrategySpec) -> Result<String, serde_json::Error> {
    Ok(sha256_hex(&canonical_json(&serde_json::to_value(spec)?)))
}

/// Hash de la familia (definición PENDIENTE, §9 punto 2): mismo instrumento y mismos tipos de
/// bloque; las condiciones de entrada y los filtros, sin orden.
pub fn family_sha256(spec: &StrategySpec) -> String {
    let sorted = |blocks: &[Block]| {
        let mut v: Vec<String> = blocks.iter().map(|b| b.kind.clone()).collect();
        v.sort();
        v
    };
    let key = serde_json::json!({
        "instrument_id": spec.market.instrument_id.as_str(),
        "entry": sorted(&spec.entry),
        "filters": sorted(&spec.filters),
        "stop": spec.stop.kind,
        "exit": spec.exit.kind,
    });
    sha256_hex(&canonical_json(&key))
}

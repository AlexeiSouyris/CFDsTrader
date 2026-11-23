//! Barra de 1 minuto de un futuro, tal como la publica la fuente (ADR 0011, #247 y #248).
//! Contrato: `contracts/futures_bar.v1.schema.json`.
//!
//! La escribe Python (`cfds_intel.databento`) y la agrupa y verifica el motor. Acepta
//! exactamente los mismos documentos que el contrato (también que `start` caiga justo en un
//! minuto y que `contract_id` sea mayor que 0).

use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Raw", into = "Raw")]
pub struct FuturesBar {
    pub source: String,
    pub dataset: String,
    /// Contrato continuo pedido (por ejemplo `ES.c.0`).
    pub symbol: String,
    /// Nuestro instrumento (CFD) al que corresponde el futuro.
    pub instrument_id: InstrumentId,
    /// Contrato real de la barra: si cambia entre dos barras, hubo un vencimiento.
    pub contract_id: u64,
    /// Inicio de la barra (UTC, justo en un minuto). Cierra en `start + 1 min`.
    pub start: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: u64,
}

impl FuturesBar {
    /// Cierre de la barra: `start + 1 min`.
    pub fn end(&self) -> DateTime<Utc> {
        self.start + chrono::TimeDelta::minutes(1)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(with = "crate::sentiment::label")]
    source: String,
    dataset: String,
    symbol: String,
    instrument_id: InstrumentId,
    contract_id: u64,
    #[serde(with = "utc_z")]
    start: DateTime<Utc>,
    #[serde(with = "crate::decimal_str")]
    open: Decimal,
    #[serde(with = "crate::decimal_str")]
    high: Decimal,
    #[serde(with = "crate::decimal_str")]
    low: Decimal,
    #[serde(with = "crate::decimal_str")]
    close: Decimal,
    volume: u64,
}

/// `^[A-Z0-9]+\.[A-Z0-9]+$`
fn is_dataset(s: &str) -> bool {
    let up = |p: &str| {
        !p.is_empty()
            && p.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    };
    s.split_once('.').is_some_and(|(a, b)| up(a) && up(b))
}

/// `^[A-Z0-9]{1,10}\.[cnv]\.[0-9]{1,2}$`
fn is_continuous(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    matches!(parts.as_slice(), [root, rule, rank]
        if (1..=10).contains(&root.len())
            && root.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            && matches!(*rule, "c" | "n" | "v")
            && (1..=2).contains(&rank.len())
            && rank.bytes().all(|b| b.is_ascii_digit()))
}

impl TryFrom<Raw> for FuturesBar {
    type Error = String;

    fn try_from(r: Raw) -> Result<Self, String> {
        if !is_dataset(&r.dataset) {
            return Err(format!("dataset inválido: {:?}", r.dataset));
        }
        if !is_continuous(&r.symbol) {
            return Err(format!("symbol no es un contrato continuo: {:?}", r.symbol));
        }
        if r.contract_id == 0 {
            return Err("contract_id tiene que ser mayor que 0".into());
        }
        if r.start.second() != 0 || r.start.nanosecond() != 0 {
            return Err(format!("start no cae justo en un minuto: {}", r.start));
        }
        Ok(Self {
            source: r.source,
            dataset: r.dataset,
            symbol: r.symbol,
            instrument_id: r.instrument_id,
            contract_id: r.contract_id,
            start: r.start,
            open: r.open,
            high: r.high,
            low: r.low,
            close: r.close,
            volume: r.volume,
        })
    }
}

impl From<FuturesBar> for Raw {
    fn from(b: FuturesBar) -> Self {
        Self {
            source: b.source,
            dataset: b.dataset,
            symbol: b.symbol,
            instrument_id: b.instrument_id,
            contract_id: b.contract_id,
            start: b.start,
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume,
        }
    }
}

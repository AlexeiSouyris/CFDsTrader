//! Desequilibrio aproximado desde el precio (spec §5.4.1, #231). Contrato:
//! `contracts/imbalance.v1.schema.json`.
//!
//! No es flujo de órdenes (IG no da volumen ni libro): viaja siempre con su rótulo. Lo calcula
//! solo el motor (`cfds-liquidity`, módulo `imbalance`); la UI lo muestra y no lo recalcula.

use std::num::NonZeroU32;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;
use crate::liquidity_stats::{Direction, StatsTimeframe};

/// El rótulo que la spec exige mostrar siempre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ImbalanceLabel {
    #[default]
    #[serde(rename = "aproximación desde el precio")]
    PriceApproximation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Imbalance {
    pub instrument_id: InstrumentId,
    pub timeframe: StatsTimeframe,
    /// Cierre de la última vela de la ventana.
    #[serde(with = "utc_z")]
    pub at: DateTime<Utc>,
    pub window_bars: NonZeroU32,
    /// Dirección si la vela de `at` es de desplazamiento.
    #[serde(deserialize_with = "Option::deserialize")]
    pub displacement: Option<Direction>,
    #[serde(with = "crate::decimal_str")]
    pub bullish_body_sum: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub bearish_body_sum: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub range_sum: Decimal,
    /// De −1 a +1; 0 si `range_sum` es 0.
    #[serde(with = "crate::decimal_str::signed")]
    pub index: Decimal,
    pub label: ImbalanceLabel,
}

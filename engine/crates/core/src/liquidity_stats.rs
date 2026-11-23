//! Estadísticas de FVG y de gaps de apertura (spec §5.4.1, #231). Contratos:
//! `contracts/fvg_event.v1.schema.json`, `contracts/opening_gap.v1.schema.json` y
//! `contracts/liquidity_stats.v1.schema.json`.
//!
//! Los calcula solo el motor (`cfds-liquidity`, módulo `stats`); la UI y Python los leen y no
//! los recalculan (CLAUDE.md §1). Son informativos: no suman al puntaje de §6.

use std::num::NonZeroU32;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::{Timeframe, utc_z};
use crate::instrument::InstrumentId;

/// `utc_z` para un campo opcional: `null` o ausente es `None`.
mod utc_z_option {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(t: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
        match t {
            Some(t) => super::utc_z::serialize(t, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
        #[derive(Deserialize)]
        struct W(#[serde(with = "super::utc_z")] DateTime<Utc>);
        Ok(Option::<W>::deserialize(d)?.map(|w| w.0))
    }
}

/// Dirección de una zona: alcista (FVG alcista o gap hacia arriba) o bajista (el espejo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Bullish,
    Bearish,
}

/// Timeframes de las estadísticas: los de la spec §2 más el diario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StatsTimeframe {
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "1d")]
    D1,
}

impl From<Timeframe> for StatsTimeframe {
    fn from(tf: Timeframe) -> Self {
        match tf {
            Timeframe::M15 => Self::M15,
            Timeframe::H1 => Self::H1,
        }
    }
}

/// Estado de una zona al cierre de `as_of`. Una zona abierta todavía puede tocarse o llenarse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneStatus {
    Open,
    Filled,
    Expired,
}

/// Una vela en la que pasó algo (toque, mitad, llenado o vencimiento).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FvgEventPoint {
    /// Cierre de la vela del evento, en UTC.
    #[serde(with = "utc_z")]
    pub at: DateTime<Utc>,
    /// Velas desde la creación (la vela siguiente a la de creación es 1).
    pub bars: NonZeroU32,
}

/// Retorno del cierre `bars` velas después del primer toque, en ATR, a favor de la dirección.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FvgReaction {
    pub bars: NonZeroU32,
    /// `None` si no hubo toque o todavía no cerraron `bars` velas después del toque.
    #[serde(with = "crate::decimal_str::signed_option")]
    pub return_atr: Option<Decimal>,
    /// Cierre de la vela `toque + bars`: desde ahí se conoce la reacción (#231, guardado).
    /// Opcional en el contrato (campo agregado después); el motor siempre lo llena junto con
    /// `return_atr`.
    #[serde(default, with = "utc_z_option")]
    pub at: Option<DateTime<Utc>>,
}

/// Un FVG (definición de §5.4) con lo que le pasó hasta `as_of`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FvgEvent {
    pub instrument_id: InstrumentId,
    pub timeframe: StatsTimeframe,
    pub direction: Direction,
    /// Cierre de la vela que crea el FVG: el FVG existe desde ahí.
    #[serde(with = "utc_z")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "crate::decimal_str")]
    pub bottom: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub top: Decimal,
    /// ATR de la vela de creación.
    #[serde(with = "crate::decimal_str")]
    pub atr: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub size_atr: Decimal,
    #[serde(deserialize_with = "Option::deserialize")]
    pub touch: Option<FvgEventPoint>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub mid: Option<FvgEventPoint>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub fill: Option<FvgEventPoint>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub expiry: Option<FvgEventPoint>,
    pub reactions: Vec<FvgReaction>,
    pub status: ZoneStatus,
    /// Cierre de la última vela usada.
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
}

/// Sesión en la que se llenó o venció un gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GapSession {
    pub day: NaiveDate,
    /// Sesiones desde la del gap (0 = la misma sesión).
    pub sessions: u32,
    /// Fin de la vela diaria de esa sesión: desde ahí se conoce el evento. Opcional en el
    /// contrato (campo agregado después); el motor siempre lo llena.
    #[serde(default, with = "utc_z_option")]
    pub at: Option<DateTime<Utc>>,
}

/// Un gap de apertura diario con lo que le pasó hasta `as_of`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningGap {
    pub instrument_id: InstrumentId,
    /// Sesión que abre con gap.
    pub day: NaiveDate,
    pub direction: Direction,
    #[serde(with = "crate::decimal_str")]
    pub prev_close: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub open: Decimal,
    /// ATR diario de la sesión anterior (lo conocido antes de la apertura).
    #[serde(with = "crate::decimal_str")]
    pub atr: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub size_atr: Decimal,
    #[serde(deserialize_with = "Option::deserialize")]
    pub fill: Option<GapSession>,
    pub same_session: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub expiry: Option<GapSession>,
    pub status: ZoneStatus,
    /// Fin de la última vela diaria usada.
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatsSubject {
    Fvg,
    OpeningGap,
}

/// Tercil del tamaño en ATR (`all` = todos los tamaños juntos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeBucket {
    Low,
    Mid,
    High,
    All,
}

/// Cortes de los terciles: `low` si tamaño ≤ `low_max`, `mid` si ≤ `mid_max`, si no `high`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SizeCuts {
    #[serde(with = "crate::decimal_str")]
    pub low_max: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub mid_max: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateMetric {
    Touch,
    Mid,
    Fill,
    SameSessionFill,
}

/// Una tasa con su IC 95 % de Wilson, o "datos insuficientes" (`rate` y el IC en null).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateStat {
    pub metric: RateMetric,
    pub successes: u32,
    pub n: u32,
    pub insufficient: bool,
    #[serde(with = "crate::decimal_str::option")]
    pub rate: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub ci_low: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub ci_high: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistMetric {
    BarsToTouch,
    BarsToMid,
    BarsToFill,
    SessionsToFill,
    Reaction,
}

/// Mediana y cuartiles de una medida, o "datos insuficientes" (valores en null).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistStat {
    pub metric: DistMetric,
    /// Solo para `reaction`: a cuántas velas del primer toque.
    #[serde(deserialize_with = "Option::deserialize")]
    pub horizon_bars: Option<NonZeroU32>,
    pub n: u32,
    pub insufficient: bool,
    #[serde(with = "crate::decimal_str::signed_option")]
    pub median: Option<Decimal>,
    #[serde(with = "crate::decimal_str::signed_option")]
    pub p25: Option<Decimal>,
    #[serde(with = "crate::decimal_str::signed_option")]
    pub p75: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub iqr: Option<Decimal>,
}

/// Agregados de un grupo: instrumento, tipo de zona, timeframe, dirección y tercil de tamaño.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiquidityStats {
    pub instrument_id: InstrumentId,
    pub subject: StatsSubject,
    pub timeframe: StatsTimeframe,
    pub direction: Direction,
    pub size_tercile: SizeBucket,
    pub size_cuts_atr: SizeCuts,
    pub min_cases: NonZeroU32,
    pub n: u32,
    pub rates: Vec<RateStat>,
    pub distributions: Vec<DistStat>,
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
}

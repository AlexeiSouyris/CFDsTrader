//! Variables de una propuesta para el modelo de ML v1 que no viajan en `proposal.v1`
//! (`contracts/ml_features.v1`, docs/spec/ml-v1.md §3, #158). Las arma el motor y las lee Python
//! (`cfds_intel.models`), que no recalcula ningún indicador (CLAUDE.md §1).
//!
//! Todo se calcula con información disponible al **cierre** de la vela de la señal
//! (`as_of = signal_at`, CLAUDE.md §5b):
//!
//! - `atr14_m15`: ATR de Wilder de 14 velas de 15m en la vela de la señal. Solo usa velas hasta
//!   ella incluida (`wilder_atr` no repinta).
//! - `realized_vol_tercile`: el último indicador diario **publicado** hasta `as_of` (fin de su
//!   vela diaria), en tercios de la volatilidad realizada de los `vol_history_years` años
//!   anteriores. Misma definición que el contexto de los análogos (spec §13); un fixture
//!   compartido con Python lo verifica.
//! - `session_band`: en qué tercio de la sesión de ese día cae `as_of`.
//!
//! Si falta cualquiera (calentamiento, sin historia, fuera de sesión, día sin calendario), la
//! propuesta no tiene fila: no se rellena (spec §3).

use cfds_core::candle::utc_z;
use cfds_core::{Candle, DailyIndicator, InstrumentId, decimal_str};
use cfds_indicators::wilder_atr;
use cfds_market_data::Session;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use cfds_backtest::TradeRecord;

/// Período del ATR del contrato (`atr14_m15`). Fijo: cambiarlo es otra versión del contrato.
pub const ATR_PERIOD: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolTercile {
    Low,
    Mid,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionBand {
    Open,
    Mid,
    Close,
}

/// `contracts/ml_features.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlFeatures {
    pub proposal_id: String,
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
    #[serde(with = "decimal_str")]
    pub atr14_m15: Decimal,
    pub realized_vol_tercile: VolTercile,
    pub session_band: SessionBand,
}

/// Sección `[features]` de `config/ml.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeaturesConfig {
    /// Años de historia para los tercios de la volatilidad realizada.
    pub vol_history_years: u32,
}

impl FeaturesConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct File {
            features: FeaturesConfig,
        }
        let cfg = toml::from_str::<File>(text)
            .map_err(|e| e.to_string())?
            .features;
        if cfg.vol_history_years == 0 {
            return Err("features.vol_history_years tiene que ser > 0".into());
        }
        Ok(cfg)
    }
}

/// ATR de 14 velas de 15m, alineado con `m15`.
pub fn atr14_m15(m15: &[Candle]) -> Vec<Option<Decimal>> {
    wilder_atr(m15, ATR_PERIOD)
}

/// El mismo día `years` años antes (el 29 de febrero pasa al 28).
fn years_before(day: NaiveDate, years: u32) -> Option<NaiveDate> {
    let year = day.year() - i32::try_from(years).ok()?;
    day.with_year(year)
        .or_else(|| NaiveDate::from_ymd_opt(year, day.month(), 28))
}

/// Tercio de la volatilidad realizada del último día publicado hasta `as_of`, frente a la de
/// los `years` años anteriores. `daily`: indicadores de un instrumento, en orden de día.
///
/// `None` si ese día no tiene volatilidad (calentamiento), si la historia no llega a `years`
/// años completos o si no hay ningún valor en ellos.
pub fn vol_tercile(
    daily: &[DailyIndicator],
    as_of: DateTime<Utc>,
    years: u32,
) -> Option<VolTercile> {
    let end = daily.iter().rposition(|d| d.published_at <= as_of)?;
    let current = daily[end].realized_vol?;
    let since = years_before(daily[end].day, years)?;
    let with_vol: Vec<&DailyIndicator> = daily[..end]
        .iter()
        .filter(|d| d.realized_vol.is_some())
        .collect();
    if with_vol.first().is_none_or(|d| d.day > since) {
        return None;
    }
    let history: Vec<Decimal> = with_vol
        .iter()
        .filter(|d| d.day >= since)
        .filter_map(|d| d.realized_vol)
        .collect();
    if history.is_empty() {
        return None;
    }
    // rango = (# valores menores) / n, comparado con 1/3 y 2/3 en enteros (exacto).
    let below = history.iter().filter(|v| **v < current).count();
    let n = history.len();
    Some(if 3 * below < n {
        VolTercile::Low
    } else if 3 * below < 2 * n {
        VolTercile::Mid
    } else {
        VolTercile::High
    })
}

/// Tercio de la sesión de ese día en el que cae `as_of` (el cierre de una vela): apertura si
/// `as_of − apertura ≤ duración / 3`, media hasta `2/3` y cierre después. `None` fuera de
/// `(apertura, cierre]`, en un día sin sesión o fuera del calendario. Usa el cierre temprano
/// del día si lo hay.
pub fn session_band(session: &Session, as_of: DateTime<Utc>) -> Option<SessionBand> {
    let day = as_of.with_timezone(&session.tz).date_naive();
    let (open, close) = session.hours(day)??;
    if as_of <= open || as_of > close {
        return None;
    }
    let elapsed = (as_of - open).num_seconds();
    let total = (close - open).num_seconds();
    Some(if 3 * elapsed <= total {
        SessionBand::Open
    } else if 3 * elapsed <= 2 * total {
        SessionBand::Mid
    } else {
        SessionBand::Close
    })
}

/// Las variables de una operación del backtest. `atr`: [`atr14_m15`] de las mismas velas de
/// 15m que recorrió el backtest. `None` si falta alguna variable.
pub fn from_backtest(
    run_id: &str,
    instrument: &InstrumentId,
    trade: &TradeRecord,
    atr: &[Option<Decimal>],
    daily: &[DailyIndicator],
    session: &Session,
    cfg: &FeaturesConfig,
) -> Option<MlFeatures> {
    let as_of = trade.signal_at;
    let atr14 = (*atr.get(trade.signal_index)?).filter(|a| *a > Decimal::ZERO)?;
    Some(MlFeatures {
        proposal_id: crate::proposal_id(run_id, instrument, as_of, trade.side),
        as_of,
        atr14_m15: atr14.normalize(),
        realized_vol_tercile: vol_tercile(daily, as_of, cfg.vol_history_years)?,
        session_band: session_band(session, as_of)?,
    })
}

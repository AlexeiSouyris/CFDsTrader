//! Variables de una propuesta para el modelo de ML v1 que no viajan en `proposal.v1`
//! (`contracts/ml_features.v1`, docs/spec/ml-v1.md §3, #158). Las arma el motor y las lee Python
//! (`cfds_intel.models`), que no recalcula ningún indicador (CLAUDE.md §1).
//!
//! Todo se calcula con información disponible al **cierre** de la vela de la señal
//! (`as_of = signal_at`, CLAUDE.md §5b):
//!
//! - `atr14_m15`: ATR de Wilder de 14 velas de 15m en la vela de la señal. Solo usa velas hasta
//!   ella incluida (`wilder_atr` no repinta).
//! - `realized_vol_tercile`: el tercio guardado (`daily_vol_tercile.v1`, calculado por
//!   `cfds_indicators::vol_terciles`) del último día **publicado** hasta `as_of` (fin de su vela
//!   diaria). Es el mismo que usan los análogos: no se recalcula acá.
//! - `session_band`: en qué tercio de la sesión de ese día cae `as_of`.
//!
//! Si falta cualquiera (calentamiento, sin historia, fuera de sesión, día sin calendario), la
//! propuesta no tiene fila: no se rellena (spec §3).

pub use cfds_core::VolTercile;
use cfds_core::candle::utc_z;
use cfds_core::{Candle, DailyVolTercile, InstrumentId, decimal_str};
use cfds_indicators::wilder_atr;
use cfds_market_data::Session;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use cfds_backtest::TradeRecord;

/// Período del ATR del contrato (`atr14_m15`). Fijo: cambiarlo es otra versión del contrato.
pub const ATR_PERIOD: usize = 14;

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

/// ATR de 14 velas de 15m, alineado con `m15`.
pub fn atr14_m15(m15: &[Candle]) -> Vec<Option<Decimal>> {
    wilder_atr(m15, ATR_PERIOD)
}

/// Tercio de la volatilidad del último día publicado hasta `as_of`. `daily`: los tercios
/// guardados de un instrumento, en orden de día. `None` si ese día no tiene tercio o si no hay
/// ninguno publicado.
pub fn vol_tercile(daily: &[DailyVolTercile], as_of: DateTime<Utc>) -> Option<VolTercile> {
    daily.iter().rfind(|d| d.published_at <= as_of)?.tercile
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
    daily: &[DailyVolTercile],
    session: &Session,
) -> Option<MlFeatures> {
    let as_of = trade.signal_at;
    let atr14 = (*atr.get(trade.signal_index)?).filter(|a| *a > Decimal::ZERO)?;
    Some(MlFeatures {
        proposal_id: crate::proposal_id(run_id, instrument, as_of, trade.side),
        as_of,
        atr14_m15: atr14.normalize(),
        realized_vol_tercile: vol_tercile(daily, as_of)?,
        session_band: session_band(session, as_of)?,
    })
}

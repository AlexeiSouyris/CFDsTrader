//! Indicadores de las velas diarias (#148, spec §13): arma los registros de
//! `daily_indicator.v1` a partir de las diarias guardadas. Función pura: sin base ni reloj.
//!
//! - Solo cuentan los **días con sesión** de la bolsa del instrumento: las velas que IG manda
//!   en fines de semana o feriados se ignoran (no entran al ATR ni a los retornos).
//! - Si el calendario no cubre un día, no se supone nada: se devuelve ese día como error.
//! - `published_at` = fin de la vela diaria: el valor no existe antes (CLAUDE.md §5b).

use cfds_core::{DailyCandle, DailyIndicator};
use cfds_indicators::{DailyConfig, realized_volatility, wilder_atr};
use cfds_market_data::Session;
use chrono::NaiveDate;

/// Registros de indicadores de `days` (diarias **cerradas** de un instrumento, en orden).
/// `Err(día)` si el calendario de sesiones no cubre ese día.
pub fn daily_indicators(
    days: &[DailyCandle],
    session: &Session,
    cfg: &DailyConfig,
) -> Result<Vec<DailyIndicator>, NaiveDate> {
    let mut with_session = Vec::with_capacity(days.len());
    for d in days {
        match session.hours(d.day) {
            None => return Err(d.day),
            Some(None) => {}
            Some(Some(_)) => with_session.push(d.clone()),
        }
    }
    let atr = wilder_atr(&with_session, cfg.atr_period);
    let closes: Vec<_> = with_session.iter().map(|d| d.close).collect();
    let vol = realized_volatility(&closes, cfg.realized_vol_window);
    let atr_period = u32::try_from(cfg.atr_period).unwrap_or(u32::MAX);
    let window = u32::try_from(cfg.realized_vol_window).unwrap_or(u32::MAX);
    Ok(with_session
        .into_iter()
        .zip(atr.into_iter().zip(vol))
        .map(|(d, (atr, realized_vol))| DailyIndicator {
            instrument_id: d.instrument_id,
            day: d.day,
            published_at: d.end,
            atr_period,
            atr,
            realized_vol_window: window,
            realized_vol,
        })
        .collect())
}

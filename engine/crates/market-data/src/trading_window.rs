//! Horario de entradas y cierre de fin de semana (spec §2, §7.1 y §8), sobre la sesión de
//! cada bolsa. Funciones puras; `None` cuando la fecha está fuera de lo que cubre el
//! calendario: nunca se supone que una bolsa abre o cierra.
//!
//! ```text
//! entrada permitida al decidir en τ (cierre de t = apertura de t+1):
//!   la bolsa está en sesión en τ
//!   y τ ≥ apertura + 15 min
//!   y, si es el último día hábil de la semana, τ < cierre − 30 min
//! último día hábil de la semana: el siguiente día con sesión cae en otra semana (ISO)
//! cierre forzado: la última vela que cabe en la sesión de ese día
//! ```

use cfds_core::Candle;
use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
use serde::Deserialize;

use crate::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradingWindowConfig {
    pub skip_after_open: TimeDelta,
    pub no_new_before_week_close: TimeDelta,
}

impl TradingWindowConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Entries {
            skip_after_open_minutes: u32,
            no_new_before_week_close_minutes: u32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            entries: Entries,
        }
        let e = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .entries;
        Ok(Self {
            skip_after_open: TimeDelta::minutes(e.skip_after_open_minutes.into()),
            no_new_before_week_close: TimeDelta::minutes(e.no_new_before_week_close_minutes.into()),
        })
    }
}

/// `true` si `date` tiene sesión y es la última de su semana. `None` si para saberlo hay que
/// mirar fechas fuera de la cobertura del calendario.
pub fn is_last_session_of_week(session: &Session, date: NaiveDate) -> Option<bool> {
    if session.hours(date)?.is_none() {
        return Some(false);
    }
    let mut next = date.succ_opt()?;
    // A lo sumo una semana y media de feriados seguidos; el calendario corta antes si no cubre.
    for _ in 0..10 {
        if session.hours(next)?.is_some() {
            return Some(next.iso_week() != date.iso_week());
        }
        next = next.succ_opt()?;
    }
    Some(true)
}

/// ¿Se puede abrir una operación decidida en `at`?
pub fn entry_allowed(
    session: &Session,
    at: DateTime<Utc>,
    cfg: &TradingWindowConfig,
) -> Option<bool> {
    let date = at.with_timezone(&session.tz).date_naive();
    let Some((open, close)) = session.hours(date)? else {
        return Some(false);
    };
    if at < open + cfg.skip_after_open || at >= close {
        return Some(false);
    }
    if is_last_session_of_week(session, date)? && at >= close - cfg.no_new_before_week_close {
        return Some(false);
    }
    Some(true)
}

/// ¿La vela es la última que cabe en la sesión del último día hábil de la semana?
pub fn is_week_close_candle(session: &Session, c: &Candle) -> Option<bool> {
    let date = c.start.with_timezone(&session.tz).date_naive();
    let Some((open, close)) = session.hours(date)? else {
        return Some(false);
    };
    let fits_last = c.start >= open && c.end() <= close && c.end() + c.timeframe.duration() > close;
    Some(fits_last && is_last_session_of_week(session, date)?)
}

/// Marcas vela por vela para el backtest: si se puede entrar al decidir en el cierre de cada
/// vela, y si cada vela es la del cierre de la semana. `None` si alguna vela cae fuera de la
/// cobertura del calendario.
pub fn flags(
    session: &Session,
    candles: &[Candle],
    cfg: &TradingWindowConfig,
) -> Option<(Vec<bool>, Vec<bool>)> {
    let entries = candles
        .iter()
        .map(|c| entry_allowed(session, c.end(), cfg))
        .collect::<Option<Vec<_>>>()?;
    let closes = candles
        .iter()
        .map(|c| is_week_close_candle(session, c))
        .collect::<Option<Vec<_>>>()?;
    Some((entries, closes))
}

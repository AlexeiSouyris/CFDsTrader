//! Horario de entradas y cierre de fin de semana (spec §2, §7.1 y §8), sobre la sesión de
//! cada bolsa. Funciones puras; `None` cuando la fecha está fuera de lo que cubre el
//! calendario: nunca se supone que una bolsa abre o cierra.
//!
//! ```text
//! entrada permitida al decidir en τ (cierre de t = apertura de t+1):
//!   la bolsa está en sesión en τ
//!   y τ ≥ apertura + 15 min
//!   y, si es el último día hábil de la semana, τ < cierre − 30 min
//!   y, si la bolsa tiene horas de entrada propias ([entries.hours.<BOLSA>]), τ está dentro
//! último día hábil de la semana: el siguiente día con sesión cae en otra semana (ISO)
//! cierre forzado: la última vela que cabe en la sesión de ese día
//! ```
//!
//! Una sesión que cruza la medianoche (el oro, ADR 0013) dura casi 23 h: solo se entra en las
//! horas en que alguien puede aprobar (spec §2). Por eso **necesita** horas de entrada propias;
//! sin ellas, o mientras estén pendientes, no se abre ninguna operación. El "día" de un instante
//! es siempre el día hábil de su sesión ([`Session::trading_day`]).

use std::collections::BTreeMap;

use cfds_core::Candle;
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeDelta, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

use crate::Session;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradingWindowConfig {
    pub skip_after_open: TimeDelta,
    pub no_new_before_week_close: TimeDelta,
    /// Horas de entrada propias por bolsa (código de `config/sessions.toml`).
    pub hours: BTreeMap<String, EntryHours>,
}

/// Horas del día en que se pueden abrir operaciones en una bolsa, además de la sesión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryHours {
    /// Todavía sin decidir: no se abre nada (las horas las elige el dueño).
    Pending { reason: String },
    /// De `from` (incluida) a `to` (excluida), hora local de `tz`, el mismo día.
    Window {
        tz: Tz,
        from: NaiveTime,
        to: NaiveTime,
    },
}

impl EntryHours {
    fn contains(&self, at: DateTime<Utc>) -> bool {
        match self {
            EntryHours::Pending { .. } => false,
            EntryHours::Window { tz, from, to } => {
                let t = at.with_timezone(tz).time();
                *from <= t && t < *to
            }
        }
    }
}

impl TradingWindowConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawHours {
            pending: Option<String>,
            timezone: Option<String>,
            from: Option<String>,
            to: Option<String>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Entries {
            skip_after_open_minutes: u32,
            no_new_before_week_close_minutes: u32,
            #[serde(default)]
            hours: BTreeMap<String, RawHours>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            entries: Entries,
        }
        let e = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .entries;
        let time = |code: &str, s: &str| {
            NaiveTime::parse_from_str(s, "%H:%M")
                .map_err(|_| format!("entries.hours.{code}: {s:?} no es HH:MM"))
        };
        let mut hours = BTreeMap::new();
        for (code, h) in e.hours {
            let parsed = match (h.pending, h.timezone, h.from, h.to) {
                (Some(reason), None, None, None) if !reason.trim().is_empty() => {
                    EntryHours::Pending { reason }
                }
                (None, Some(tz), Some(from), Some(to)) => {
                    let tz: Tz = tz
                        .parse()
                        .map_err(|_| format!("entries.hours.{code}: zona horaria {tz:?}"))?;
                    let (from, to) = (time(&code, &from)?, time(&code, &to)?);
                    if from >= to {
                        return Err(format!(
                            "entries.hours.{code}: from tiene que ser anterior a to"
                        ));
                    }
                    EntryHours::Window { tz, from, to }
                }
                _ => {
                    return Err(format!(
                        "entries.hours.{code}: usar {{ timezone, from, to }} o {{ pending = \"motivo\" }}"
                    ));
                }
            };
            hours.insert(code, parsed);
        }
        Ok(Self {
            skip_after_open: TimeDelta::minutes(e.skip_after_open_minutes.into()),
            no_new_before_week_close: TimeDelta::minutes(e.no_new_before_week_close_minutes.into()),
            hours,
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
    let date = session.trading_day(at);
    let Some((open, close)) = session.hours(date)? else {
        return Some(false);
    };
    if at < open + cfg.skip_after_open || at >= close {
        return Some(false);
    }
    match cfg.hours.get(&session.exchange) {
        Some(h) if !h.contains(at) => return Some(false),
        // Una sesión de casi 23 h sin horas de entrada propias no abre nada (spec §2).
        None if session.is_overnight() => return Some(false),
        _ => {}
    }
    if is_last_session_of_week(session, date)? && at >= close - cfg.no_new_before_week_close {
        return Some(false);
    }
    Some(true)
}

/// ¿La vela es la última que cabe en la sesión del último día hábil de la semana?
pub fn is_week_close_candle(session: &Session, c: &Candle) -> Option<bool> {
    let date = session.trading_day(c.start);
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

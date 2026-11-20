//! Calendario de sesiones por bolsa (spec §2 y §12, Issue #9).
//!
//! - Las sesiones se definen en la hora **local** de la bolsa (`config/sessions.toml`), así que
//!   los cambios de horario de verano de EE. UU. y Europa (que no coinciden) se resuelven solos.
//! - Feriados y cierres anticipados vienen de `config/holidays/<BOLSA>.toml`, con su fuente.
//! - Fuera del rango que cubre el archivo de feriados, el calendario responde "no sé" (`None`):
//!   nunca supone que un día es hábil o feriado sin datos.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use cfds_core::{InstrumentId, Timeframe};
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc, Weekday};
use chrono_tz::Tz;
use serde::Deserialize;

use crate::validation::SessionCalendar;

#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    #[error("no se pudo leer {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("{path} no es TOML válido: {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
    #[error("calendario inválido: {0}")]
    Invalid(String),
}

/// Sesión de contado de una bolsa, con sus feriados.
#[derive(Debug, Clone)]
pub struct Session {
    pub exchange: String,
    pub tz: Tz,
    pub open: NaiveTime,
    pub close: NaiveTime,
    closed: BTreeSet<NaiveDate>,
    early_closes: BTreeMap<NaiveDate, NaiveTime>,
    covered: (NaiveDate, NaiveDate),
}

impl Session {
    /// Apertura y cierre en UTC de ese día local. `Some(None)` = no hay sesión (fin de semana
    /// o feriado). `None` = la fecha está fuera de lo que cubre el archivo de feriados.
    pub fn hours(&self, date: NaiveDate) -> Option<Option<(DateTime<Utc>, DateTime<Utc>)>> {
        if date < self.covered.0 || date > self.covered.1 {
            return None;
        }
        if matches!(date.weekday(), Weekday::Sat | Weekday::Sun) || self.closed.contains(&date) {
            return Some(None);
        }
        let close = self.early_closes.get(&date).copied().unwrap_or(self.close);
        let to_utc = |t: NaiveTime| {
            // Las bolsas abren y cierran lejos de la hora del cambio de horario (2–3 AM),
            // así que la hora local siempre existe y es única.
            self.tz
                .from_local_datetime(&date.and_time(t))
                .single()
                .map(|d| d.with_timezone(&Utc))
        };
        Some(Some((to_utc(self.open)?, to_utc(close)?)))
    }

    /// `true` si la bolsa está en sesión en el instante `t`. `None` si la fecha no está cubierta.
    pub fn is_open_at(&self, t: DateTime<Utc>) -> Option<bool> {
        let date = t.with_timezone(&self.tz).date_naive();
        Some(self.hours(date)?.is_some_and(|(o, c)| o <= t && t < c))
    }

    /// Velas de `timeframe` que caben enteras en la sesión, con inicio en `[from, to)`.
    pub fn candle_starts(
        &self,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Option<Vec<DateTime<Utc>>> {
        let step = timeframe.duration();
        let mut out = Vec::new();
        let mut date = from.with_timezone(&self.tz).date_naive();
        let last = to.with_timezone(&self.tz).date_naive();
        while date <= last {
            if let Some((open, close)) = self.hours(date)? {
                let mut s = align_up(open, step);
                while s + step <= close {
                    if from <= s && s < to {
                        out.push(s);
                    }
                    s += step;
                }
            }
            date = date.succ_opt()?;
        }
        Some(out)
    }
}

/// Primer múltiplo de `step` (contado desde la época Unix, en UTC) que es ≥ `t`.
fn align_up(t: DateTime<Utc>, step: TimeDelta) -> DateTime<Utc> {
    let rem = t.timestamp().rem_euclid(step.num_seconds());
    if rem == 0 {
        t
    } else {
        t + TimeDelta::seconds(step.num_seconds() - rem)
    }
}

/// Calendario de todas las bolsas, y qué bolsa usa cada instrumento.
#[derive(Debug, Clone, Default)]
pub struct ExchangeCalendar {
    sessions: HashMap<String, Session>,
    instruments: HashMap<InstrumentId, String>,
}

impl ExchangeCalendar {
    /// Lee `config_dir/sessions.toml` y los archivos de feriados que referencia.
    /// `instruments` asigna cada instrumento a una bolsa (viene de `config/instruments.toml`, #7).
    pub fn load(
        config_dir: &Path,
        instruments: HashMap<InstrumentId, String>,
    ) -> Result<Self, CalendarError> {
        let sessions_path = config_dir.join("sessions.toml");
        let raw: RawSessions = read_toml(&sessions_path)?;
        let mut sessions = HashMap::new();
        for (code, s) in raw.sessions {
            let holidays: RawHolidays = read_toml(&config_dir.join(&s.holidays))?;
            sessions.insert(code.clone(), build_session(&code, s, holidays)?);
        }
        if let Some((id, ex)) = instruments
            .iter()
            .find(|(_, ex)| !sessions.contains_key(*ex))
        {
            return Err(CalendarError::Invalid(format!(
                "{id} usa la bolsa {ex}, que no está en sessions.toml"
            )));
        }
        Ok(Self {
            sessions,
            instruments,
        })
    }

    pub fn session(&self, exchange: &str) -> Option<&Session> {
        self.sessions.get(exchange)
    }

    pub fn session_for(&self, instrument: &InstrumentId) -> Option<&Session> {
        self.sessions.get(self.instruments.get(instrument)?)
    }
}

impl SessionCalendar for ExchangeCalendar {
    fn expected_starts(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Option<Vec<DateTime<Utc>>> {
        self.session_for(instrument)?
            .candle_starts(timeframe, from, to)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSessions {
    sessions: BTreeMap<String, RawSession>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSession {
    #[allow(dead_code)] // solo informativo
    name: String,
    timezone: String,
    open: String,
    close: String,
    holidays: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHolidays {
    exchange: String,
    #[allow(dead_code)] // se exige para que todo archivo diga su fuente
    source: String,
    #[allow(dead_code)]
    generated_on: String,
    from: NaiveDate,
    to: NaiveDate,
    closed: Vec<NaiveDate>,
    early_closes: BTreeMap<NaiveDate, String>,
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, CalendarError> {
    let p = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|source| CalendarError::Io {
        path: p.clone(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| CalendarError::Parse { path: p, source })
}

fn parse_time(code: &str, what: &str, s: &str) -> Result<NaiveTime, CalendarError> {
    NaiveTime::parse_from_str(s, "%H:%M")
        .map_err(|_| CalendarError::Invalid(format!("{code}: {what} {s:?} no es HH:MM")))
}

fn build_session(code: &str, s: RawSession, h: RawHolidays) -> Result<Session, CalendarError> {
    let invalid = |msg: String| Err(CalendarError::Invalid(format!("{code}: {msg}")));
    let tz: Tz = match s.timezone.parse() {
        Ok(tz) => tz,
        Err(_) => return invalid(format!("zona horaria desconocida {:?}", s.timezone)),
    };
    let (open, close) = (
        parse_time(code, "open", &s.open)?,
        parse_time(code, "close", &s.close)?,
    );
    if open >= close {
        return invalid("open tiene que ser anterior a close".into());
    }
    if h.exchange != code {
        return invalid(format!(
            "el archivo de feriados es de {}, no de {code}",
            h.exchange
        ));
    }
    if h.from > h.to {
        return invalid("el rango de feriados está invertido".into());
    }
    let mut early_closes = BTreeMap::new();
    for (date, t) in h.early_closes {
        let t = parse_time(code, "early_close", &t)?;
        if t <= open || t >= close {
            return invalid(format!("cierre anticipado {date} {t} fuera de la sesión"));
        }
        early_closes.insert(date, t);
    }
    Ok(Session {
        exchange: code.to_owned(),
        tz,
        open,
        close,
        closed: h.closed.into_iter().collect(),
        early_closes,
        covered: (h.from, h.to),
    })
}

//! Validación de velas diarias (#103, spec §12). Igual que con las intradía, cada problema se
//! reporta y nada se corrige en silencio.
//!
//! ```text
//! start = medianoche local de `day` en la zona de los precios; end = la del día siguiente
//! días en orden estricto (sin repetir), un solo instrumento, precios > 0, OHLC coherente
//! hueco: un día con sesión en la bolsa del instrumento, entre dos velas, sin vela
//! ```

use cfds_core::{DailyCandle, InstrumentId};
use chrono::{DateTime, Days, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;

use crate::Session;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyIssue {
    pub index: usize,
    pub day: NaiveDate,
    pub kind: DailyIssueKind,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DailyIssueKind {
    #[error("la serie mezcla instrumentos: se esperaba {0}")]
    MixedInstruments(InstrumentId),
    #[error("precio {field} ≤ 0: {value}")]
    NonPositivePrice { field: &'static str, value: Decimal },
    #[error("OHLC incoherente: O {open} H {high} L {low} C {close}")]
    InconsistentOhlc {
        open: Decimal,
        high: Decimal,
        low: Decimal,
        close: Decimal,
    },
    #[error("inicio y fin no son las medianoches locales del día: se esperaba {start} → {end}")]
    WrongBounds {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
    #[error("día repetido o fuera de orden (el anterior es {previous})")]
    OutOfOrder { previous: NaiveDate },
    #[error("faltan {count} días con sesión en la bolsa; el primero es {first}")]
    MissingDays { count: usize, first: NaiveDate },
    #[error("el calendario no cubre {day}: no se puede saber si falta")]
    CalendarUnavailable { day: NaiveDate },
}

/// Medianoche local de `day` y del día siguiente, en UTC. En Europa y EE. UU. el cambio de
/// horario es de madrugada, así que la medianoche siempre existe y es única.
pub fn local_day_bounds(day: NaiveDate, tz: Tz) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let midnight = |d: NaiveDate| {
        tz.from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
            .single()
            .map(|x| x.with_timezone(&Utc))
    };
    Some((
        midnight(day)?,
        midnight(day.checked_add_days(Days::new(1))?)?,
    ))
}

/// Valida velas diarias de **un** instrumento, en orden. `tz` es la zona de los precios y
/// `session`, la bolsa del instrumento (para saber qué días debería haber vela).
pub fn validate_daily(candles: &[DailyCandle], tz: Tz, session: &Session) -> Vec<DailyIssue> {
    let Some(first) = candles.first() else {
        return Vec::new();
    };
    let mut issues = Vec::new();
    for (i, c) in candles.iter().enumerate() {
        let mut push = |kind| {
            issues.push(DailyIssue {
                index: i,
                day: c.day,
                kind,
            })
        };
        if c.instrument_id != first.instrument_id {
            push(DailyIssueKind::MixedInstruments(
                first.instrument_id.clone(),
            ));
            continue;
        }
        for (field, value) in [
            ("open", c.open),
            ("high", c.high),
            ("low", c.low),
            ("close", c.close),
        ] {
            if value <= Decimal::ZERO {
                push(DailyIssueKind::NonPositivePrice { field, value });
            }
        }
        if c.high < c.low || c.high < c.open.max(c.close) || c.low > c.open.min(c.close) {
            push(DailyIssueKind::InconsistentOhlc {
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
            });
        }
        match local_day_bounds(c.day, tz) {
            Some((start, end)) if (start, end) == (c.start, c.end) => {}
            Some((start, end)) => push(DailyIssueKind::WrongBounds { start, end }),
            None => push(DailyIssueKind::CalendarUnavailable { day: c.day }),
        }
        let Some(prev) = i.checked_sub(1).map(|p| &candles[p]) else {
            continue;
        };
        if c.day <= prev.day {
            push(DailyIssueKind::OutOfOrder { previous: prev.day });
            continue;
        }
        let mut missing = Vec::new();
        let mut unknown = None;
        for d in prev.day.iter_days().skip(1).take_while(|d| *d < c.day) {
            match session.hours(d) {
                Some(Some(_)) => missing.push(d),
                Some(None) => {}
                None => {
                    unknown = Some(d);
                    break;
                }
            }
        }
        if let Some(day) = unknown {
            push(DailyIssueKind::CalendarUnavailable { day });
        } else if let Some(&first) = missing.first() {
            push(DailyIssueKind::MissingDays {
                count: missing.len(),
                first,
            });
        }
    }
    issues
}

//! Validación de velas diarias (#103, spec §12). Igual que con las intradía, cada problema se
//! reporta y nada se corrige en silencio.
//!
//! ```text
//! start ∈ [medianoche local de `day`, esa medianoche + tolerancia]   (zona de los precios)
//! end   = máx(medianoche local del día siguiente, start + 24 h)
//! días en orden estricto (sin repetir), un solo instrumento, precios > 0, OHLC coherente
//! hueco: un día con sesión en la bolsa del instrumento, entre dos velas, sin vela
//! ```

use cfds_core::{DailyCandle, InstrumentId};
use chrono::{DateTime, Days, NaiveDate, TimeDelta, TimeZone, Utc};
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
    #[error(
        "límites del día incorrectos: el inicio tiene que estar entre {earliest} y {latest}, y el fin ser {end}"
    )]
    WrongBounds {
        earliest: DateTime<Utc>,
        latest: DateTime<Utc>,
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

/// Día local y fin de una vela diaria que empieza en `start`. El inicio puede llegar hasta
/// `tolerance` después de la medianoche local: medido en IG, en la semana siguiente al cambio de
/// horario de marzo de 2022 las velas siguieron empezando a la hora de invierno. El fin es el
/// más tardío entre la medianoche siguiente y `start + 24 h`, así una vela nunca se da por
/// cerrada antes de tiempo. `None` si `start` no cae en esa ventana.
pub fn daily_bounds(
    start: DateTime<Utc>,
    tz: Tz,
    tolerance: TimeDelta,
) -> Option<(NaiveDate, DateTime<Utc>)> {
    let day = start.with_timezone(&tz).date_naive();
    let (midnight, next) = local_day_bounds(day, tz)?;
    (midnight <= start && start <= midnight + tolerance)
        .then(|| (day, next.max(start + TimeDelta::hours(24))))
}

/// Valida velas diarias de **un** instrumento, en orden. `tz` es la zona de los precios,
/// `tolerance` la demora máxima del inicio (ver [`daily_bounds`]) y `session`, la bolsa del
/// instrumento (para saber qué días debería haber vela).
pub fn validate_daily(
    candles: &[DailyCandle],
    tz: Tz,
    tolerance: TimeDelta,
    session: &Session,
) -> Vec<DailyIssue> {
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
        match (
            daily_bounds(c.start, tz, tolerance),
            local_day_bounds(c.day, tz),
        ) {
            (Some((day, end)), _) if day == c.day && end == c.end => {}
            (_, Some((midnight, next))) => push(DailyIssueKind::WrongBounds {
                earliest: midnight,
                latest: midnight + tolerance,
                end: next.max(c.start + TimeDelta::hours(24)),
            }),
            (_, None) => push(DailyIssueKind::CalendarUnavailable { day: c.day }),
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

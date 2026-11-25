//! H2 del programa Nasdaq (#526): días de CPI y empleo del BLS (Savor y Wilson, 2013), con la
//! regla congelada en `docs/analysis/2026-11-programa-nasdaq-preregistro.md` §2 y §4.
//!
//! ```text
//! D        = fecha (hora de Nueva York) de `scheduled_at` de la fila vigente
//! solo     = días con sesión de contado de XNYS y datos de las 08:30 ET
//! entrada  = apertura de la vela de las 08:00 ET de D (decisión al cierre de la de 07:45: t + 1)
//! salida   = apertura de la última vela de 15 m de la sesión de contado de D
//!            (15:45 ET un día normal; 12:45 ET un cierre temprano)
//! vale     = la última fila del mismo dato con published_at ≤ decisión de entrada,
//!            si es `scheduled` o `rescheduled` y es esta misma fila (`cancelled`: no se opera)
//! CPI y empleo el mismo día: se opera una sola vez
//! ```
//! Función pura: misma entrada, misma salida. Los retornos y costos los calcula
//! `managed_returns_with_spreads` (la entrada fuera de sesión paga el spread de fuera de sesión).

use cfds_core::Candle;
use cfds_market_data::Session;
use chrono::{NaiveDate, NaiveTime, TimeDelta};
use chrono_tz::Tz;

use super::pre_fomc::{
    CalendarRow, EventOutcome, EventWindow, ReleaseStatus, Skip, decision_before, local,
};

/// Horas de la regla (`config/research.toml`, `[macro_day]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroDayRule {
    pub timezone: Tz,
    /// Hora local de la vela de la entrada el día del dato.
    pub entry_time: NaiveTime,
    /// Hora local de los datos que se operan (los de otra hora no se operan).
    pub release_time: NaiveTime,
}

/// Ventanas de H2 de cada fila del calendario (en orden de `scheduled_at`; las canceladas al
/// final), con la razón de las que no se operan. `rows` trae las filas de las dos series; la
/// clave (`key`) identifica el dato (serie y período de referencia).
pub fn macro_day_windows(
    m15: &[Candle],
    session: &Session,
    rows: &[CalendarRow],
    rule: &MacroDayRule,
) -> Vec<EventOutcome> {
    let mut order: Vec<&CalendarRow> = rows.iter().collect();
    order.sort_by(|a, b| {
        (
            a.scheduled_at.is_none(),
            a.scheduled_at,
            a.published_at,
            &a.key,
        )
            .cmp(&(
                b.scheduled_at.is_none(),
                b.scheduled_at,
                b.published_at,
                &b.key,
            ))
    });
    let mut out = Vec::with_capacity(order.len());
    let mut last: Option<(NaiveDate, EventWindow)> = None;
    for row in order {
        let window = one_window(m15, session, rows, row, rule).and_then(|(day, w)| match last {
            Some((d, _)) if d == day => Err(Skip::SameDay),
            Some((_, prev)) if w.decision_in < prev.exit() => Err(Skip::Overlap),
            _ => Ok((day, w)),
        });
        if let Ok(dw) = window {
            last = Some(dw);
        }
        out.push(EventOutcome {
            row: row.clone(),
            window: window.map(|(_, w)| w),
        });
    }
    out
}

fn one_window(
    m15: &[Candle],
    session: &Session,
    rows: &[CalendarRow],
    row: &CalendarRow,
    rule: &MacroDayRule,
) -> Result<(NaiveDate, EventWindow), Skip> {
    let scheduled = match (row.status, row.scheduled_at) {
        (ReleaseStatus::Unscheduled, _) => return Err(Skip::Unscheduled),
        (ReleaseStatus::Cancelled, _) | (_, None) => return Err(Skip::Cancelled),
        (_, Some(t)) => t,
    };
    let local_at = scheduled.with_timezone(&rule.timezone);
    let day = local_at.date_naive();
    let (_, close) = match session.hours(day) {
        None => return Err(Skip::OutsideCalendar),
        Some(None) => return Err(Skip::NoCashSession),
        Some(Some(h)) => h,
    };
    if local_at.time() != rule.release_time {
        return Err(Skip::OtherTime);
    }
    let entry_at = local(rule.timezone, day, rule.entry_time).ok_or(Skip::OutsideCalendar)?;
    let decision_in = decision_before(m15, entry_at).ok_or(Skip::MissingEntryCandle)?;
    let decision_out =
        decision_before(m15, close - TimeDelta::minutes(15)).ok_or(Skip::MissingExitCandle)?;
    if decision_out <= decision_in {
        return Err(Skip::ExitBeforeEntry);
    }
    // La fila que valía en la decisión de entrada (su cierre = la apertura de la entrada).
    let decided_at = m15[decision_in].end();
    let current = rows
        .iter()
        .filter(|r| r.key == row.key && r.published_at <= decided_at)
        .max_by_key(|r| r.published_at)
        .ok_or(Skip::NotPublished)?;
    if current != row {
        return Err(Skip::Superseded);
    }
    Ok((
        day,
        EventWindow {
            decision_in,
            decision_out,
        },
    ))
}

//! H1 del programa Nasdaq (#526): deriva antes del comunicado programado del FOMC (Lucca y
//! Moench, 2015), con la regla congelada en `docs/analysis/2026-11-programa-nasdaq-preregistro.md`
//! §2 y §3.
//!
//! ```text
//! D        = fecha (hora de Nueva York) de `scheduled_at` del comunicado
//! entrada  = apertura de la vela de las 14:00 ET del día hábil anterior a D
//!            (decisión al cierre de la vela anterior: t + 1)
//! salida   = apertura de la vela que empieza 30 min antes de `scheduled_at` (piso a 15 min)
//! vale     = la última fila del mismo comunicado con published_at ≤ decisión de entrada,
//!            si es `scheduled` o `rescheduled` y es esta misma fila
//! no programados (`unscheduled`): se ignoran (no se operan y no anulan a una programada)
//! ```
//! Función pura: misma entrada, misma salida. Los retornos y costos los calcula
//! `managed_returns` con el calendario de pesos que sale de acá (1 en cada ventana, 0 fuera).

use cfds_core::Candle;
use cfds_market_data::Session;
use chrono::{DateTime, NaiveTime, TimeDelta, TimeZone, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;

use super::buy_and_hold::in_session;
use super::index_at;

/// Estado de una fila de `official_release.v1` (ADR 0016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseStatus {
    Scheduled,
    Rescheduled,
    Cancelled,
    Unscheduled,
}

impl ReleaseStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "scheduled" => Some(Self::Scheduled),
            "rescheduled" => Some(Self::Rescheduled),
            "cancelled" => Some(Self::Cancelled),
            "unscheduled" => Some(Self::Unscheduled),
            _ => None,
        }
    }
}

/// Una fila del calendario oficial (`official_releases`), con lo que usa la regla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarRow {
    /// Identifica el comunicado: serie y período de referencia (la fecha de la reunión).
    pub key: String,
    pub status: ReleaseStatus,
    /// `None` solo si se canceló.
    pub scheduled_at: Option<DateTime<Utc>>,
    pub published_at: DateTime<Utc>,
}

/// Horas de la regla (`config/research.toml`, `[pre_fomc]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreFomcConfig {
    pub timezone: Tz,
    /// Hora local de la vela de la entrada, el día hábil anterior al comunicado.
    pub entry_time: NaiveTime,
    /// Minutos antes de la hora del comunicado en que empieza la vela de la salida.
    pub exit_minutes_before: u32,
}

/// Por qué un comunicado no se opera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// No programado: no se sabía antes (§2.4).
    Unscheduled,
    /// La fila es una cancelación.
    Cancelled,
    /// La fila todavía no estaba publicada en la decisión.
    NotPublished,
    /// En la decisión valía otra fila del mismo comunicado (más nueva).
    Superseded,
    /// La fecha queda fuera del calendario de feriados.
    OutsideCalendar,
    /// El día hábil anterior no tiene la vela de la entrada dentro de la sesión (cierre temprano).
    NoEntryInSession,
    /// Falta la vela de la decisión o de la ejecución, o no son consecutivas (entrada).
    MissingEntryCandle,
    /// Lo mismo para la salida.
    MissingExitCandle,
    /// La salida no queda después de la entrada.
    ExitBeforeEntry,
    /// Se superpone con la ventana del comunicado anterior operado.
    Overlap,
}

/// Ventana operable: decisiones (al cierre) de la entrada y de la salida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventWindow {
    pub decision_in: usize,
    pub decision_out: usize,
}

impl EventWindow {
    /// Vela de la entrada (apertura) = la siguiente a la decisión.
    pub fn entry(&self) -> usize {
        self.decision_in + 1
    }

    /// Vela de la salida (apertura) = la siguiente a la decisión.
    pub fn exit(&self) -> usize {
        self.decision_out + 1
    }
}

/// Resultado de un comunicado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventOutcome {
    pub row: CalendarRow,
    pub window: Result<EventWindow, Skip>,
}

/// La vela de la decisión que ejecuta en la apertura de `start`: tienen que existir las dos y
/// ser consecutivas.
fn decision_before(m15: &[Candle], start: DateTime<Utc>) -> Option<usize> {
    let i = index_at(m15, start)?;
    let d = i.checked_sub(1)?;
    (m15[d].end() == start).then_some(d)
}

/// Inicio en UTC de la hora local `t` del día `day` en `tz`.
fn local(tz: Tz, day: chrono::NaiveDate, t: NaiveTime) -> Option<DateTime<Utc>> {
    tz.from_local_datetime(&day.and_time(t))
        .single()
        .map(|d| d.with_timezone(&Utc))
}

/// Piso de `t` a un múltiplo de 15 minutos (UTC; Nueva York tiene offsets de horas enteras).
fn floor_15(t: DateTime<Utc>) -> DateTime<Utc> {
    let secs = t.timestamp();
    let floored = secs - secs.rem_euclid(15 * 60);
    DateTime::from_timestamp(floored, 0).unwrap_or(t)
}

/// Ventanas de H1 de cada fila del calendario (en orden de `scheduled_at`; las canceladas al
/// final), con la razón de las que no se operan.
pub fn pre_fomc_windows(
    m15: &[Candle],
    session: &Session,
    rows: &[CalendarRow],
    cfg: &PreFomcConfig,
) -> Vec<EventOutcome> {
    let mut order: Vec<&CalendarRow> = rows.iter().collect();
    order.sort_by_key(|r| (r.scheduled_at.is_none(), r.scheduled_at, r.published_at));
    let mut out = Vec::with_capacity(order.len());
    let mut last_exit: Option<usize> = None;
    for row in order {
        let window = one_window(m15, session, rows, row, cfg).and_then(|w| {
            if last_exit.is_some_and(|x| w.decision_in < x) {
                Err(Skip::Overlap)
            } else {
                Ok(w)
            }
        });
        if let Ok(w) = window {
            last_exit = Some(w.exit());
        }
        out.push(EventOutcome {
            row: row.clone(),
            window,
        });
    }
    out
}

fn one_window(
    m15: &[Candle],
    session: &Session,
    rows: &[CalendarRow],
    row: &CalendarRow,
    cfg: &PreFomcConfig,
) -> Result<EventWindow, Skip> {
    let scheduled = match (row.status, row.scheduled_at) {
        (ReleaseStatus::Unscheduled, _) => return Err(Skip::Unscheduled),
        (ReleaseStatus::Cancelled, _) | (_, None) => return Err(Skip::Cancelled),
        (_, Some(t)) => t,
    };
    let day = scheduled.with_timezone(&cfg.timezone).date_naive();
    // Día hábil anterior (a lo sumo dos semanas atrás).
    let mut prev = day;
    let (prev_day, (_, prev_close)) = loop {
        prev = prev.pred_opt().ok_or(Skip::OutsideCalendar)?;
        if day - prev > TimeDelta::days(14) {
            return Err(Skip::OutsideCalendar);
        }
        match session.hours(prev) {
            None => return Err(Skip::OutsideCalendar),
            Some(None) => {}
            Some(Some(h)) => break (prev, h),
        }
    };
    let entry_at = local(cfg.timezone, prev_day, cfg.entry_time).ok_or(Skip::OutsideCalendar)?;
    if entry_at + TimeDelta::minutes(15) > prev_close {
        return Err(Skip::NoEntryInSession);
    }
    let decision_in = decision_before(m15, entry_at).ok_or(Skip::MissingEntryCandle)?;
    if !in_session(session, &m15[decision_in + 1]) {
        return Err(Skip::NoEntryInSession);
    }
    let exit_at = floor_15(scheduled - TimeDelta::minutes(i64::from(cfg.exit_minutes_before)));
    let decision_out = decision_before(m15, exit_at).ok_or(Skip::MissingExitCandle)?;
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
    Ok(EventWindow {
        decision_in,
        decision_out,
    })
}

/// Calendario de pesos para `managed_returns`: 0 desde la primera vela, 1 al decidir cada
/// entrada y 0 al decidir cada salida.
pub fn event_schedule(windows: &[EventWindow]) -> Vec<(usize, Decimal)> {
    let mut s = vec![(0, Decimal::ZERO)];
    for w in windows {
        s.push((w.decision_in, Decimal::ONE));
        s.push((w.decision_out, Decimal::ZERO));
    }
    s
}

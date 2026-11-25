//! Familia D1: cambio de mes (Ariel, 1987; Lakonishok y Smidt, 1988; McConnell y Xu, 2008), con
//! la definición de #466 §3.D y el detalle del pre-registro §2.4.
//!
//! ```text
//! cada mes con sesión:
//!   decisión al cierre de la anteúltima vela de 15m del último día hábil del mes
//!     (si no está permitida, la primera decisión permitida siguiente dentro de la ventana)
//!   largo en la apertura de la vela siguiente; stop = referencia − stop_atr × ATR diario
//!     (ATR de las diarias de contado de los días ANTERIORES a la decisión)
//!   sale al cierre de la sesión del día hábil `exit_business_day` del mes nuevo
//!   un cierre forzado (fin de semana, vencimiento) parte la ventana: se reabre en la primera
//!     decisión permitida siguiente, con su propio stop
//! ```

use cfds_core::Candle;
use cfds_indicators::{Hlc, wilder_atr};
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{Datelike, Months, NaiveDate, TimeDelta};
use rust_decimal::Decimal;

use super::{Plan, TurnOfMonthConfig, legs};

/// Diaria de la sesión de contado: las velas de 15m que caben enteras en la sesión de `day`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBar {
    pub day: NaiveDate,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
}

impl Hlc for SessionBar {
    fn high(&self) -> Decimal {
        self.high
    }
    fn low(&self) -> Decimal {
        self.low
    }
    fn close(&self) -> Decimal {
        self.close
    }
}

/// Una diaria por día con sesión y con velas dentro de ella, en orden. La diaria de un día
/// existe recién cuando cerró su sesión; quien la usa para decidir tiene que mirar solo los
/// días anteriores.
pub fn session_bars(m15: &[Candle], session: &Session) -> Vec<SessionBar> {
    let mut out: Vec<SessionBar> = Vec::new();
    for c in m15 {
        let day = session.trading_day(c.start);
        let Some(Some((open, close))) = session.hours(day) else {
            continue;
        };
        if c.start < open || c.end() > close {
            continue;
        }
        match out.last_mut() {
            Some(b) if b.day == day => {
                b.high = b.high.max(c.high);
                b.low = b.low.min(c.low);
                b.close = c.close;
            }
            _ => out.push(SessionBar {
                day,
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
            }),
        }
    }
    out
}

/// ATR de la última diaria cerrada antes del día `day` (`atr` alineado con `bars`).
pub(crate) fn atr_before(
    bars: &[SessionBar],
    atr: &[Option<Decimal>],
    day: NaiveDate,
) -> Option<Decimal> {
    let i = bars.partition_point(|b| b.day < day);
    atr.get(i.checked_sub(1)?).copied().flatten()
}

/// Días con sesión del mes de `day`, en orden. `None` si el calendario no cubre el mes.
pub(crate) fn business_days(session: &Session, year: i32, month: u32) -> Option<Vec<NaiveDate>> {
    let mut out = Vec::new();
    let mut d = NaiveDate::from_ymd_opt(year, month, 1)?;
    while d.month() == month {
        if session.hours(d)?.is_some() {
            out.push(d);
        }
        d = d.succ_opt()?;
    }
    Some(out)
}

/// Operaciones de la familia D1 sobre `m15`. `entries` y `force_close` son las marcas de la
/// preparación. Función pura: cada decisión usa solo velas cerradas y diarias de días
/// anteriores; las horas de entrada y salida salen del calendario (conocido de antemano).
pub fn turn_of_month_plans(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    force_close: &[bool],
    cfg: &TurnOfMonthConfig,
) -> Vec<Plan> {
    let (Some(first), Some(last)) = (m15.first(), m15.last()) else {
        return Vec::new();
    };
    let bars = session_bars(m15, session);
    let atr = wilder_atr(&bars, cfg.atr_period);
    let atr_before = |day: NaiveDate| atr_before(&bars, &atr, day);
    let start_month = session.trading_day(first.start).with_day(1);
    let end_day = session.trading_day(last.start);
    let mut plans = Vec::new();
    let mut month = start_month;
    let mut event = 0usize;
    while let Some(m) = month.filter(|m| *m <= end_day) {
        month = m.checked_add_months(Months::new(1));
        let Some(next) = month else { break };
        let (Some(this_days), Some(next_days)) = (
            business_days(session, m.year(), m.month()),
            business_days(session, next.year(), next.month()),
        ) else {
            continue;
        };
        let n = usize::try_from(cfg.exit_business_day).unwrap_or(usize::MAX);
        let (Some(&last_day), Some(&exit_day)) = (this_days.last(), next_days.get(n - 1)) else {
            continue;
        };
        event += 1;
        plans.extend(event_plans(
            m15,
            session,
            entries,
            force_close,
            cfg,
            &atr_before,
            (last_day, exit_day),
            event,
        ));
    }
    plans
}

#[allow(clippy::too_many_arguments)]
fn event_plans(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    force_close: &[bool],
    cfg: &TurnOfMonthConfig,
    atr_before: &dyn Fn(NaiveDate) -> Option<Decimal>,
    (last_day, exit_day): (NaiveDate, NaiveDate),
    event: usize,
) -> Vec<Plan> {
    let mut out = Vec::new();
    let (Some(Some((_, last_close))), Some(Some((exit_open, exit_close)))) =
        (session.hours(last_day), session.hours(exit_day))
    else {
        return out;
    };
    // Última vela de la sesión de salida: si no está (datos cortados), no se opera el evento.
    let Some(exit_index) = m15
        .partition_point(|c| c.start < exit_close)
        .checked_sub(1)
        .filter(|&k| m15[k].start >= exit_open && m15[k].end() <= exit_close)
    else {
        return out;
    };
    // La decisión de la ventana es al cierre de la anteúltima vela del último día hábil.
    let from = m15.partition_point(|c| c.start < last_close - TimeDelta::minutes(30));
    for (t, end) in legs(m15, entries, force_close, from, exit_index) {
        let reference = m15[t].close;
        if let Some(a) = atr_before(session.trading_day(m15[t].start)) {
            out.push(Plan {
                event,
                signal_index: t,
                side: Side::Long,
                reference,
                stop: reference - cfg.stop_atr * a,
                risk_atr: a,
                exit_index: end,
                target_r: None,
            });
        }
    }
    out
}

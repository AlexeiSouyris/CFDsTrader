//! Familia B: momentum intradía del mercado, primera media hora → última media hora (Gao, Han,
//! Li y Zhou, 2018; Baltussen, Da, Lammers y Martens, 2021), con la definición de #466 §3.B y
//! el detalle del pre-registro de la ronda 2 §2.3.
//!
//! ```text
//! cada día hábil d (horarios de la bolsa, America/New_York):
//!   señal = signo del retorno desde el cierre de la sesión del día hábil anterior hasta el
//!           cierre de la vela que termina `signal_minutes_after_open` después de la apertura,
//!           encadenado en los vencimientos (si falta una de las dos velas, no se opera)
//!   decisión al cierre de la vela que termina `decision_minutes_before_close` antes del cierre
//!     (tiene que ser una decisión permitida: el último día de la semana no lo es)
//!   entrada en la apertura de la siguiente; salida al cierre de la vela que termina en el cierre
//!   stop = referencia ∓ stop_atr × ATR de 15m de la vela de la decisión
//! ```

use cfds_core::Candle;
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::Decimal;

use super::orb::candle_days;
use super::{IntradayMomentumConfig, Plan, chained_return, decision_allowed, index_at};

/// Día hábil anterior a `day` (con sesión), mirando hasta dos semanas hacia atrás.
fn previous_business_day(session: &Session, day: NaiveDate) -> Option<NaiveDate> {
    let mut d = day;
    for _ in 0..14 {
        d = d.pred_opt()?;
        if session.hours(d)?.is_some() {
            return Some(d);
        }
    }
    None
}

/// Operaciones de la familia B sobre `m15`. `entries` son las marcas de entrada de la
/// preparación, `atr` el ATR de 15m alineado con `m15` y `rolls` los inicios de la primera vela
/// de cada contrato nuevo. Función pura: la señal de un día solo usa velas cerradas hasta la
/// decisión.
pub fn intraday_momentum_plans(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    atr: &[Option<Decimal>],
    rolls: &[DateTime<Utc>],
    cfg: &IntradayMomentumConfig,
) -> Vec<Plan> {
    candle_days(m15, session)
        .into_iter()
        .enumerate()
        .filter_map(|(event, day)| momentum_day(m15, session, entries, atr, rolls, cfg, day, event))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn momentum_day(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    atr: &[Option<Decimal>],
    rolls: &[DateTime<Utc>],
    cfg: &IntradayMomentumConfig,
    day: NaiveDate,
    event: usize,
) -> Option<Plan> {
    let (open, close) = session.hours(day)??;
    let (_, previous_close) = session.hours(previous_business_day(session, day)?)??;
    let step = TimeDelta::minutes(15);
    let ending_at = |t: DateTime<Utc>| index_at(m15, t - step);
    let previous = ending_at(previous_close)?;
    let signal_end = open + TimeDelta::minutes(i64::from(cfg.signal_minutes_after_open));
    let signal = ending_at(signal_end)?;
    let decision =
        ending_at(close - TimeDelta::minutes(i64::from(cfg.decision_minutes_before_close)))?;
    let exit_index = ending_at(close)?;
    if decision <= signal || exit_index <= decision || !decision_allowed(m15, entries, decision) {
        return None;
    }
    let ret = chained_return(m15, rolls, previous, signal)?;
    let side = if ret > Decimal::ZERO {
        Side::Long
    } else if ret < Decimal::ZERO {
        Side::Short
    } else {
        return None;
    };
    let a = atr.get(decision).copied().flatten()?;
    let reference = m15[decision].close;
    Some(Plan {
        event,
        signal_index: decision,
        side,
        reference,
        stop: match side {
            Side::Long => reference - cfg.stop_atr * a,
            Side::Short => reference + cfg.stop_atr * a,
        },
        risk_atr: a,
        exit_index,
        target_r: None,
        target_cap: None,
    })
}

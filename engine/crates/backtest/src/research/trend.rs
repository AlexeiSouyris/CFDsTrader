//! Familia A: tendencia mensual, time-series momentum (Moskowitz, Ooi y Pedersen, 2012; Hurst,
//! Ooi y Pedersen, 2017), con la definición de #466 §3.A y el detalle del pre-registro de la
//! ronda 2 §2.2.
//!
//! ```text
//! cada mes con sesión:
//!   revisión = primera decisión permitida desde el cierre de la anteúltima vela de 15m del
//!              último día hábil del mes
//!   señal = signo del retorno desde la revisión de `lookback_months` meses antes hasta esta,
//!           dentro de cada contrato y encadenado en los vencimientos (sin historia: nada)
//!   patas hasta la revisión siguiente: un cierre forzado (fin de semana, vencimiento) cierra y
//!     se reabre en la primera decisión permitida siguiente, en la misma dirección
//!   stop = referencia ∓ stop_atr × ATR diario (diarias de contado de los días ANTERIORES)
//!   en cada revisión se cierra y se abre de nuevo (aunque el signo no cambie)
//! ```

use cfds_core::Candle;
use cfds_indicators::wilder_atr;
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, Months, TimeDelta, Utc};

use super::turn_of_month::{atr_before, business_days};
use super::{Plan, TrendConfig, chained_return, decision_allowed, legs, session_bars};

/// Decimales del ATR diario de la familia (ver `trend_plans`).
const ATR_DECIMALS: u32 = 8;

/// Revisión mensual de cada mes con sesión: `(año × 12 + mes − 1, índice de la decisión)`, en
/// orden. La revisión de un mes es la primera decisión permitida desde el cierre de la
/// anteúltima vela de la sesión de su último día hábil, y antes del punto de partida de la
/// revisión del mes siguiente. Un mes sin decisión permitida en ese tramo no tiene revisión.
pub fn monthly_reviews(m15: &[Candle], session: &Session, entries: &[bool]) -> Vec<(i32, usize)> {
    let (Some(first), Some(last)) = (m15.first(), m15.last()) else {
        return Vec::new();
    };
    // Punto de partida de cada mes: la vela que empieza 30 minutos antes del cierre del último
    // día hábil (la anteúltima de la sesión).
    let mut starts: Vec<(i32, DateTime<Utc>)> = Vec::new();
    let mut month = session.trading_day(first.start).with_day(1);
    let end_day = session.trading_day(last.start);
    while let Some(m) = month.filter(|m| *m <= end_day) {
        month = m.checked_add_months(Months::new(1));
        let Some(days) = business_days(session, m.year(), m.month()) else {
            continue;
        };
        let Some(Some((_, close))) = days.last().and_then(|d| session.hours(*d)) else {
            continue;
        };
        let key = m.year() * 12 + i32::try_from(m.month0()).unwrap_or(0);
        starts.push((key, close - TimeDelta::minutes(30)));
    }
    let mut out = Vec::new();
    for (k, &(key, start)) in starts.iter().enumerate() {
        let from = m15.partition_point(|c| c.start < start);
        let until = starts
            .get(k + 1)
            .map_or(m15.len(), |&(_, s)| m15.partition_point(|c| c.start < s));
        if let Some(t) = (from..until).find(|&i| decision_allowed(m15, entries, i)) {
            out.push((key, t));
        }
    }
    out
}

/// Operaciones de la familia A sobre `m15`. `entries` y `force_close` son las marcas de la
/// preparación; `rolls`, los inicios de la primera vela de cada contrato nuevo. Función pura:
/// cada señal usa solo velas cerradas hasta su revisión y el ATR de diarias de días anteriores.
pub fn trend_plans(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    force_close: &[bool],
    rolls: &[DateTime<Utc>],
    cfg: &TrendConfig,
) -> Vec<Plan> {
    let bars = session_bars(m15, session);
    let atr = wilder_atr(&bars, cfg.atr_period);
    let reviews = monthly_reviews(m15, session, entries);
    let lookback = i32::try_from(cfg.lookback_months).unwrap_or(i32::MAX);
    let mut plans = Vec::new();
    for (event, &(key, t)) in reviews.iter().enumerate() {
        let Some(&(_, base)) = reviews.iter().find(|(k, _)| *k == key - lookback) else {
            continue; // sin `lookback_months` meses de historia
        };
        let Some(ret) = chained_return(m15, rolls, base, t) else {
            continue;
        };
        let side = if ret > rust_decimal::Decimal::ZERO {
            Side::Long
        } else if ret < rust_decimal::Decimal::ZERO {
            Side::Short
        } else {
            continue;
        };
        let exit_index = reviews
            .get(event + 1)
            .map_or(m15.len().saturating_sub(1), |&(_, next)| next);
        for (s, end) in legs(m15, entries, force_close, t, exit_index) {
            // ATR redondeado a 8 decimales: así `referencia ∓ 3 × ATR` es exacto en Decimal y
            // el stop queda justo en el tope de 3 ATR del riesgo, no un redondeo más allá.
            let Some(a) = atr_before(&bars, &atr, session.trading_day(m15[s].start))
                .map(|a| a.round_dp(ATR_DECIMALS))
            else {
                continue;
            };
            let reference = m15[s].close;
            plans.push(Plan {
                event,
                signal_index: s,
                side,
                reference,
                stop: match side {
                    Side::Long => reference - cfg.stop_atr * a,
                    Side::Short => reference + cfg.stop_atr * a,
                },
                risk_atr: a,
                exit_index: end,
                target_r: None,
            });
        }
    }
    plans
}

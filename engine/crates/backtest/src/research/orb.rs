//! Familia C: ruptura del rango de apertura (Crabel, 1990; Holmberg, Lönnbark y Lundström,
//! 2013), con la definición de #466 §3.C y el detalle del pre-registro §2.3.
//!
//! ```text
//! cada día con sesión de contado:
//!   rango = máximo y mínimo de las primeras `range_candles` velas de 15m (si falta una, no)
//!   ruptura = primera vela siguiente cuyo CIERRE queda fuera del rango, que cierra a
//!             `last_breakout_close` (hora local) o antes y antes del cierre de la sesión
//!   largo si cierra arriba del máximo, corto si cierra abajo del mínimo; stop en el otro extremo
//!   entrada en la apertura de la vela siguiente (sin hueco de datos), si la decisión está
//!   permitida por las marcas de entrada; salida al cierre de la última vela de la sesión
//! ```
//! Una sola ruptura por día: si la primera no se puede operar, ese día no se opera.

use cfds_core::Candle;
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{NaiveDate, TimeDelta, TimeZone, Utc};
use rust_decimal::Decimal;

use super::{OrbConfig, Plan, index_at};

/// Días locales de la bolsa que tienen alguna vela, en orden.
pub(crate) fn candle_days(candles: &[Candle], session: &Session) -> Vec<NaiveDate> {
    let mut days: Vec<NaiveDate> = Vec::new();
    for c in candles {
        let d = session.trading_day(c.start);
        if days.last() != Some(&d) {
            days.push(d);
        }
    }
    days
}

/// Operaciones de la familia C sobre `m15`. `entries` son las marcas de entrada de la
/// preparación (horario, calidad de datos, vencimientos) y `atr`, el ATR de 15m del riesgo,
/// alineados con `m15`. Función pura: la señal de un día solo usa velas cerradas hasta la
/// decisión.
pub fn orb_plans(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    atr: &[Option<Decimal>],
    cfg: &OrbConfig,
) -> Vec<Plan> {
    candle_days(m15, session)
        .into_iter()
        .enumerate()
        .filter_map(|(event, day)| orb_day(m15, session, entries, atr, cfg, day, event))
        .collect()
}

fn orb_day(
    m15: &[Candle],
    session: &Session,
    entries: &[bool],
    atr: &[Option<Decimal>],
    cfg: &OrbConfig,
    day: NaiveDate,
    event: usize,
) -> Option<Plan> {
    let (open, close) = session.hours(day)??;
    let step = TimeDelta::minutes(15);
    let first = index_at(m15, open)?;
    let (mut high, mut low) = (m15[first].high, m15[first].low);
    for k in 1..cfg.range_candles {
        let c = &m15[index_at(m15, open + step * i32::try_from(k).ok()?)?];
        high = high.max(c.high);
        low = low.min(c.low);
    }
    let last_close = session
        .tz
        .from_local_datetime(&day.and_time(cfg.last_breakout_close))
        .single()?
        .with_timezone(&Utc);
    let scan_from = open + step * i32::try_from(cfg.range_candles).ok()?;
    let mut j = m15.partition_point(|c| c.start < scan_from);
    while let Some(c) = m15.get(j) {
        if c.end() > last_close || c.end() >= close {
            return None;
        }
        let side = if c.close > high {
            Side::Long
        } else if c.close < low {
            Side::Short
        } else {
            j += 1;
            continue;
        };
        if !entries.get(j).copied().unwrap_or(false) {
            return None;
        }
        if m15.get(j + 1)?.start != c.end() {
            return None;
        }
        let risk_atr = atr.get(j).copied().flatten()?;
        let exit_index = m15.partition_point(|x| x.start < close).checked_sub(1)?;
        if exit_index <= j || m15[exit_index].end() > close {
            return None;
        }
        return Some(Plan {
            event,
            signal_index: j,
            side,
            reference: c.close,
            stop: match side {
                Side::Long => low,
                Side::Short => high,
            },
            risk_atr,
            exit_index,
            target_r: None,
            target_cap: None,
        });
    }
    None
}

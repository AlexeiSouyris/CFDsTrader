//! Descriptivo sin voto de la ronda 3 (pre-registro §5): la deriva nocturna en la apertura de
//! Europa (Boyarchenko, Larsen y Whelan, 2023). Retorno de cada día hábil local desde la apertura
//! de la vela que empieza a la hora `from` hasta el cierre de la que termina a la hora `to`.

use cfds_core::Candle;
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeDelta, Utc, Weekday};
use chrono_tz::Tz;
use rust_decimal::Decimal;

/// `(día local, retorno)` de cada día de lunes a viernes con todas las velas de `[from, to)`
/// contiguas y sin un vencimiento (`rolls`) dentro. La ventana no puede cruzar la medianoche.
pub fn window_returns(
    m15: &[Candle],
    rolls: &[DateTime<Utc>],
    tz: Tz,
    from: NaiveTime,
    to: NaiveTime,
) -> Vec<(NaiveDate, Decimal)> {
    let mut out = Vec::new();
    if to <= from {
        return out;
    }
    let n = usize::try_from((to - from).num_minutes() / 15).unwrap_or(0);
    for (i, c) in m15.iter().enumerate() {
        let local = c.start.with_timezone(&tz);
        if local.time() != from || matches!(local.weekday(), Weekday::Sat | Weekday::Sun) {
            continue;
        }
        let Some(window) = m15.get(i..i + n) else {
            continue;
        };
        let contiguous = window
            .windows(2)
            .all(|w| w[0].end() == w[1].start && w[1].start - w[0].start == TimeDelta::minutes(15));
        let last = &window[n - 1];
        if !contiguous
            || last.end().with_timezone(&tz).time() != to
            || rolls.iter().any(|&r| c.start < r && r <= last.start)
            || c.open.is_zero()
        {
            continue;
        }
        out.push((local.date_naive(), last.close / c.open - Decimal::ONE));
    }
    out
}

//! Chequeos **descriptivos** pedidos por la revisión externa (#512): no votan, no gastan
//! presupuesto de error y no cambian ningún veredicto registrado. Solo los usa el ejemplo
//! `chequeos_revision`.
//!
//! ```text
//! caída fuerte el día d:  r_d < P, con P = el k-ésimo menor de los `lookback` retornos de
//!                         cierre a cierre de sesión anteriores, k = ⌈quantile × lookback⌉
//! deriva tras caída:      ventana nocturna del día D si el último cierre de sesión terminado
//!                         antes de su inicio fue una caída fuerte
//! cartera mezclada:       w × comprar y mantener + (1 − w) × s × V, con
//!                         s = desvío(comprar y mantener) / desvío(V) del entrenamiento
//! ```

use cfds_core::Candle;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;

use super::chained_return;
use super::vol_managed::sample_sd;

/// Retorno de cada día de cierre de sesión a cierre de sesión (encadenado en los vencimientos):
/// `(fin de la vela de cierre, retorno desde el cierre anterior)`. `closes` son los índices de
/// `session_close_indices`.
pub fn session_close_returns(
    m15: &[Candle],
    rolls: &[DateTime<Utc>],
    closes: &[usize],
) -> Vec<(DateTime<Utc>, Decimal)> {
    closes
        .windows(2)
        .filter_map(|w| Some((m15[w[1]].end(), chained_return(m15, rolls, w[0], w[1])?)))
        .collect()
}

/// Marca de caída fuerte de cada retorno: `r_d` menor que el k-ésimo menor de los `lookback`
/// anteriores, `k = ⌈quantile × lookback⌉`. `None` en el calentamiento (menos de `lookback`
/// anteriores) o si los parámetros no sirven. Usa solo días ya cerrados.
pub fn strong_drop_flags(
    returns: &[Decimal],
    lookback: usize,
    quantile: Decimal,
) -> Vec<Option<bool>> {
    let k = (quantile * Decimal::from(lookback))
        .ceil()
        .try_into()
        .unwrap_or(0usize);
    returns
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if k == 0 || k > lookback || i < lookback {
                return None;
            }
            let mut prev = returns[i - lookback..i].to_vec();
            prev.sort();
            Some(*r < prev[k - 1])
        })
        .collect()
}

/// Cada día de la ventana nocturna (`window_returns`) con la marca del último cierre de sesión
/// que terminó en o antes del inicio de la ventana (`from` en `tz`). Se descartan los días sin
/// cierre anterior o con la marca en `None` (calentamiento).
pub fn drift_after_close(
    drift: &[(NaiveDate, Decimal)],
    tz: Tz,
    from: NaiveTime,
    closes: &[(DateTime<Utc>, Option<bool>)],
) -> Vec<(NaiveDate, Decimal, bool)> {
    drift
        .iter()
        .filter_map(|&(d, r)| {
            let start = d
                .and_time(from)
                .and_local_timezone(tz)
                .single()?
                .with_timezone(&Utc);
            let n = closes.partition_point(|(t, _)| *t <= start);
            let flag = closes.get(n.checked_sub(1)?)?.1?;
            Some((d, r, flag))
        })
        .collect()
}

/// Escala `s` que lleva el desvío de los retornos mensuales de `v` al de `hold` (desvíos
/// muestrales, misma cantidad de meses). `None` con menos de 2 meses o desvío de `v` cero.
pub fn vol_match_scale(hold: &[Decimal], v: &[Decimal]) -> Option<Decimal> {
    if hold.len() != v.len() {
        return None;
    }
    let (h, s) = (sample_sd(hold)?, sample_sd(v)?);
    (!s.is_zero()).then(|| h / s)
}

/// Retorno de un mes de la cartera mezclada: `w × hold + (1 − w) × scale × v` (rebalanceo
/// mensual entre las dos partes, sin costo extra: es el mismo índice).
pub fn blend_return(hold: Decimal, v: Decimal, hold_weight: Decimal, scale: Decimal) -> Decimal {
    hold_weight * hold + (Decimal::ONE - hold_weight) * scale * v
}

//! Carga incremental de velas cerradas de IBKR (#536): qué ventanas pedir, cuántas por corrida y
//! cada cuánto, sin pasar los límites de IBKR (ver `docs/analysis/2026-11-ibkr-medicion-historia.md`).
//! Funciones puras: la conexión y el guardado los hace el ejemplo `ibkr_load`.

use chrono::{DateTime, DurationRound, TimeDelta, Utc};

use crate::config::ContractKind;
use crate::session::{ApiError, BarSize, SessionError};

/// `[load]` de `config/ibkr.toml`, ya validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadConfig {
    /// Fuente que se guarda con cada vela.
    pub source: String,
    /// Pausa entre dos pedidos de historia.
    pub pause: std::time::Duration,
    /// Tope de pedidos de velas por corrida.
    pub max_requests: usize,
    /// Si no hay nada guardado de la serie y no se pide otra fecha, desde cuántos días atrás.
    pub backfill: TimeDelta,
    pub window_1m: TimeDelta,
    pub window_15m: TimeDelta,
    pub window_1h: TimeDelta,
}

impl LoadConfig {
    /// Ventana de un pedido para ese tamaño. `None`: ese tamaño no se guarda (diarias).
    pub fn window(&self, size: BarSize) -> Option<TimeDelta> {
        match size {
            BarSize::M1 => Some(self.window_1m),
            BarSize::M15 => Some(self.window_15m),
            BarSize::H1 => Some(self.window_1h),
            BarSize::D1 => None,
        }
    }
}

/// Por qué no se carga una serie.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadRefusal {
    #[error("el futuro continuo de IBKR está ajustado por vencimiento (repinta): no se guarda")]
    ContinuousFuture,
    #[error("las diarias de IBKR vienen fechadas por sesión, sin hora: no se guardan")]
    Daily,
}

/// Se puede guardar este tipo de contrato y tamaño.
pub fn check_storable(kind: ContractKind, size: BarSize) -> Result<(), LoadRefusal> {
    if kind == ContractKind::ContinuousFuture {
        return Err(LoadRefusal::ContinuousFuture);
    }
    if size == BarSize::D1 {
        return Err(LoadRefusal::Daily);
    }
    Ok(())
}

/// Inicio de la vela en curso a `now`: las velas que empiezan antes ya cerraron.
pub fn closed_until(now: DateTime<Utc>, size: BarSize) -> DateTime<Utc> {
    now.duration_trunc(size.duration()).unwrap_or(now)
}

/// Desde dónde seguir: después de la última guardada; si no hay, desde `default_from`. Nunca
/// antes de la primera vela que tiene IBKR (`head`), redondeado al tamaño.
pub fn resume_from(
    last_stored: Option<DateTime<Utc>>,
    default_from: DateTime<Utc>,
    head: Option<DateTime<Utc>>,
    size: BarSize,
) -> DateTime<Utc> {
    let from = match last_stored {
        Some(last) => last + size.duration(),
        None => default_from,
    };
    let from = head.map_or(from, |h| from.max(h));
    from.duration_trunc(size.duration()).unwrap_or(from)
}

/// Ventanas `[desde, hasta)` consecutivas de `window` que cubren `[from, to)`, como mucho `max`.
pub fn windows(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    window: TimeDelta,
    max: usize,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut out = Vec::new();
    let mut a = from;
    while a < to && out.len() < max && window > TimeDelta::zero() {
        let b = (a + window).min(to);
        out.push((a, b));
        a = b;
    }
    out
}

/// Qué hacer con el error de un pedido de velas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    /// IBKR no tiene velas en esa ventana (fin de semana, contrato que todavía no se operaba):
    /// se sigue con la siguiente.
    Empty,
    /// Violación de ritmo (pacing): se para la corrida.
    Pacing,
    /// Cualquier otro: se para la corrida.
    Stop,
}

pub fn classify(e: &SessionError) -> OnError {
    match e {
        SessionError::Rejected(ApiError::Rejected(m)) => {
            let m = m.to_ascii_lowercase();
            if m.contains("pacing") {
                OnError::Pacing
            } else if m.contains("returned no data") {
                OnError::Empty
            } else {
                OnError::Stop
            }
        }
        _ => OnError::Stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 25, h, m, 0).unwrap()
    }

    #[test]
    fn only_closed_bars_are_planned() {
        assert_eq!(closed_until(t(10, 37), BarSize::H1), t(10, 0));
        assert_eq!(closed_until(t(10, 37), BarSize::M15), t(10, 30));
        assert_eq!(closed_until(t(10, 37), BarSize::M1), t(10, 37));
    }

    #[test]
    fn resume_after_the_last_stored_never_before_head() {
        let def = t(0, 0);
        assert_eq!(resume_from(Some(t(5, 0)), def, None, BarSize::H1), t(6, 0));
        assert_eq!(resume_from(None, def, None, BarSize::H1), def);
        assert_eq!(resume_from(None, def, Some(t(3, 20)), BarSize::H1), t(3, 0));
        assert_eq!(resume_from(None, t(2, 7), None, BarSize::M15), t(2, 0));
    }

    #[test]
    fn windows_cover_the_range_up_to_the_cap() {
        let w = windows(t(0, 0), t(5, 0), TimeDelta::hours(2), 10);
        assert_eq!(
            w,
            vec![(t(0, 0), t(2, 0)), (t(2, 0), t(4, 0)), (t(4, 0), t(5, 0))]
        );
        assert_eq!(windows(t(0, 0), t(5, 0), TimeDelta::hours(2), 1).len(), 1);
        assert!(windows(t(5, 0), t(5, 0), TimeDelta::hours(2), 10).is_empty());
    }

    #[test]
    fn continuous_and_daily_are_never_stored() {
        assert_eq!(
            check_storable(ContractKind::ContinuousFuture, BarSize::H1),
            Err(LoadRefusal::ContinuousFuture)
        );
        assert_eq!(
            check_storable(ContractKind::Future, BarSize::D1),
            Err(LoadRefusal::Daily)
        );
        assert!(check_storable(ContractKind::Stock, BarSize::M1).is_ok());
    }

    #[test]
    fn pacing_stops_and_no_data_is_empty() {
        let rej = |m: &str| SessionError::Rejected(ApiError::Rejected(m.into()));
        assert_eq!(
            classify(&rej(
                "[162] Historical Market Data Service error message:Historical data request pacing violation"
            )),
            OnError::Pacing
        );
        assert_eq!(
            classify(&rej(
                "[162] Historical Market Data Service error message:HMDS query returned no data: MNQZ6@CME Trades"
            )),
            OnError::Empty
        );
        assert_eq!(
            classify(&rej("[162] No market data permissions for IDEALPRO CASH")),
            OnError::Stop
        );
    }
}

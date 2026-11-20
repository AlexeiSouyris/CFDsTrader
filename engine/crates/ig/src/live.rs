//! Modo en vivo (#68): qué velas cerradas pedir y cuándo. Funciones puras: sin red ni reloj.
//!
//! IG no da velas por streaming a esta cuenta (las suscripciones `CHART` de la Streaming API
//! responden "Insufficient permissions"), así que cada vela se pide por REST apenas cierra,
//! con [`crate::Recent`] (1 punto del cupo por vela). Solo se piden las velas que el calendario
//! de sesiones espera: fuera de horario no se gasta nada.
//!
//! Regla (CLAUDE.md §5b): solo velas **cerradas**, y además `delay` después del cierre, para que
//! IG ya las tenga consolidadas.

use cfds_core::{InstrumentId, Timeframe};
use cfds_market_data::SessionCalendar;
use chrono::{DateTime, TimeDelta, Utc};

/// Rango `[from, to)` de velas cerradas que toca pedir ahora, o `None` si no hay ninguna.
///
/// - `last_start`: inicio de la última vela ya guardada; sin ella se arranca `lookback` atrás.
/// - Una vela está lista cuando `start + timeframe + delay <= now`.
/// - `None` también si el calendario no cubre esas fechas: nunca se supone "cerrado".
pub fn due_range(
    calendar: &impl SessionCalendar,
    instrument: &InstrumentId,
    tf: Timeframe,
    last_start: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    delay: TimeDelta,
    lookback: TimeDelta,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let from = last_start.map_or(now - lookback, |s| s + tf.duration());
    let ready_until = now - delay;
    let starts = calendar.expected_starts(instrument, tf, from, ready_until)?;
    let ready: Vec<_> = starts
        .into_iter()
        .filter(|s| *s + tf.duration() <= ready_until)
        .collect();
    Some((*ready.first()?, *ready.last()? + tf.duration()))
}

/// Próximo momento en que una vela de la sesión queda lista (`cierre + delay`) después de
/// `now`, mirando hasta `horizon` adelante. `None` si no hay ninguna en ese plazo.
pub fn next_wake(
    calendar: &impl SessionCalendar,
    instrument: &InstrumentId,
    tf: Timeframe,
    now: DateTime<Utc>,
    delay: TimeDelta,
    horizon: TimeDelta,
) -> Option<DateTime<Utc>> {
    calendar
        .expected_starts(instrument, tf, now - tf.duration() - delay, now + horizon)?
        .into_iter()
        .map(|s| s + tf.duration() + delay)
        .find(|ready| *ready > now)
}

/// Kill switch (CLAUDE.md §5b): `true` si una vela que cerró en `closed_at` todavía no se pudo
/// guardar y ya pasó `give_up`. El modo en vivo se detiene en vez de seguir sin datos.
pub fn overdue(closed_at: DateTime<Utc>, now: DateTime<Utc>, give_up: TimeDelta) -> bool {
    now - closed_at > give_up
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use cfds_market_data::ExchangeCalendar;

    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn us100() -> InstrumentId {
        "US100".parse().unwrap()
    }

    fn calendar() -> ExchangeCalendar {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
        ExchangeCalendar::load(&dir, HashMap::from([(us100(), "XNYS".to_string())])).unwrap()
    }

    const DELAY: TimeDelta = TimeDelta::seconds(20);
    const DAY: TimeDelta = TimeDelta::hours(24);

    fn due(tf: Timeframe, last: Option<&str>, now: &str) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        due_range(&calendar(), &us100(), tf, last.map(at), at(now), DELAY, DAY)
    }

    #[test]
    fn on_startup_it_fills_the_session_within_the_lookback() {
        // Viernes 20-11-2026: Nueva York abre 14:30 y cierra 21:00 UTC.
        assert_eq!(
            due(Timeframe::M15, None, "2026-11-20T21:12:00Z"),
            Some((at("2026-11-20T14:30:00Z"), at("2026-11-20T21:00:00Z")))
        );
        // En 1h solo cuentan las velas enteras dentro de la sesión: de 15:00 a 21:00.
        assert_eq!(
            due(Timeframe::H1, None, "2026-11-20T21:12:00Z"),
            Some((at("2026-11-20T15:00:00Z"), at("2026-11-20T21:00:00Z")))
        );
    }

    #[test]
    fn a_candle_is_requested_only_after_close_plus_delay() {
        let last = Some("2026-11-20T14:30:00Z");
        // La vela de las 14:45 cierra 15:00; con 20 s de espera, está lista a las 15:00:20.
        assert_eq!(due(Timeframe::M15, last, "2026-11-20T15:00:19Z"), None);
        assert_eq!(
            due(Timeframe::M15, last, "2026-11-20T15:00:20Z"),
            Some((at("2026-11-20T14:45:00Z"), at("2026-11-20T15:00:00Z")))
        );
        // Al día: nada que pedir.
        assert_eq!(
            due(
                Timeframe::M15,
                Some("2026-11-20T20:45:00Z"),
                "2026-11-20T21:30:00Z"
            ),
            None
        );
    }

    #[test]
    fn nothing_is_requested_outside_the_session_or_the_calendar() {
        // Sábado: la última vela guardada es la del viernes 20:45.
        assert_eq!(
            due(
                Timeframe::M15,
                Some("2026-11-20T20:45:00Z"),
                "2026-11-21T12:00:00Z"
            ),
            None
        );
        // Fechas que el archivo de feriados no cubre: no se supone "cerrado", no se pide.
        assert_eq!(due(Timeframe::M15, None, "2040-01-10T18:00:00Z"), None);
    }

    #[test]
    fn next_wake_is_the_next_close_plus_delay() {
        let wake = |now: &str, tf| {
            next_wake(
                &calendar(),
                &us100(),
                tf,
                at(now),
                DELAY,
                TimeDelta::days(5),
            )
        };
        assert_eq!(
            wake("2026-11-20T15:00:20Z", Timeframe::M15),
            Some(at("2026-11-20T15:15:20Z"))
        );
        assert_eq!(
            wake("2026-11-20T15:00:19Z", Timeframe::M15),
            Some(at("2026-11-20T15:00:20Z"))
        );
        // Después del cierre del viernes, la próxima es el lunes 23: abre 14:30, cierra 14:45.
        assert_eq!(
            wake("2026-11-20T21:30:00Z", Timeframe::M15),
            Some(at("2026-11-23T14:45:20Z"))
        );
        assert_eq!(
            wake("2026-11-20T21:30:00Z", Timeframe::H1),
            Some(at("2026-11-23T16:00:20Z"))
        );
        assert_eq!(
            next_wake(
                &calendar(),
                &us100(),
                Timeframe::M15,
                at("2026-11-20T21:30:00Z"),
                DELAY,
                TimeDelta::days(1)
            ),
            None
        );
    }

    #[test]
    fn overdue_only_after_give_up() {
        let closed = at("2026-11-20T15:00:00Z");
        let give_up = TimeDelta::minutes(10);
        assert!(!overdue(closed, at("2026-11-20T15:10:00Z"), give_up));
        assert!(overdue(closed, at("2026-11-20T15:10:01Z"), give_up));
    }
}

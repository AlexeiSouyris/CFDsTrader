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

use crate::budget::{Usage, WeekSpend, check};
use crate::config::UsageBudget;

/// Código de salida de `ig_live` cuando se pone en pausa por falta de cupo (#360). Es distinto
/// del kill switch por datos (código 1): no hay un error de datos, solo no se puede pedir nada
/// hasta que IG renueve el cupo.
pub const PAUSED_EXIT_CODE: i32 = 3;

/// Rango `[from, to)` de velas cerradas que toca pedir ahora, o `None` si no hay ninguna.
///
/// - `last_start`: inicio de la última vela ya guardada; sin ella se arranca `lookback` atrás.
///   Tampoco se va más atrás que `lookback` aunque la última guardada sea más vieja (#124): los
///   huecos viejos se completan con `ig_backfill`, que mira la base y pide solo lo que falta.
///   Así una corrida no queda trabada pidiendo un rango que el presupuesto nunca deja pasar.
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
    let floor = now - lookback;
    let from = last_start.map_or(floor, |s| (s + tf.duration()).max(floor));
    let ready_until = now - delay;
    let starts = calendar.expected_starts(instrument, tf, from, ready_until)?;
    let ready: Vec<_> = starts
        .into_iter()
        .filter(|s| *s + tf.duration() <= ready_until)
        .collect();
    Some((*ready.first()?, *ready.last()? + tf.duration()))
}

/// Puntos del cupo que cuesta pedir `[from, to)` con [`crate::Recent`] (sin margen: 1 por
/// vela). Es la misma cuenta que hace el proveedor antes de cada pedido.
pub fn due_points(from: DateTime<Utc>, to: DateTime<Utc>, tf: Timeframe) -> u64 {
    if to <= from {
        return 0;
    }
    crate::model::estimated_points(from, to - TimeDelta::seconds(1), tf, TimeDelta::zero())
}

/// Puntos que cuesta mantener al día `instruments` × `timeframes` en `[from, to)`: una vela
/// por cada inicio que el calendario espera y que cierra antes de `to` (#124). `None` si el
/// calendario no cubre el rango. Sirve para comparar con `budget.live`.
pub fn session_points(
    calendar: &impl SessionCalendar,
    instruments: &[InstrumentId],
    timeframes: &[Timeframe],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Option<u64> {
    let mut total = 0u64;
    for id in instruments {
        for &tf in timeframes {
            let starts = calendar.expected_starts(id, tf, from, to)?;
            let closed = starts.iter().filter(|s| **s + tf.duration() <= to).count();
            total = total.saturating_add(u64::try_from(closed).unwrap_or(u64::MAX));
        }
    }
    Some(total)
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

/// Motivo para no pedir nada en este ciclo si **ni una vela** (1 punto) entra en el
/// presupuesto (#360): la parte del uso está agotada o la cuenta ya está en la reserva. Las reglas
/// son las de [`check`] (#167); acá no se agrega ni se afloja ninguna.
pub fn out_of_allowance(usage: Usage, spend: &WeekSpend, budget: &UsageBudget) -> Option<String> {
    check(usage, 1, spend, budget).err()
}

/// Mensaje de la pausa por cupo (#360): «en pausa hasta <renovación UTC>» y el motivo de
/// [`check`]. Sin una renovación observada, se dice que se espera la renovación sin fecha.
pub fn pause_message(why: &str, resets_at: Option<DateTime<Utc>>) -> String {
    let until = resets_at.map_or_else(
        || "la renovación del cupo de IG (fecha sin observar)".to_owned(),
        |r| r.format("%Y-%m-%d %H:%M UTC").to_string(),
    );
    format!("en pausa hasta {until}: sin cupo de IG, no se pide nada ({why})")
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

    #[test]
    fn due_points_is_one_per_candle() {
        let (f, t) = (at("2026-11-20T15:00:00Z"), at("2026-11-20T16:00:00Z"));
        assert_eq!(due_points(f, t, Timeframe::M15), 4);
        assert_eq!(due_points(f, t, Timeframe::H1), 1);
        assert_eq!(due_points(f, f, Timeframe::H1), 0);
    }

    #[test]
    fn an_old_last_candle_does_not_reach_past_the_lookback() {
        // Última guardada hace una semana: se arranca 24 h atrás, como sin datos.
        assert_eq!(
            due(
                Timeframe::M15,
                Some("2026-11-13T20:45:00Z"),
                "2026-11-20T21:12:00Z"
            ),
            due(Timeframe::M15, None, "2026-11-20T21:12:00Z")
        );
    }

    const BUDGET: UsageBudget = UsageBudget {
        live: 3_400,
        history: 5_000,
        off_data_machine: 1_000,
        reserve: 600,
    };

    #[test]
    fn below_the_reserve_not_even_one_candle_is_requested() {
        // El caso del #360: a la cuenta le quedan 320 puntos, con una reserva de 600.
        let spend = WeekSpend {
            live: 1,
            remaining: Some(320),
            resets_at: Some(at("2026-11-27T20:30:39Z")),
            ..WeekSpend::default()
        };
        let why = out_of_allowance(Usage::Live, &spend, &BUDGET).expect("sin cupo");
        assert!(why.contains("budget.reserve"), "{why}");
        let msg = pause_message(&why, spend.resets_at);
        assert!(
            msg.starts_with("en pausa hasta 2026-11-27 20:30 UTC: "),
            "{msg}"
        );
        assert!(msg.contains("no se pide nada"), "{msg}");
    }

    #[test]
    fn an_exhausted_live_share_also_pauses() {
        let spend = WeekSpend {
            live: 3_400,
            remaining: Some(5_000),
            resets_at: Some(at("2026-11-27T20:30:00Z")),
            ..WeekSpend::default()
        };
        let why = out_of_allowance(Usage::Live, &spend, &BUDGET).expect("sin cupo");
        assert!(why.contains("budget.live"), "{why}");
    }

    #[test]
    fn with_room_for_one_candle_there_is_no_pause() {
        // Exactamente una vela por encima de la reserva: se pide (la regla de #167 no cambia).
        let spend = WeekSpend {
            remaining: Some(601),
            ..WeekSpend::default()
        };
        assert_eq!(out_of_allowance(Usage::Live, &spend, &BUDGET), None);
        // Sin observaciones en la ventana: solo manda la parte del uso.
        assert_eq!(
            out_of_allowance(Usage::Live, &WeekSpend::default(), &BUDGET),
            None
        );
    }

    #[test]
    fn a_gap_that_does_not_fit_is_refused_even_if_one_candle_fits() {
        // El hueco de 33 velas no entra aunque entre una sola: `ig_live` no lo pide a medias,
        // se pone en pausa (ver ig_live). Llenarlo es de `ig_backfill` (#137).
        let spend = WeekSpend {
            remaining: Some(620),
            ..WeekSpend::default()
        };
        assert_eq!(out_of_allowance(Usage::Live, &spend, &BUDGET), None);
        assert!(check(Usage::Live, 33, &spend, &BUDGET).is_err());
    }

    #[test]
    fn without_an_observed_renewal_the_pause_says_so() {
        let msg = pause_message("motivo", None);
        assert!(
            msg.starts_with("en pausa hasta la renovación del cupo de IG (fecha sin observar)"),
            "{msg}"
        );
    }

    #[test]
    fn a_week_of_live_candles_fits_the_live_budget() {
        let catalog = cfds_market_data::Catalog::from_toml(include_str!(
            "../../../../config/instruments.toml"
        ))
        .unwrap();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
        let calendar = ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap();
        let ids: Vec<_> = catalog.instruments.keys().cloned().collect();
        let cfg = crate::IgConfig::from_toml(include_str!("../../../../config/ig.toml")).unwrap();
        // Semana del 16 al 20-11-2026, sin feriados. Por día: 3 índices de EE. UU. con 26 velas
        // de 15m y 6 de 1h, y DE40 con 34 y 8 → 138; por semana, 690.
        let week = session_points(
            &calendar,
            &ids,
            &cfg.live.timeframes,
            at("2026-11-16T00:00:00Z"),
            at("2026-11-23T00:00:00Z"),
        )
        .unwrap();
        assert_eq!(ids.len(), 4);
        assert_eq!(week, 690);
        assert!(week <= cfg.budget.live, "{week} > budget.live");
    }
}

//! Huecos de la base (#137, propuesta 3 del #132): qué velas faltan, para pedirle al proveedor
//! **solo eso**. Con IG cada vela pedida gasta cupo semanal aunque ya estuviera guardada.
//!
//! Funciones puras: las velas esperadas salen del calendario de sesiones y las guardadas, de la
//! base. Si el calendario no cubre una fecha, no se supone nada: se devuelve `None`.

use std::collections::BTreeSet;

use cfds_core::Timeframe;
use cfds_market_data::Session;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};

/// Tramos `[desde, hasta)` que hay que pedir para completar las velas de 15m o 1h.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapPlan<T> {
    /// Velas esperadas según el calendario, ya cerradas.
    pub expected: usize,
    /// De esas, cuántas ya están guardadas.
    pub stored: usize,
    /// Tramos a pedir, en orden, sin superponerse.
    pub ranges: Vec<(T, T)>,
}

impl<T> GapPlan<T> {
    /// Velas que faltan (las que se van a pedir).
    pub fn missing(&self) -> usize {
        self.expected - self.stored
    }
}

/// Corridas de elementos de `expected` (en orden) que no están en `have`, como
/// `(primero, último)`. Un elemento guardado corta la corrida; lo que no está en `expected`
/// (fines de semana, feriados, fuera de sesión) no la corta.
pub fn missing_runs<T: Ord + Copy>(expected: &[T], have: &BTreeSet<T>) -> Vec<(T, T)> {
    missing_runs_by(expected, have, |_, _| true)
}

/// Igual que [`missing_runs`], pero la corrida también se corta entre dos esperados seguidos
/// para los que `adjacent(anterior, siguiente)` es falso.
pub fn missing_runs_by<T: Ord + Copy>(
    expected: &[T],
    have: &BTreeSet<T>,
    adjacent: impl Fn(T, T) -> bool,
) -> Vec<(T, T)> {
    let mut runs = Vec::new();
    let mut current: Option<(T, T)> = None;
    for &e in expected {
        if have.contains(&e) {
            runs.extend(current.take());
            continue;
        }
        current = match current {
            Some((first, last)) if adjacent(last, e) => Some((first, e)),
            other => {
                runs.extend(other);
                Some((e, e))
            }
        };
    }
    runs.extend(current);
    runs
}

/// Plan para velas de 15m o 1h. `expected` son los inicios del calendario
/// (`SessionCalendar::expected_starts`); solo cuentan las velas ya cerradas en `now`.
///
/// Un tramo nunca cruza el cierre de una sesión: IG cobra también las velas de la noche y del
/// fin de semana (un CFD cotiza casi todo el día), y eso cuesta mucho más que el margen de
/// un pedido extra.
pub fn intraday_gaps(
    expected: &[DateTime<Utc>],
    stored: &BTreeSet<DateTime<Utc>>,
    timeframe: Timeframe,
    now: DateTime<Utc>,
) -> GapPlan<DateTime<Utc>> {
    let step = timeframe.duration();
    let closed: Vec<_> = expected
        .iter()
        .copied()
        .filter(|s| *s + step <= now)
        .collect();
    let ranges = missing_runs_by(&closed, stored, |a, b| b - a == step)
        .into_iter()
        .map(|(first, last)| (first, last + step))
        .collect();
    GapPlan {
        expected: closed.len(),
        stored: closed.iter().filter(|s| stored.contains(s)).count(),
        ranges,
    }
}

/// Días con sesión en `[from, to)`. `None` si el calendario no cubre alguno.
pub fn session_days(session: &Session, from: NaiveDate, to: NaiveDate) -> Option<Vec<NaiveDate>> {
    let mut days = Vec::new();
    for d in from.iter_days().take_while(|d| *d < to) {
        if session.hours(d)?.is_some() {
            days.push(d);
        }
    }
    Some(days)
}

/// Plan para velas diarias. `expected` son los días con sesión (`session_days`) y `closed_by`
/// es el primer día que todavía no cerró: los días desde ahí no se piden.
pub fn daily_gaps(
    expected: &[NaiveDate],
    stored: &BTreeSet<NaiveDate>,
    closed_by: NaiveDate,
) -> GapPlan<NaiveDate> {
    let closed: Vec<_> = expected
        .iter()
        .copied()
        .filter(|d| *d < closed_by)
        .collect();
    let ranges = missing_runs(&closed, stored)
        .into_iter()
        .map(|(first, last)| (first, last + TimeDelta::days(1)))
        .collect();
    GapPlan {
        expected: closed.len(),
        stored: closed.iter().filter(|d| stored.contains(d)).count(),
        ranges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set<T: Ord + Copy>(xs: &[T]) -> BTreeSet<T> {
        xs.iter().copied().collect()
    }

    #[test]
    fn runs_cover_start_middle_and_end_gaps() {
        let expected = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(
            missing_runs(&expected, &set(&[3, 4, 7])),
            vec![(1, 2), (5, 6), (8, 8)]
        );
    }

    #[test]
    fn nothing_stored_is_one_run_and_everything_stored_is_none() {
        let expected = [1, 2, 3];
        assert_eq!(missing_runs(&expected, &set(&[])), vec![(1, 3)]);
        assert_eq!(missing_runs(&expected, &set(&[1, 2, 3])), vec![]);
        assert_eq!(missing_runs::<i32>(&[], &set(&[1])), vec![]);
    }

    #[test]
    fn stored_values_outside_expected_do_not_count() {
        // Una vela fuera de sesión guardada no corta ni suma.
        assert_eq!(missing_runs(&[2, 4], &set(&[3])), vec![(2, 4)]);
    }

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().expect("fecha de test")
    }

    #[test]
    fn intraday_requests_only_missing_closed_candles() {
        let expected = [
            t("2026-11-20T14:30:00Z"),
            t("2026-11-20T14:45:00Z"),
            t("2026-11-20T15:00:00Z"),
            t("2026-11-20T15:15:00Z"),
            t("2026-11-20T15:30:00Z"),
        ];
        let stored = set(&[t("2026-11-20T15:00:00Z")]);
        // A las 15:40 la vela de las 15:30 sigue abierta: no se pide.
        let plan = intraday_gaps(
            &expected,
            &stored,
            Timeframe::M15,
            t("2026-11-20T15:40:00Z"),
        );
        assert_eq!(plan.expected, 4);
        assert_eq!(plan.stored, 1);
        assert_eq!(plan.missing(), 3);
        assert_eq!(
            plan.ranges,
            vec![
                (t("2026-11-20T14:30:00Z"), t("2026-11-20T15:00:00Z")),
                (t("2026-11-20T15:15:00Z"), t("2026-11-20T15:30:00Z")),
            ]
        );
    }

    #[test]
    fn intraday_ranges_never_cross_the_session_close() {
        // Última vela de 1h del viernes y primera del lunes: dos pedidos, no uno con el fin de semana.
        let expected = [t("2026-11-20T20:00:00Z"), t("2026-11-23T15:00:00Z")];
        let plan = intraday_gaps(
            &expected,
            &set(&[]),
            Timeframe::H1,
            t("2026-11-24T00:00:00Z"),
        );
        assert_eq!(
            plan.ranges,
            vec![
                (t("2026-11-20T20:00:00Z"), t("2026-11-20T21:00:00Z")),
                (t("2026-11-23T15:00:00Z"), t("2026-11-23T16:00:00Z")),
            ]
        );
    }

    #[test]
    fn runs_by_split_where_not_adjacent() {
        assert_eq!(
            missing_runs_by(&[1, 2, 5, 6, 8], &set(&[8]), |a, b| b - a == 1),
            vec![(1, 2), (5, 6)]
        );
    }

    #[test]
    fn intraday_with_everything_stored_requests_nothing() {
        let expected = [t("2026-11-20T15:00:00Z"), t("2026-11-20T16:00:00Z")];
        let plan = intraday_gaps(
            &expected,
            &set(&expected),
            Timeframe::H1,
            t("2026-11-21T00:00:00Z"),
        );
        assert_eq!((plan.expected, plan.stored, plan.missing()), (2, 2, 0));
        assert!(plan.ranges.is_empty());
    }

    fn d(s: &str) -> NaiveDate {
        s.parse().expect("día de test")
    }

    #[test]
    fn daily_runs_jump_weekends_and_stop_at_the_open_day() {
        // Jueves 19, viernes 20, lunes 23 y martes 24 con sesión; el 24 todavía no cerró.
        let expected = [
            d("2026-11-19"),
            d("2026-11-20"),
            d("2026-11-23"),
            d("2026-11-24"),
        ];
        let plan = daily_gaps(&expected, &set(&[d("2026-11-19")]), d("2026-11-24"));
        assert_eq!((plan.expected, plan.stored, plan.missing()), (3, 1, 2));
        // Viernes y lunes faltan: un solo tramo, que cruza el fin de semana.
        assert_eq!(plan.ranges, vec![(d("2026-11-20"), d("2026-11-24"))]);
    }

    fn calendar() -> cfds_market_data::ExchangeCalendar {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../config");
        let catalog = cfds_market_data::Catalog::from_toml(
            &std::fs::read_to_string(root.join("instruments.toml")).expect("catálogo"),
        )
        .expect("catálogo válido");
        cfds_market_data::ExchangeCalendar::load(&root, catalog.exchange_map()).expect("calendario")
    }

    #[test]
    fn session_days_skip_weekends_and_holidays_from_the_repository_calendar() {
        let cal = calendar();
        let nyse = cal.session("XNYS").expect("NYSE");
        // Jueves 26-11-2026 es Thanksgiving (sin sesión); 28 y 29 son fin de semana.
        assert_eq!(
            session_days(nyse, d("2026-11-25"), d("2026-11-30")),
            Some(vec![d("2026-11-25"), d("2026-11-27")])
        );
    }

    #[test]
    fn session_days_refuse_dates_the_calendar_does_not_cover() {
        let cal = calendar();
        let nyse = cal.session("XNYS").expect("NYSE");
        assert_eq!(session_days(nyse, d("2019-01-01"), d("2019-01-10")), None);
    }
}

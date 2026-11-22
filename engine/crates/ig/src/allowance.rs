//! Cupo semanal de históricos de IG (#140, propuestas 1 y 2 del #132).
//!
//! Cada respuesta de `GET /prices` dice cuánto queda del cupo de **toda la cuenta** y en cuánto
//! se renueva. Con eso, una máquina que registra sus pedidos puede deducir cuánto se gastó desde
//! otro lado (otra computadora o una sesión en la nube) entre dos de sus corridas.
//!
//! Suposición que **no** se da por cierta: cada precio devuelto cuesta 1 punto. Se controla con
//! lo que bajó el cupo entre dos respuestas de la misma corrida, y el resumen dice si coincidió.

use chrono::{DateTime, Utc};

/// Lo que dijo IG del cupo en una respuesta de `GET /prices`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowanceObservation {
    /// Cuándo llegó la respuesta.
    pub at: DateTime<Utc>,
    /// Precios (velas) que trajo esa respuesta.
    pub points_returned: u64,
    /// `remainingAllowance` después de esa respuesta.
    pub remaining: u64,
    /// `totalAllowance`.
    pub total: u64,
    /// `at` + `allowanceExpiry`.
    pub resets_at: DateTime<Utc>,
}

/// Qué pasó con el cupo en una corrida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowanceSummary {
    /// Precios devueltos en la corrida (≈ puntos gastados, si se cumple la suposición).
    pub points_returned: u64,
    /// Cupo restante y renovación según la última respuesta.
    pub remaining: u64,
    pub total: u64,
    pub resets_at: DateTime<Utc>,
    /// Lo que bajó el cupo antes de esta corrida sin que lo explique esta máquina, desde la
    /// última observación registrada. `None` si no hay observación previa. Negativo si la
    /// suposición de 1 punto por precio no se cumple.
    pub elsewhere: Option<i64>,
    /// `Some(true)` si en todas las respuestas seguidas de la corrida el cupo bajó lo mismo que
    /// los precios devueltos; `Some(false)` si alguna no; `None` si hubo una sola respuesta.
    pub one_point_per_price: Option<bool>,
}

/// Resume una corrida (`run`, en orden) a partir de la última observación registrada antes
/// (`previous`). `None` si la corrida no hizo ningún pedido.
pub fn summarize(
    previous: Option<&AllowanceObservation>,
    run: &[AllowanceObservation],
) -> Option<AllowanceSummary> {
    let (first, last) = (run.first()?, run.last()?);
    let pairs: Vec<_> = run
        .windows(2)
        .filter(|w| !renewed_between(&w[0], &w[1]))
        .map(|w| w[0].remaining.checked_sub(w[1].remaining) == Some(w[1].points_returned))
        .collect();
    let one_point_per_price = (!pairs.is_empty()).then(|| pairs.iter().all(|ok| *ok));
    // Cupo justo antes del primer pedido de la corrida.
    let before = i128::from(first.remaining) + i128::from(first.points_returned);
    let elsewhere = previous.map(|p| {
        let start = if renewed_between(p, first) {
            first.total
        } else {
            p.remaining
        };
        to_i64(i128::from(start) - before)
    });
    Some(AllowanceSummary {
        points_returned: run.iter().map(|o| o.points_returned).sum(),
        remaining: last.remaining,
        total: last.total,
        resets_at: last.resets_at,
        elsewhere,
        one_point_per_price,
    })
}

/// El cupo se renovó entre `a` y `b` si `b` llegó después de la renovación anunciada en `a`.
fn renewed_between(a: &AllowanceObservation, b: &AllowanceObservation) -> bool {
    b.at >= a.resets_at
}

fn to_i64(v: i128) -> i64 {
    i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().expect("fecha de test")
    }

    fn obs(at: &str, points: u64, remaining: u64) -> AllowanceObservation {
        AllowanceObservation {
            at: t(at),
            points_returned: points,
            remaining,
            total: 10_000,
            resets_at: t("2026-11-24T12:00:00Z"),
        }
    }

    #[test]
    fn a_run_without_requests_has_no_summary() {
        assert_eq!(summarize(None, &[]), None);
    }

    #[test]
    fn spend_elsewhere_is_what_the_allowance_lost_between_runs() {
        let previous = obs("2026-11-22T10:00:00Z", 30, 9_000);
        // Antes de esta corrida quedaban 8_700 (8_680 + 20): otro gastó 300.
        let run = [
            obs("2026-11-22T18:00:00Z", 20, 8_680),
            obs("2026-11-22T18:00:01Z", 15, 8_665),
        ];
        let s = summarize(Some(&previous), &run).expect("hubo pedidos");
        assert_eq!(s.points_returned, 35);
        assert_eq!(s.remaining, 8_665);
        assert_eq!(s.elsewhere, Some(300));
        assert_eq!(s.one_point_per_price, Some(true));
        assert_eq!(s.resets_at, t("2026-11-24T12:00:00Z"));
    }

    #[test]
    fn nothing_spent_elsewhere_gives_zero() {
        let previous = obs("2026-11-22T10:00:00Z", 30, 9_000);
        let run = [obs("2026-11-22T18:00:00Z", 10, 8_990)];
        let s = summarize(Some(&previous), &run).expect("hubo pedidos");
        assert_eq!(s.elsewhere, Some(0));
        assert_eq!(s.one_point_per_price, None, "una sola respuesta no alcanza");
    }

    #[test]
    fn without_a_previous_observation_elsewhere_is_unknown() {
        let s = summarize(None, &[obs("2026-11-22T18:00:00Z", 10, 8_990)]).expect("hubo pedidos");
        assert_eq!(s.elsewhere, None);
    }

    #[test]
    fn a_renewal_in_between_starts_from_the_full_allowance() {
        let previous = obs("2026-11-23T10:00:00Z", 30, 500);
        let mut first = obs("2026-11-24T13:00:00Z", 40, 9_900);
        first.resets_at = t("2026-12-01T12:00:00Z");
        // Renovó a 10_000; antes de la corrida quedaban 9_940: otro gastó 60.
        let s = summarize(Some(&previous), &[first]).expect("hubo pedidos");
        assert_eq!(s.elsewhere, Some(60));
    }

    #[test]
    fn the_one_point_per_price_assumption_is_checked_not_assumed() {
        let run = [
            obs("2026-11-22T18:00:00Z", 20, 8_680),
            obs("2026-11-22T18:00:01Z", 15, 8_660), // bajó 20, no 15
        ];
        let s = summarize(None, &run).expect("hubo pedidos");
        assert_eq!(s.one_point_per_price, Some(false));
    }

    #[test]
    fn a_renewal_inside_the_run_is_not_counted_as_a_mismatch() {
        let a = obs("2026-11-24T11:59:59Z", 10, 100);
        let mut b = obs("2026-11-24T12:00:01Z", 10, 9_990);
        b.resets_at = t("2026-12-01T12:00:01Z");
        let c = AllowanceObservation {
            at: b.at + TimeDelta::seconds(1),
            points_returned: 5,
            remaining: 9_985,
            ..b
        };
        let s = summarize(None, &[a, b, c]).expect("hubo pedidos");
        assert_eq!(s.one_point_per_price, Some(true));
        assert_eq!(s.remaining, 9_985);
    }

    #[test]
    fn a_negative_elsewhere_is_reported_as_is() {
        // Si IG cobrara menos de 1 punto por precio, "otro lado" daría negativo: se muestra.
        let previous = obs("2026-11-22T10:00:00Z", 30, 9_000);
        let s = summarize(Some(&previous), &[obs("2026-11-22T18:00:00Z", 50, 8_990)])
            .expect("hubo pedidos");
        assert_eq!(s.elsewhere, Some(-40));
    }
}

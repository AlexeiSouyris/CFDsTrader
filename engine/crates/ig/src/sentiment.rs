//! Sentimiento de clientes de IG (#27): de la lectura de la API a observaciones para guardar.
//! Funciones puras.
//!
//! Solo se **graba** el dato, con la hora UTC de la consulta. No se interpreta acá: eso le
//! corresponde a Python (fase 4b). El % de clientes de IG no tiene historial público, por eso
//! se graba desde ya.

use cfds_core::sentiment::{IG_CLIENT, LONG_PCT};
use cfds_core::{InstrumentId, SentimentObservation};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// Observaciones `ig_client` / `long_pct` de una consulta hecha en `at`. Los mercados sin dato
/// (`None`) no se graban: un 0 % inventado sería peor que un hueco.
///
/// `observed_for` y `published_at` son los dos `at`: IG publica el dato en el momento en que se
/// lo pide, y el sistema no lo conoce antes.
pub fn observations(
    readings: &[(InstrumentId, Option<Decimal>)],
    at: DateTime<Utc>,
) -> Vec<SentimentObservation> {
    readings
        .iter()
        .filter_map(|(id, long)| {
            long.map(|value| SentimentObservation {
                source: IG_CLIENT.into(),
                series: LONG_PCT.into(),
                instrument_id: Some(id.clone()),
                observed_for: at,
                published_at: at,
                value,
            })
        })
        .collect()
}

/// Próxima consulta: el siguiente múltiplo de `interval` (contado desde la época Unix, en UTC)
/// estrictamente posterior a `now`. Así las consultas caen siempre en los mismos minutos
/// (:00, :15, :30, :45 con 15 minutos).
pub fn next_poll(now: DateTime<Utc>, interval: chrono::TimeDelta) -> DateTime<Utc> {
    let step = interval.num_seconds().max(1);
    let next = (now.timestamp().div_euclid(step) + 1) * step;
    DateTime::from_timestamp(next, 0).unwrap_or(now)
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use rust_decimal::dec;

    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn readings_become_append_only_observations_without_inventing_zeros() {
        let t = at("2026-11-20T21:45:00Z");
        let us: InstrumentId = "US100".parse().unwrap();
        let de: InstrumentId = "DE40".parse().unwrap();
        let obs = observations(&[(us.clone(), Some(dec!(68.0))), (de, None)], t);
        assert_eq!(
            obs,
            [SentimentObservation {
                source: "ig_client".into(),
                series: "long_pct".into(),
                instrument_id: Some(us),
                observed_for: t,
                published_at: t,
                value: dec!(68.0),
            }]
        );
    }

    #[test]
    fn polls_fall_on_fixed_minutes() {
        let q = TimeDelta::minutes(15);
        assert_eq!(
            next_poll(at("2026-11-20T21:44:59Z"), q),
            at("2026-11-20T21:45:00Z")
        );
        assert_eq!(
            next_poll(at("2026-11-20T21:45:00Z"), q),
            at("2026-11-20T22:00:00Z")
        );
        assert_eq!(
            next_poll(at("2026-11-20T23:59:30Z"), q),
            at("2026-11-21T00:00:00Z")
        );
    }
}

//! Caché en memoria de las velas diarias y del VIX para el bloque `analysis` de la vista de
//! liquidez (#511). Antes, cada pedido de 15m leía todas las diarias del instrumento y el VIX de
//! PostgreSQL; ahora se leen una vez y se vuelven a leer solo cuando aparece algo nuevo:
//!
//! - **Diarias, por instrumento:** el gateway pregunta en cada pedido cuál es el fin de la última
//!   diaria terminada hasta `as_of` (una consulta de un valor). Si es posterior a la última que
//!   tiene el caché, hay un día nuevo y se vuelven a leer todas.
//! - **VIX:** en cada pedido se leen solo los cierres publicados desde el último que tiene el
//!   caché; los que tienen `published_at` posterior se suman.
//!
//! **Sin mirar el futuro:** lo que devuelve el caché para un `as_of` es solo lo que ya existía
//! en ese momento (diarias con `end ≤ as_of`, VIX con `published_at ≤ as_of`), aunque el caché
//! tenga cosas posteriores porque otro pedido lo llenó con un `as_of` más nuevo.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use cfds_core::{DailyCandle, InstrumentId};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// Un cierre del VIX: `(published_at, cierre)`, en orden de publicación.
pub type VixClose = (DateTime<Utc>, Decimal);

/// Las diarias y el VIX ya leídos. Se comparte entre pedidos (un `Mutex` que nunca se tiene
/// tomado mientras se espera a la base).
#[derive(Debug, Default)]
pub struct SeriesCache {
    daily: Mutex<HashMap<InstrumentId, Arc<Vec<DailyCandle>>>>,
    vix: Mutex<Arc<Vec<VixClose>>>,
}

impl SeriesCache {
    /// Las diarias de `id` terminadas a más tardar en `as_of`, si el caché está al día:
    /// `last_end` es el fin de la última diaria terminada hasta `as_of` según la base. `None` =
    /// hay que leerlas de nuevo (no están o apareció un día nuevo).
    pub fn daily_known(
        &self,
        id: &InstrumentId,
        as_of: DateTime<Utc>,
        last_end: Option<DateTime<Utc>>,
    ) -> Option<Vec<DailyCandle>> {
        let days = self
            .daily
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()?;
        let cached_end = days.iter().map(|d| d.end).max();
        if last_end > cached_end {
            return None;
        }
        Some(known_days(&days, as_of))
    }

    /// Guarda las diarias leídas de `id` (todas las que había) y devuelve las de `as_of`.
    pub fn store_daily(
        &self,
        id: &InstrumentId,
        days: Vec<DailyCandle>,
        as_of: DateTime<Utc>,
    ) -> Vec<DailyCandle> {
        let out = known_days(&days, as_of);
        self.daily
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.clone(), Arc::new(days));
        out
    }

    /// Desde cuándo hay que leer el VIX: el último `published_at` del caché (`None` = vacío).
    pub fn vix_last(&self) -> Option<DateTime<Utc>> {
        self.vix
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .last()
            .map(|v| v.0)
    }

    /// Suma los cierres leídos con `published_at` posterior al último del caché (los demás ya
    /// están) y devuelve los publicados a más tardar en `as_of`.
    pub fn add_vix(&self, read: &[VixClose], as_of: DateTime<Utc>) -> Vec<VixClose> {
        let mut guard = self.vix.lock().unwrap_or_else(PoisonError::into_inner);
        let last = guard.last().map(|v| v.0);
        let fresh: Vec<VixClose> = read
            .iter()
            .filter(|v| last.is_none_or(|l| v.0 > l))
            .copied()
            .collect();
        if !fresh.is_empty() {
            let mut all = Vec::with_capacity(guard.len() + fresh.len());
            all.extend_from_slice(&guard);
            all.extend(fresh);
            all.sort_by_key(|v| v.0);
            *guard = Arc::new(all);
        }
        guard.iter().filter(|v| v.0 <= as_of).copied().collect()
    }
}

fn known_days(days: &[DailyCandle], as_of: DateTime<Utc>) -> Vec<DailyCandle> {
    days.iter().filter(|d| d.end <= as_of).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeDelta};
    use rust_decimal::dec;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn id() -> InstrumentId {
        "US100".parse().unwrap()
    }

    /// Diaria que empieza a las 22:00 UTC del día anterior y termina a las 21:00 de `day`.
    fn day(day: &str) -> DailyCandle {
        let d: NaiveDate = day.parse().unwrap();
        let end = d.and_hms_opt(21, 0, 0).unwrap().and_utc();
        DailyCandle {
            instrument_id: id(),
            day: d,
            start: end - TimeDelta::hours(23),
            end,
            open: dec!(100),
            high: dec!(110),
            low: dec!(90),
            close: dec!(105),
            volume: None,
        }
    }

    #[test]
    fn the_daily_cache_never_hands_out_days_that_had_not_ended() {
        let c = SeriesCache::default();
        assert!(
            c.daily_known(&id(), at("2026-11-20T00:00:00Z"), None)
                .is_none()
        );
        // Un pedido nuevo llena el caché con días hasta el 20.
        let all = vec![day("2026-11-18"), day("2026-11-19"), day("2026-11-20")];
        let got = c.store_daily(&id(), all, at("2026-11-20T22:00:00Z"));
        assert_eq!(got.len(), 3);
        // Un pedido del pasado (al desplazar el gráfico) ve solo los días terminados entonces.
        let past = at("2026-11-19T20:59:00Z");
        let got = c
            .daily_known(&id(), past, Some(at("2026-11-18T21:00:00Z")))
            .unwrap();
        assert_eq!(got, vec![day("2026-11-18")]);
        // Justo al terminar el 19, ya lo ve.
        let got = c
            .daily_known(
                &id(),
                at("2026-11-19T21:00:00Z"),
                Some(at("2026-11-19T21:00:00Z")),
            )
            .unwrap();
        assert_eq!(got.len(), 2);
        // Antes de la primera diaria, nada.
        let got = c
            .daily_known(&id(), at("2026-11-01T00:00:00Z"), None)
            .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn a_new_day_in_the_database_invalidates_the_cache() {
        let c = SeriesCache::default();
        c.store_daily(&id(), vec![day("2026-11-19")], at("2026-11-20T10:00:00Z"));
        let now = at("2026-11-20T21:15:00Z");
        // Sin día nuevo en la base: sirve el caché.
        assert!(
            c.daily_known(&id(), now, Some(at("2026-11-19T21:00:00Z")))
                .is_some()
        );
        // Terminó el 20 y ya está guardado: hay que leer de nuevo.
        assert!(
            c.daily_known(&id(), now, Some(at("2026-11-20T21:00:00Z")))
                .is_none()
        );
        // Cada instrumento tiene lo suyo.
        let other: InstrumentId = "DE40".parse().unwrap();
        assert!(c.daily_known(&other, now, None).is_none());
    }

    #[test]
    fn the_vix_cache_adds_only_later_publications_and_never_hands_out_the_future() {
        let c = SeriesCache::default();
        assert_eq!(c.vix_last(), None);
        let p1 = (at("2026-11-18T22:00:00Z"), dec!(15));
        let p2 = (at("2026-11-19T22:00:00Z"), dec!(18));
        let p3 = (at("2026-11-20T22:00:00Z"), dec!(21));
        // Un pedido de ahora lee todo.
        let got = c.add_vix(&[p1, p2, p3], at("2026-11-20T22:00:00Z"));
        assert_eq!(got, vec![p1, p2, p3]);
        assert_eq!(c.vix_last(), Some(p3.0));
        // Un pedido del pasado no ve lo publicado después de su `as_of`.
        assert_eq!(c.add_vix(&[], at("2026-11-19T21:59:59Z")), vec![p1]);
        assert_eq!(c.add_vix(&[], at("2026-11-19T22:00:00Z")), vec![p1, p2]);
        // Lo que se vuelve a leer desde el último no se duplica; lo posterior se suma.
        let p4 = (at("2026-11-21T22:00:00Z"), dec!(25));
        let got = c.add_vix(&[p3, p4], at("2026-11-22T00:00:00Z"));
        assert_eq!(got, vec![p1, p2, p3, p4]);
        assert_eq!(c.vix_last(), Some(p4.0));
    }
}

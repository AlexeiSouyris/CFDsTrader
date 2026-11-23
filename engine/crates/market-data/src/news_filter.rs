//! Filtro de noticias de alto impacto (spec §7.2), sobre el calendario económico (#91).
//! Función pura.
//!
//! ```text
//! τ = cierre de t = apertura de t+1 (cuando se decide y se entra)
//! bloquea en t ⇔ hay un evento con impacto bloqueante, del instrumento o de uno de sus países,
//!                ya publicado en τ (published_at ≤ τ) y con |scheduled_at − τ| ≤ ventana
//!
//! cobertura (#243): la vela tiene calendario en τ ⇔ hay una lectura con un evento nuevo
//!                   (un published_at) en [τ − calendar_stale_after, τ]
//! ```
//! Sin calendario no se sabe si había un evento: el filtro no bloquea, pero [`NewsFilter`]
//! cuenta esas velas y sus tramos para avisarlo.

use std::collections::BTreeMap;

use cfds_core::{Candle, EconomicEvent, Impact, InstrumentId};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsFilterConfig {
    pub window: TimeDelta,
    pub blocking: Vec<Impact>,
    /// Países cuyos eventos cuentan para cada instrumento.
    pub countries: BTreeMap<InstrumentId, Vec<String>>,
    /// Silencio máximo de la ingesta para que una vela tenga calendario.
    pub calendar_stale_after: TimeDelta,
}

impl NewsFilterConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct News {
            window_minutes: u32,
            blocking_impacts: Vec<Impact>,
            calendar_stale_after_days: u32,
            countries: BTreeMap<InstrumentId, Vec<String>>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            news: News,
        }
        let n = toml::from_str::<Raw>(text).map_err(|e| e.to_string())?.news;
        if n.window_minutes == 0 {
            return Err("news.window_minutes tiene que ser mayor que 0".into());
        }
        if n.calendar_stale_after_days == 0 {
            return Err("news.calendar_stale_after_days tiene que ser mayor que 0".into());
        }
        if n.blocking_impacts.is_empty() {
            return Err("news.blocking_impacts no puede estar vacío".into());
        }
        for (id, cs) in &n.countries {
            if cs.is_empty() {
                return Err(format!("news.countries.{id}: sin países"));
            }
            if let Some(c) = cs
                .iter()
                .find(|c| !(c.len() == 2 && c.chars().all(|x| x.is_ascii_uppercase())))
            {
                return Err(format!(
                    "news.countries.{id}: {c:?} no es ISO 3166-1 alfa-2"
                ));
            }
        }
        Ok(Self {
            window: TimeDelta::minutes(i64::from(n.window_minutes)),
            blocking: n.blocking_impacts,
            countries: n.countries,
            calendar_stale_after: TimeDelta::days(i64::from(n.calendar_stale_after_days)),
        })
    }
}

/// Para cada vela de `candles` (de un solo instrumento), `true` si el filtro de noticias deja
/// abrir al decidir en su cierre. Un instrumento sin países en la configuración solo se bloquea
/// por eventos propios.
pub fn news_flags(
    candles: &[Candle],
    events: &[EconomicEvent],
    cfg: &NewsFilterConfig,
) -> Vec<bool> {
    candles
        .iter()
        .map(|c| {
            let countries = cfg.countries.get(&c.instrument_id);
            let tau = c.end();
            !events.iter().any(|e| {
                cfg.blocking.contains(&e.impact)
                    && e.published_at <= tau
                    && (e.scheduled_at - tau).abs() <= cfg.window
                    && (e.instrument_id.as_ref() == Some(&c.instrument_id)
                        || e.country
                            .as_ref()
                            .is_some_and(|x| countries.is_some_and(|cs| cs.contains(x))))
            })
        })
        .collect()
}

/// Filtro de noticias de una serie, con la cobertura del calendario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsFilter {
    /// `true` si deja abrir al decidir en el cierre de cada vela.
    pub allowed: Vec<bool>,
    /// `true` si en el cierre de cada vela había calendario (ver [`calendar_coverage`]).
    pub covered: Vec<bool>,
}

impl NewsFilter {
    /// Filtro y cobertura de `candles`. `sightings`: los `published_at` distintos del calendario
    /// guardado, en orden (ver `PgStore::economic_event_sightings`).
    pub fn new(
        candles: &[Candle],
        events: &[EconomicEvent],
        sightings: &[DateTime<Utc>],
        cfg: &NewsFilterConfig,
    ) -> Self {
        Self {
            allowed: news_flags(candles, events, cfg),
            covered: calendar_coverage(candles, sightings, cfg.calendar_stale_after),
        }
    }

    pub fn blocked(&self) -> usize {
        self.allowed.iter().filter(|ok| !**ok).count()
    }

    pub fn uncovered(&self) -> usize {
        self.covered.iter().filter(|ok| !**ok).count()
    }

    /// Tramos seguidos de velas sin calendario: (inicio de la primera, cierre de la última).
    pub fn uncovered_spans(&self, candles: &[Candle]) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
        let mut spans: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();
        let mut open = false;
        for (c, covered) in candles.iter().zip(&self.covered) {
            match (covered, open, spans.last_mut()) {
                (false, true, Some(last)) => last.1 = c.end(),
                (false, _, _) => {
                    spans.push((c.start, c.end()));
                    open = true;
                }
                (true, _, _) => open = false,
            }
        }
        spans
    }
}

/// Para cada vela, `true` si al cerrarla había calendario: la última lectura con un evento nuevo
/// (`sightings`, ordenados) es de a lo sumo `stale_after` antes del cierre. Solo mira lecturas
/// hasta el cierre: sin look-ahead.
pub fn calendar_coverage(
    candles: &[Candle],
    sightings: &[DateTime<Utc>],
    stale_after: TimeDelta,
) -> Vec<bool> {
    candles
        .iter()
        .map(|c| {
            let tau = c.end();
            let seen = sightings.partition_point(|s| *s <= tau);
            seen > 0 && tau - sightings[seen - 1] <= stale_after
        })
        .collect()
}

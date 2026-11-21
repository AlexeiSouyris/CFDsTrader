//! Filtro de noticias de alto impacto (spec §7.2), sobre el calendario económico (#91).
//! Función pura.
//!
//! ```text
//! τ = cierre de t = apertura de t+1 (cuando se decide y se entra)
//! bloquea en t ⇔ hay un evento con impacto bloqueante, del instrumento o de uno de sus países,
//!                ya publicado en τ (published_at ≤ τ) y con |scheduled_at − τ| ≤ ventana
//! ```

use std::collections::BTreeMap;

use cfds_core::{Candle, EconomicEvent, Impact, InstrumentId};
use chrono::TimeDelta;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsFilterConfig {
    pub window: TimeDelta,
    pub blocking: Vec<Impact>,
    /// Países cuyos eventos cuentan para cada instrumento.
    pub countries: BTreeMap<InstrumentId, Vec<String>>,
}

impl NewsFilterConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct News {
            window_minutes: u32,
            blocking_impacts: Vec<Impact>,
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

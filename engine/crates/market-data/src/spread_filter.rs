//! Filtro de spread (spec §7.3), sobre el spread de cierre de cada vela (#95). Función pura.
//!
//! ```text
//! franja(x)  = minutos desde la medianoche local de la bolsa, redondeado a `slot_minutes`
//! historia   = spreads de velas con la misma franja que t, con inicio en [t − lookback_days, t)
//! se puede abrir en t ⇔ hay spread en t, |historia| ≥ min_samples
//!                       y spread[t] ≤ max_multiple × promedio(historia)
//! ```
//! Sin spread en `t` o sin historia suficiente no se puede juzgar, y entonces **no** se abre.

use std::collections::{BTreeMap, HashMap};

use cfds_core::{Candle, CandleSpread};
use chrono::{DateTime, TimeDelta, Timelike, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpreadFilterConfig {
    pub max_multiple: Decimal,
    pub lookback: TimeDelta,
    pub slot_minutes: u32,
    pub min_samples: usize,
}

impl SpreadFilterConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Spread {
            max_multiple: String,
            lookback_days: u32,
            slot_minutes: u32,
            min_samples: usize,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            spread: Spread,
        }
        let s = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .spread;
        let max_multiple: Decimal = s
            .max_multiple
            .parse()
            .map_err(|e| format!("spread.max_multiple: {e}"))?;
        if max_multiple <= Decimal::ONE {
            return Err("spread.max_multiple tiene que ser mayor que 1".into());
        }
        if s.lookback_days == 0 || s.min_samples == 0 {
            return Err(
                "spread.lookback_days y spread.min_samples tienen que ser mayores que 0".into(),
            );
        }
        if s.slot_minutes == 0 || (24 * 60) % s.slot_minutes != 0 {
            return Err("spread.slot_minutes tiene que dividir exacto al día".into());
        }
        Ok(Self {
            max_multiple,
            lookback: TimeDelta::days(i64::from(s.lookback_days)),
            slot_minutes: s.slot_minutes,
            min_samples: s.min_samples,
        })
    }
}

fn slot(at: DateTime<Utc>, tz: Tz, minutes: u32) -> u32 {
    let local = at.with_timezone(&tz);
    (local.hour() * 60 + local.minute()) / minutes
}

/// Para cada vela de `candles`, `true` si el filtro de spread deja abrir al decidir en su
/// cierre. `spreads` son los spreads de cierre del mismo instrumento y timeframe (en cualquier
/// orden); `tz` es la zona de la bolsa del instrumento.
pub fn spread_flags(
    candles: &[Candle],
    spreads: &[CandleSpread],
    tz: Tz,
    cfg: &SpreadFilterConfig,
) -> Vec<bool> {
    let at: BTreeMap<DateTime<Utc>, Decimal> =
        spreads.iter().map(|s| (s.start, s.spread)).collect();
    // Por franja: inicios en orden y sumas acumuladas, para promediar cualquier rango.
    let mut by_slot: HashMap<u32, (Vec<DateTime<Utc>>, Vec<Decimal>)> = HashMap::new();
    for (&start, &spread) in &at {
        let (starts, sums) = by_slot
            .entry(slot(start, tz, cfg.slot_minutes))
            .or_default();
        let prev = sums.last().copied().unwrap_or(Decimal::ZERO);
        starts.push(start);
        sums.push(prev + spread);
    }
    candles
        .iter()
        .map(|c| {
            let Some(&current) = at.get(&c.start) else {
                return false;
            };
            let Some((starts, sums)) = by_slot.get(&slot(c.start, tz, cfg.slot_minutes)) else {
                return false;
            };
            let lo = starts.partition_point(|s| *s < c.start - cfg.lookback);
            let hi = starts.partition_point(|s| *s < c.start);
            let n = hi - lo;
            if n < cfg.min_samples {
                return false;
            }
            let before = lo.checked_sub(1).map_or(Decimal::ZERO, |i| sums[i]);
            let mean = (sums[hi - 1] - before) / Decimal::from(n);
            current <= cfg.max_multiple * mean
        })
        .collect()
}

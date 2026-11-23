//! Régimen de mercado y estados emocionales que salen del precio (spec §9.1, #220).
//!
//! Hoy: FOMO de precio y pánico o clímax de venta en velas de 1h, con los valores de su regla
//! (`price_emotions_with_values`, para `emotional_state.v1`). El detector de régimen del
//! ADR 0003 §A y la distribución (pendiente de volumen real, spec §9.1) todavía no existen.
//!
//! ```text
//! FOMO:   (C[t] − EMA20[t]) / ATR14[t] ≥ 3   y   C[t] ≥ máx(C[t−19..=t])
//! clímax: (EMA20[t] − C[t]) / ATR14[t] ≥ 3   y   C[t] ≤ mín(C[t−19..=t])
//! ```
//! - Solo velas cerradas: el estado de `t` se conoce al cierre de `t` y depende solo de
//!   `0..=t` (sin repintado, hay test).
//! - Calentamiento: no se evalúa sin `lookback` velas, sin EMA ni sin ATR (> 0).
//! - Un empate con un cierre anterior cuenta como "el más alto" (o "el más bajo").
//! - Informativo: no suma al puntaje hasta validarse en la fase 5 y votarse.

use std::str::FromStr;

use cfds_core::Candle;
use cfds_indicators::{ema, wilder_atr};
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriceEmotion {
    /// FOMO de precio: muy por encima de su media y en máximo de la ventana.
    Fomo,
    /// Pánico o clímax de venta: muy por debajo de su media y en mínimo de la ventana.
    SellingClimax,
}

impl PriceEmotion {
    pub fn as_str(self) -> &'static str {
        match self {
            PriceEmotion::Fomo => "fomo",
            PriceEmotion::SellingClimax => "selling_climax",
        }
    }
}

/// Sección `[emotions]` de `config/regime.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmotionConfig {
    pub ema_period: usize,
    pub atr_period: usize,
    pub atr_multiple: Decimal,
    pub lookback: usize,
}

impl EmotionConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct File {
            emotions: Raw,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            ema_period: usize,
            atr_period: usize,
            atr_multiple: String,
            lookback: usize,
        }
        let r = toml::from_str::<File>(text)
            .map_err(|e| e.to_string())?
            .emotions;
        let atr_multiple = Decimal::from_str(&r.atr_multiple)
            .map_err(|e| format!("emotions.atr_multiple: {e}"))?;
        if r.ema_period == 0
            || r.atr_period == 0
            || r.lookback == 0
            || atr_multiple <= Decimal::ZERO
        {
            return Err("emotions: períodos, lookback y atr_multiple tienen que ser > 0".into());
        }
        Ok(Self {
            ema_period: r.ema_period,
            atr_period: r.atr_period,
            atr_multiple,
            lookback: r.lookback,
        })
    }
}

/// Los valores de la regla en la vela que activó el estado (spec §9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceEmotionValues {
    pub close: Decimal,
    pub ema: Decimal,
    pub atr: Decimal,
    /// `(cierre − EMA) / ATR`, con signo (negativo en el clímax de venta).
    pub stretch: Decimal,
}

/// Estado emocional de precio de cada vela de 1h **cerrada**, alineado por índice.
pub fn price_emotions(h1: &[Candle], cfg: &EmotionConfig) -> Vec<Option<PriceEmotion>> {
    price_emotions_with_values(h1, cfg)
        .into_iter()
        .map(|e| e.map(|(kind, _)| kind))
        .collect()
}

/// Como [`price_emotions`], con los valores de la regla de cada estado activo.
pub fn price_emotions_with_values(
    h1: &[Candle],
    cfg: &EmotionConfig,
) -> Vec<Option<(PriceEmotion, PriceEmotionValues)>> {
    let ema = ema(h1, cfg.ema_period);
    let atr = wilder_atr(h1, cfg.atr_period);
    (0..h1.len())
        .map(|t| {
            let first = (t + 1).checked_sub(cfg.lookback)?;
            let (ema, atr) = (ema[t]?, atr[t]?);
            if atr <= Decimal::ZERO {
                return None;
            }
            let close = h1[t].close;
            let window = h1[first..=t].iter().map(|c| c.close);
            let stretch = (close - ema) / atr;
            let values = PriceEmotionValues {
                close,
                ema,
                atr,
                stretch,
            };
            if stretch >= cfg.atr_multiple && window.clone().all(|c| c <= close) {
                Some((PriceEmotion::Fomo, values))
            } else if -stretch >= cfg.atr_multiple && window.clone().all(|c| c >= close) {
                Some((PriceEmotion::SellingClimax, values))
            } else {
                None
            }
        })
        .collect()
}

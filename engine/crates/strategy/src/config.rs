//! `config/strategy.toml`, validado al cargar.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreConfig {
    pub min_score: i32,
    pub trend_with: i32,
    pub trend_against: i32,
    pub sma_side: i32,
    pub candle_pattern: i32,
    pub double_break: i32,
    pub level_rejection: i32,
    pub sweep: i32,
    pub fvg_or_order_block: i32,
    pub recent_choch: i32,
    pub news_min: i32,
    pub news_max: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopConfig {
    pub atr_buffer: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceConfig {
    pub sma_period: usize,
    /// Calentamiento (spec §3): velas de 15m necesarias antes de la primera señal.
    pub min_m15_candles: usize,
    /// Calentamiento (spec §3): velas de 1h **cerradas** necesarias antes de la primera señal.
    pub min_h1_candles: usize,
}

/// Ajuste de noticias y alerta de salida (spec §9, #130).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewsConfig {
    /// Solo cuentan las noticias publicadas en esta ventana antes del cierre de la vela.
    pub window: chrono::TimeDelta,
    /// Alerta de salida: relevancia mínima.
    pub alert_min_relevance: Decimal,
    /// Alerta de salida: |sentimiento| mínimo.
    pub alert_min_abs_sentiment: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategyConfig {
    pub score: ScoreConfig,
    pub stop: StopConfig,
    pub evidence: EvidenceConfig,
    pub news: NewsConfig,
}

impl StrategyConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawStop {
            atr_buffer: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawNews {
            window_hours: u32,
            alert_min_relevance: String,
            alert_min_abs_sentiment: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            score: ScoreConfig,
            stop: RawStop,
            evidence: EvidenceConfig,
            news: RawNews,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let s = raw.score;
        if s.min_score <= 0 {
            return Err("score.min_score tiene que ser mayor que 0".into());
        }
        if s.trend_against > 0 {
            return Err("score.trend_against no puede sumar puntos".into());
        }
        let positive = [
            s.trend_with,
            s.sma_side,
            s.candle_pattern,
            s.double_break,
            s.level_rejection,
            s.sweep,
            s.fvg_or_order_block,
            s.recent_choch,
        ];
        if positive.iter().any(|&w| w < 0) {
            return Err("los pesos de las señales a favor no pueden ser negativos".into());
        }
        if s.news_min > 0 || s.news_max < 0 {
            return Err("el rango de noticias tiene que incluir el 0".into());
        }
        let atr_buffer =
            Decimal::from_str(&raw.stop.atr_buffer).map_err(|e| format!("stop.atr_buffer: {e}"))?;
        if atr_buffer < Decimal::ZERO {
            return Err("stop.atr_buffer no puede ser negativo".into());
        }
        if raw.evidence.sma_period == 0 {
            return Err("evidence.sma_period tiene que ser mayor que 0".into());
        }
        if raw.evidence.min_m15_candles == 0 || raw.evidence.min_h1_candles == 0 {
            return Err(
                "el calentamiento (evidence.min_*_candles) tiene que ser mayor que 0".into(),
            );
        }
        if raw.news.window_hours == 0 || raw.news.window_hours > 24 {
            return Err("news.window_hours tiene que estar entre 1 y 24".into());
        }
        let unit = |name: &str, text: &str| -> Result<Decimal, String> {
            let v = Decimal::from_str(text).map_err(|e| format!("news.{name}: {e}"))?;
            if v < Decimal::ZERO || v > Decimal::ONE {
                return Err(format!("news.{name} tiene que estar entre 0 y 1"));
            }
            Ok(v)
        };
        let news = NewsConfig {
            window: chrono::TimeDelta::hours(i64::from(raw.news.window_hours)),
            alert_min_relevance: unit("alert_min_relevance", &raw.news.alert_min_relevance)?,
            alert_min_abs_sentiment: unit(
                "alert_min_abs_sentiment",
                &raw.news.alert_min_abs_sentiment,
            )?,
        };
        Ok(Self {
            score: s,
            stop: StopConfig { atr_buffer },
            evidence: raw.evidence,
            news,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    pub(crate) const REAL: &str = include_str!("../../../../config/strategy.toml");

    #[test]
    fn news_settings_are_validated() {
        let c = StrategyConfig::from_toml(REAL).unwrap();
        assert_eq!(c.news.window, chrono::TimeDelta::hours(4));
        assert_eq!(c.news.alert_min_relevance, dec!(0.7));
        assert_eq!(c.news.alert_min_abs_sentiment, dec!(0.7));
        for (old, new) in [
            ("window_hours = 4", "window_hours = 0"),
            ("window_hours = 4", "window_hours = 25"),
            (
                "alert_min_relevance = \"0.7\"",
                "alert_min_relevance = \"1.1\"",
            ),
            (
                "alert_min_abs_sentiment = \"0.7\"",
                "alert_min_abs_sentiment = \"-0.1\"",
            ),
        ] {
            assert!(
                StrategyConfig::from_toml(&REAL.replace(old, new)).is_err(),
                "{new}"
            );
        }
    }

    #[test]
    fn the_repository_config_matches_the_spec() {
        let c = StrategyConfig::from_toml(REAL).unwrap();
        assert_eq!(
            c.score,
            ScoreConfig {
                min_score: 5,
                trend_with: 2,
                trend_against: -3,
                sma_side: 1,
                candle_pattern: 1,
                double_break: 2,
                level_rejection: 1,
                sweep: 2,
                fvg_or_order_block: 1,
                recent_choch: 1,
                news_min: -2,
                news_max: 2,
            }
        );
        assert_eq!(c.stop.atr_buffer, dec!(0.5));
        assert_eq!(c.evidence.sma_period, 50);
        assert_eq!(
            (c.evidence.min_m15_candles, c.evidence.min_h1_candles),
            (200, 60)
        );
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace("min_score = 5", "min_score = 0"),
            REAL.replace("trend_against = -3", "trend_against = 3"),
            REAL.replace("sweep = 2", "sweep = -2"),
            REAL.replace("news_min = -2", "news_min = 1"),
            REAL.replace(r#""0.5""#, r#""-0.5""#),
            REAL.replace("recent_choch", "choch_reciente"),
            REAL.replace("sma_period = 50", "sma_period = 0"),
            REAL.replace("min_m15_candles = 200", "min_m15_candles = 0"),
            REAL.replace("min_h1_candles = 60", "min_h1_candles = 0"),
        ] {
            assert!(StrategyConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

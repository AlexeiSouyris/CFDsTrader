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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrategyConfig {
    pub score: ScoreConfig,
    pub stop: StopConfig,
    pub evidence: EvidenceConfig,
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
        struct Raw {
            score: ScoreConfig,
            stop: RawStop,
            evidence: EvidenceConfig,
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
        Ok(Self {
            score: s,
            stop: StopConfig { atr_buffer },
            evidence: raw.evidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    pub(crate) const REAL: &str = include_str!("../../../../config/strategy.toml");

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
        ] {
            assert!(StrategyConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

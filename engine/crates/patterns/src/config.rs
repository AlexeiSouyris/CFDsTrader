//! `config/patterns.toml`, validado al cargar.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PivotConfig {
    pub strength: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandleConfig {
    pub prior_move_lookback: usize,
    pub min_wick_to_body: Decimal,
    pub max_opposite_wick_to_range: Decimal,
    pub min_body_to_range: Decimal,
    pub min_range_to_atr: Decimal,
    pub atr_period: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternsConfig {
    pub pivots: PivotConfig,
    pub candles: CandleConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    pivots: RawPivots,
    candles: RawCandles,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPivots {
    strength: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCandles {
    prior_move_lookback: usize,
    min_wick_to_body: String,
    max_opposite_wick_to_range: String,
    min_body_to_range: String,
    min_range_to_atr: String,
    atr_period: usize,
}

fn ratio(name: &str, s: &str) -> Result<Decimal, String> {
    let d =
        Decimal::from_str(s).map_err(|e| format!("candles.{name}: {s:?} no es decimal ({e})"))?;
    if d > Decimal::ZERO {
        Ok(d)
    } else {
        Err(format!("candles.{name} tiene que ser mayor que 0"))
    }
}

impl PatternsConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let c = raw.candles;
        if raw.pivots.strength == 0 || c.prior_move_lookback == 0 || c.atr_period == 0 {
            return Err(
                "pivots.strength, prior_move_lookback y atr_period tienen que ser > 0".into(),
            );
        }
        let candles = CandleConfig {
            prior_move_lookback: c.prior_move_lookback,
            min_wick_to_body: ratio("min_wick_to_body", &c.min_wick_to_body)?,
            max_opposite_wick_to_range: ratio(
                "max_opposite_wick_to_range",
                &c.max_opposite_wick_to_range,
            )?,
            min_body_to_range: ratio("min_body_to_range", &c.min_body_to_range)?,
            min_range_to_atr: ratio("min_range_to_atr", &c.min_range_to_atr)?,
            atr_period: c.atr_period,
        };
        if candles.max_opposite_wick_to_range >= Decimal::ONE
            || candles.min_body_to_range >= Decimal::ONE
        {
            return Err("las fracciones del rango tienen que ser menores que 1".into());
        }
        Ok(Self {
            pivots: PivotConfig {
                strength: raw.pivots.strength,
            },
            candles,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/patterns.toml");

    #[test]
    fn the_repository_config_matches_the_spec() {
        let c = PatternsConfig::from_toml(REAL).unwrap();
        assert_eq!(c.pivots.strength, 3);
        assert_eq!(c.candles.prior_move_lookback, 5);
        assert_eq!(c.candles.min_wick_to_body, dec!(2));
        assert_eq!(c.candles.max_opposite_wick_to_range, dec!(0.3));
        assert_eq!(c.candles.min_body_to_range, dec!(0.05));
        assert_eq!(c.candles.min_range_to_atr, dec!(0.5));
        assert_eq!(c.candles.atr_period, 14);
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace("strength = 3", "strength = 0"),
            REAL.replace(r#"min_wick_to_body = "2""#, r#"min_wick_to_body = "-2""#),
            REAL.replace(
                r#"max_opposite_wick_to_range = "0.3""#,
                r#"max_opposite_wick_to_range = "1.5""#,
            ),
            REAL.replace("atr_period", "atr_periodo"),
        ] {
            assert!(PatternsConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

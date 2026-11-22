//! Tendencia (spec §4), pensada para velas de **1h cerradas**:
//!
//! ```text
//! pendiente = SMA(n)[t] − SMA(n)[t − k]
//! cierre > SMA(n) y pendiente > 0  → ALCISTA
//! cierre < SMA(n) y pendiente < 0  → BAJISTA
//! si no                            → NEUTRAL
//! ```
//! Valores iniciales en `config/indicators.toml`: n = 50, k = 5.
//! Calentamiento: `None` hasta tener `n + k` velas.

use cfds_core::Candle;
use serde::Deserialize;

use crate::moving_average::sma;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    Up,
    Down,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrendConfig {
    pub sma_period: usize,
    pub slope_lookback: usize,
}

impl TrendConfig {
    /// Lee la sección `[trend]` de `config/indicators.toml`.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct File {
            trend: TrendConfig,
            /// Lo lee [`crate::DailyConfig`].
            #[allow(dead_code)]
            daily: toml::Table,
        }
        let cfg = toml::from_str::<File>(text)
            .map_err(|e| e.to_string())?
            .trend;
        if cfg.sma_period == 0 || cfg.slope_lookback == 0 {
            return Err(
                "trend.sma_period y trend.slope_lookback tienen que ser mayores que 0".into(),
            );
        }
        Ok(cfg)
    }
}

/// Tendencia en cada vela, alineada por índice (`None` durante el calentamiento).
pub fn trend(candles: &[Candle], cfg: &TrendConfig) -> Vec<Option<Trend>> {
    let avg = sma(candles, cfg.sma_period);
    candles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let now = avg[i]?;
            let before = avg[i.checked_sub(cfg.slope_lookback)?]?;
            let slope = now - before;
            Some(
                if c.close > now && slope.is_sign_positive() && !slope.is_zero() {
                    Trend::Up
                } else if c.close < now && slope.is_sign_negative() && !slope.is_zero() {
                    Trend::Down
                } else {
                    Trend::Neutral
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moving_average::tests::closes;
    use rust_decimal::Decimal;

    fn cfg() -> TrendConfig {
        TrendConfig::from_toml(include_str!("../../../../config/indicators.toml")).unwrap()
    }

    fn small() -> TrendConfig {
        TrendConfig {
            sma_period: 3,
            slope_lookback: 2,
        }
    }

    fn series(v: &[i64]) -> Vec<Candle> {
        closes(&v.iter().map(|&x| Decimal::from(x)).collect::<Vec<_>>())
    }

    #[test]
    fn the_repository_config_is_the_spec() {
        assert_eq!(
            cfg(),
            TrendConfig {
                sma_period: 50,
                slope_lookback: 5
            }
        );
    }

    #[test]
    fn rising_prices_are_an_uptrend_after_warmup() {
        let t = trend(&series(&[1, 2, 3, 4, 5, 6]), &small());
        // SMA(3) existe desde el índice 2; la pendiente necesita 2 velas más → desde el índice 4.
        assert_eq!(&t[..4], &[None; 4]);
        assert_eq!(&t[4..], &[Some(Trend::Up), Some(Trend::Up)]);
    }

    #[test]
    fn falling_prices_are_a_downtrend() {
        let t = trend(&series(&[6, 5, 4, 3, 2, 1]), &small());
        assert_eq!(t[5], Some(Trend::Down));
    }

    #[test]
    fn price_and_slope_disagreeing_is_neutral() {
        // SMA(3) sube (2 → 4 → 9,67) pero el último cierre (6) queda por debajo de la SMA: neutral.
        let t = trend(&series(&[1, 2, 3, 4, 5, 18, 6]), &small());
        assert_eq!(t[6], Some(Trend::Neutral));
        // Precio plano: pendiente 0 → neutral, nunca alcista.
        let flat = trend(&series(&[5, 5, 5, 5, 5, 5]), &small());
        assert_eq!(flat[5], Some(Trend::Neutral));
    }

    #[test]
    fn rejects_zero_parameters() {
        let bad = "[trend]\nsma_period = 0\nslope_lookback = 5\n";
        assert!(TrendConfig::from_toml(bad).is_err());
    }
}

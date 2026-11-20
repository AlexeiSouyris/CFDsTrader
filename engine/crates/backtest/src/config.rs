//! `config/backtest.toml`, validado al cargar.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitConfig {
    pub tp1_r: Decimal,
    pub tp1_fraction: Decimal,
    pub trailing_atr: Decimal,
    pub time_exit_bars: usize,
    pub time_exit_min_r: Decimal,
}

impl ExitConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Exits {
            tp1_r: String,
            tp1_fraction: String,
            trailing_atr: String,
            time_exit_bars: usize,
            time_exit_min_r: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            exits: Exits,
        }
        let e = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .exits;
        let dec = |s: &str, name: &str| -> Result<Decimal, String> {
            let d = Decimal::from_str(s).map_err(|e| format!("{name}: {e}"))?;
            if d <= Decimal::ZERO {
                return Err(format!("{name} tiene que ser mayor que 0"));
            }
            Ok(d)
        };
        let cfg = Self {
            tp1_r: dec(&e.tp1_r, "tp1_r")?,
            tp1_fraction: dec(&e.tp1_fraction, "tp1_fraction")?,
            trailing_atr: dec(&e.trailing_atr, "trailing_atr")?,
            time_exit_bars: e.time_exit_bars,
            time_exit_min_r: dec(&e.time_exit_min_r, "time_exit_min_r")?,
        };
        if cfg.tp1_fraction >= Decimal::ONE {
            return Err(
                "tp1_fraction tiene que ser menor que 1 (queda un resto para el trailing)".into(),
            );
        }
        if cfg.time_exit_bars == 0 {
            return Err("time_exit_bars tiene que ser mayor que 0".into());
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/backtest.toml");

    #[test]
    fn the_repository_config_matches_the_spec() {
        assert_eq!(
            ExitConfig::from_toml(REAL).unwrap(),
            ExitConfig {
                tp1_r: dec!(2),
                tp1_fraction: dec!(0.5),
                trailing_atr: dec!(2),
                time_exit_bars: 32,
                time_exit_min_r: dec!(1),
            }
        );
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace(r#"tp1_fraction = "0.5""#, r#"tp1_fraction = "1""#),
            REAL.replace(r#"tp1_r = "2""#, r#"tp1_r = "0""#),
            REAL.replace("time_exit_bars = 32", "time_exit_bars = 0"),
            REAL.replace("trailing_atr", "trailing"),
        ] {
            assert!(ExitConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

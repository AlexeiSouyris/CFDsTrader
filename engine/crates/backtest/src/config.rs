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

/// Slippage supuesto por ejecución (spec §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostConfig {
    pub slippage_spread_fraction: Decimal,
    pub slippage_ticks: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BacktestConfig {
    pub exits: ExitConfig,
    pub costs: CostConfig,
}

impl ExitConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        Ok(BacktestConfig::from_toml(text)?.exits)
    }
}

impl BacktestConfig {
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
        struct Costs {
            slippage_spread_fraction: String,
            slippage_ticks: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            exits: Exits,
            costs: Costs,
        }
        let raw = toml::from_str::<Raw>(text).map_err(|e| e.to_string())?;
        let e = raw.exits;
        let dec = |s: &str, name: &str| -> Result<Decimal, String> {
            let d = Decimal::from_str(s).map_err(|e| format!("{name}: {e}"))?;
            if d <= Decimal::ZERO {
                return Err(format!("{name} tiene que ser mayor que 0"));
            }
            Ok(d)
        };
        let cfg = ExitConfig {
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
        // El slippage nunca puede ser cero (spec §11: costos nunca en cero).
        let costs = CostConfig {
            slippage_spread_fraction: dec(
                &raw.costs.slippage_spread_fraction,
                "slippage_spread_fraction",
            )?,
            slippage_ticks: dec(&raw.costs.slippage_ticks, "slippage_ticks")?,
        };
        Ok(Self { exits: cfg, costs })
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
        assert_eq!(
            BacktestConfig::from_toml(REAL).unwrap().costs,
            CostConfig {
                slippage_spread_fraction: dec!(0.1),
                slippage_ticks: dec!(1),
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
            REAL.replace(r#"slippage_ticks = "1""#, r#"slippage_ticks = "0""#),
            REAL.replace(
                r#"slippage_spread_fraction = "0.1""#,
                r#"slippage_spread_fraction = "-0.1""#,
            ),
        ] {
            assert!(ExitConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

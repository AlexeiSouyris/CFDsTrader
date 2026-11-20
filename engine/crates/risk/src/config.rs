//! `config/risk.toml`, validado al cargar.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskConfig {
    pub risk_per_trade_pct: Decimal,
    pub max_daily_loss_pct: Decimal,
    pub max_drawdown_pct: Decimal,
    pub max_open_positions: usize,
    pub max_positions_per_instrument: usize,
    pub min_stop_atr: Decimal,
    pub max_stop_atr: Decimal,
}

impl RiskConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Limits {
            risk_per_trade_pct: String,
            max_daily_loss_pct: String,
            max_drawdown_pct: String,
            max_open_positions: usize,
            max_positions_per_instrument: usize,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct StopDistance {
            min_atr: String,
            max_atr: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            limits: Limits,
            stop_distance: StopDistance,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let dec = |s: &str, name: &str| -> Result<Decimal, String> {
            let d = Decimal::from_str(s).map_err(|e| format!("{name}: {e}"))?;
            if d <= Decimal::ZERO {
                return Err(format!("{name} tiene que ser mayor que 0"));
            }
            Ok(d)
        };
        let l = raw.limits;
        let percent = |s: &str, name: &str| -> Result<Decimal, String> {
            let d = dec(s, name)?;
            if d >= Decimal::ONE_HUNDRED {
                return Err(format!("{name} tiene que ser menor que 100"));
            }
            Ok(d)
        };
        let cfg = Self {
            risk_per_trade_pct: percent(&l.risk_per_trade_pct, "risk_per_trade_pct")?,
            max_daily_loss_pct: percent(&l.max_daily_loss_pct, "max_daily_loss_pct")?,
            max_drawdown_pct: percent(&l.max_drawdown_pct, "max_drawdown_pct")?,
            max_open_positions: l.max_open_positions,
            max_positions_per_instrument: l.max_positions_per_instrument,
            min_stop_atr: dec(&raw.stop_distance.min_atr, "min_atr")?,
            max_stop_atr: dec(&raw.stop_distance.max_atr, "max_atr")?,
        };
        if cfg.max_open_positions == 0 || cfg.max_positions_per_instrument == 0 {
            return Err("los máximos de posiciones tienen que ser mayores que 0".into());
        }
        if cfg.min_stop_atr >= cfg.max_stop_atr {
            return Err("stop_distance.min_atr tiene que ser menor que max_atr".into());
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/risk.toml");

    #[test]
    fn the_repository_config_matches_the_spec() {
        assert_eq!(
            RiskConfig::from_toml(REAL).unwrap(),
            RiskConfig {
                risk_per_trade_pct: dec!(1),
                max_daily_loss_pct: dec!(3),
                max_drawdown_pct: dec!(15),
                max_open_positions: 5,
                max_positions_per_instrument: 1,
                min_stop_atr: dec!(0.5),
                max_stop_atr: dec!(3),
            }
        );
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace(r#"risk_per_trade_pct = "1""#, r#"risk_per_trade_pct = "0""#),
            REAL.replace(r#"max_drawdown_pct = "15""#, r#"max_drawdown_pct = "100""#),
            REAL.replace("max_open_positions = 5", "max_open_positions = 0"),
            REAL.replace(r#"min_atr = "0.5""#, r#"min_atr = "3""#),
            REAL.replace("max_atr", "maximo_atr"),
        ] {
            assert!(RiskConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

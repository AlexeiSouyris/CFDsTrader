//! Configuración de la validación, leída de `config/data_validation.toml`.

use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config/data_validation.toml no se pudo leer: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("config/data_validation.toml inválida: {0}")]
    Invalid(&'static str),
}

/// Umbrales de la validación de velas. Se validan al cargar, nunca a mitad de camino.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationConfig {
    /// Toda vela empieza en un múltiplo de estos minutos (UTC).
    pub grid_minutes: u32,
    /// Período del ATR de Wilder usado como escala.
    pub atr_period: usize,
    /// Salto máximo, en múltiplos del ATR, sin corte de sesión.
    pub max_jump_atr_multiple: Decimal,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    grid: RawGrid,
    outliers: RawOutliers,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGrid {
    minutes: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutliers {
    atr_period: usize,
    max_jump_atr_multiple: u32,
}

impl ValidationConfig {
    pub fn from_toml(text: &str) -> Result<Self, ConfigError> {
        let raw: Raw = toml::from_str(text)?;
        if raw.grid.minutes == 0 || 60 % raw.grid.minutes != 0 {
            return Err(ConfigError::Invalid(
                "grid.minutes tiene que dividir a 60 (por ejemplo 5, 15 o 30)",
            ));
        }
        if raw.outliers.atr_period == 0 {
            return Err(ConfigError::Invalid(
                "outliers.atr_period tiene que ser mayor que 0",
            ));
        }
        if raw.outliers.max_jump_atr_multiple == 0 {
            return Err(ConfigError::Invalid(
                "outliers.max_jump_atr_multiple tiene que ser mayor que 0",
            ));
        }
        Ok(Self {
            grid_minutes: raw.grid.minutes,
            atr_period: raw.outliers.atr_period,
            max_jump_atr_multiple: Decimal::from(raw.outliers.max_jump_atr_multiple),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_config_is_valid() {
        let text = include_str!("../../../../config/data_validation.toml");
        let cfg = ValidationConfig::from_toml(text).unwrap();
        assert_eq!(cfg.grid_minutes, 15);
        assert_eq!(cfg.atr_period, 14);
        assert_eq!(cfg.max_jump_atr_multiple, Decimal::from(10));
    }

    #[test]
    fn rejects_invalid_values_and_unknown_keys() {
        let base = |grid: &str, period: &str, mult: &str| {
            format!(
                "[grid]\nminutes = {grid}\n[outliers]\natr_period = {period}\nmax_jump_atr_multiple = {mult}\n"
            )
        };
        assert!(ValidationConfig::from_toml(&base("15", "14", "10")).is_ok());
        assert!(ValidationConfig::from_toml(&base("0", "14", "10")).is_err());
        assert!(ValidationConfig::from_toml(&base("7", "14", "10")).is_err());
        assert!(ValidationConfig::from_toml(&base("15", "0", "10")).is_err());
        assert!(ValidationConfig::from_toml(&base("15", "14", "0")).is_err());
        let typo = base("15", "14", "10").replace("atr_period", "atr_periodo");
        assert!(ValidationConfig::from_toml(&typo).is_err());
    }
}

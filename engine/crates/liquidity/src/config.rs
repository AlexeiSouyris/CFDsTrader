//! `config/liquidity.toml`, validado al cargar.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::levels::{LevelsConfig, RawEqual, RawLevels, RawSessionsUtc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiquidityConfig {
    pub fvg_lookback_bars: usize,
    pub min_impulse_body_atr: Decimal,
    pub atr_period: usize,
    pub choch_recent_bars: usize,
    /// Mapa de liquidez (spec §5.4.1).
    pub levels: LevelsConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    fvg: RawFvg,
    order_blocks: RawOb,
    structure: RawStructure,
    equal: RawEqual,
    levels: RawLevels,
    sessions_utc: RawSessionsUtc,
    // Estadísticas de §5.4.1 (#231): las lee y valida `stats::StatsConfig`.
    #[serde(rename = "fvg_stats")]
    _fvg_stats: toml::Table,
    #[serde(rename = "gaps")]
    _gaps: toml::Table,
    #[serde(rename = "stats")]
    _stats: toml::Table,
    // Desequilibrio de §5.4.1 (#231): lo lee y valida `imbalance::ImbalanceConfig`.
    #[serde(rename = "imbalance")]
    _imbalance: toml::Table,
    // Números redondos y volumen de la barrida (#497): los leen y validan
    // `levels::RoundNumbersConfig` y `sweep::SweepVolumeConfig`.
    #[serde(rename = "round_numbers")]
    _round_numbers: toml::Table,
    #[serde(rename = "sweep_volume")]
    _sweep_volume: toml::Table,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFvg {
    lookback_bars: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOb {
    min_impulse_body_atr: String,
    atr_period: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStructure {
    choch_recent_bars: usize,
}

impl LiquidityConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let body = Decimal::from_str(&raw.order_blocks.min_impulse_body_atr)
            .map_err(|e| format!("order_blocks.min_impulse_body_atr: {e}"))?;
        if body <= Decimal::ZERO
            || raw.fvg.lookback_bars == 0
            || raw.order_blocks.atr_period == 0
            || raw.structure.choch_recent_bars == 0
        {
            return Err("todos los parámetros de liquidez tienen que ser mayores que 0".into());
        }
        crate::stats::StatsConfig::from_toml(text)?;
        crate::imbalance::ImbalanceConfig::from_toml(text)?;
        crate::levels::RoundNumbersConfig::from_toml(text)?;
        crate::sweep::SweepVolumeConfig::from_toml(text)?;
        Ok(Self {
            fvg_lookback_bars: raw.fvg.lookback_bars,
            min_impulse_body_atr: body,
            atr_period: raw.order_blocks.atr_period,
            choch_recent_bars: raw.structure.choch_recent_bars,
            levels: LevelsConfig::from_raw(raw.equal, raw.levels, raw.sessions_utc)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/liquidity.toml");

    #[test]
    fn the_repository_config_matches_the_spec() {
        let c = LiquidityConfig::from_toml(REAL).unwrap();
        assert_eq!(
            c,
            LiquidityConfig {
                fvg_lookback_bars: 50,
                min_impulse_body_atr: dec!(1.5),
                atr_period: 14,
                choch_recent_bars: 12,
                levels: c.levels,
            }
        );
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace("lookback_bars = 50", "lookback_bars = 0"),
            REAL.replace(r#""1.5""#, r#""-1""#),
            REAL.replace("choch_recent_bars", "choch_reciente"),
        ] {
            assert!(LiquidityConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

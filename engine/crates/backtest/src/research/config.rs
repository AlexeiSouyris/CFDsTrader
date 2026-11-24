//! `config/research.toml`, validado al cargar.

use std::str::FromStr;

use chrono::{NaiveDate, NaiveTime};
use rust_decimal::Decimal;
use serde::Deserialize;

/// Familia C (ruptura del rango de apertura).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrbConfig {
    /// Velas de 15m que forman el rango desde la apertura.
    pub range_candles: u32,
    /// Hora local de la bolsa hasta la que puede cerrar la vela de la ruptura.
    pub last_breakout_close: NaiveTime,
}

/// Familia D1 (cambio de mes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnOfMonthConfig {
    /// Día hábil del mes nuevo en cuyo cierre se sale (1 = el primero).
    pub exit_business_day: u32,
    pub atr_period: usize,
    pub stop_atr: Decimal,
}

/// Regla de decisión pre-registrada (#466 §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleConfig {
    pub alpha: Decimal,
    pub dsr_trials: u32,
    pub dsr_min: Decimal,
    pub cost_stress: Decimal,
    pub second_half_from: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchConfig {
    pub orb: OrbConfig,
    pub turn_of_month: TurnOfMonthConfig,
    pub rule: RuleConfig,
}

impl ResearchConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Orb {
            range_candles: u32,
            last_breakout_close: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Tom {
            exit_business_day: u32,
            atr_period: usize,
            stop_atr: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Rule {
            alpha: String,
            dsr_trials: u32,
            dsr_min: String,
            cost_stress: String,
            second_half_from: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            orb: Orb,
            turn_of_month: Tom,
            rule: Rule,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let dec = |s: &str, name: &str| -> Result<Decimal, String> {
            let d = Decimal::from_str(s).map_err(|e| format!("{name}: {e}"))?;
            if d <= Decimal::ZERO {
                return Err(format!("{name} tiene que ser mayor que 0"));
            }
            Ok(d)
        };
        if raw.orb.range_candles == 0 {
            return Err("orb.range_candles tiene que ser mayor que 0".into());
        }
        if raw.turn_of_month.exit_business_day == 0 {
            return Err("turn_of_month.exit_business_day tiene que ser mayor que 0".into());
        }
        if raw.turn_of_month.atr_period == 0 {
            return Err("turn_of_month.atr_period tiene que ser mayor que 0".into());
        }
        let alpha = dec(&raw.rule.alpha, "rule.alpha")?;
        let dsr_min = dec(&raw.rule.dsr_min, "rule.dsr_min")?;
        if alpha >= Decimal::ONE || dsr_min >= Decimal::ONE {
            return Err("rule.alpha y rule.dsr_min tienen que ser menores que 1".into());
        }
        if raw.rule.dsr_trials < 2 {
            return Err("rule.dsr_trials tiene que ser al menos 2".into());
        }
        let cost_stress = dec(&raw.rule.cost_stress, "rule.cost_stress")?;
        if cost_stress < Decimal::ONE {
            return Err("rule.cost_stress no puede achicar los costos".into());
        }
        Ok(Self {
            orb: OrbConfig {
                range_candles: raw.orb.range_candles,
                last_breakout_close: NaiveTime::parse_from_str(
                    &raw.orb.last_breakout_close,
                    "%H:%M",
                )
                .map_err(|_| "orb.last_breakout_close no es HH:MM".to_owned())?,
            },
            turn_of_month: TurnOfMonthConfig {
                exit_business_day: raw.turn_of_month.exit_business_day,
                atr_period: raw.turn_of_month.atr_period,
                stop_atr: dec(&raw.turn_of_month.stop_atr, "turn_of_month.stop_atr")?,
            },
            rule: RuleConfig {
                alpha,
                dsr_trials: raw.rule.dsr_trials,
                dsr_min,
                cost_stress,
                second_half_from: NaiveDate::parse_from_str(&raw.rule.second_half_from, "%Y-%m-%d")
                    .map_err(|_| "rule.second_half_from no es AAAA-MM-DD".to_owned())?,
            },
        })
    }
}

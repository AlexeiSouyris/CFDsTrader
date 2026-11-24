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

/// Familia A (tendencia mensual, time-series momentum), ronda 2 (#486).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrendConfig {
    /// Meses hacia atrás del retorno de la señal.
    pub lookback_months: u32,
    /// Período del ATR diario del stop (y del riesgo).
    pub atr_period: usize,
    pub stop_atr: Decimal,
}

/// Familia B (momentum intradía: primera media hora → última media hora), ronda 2 (#486).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntradayMomentumConfig {
    /// La señal termina al cierre de la vela que termina estos minutos después de la apertura.
    pub signal_minutes_after_open: u32,
    /// La decisión es al cierre de la vela que termina estos minutos antes del cierre.
    pub decision_minutes_before_close: u32,
    /// Período del ATR de 15m del stop (y del riesgo).
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
    /// Regla de la ronda 1 (C y D1).
    pub rule: RuleConfig,
    pub trend: TrendConfig,
    pub intraday_momentum: IntradayMomentumConfig,
    /// Regla de la ronda 2 (A y B): la misma de #466 §4.3, con su nivel y su N.
    pub rule_round2: RuleConfig,
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
        struct Trend {
            lookback_months: u32,
            atr_period: usize,
            stop_atr: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Intraday {
            signal_minutes_after_open: u32,
            decision_minutes_before_close: u32,
            atr_period: usize,
            stop_atr: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            orb: Orb,
            turn_of_month: Tom,
            rule: Rule,
            trend: Trend,
            intraday_momentum: Intraday,
            rule_round2: Rule,
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
        let rule = |r: &Rule, name: &str| -> Result<RuleConfig, String> {
            let alpha = dec(&r.alpha, &format!("{name}.alpha"))?;
            let dsr_min = dec(&r.dsr_min, &format!("{name}.dsr_min"))?;
            if alpha >= Decimal::ONE || dsr_min >= Decimal::ONE {
                return Err(format!(
                    "{name}.alpha y {name}.dsr_min tienen que ser menores que 1"
                ));
            }
            if r.dsr_trials < 2 {
                return Err(format!("{name}.dsr_trials tiene que ser al menos 2"));
            }
            let cost_stress = dec(&r.cost_stress, &format!("{name}.cost_stress"))?;
            if cost_stress < Decimal::ONE {
                return Err(format!("{name}.cost_stress no puede achicar los costos"));
            }
            Ok(RuleConfig {
                alpha,
                dsr_trials: r.dsr_trials,
                dsr_min,
                cost_stress,
                second_half_from: NaiveDate::parse_from_str(&r.second_half_from, "%Y-%m-%d")
                    .map_err(|_| format!("{name}.second_half_from no es AAAA-MM-DD"))?,
            })
        };
        if raw.trend.lookback_months == 0 || raw.trend.atr_period == 0 {
            return Err(
                "trend.lookback_months y trend.atr_period tienen que ser mayores que 0".into(),
            );
        }
        let m = &raw.intraday_momentum;
        if m.atr_period == 0
            || m.signal_minutes_after_open == 0
            || m.decision_minutes_before_close == 0
            || !m.signal_minutes_after_open.is_multiple_of(15)
            || !m.decision_minutes_before_close.is_multiple_of(15)
        {
            return Err(
                "intraday_momentum: los minutos tienen que ser múltiplos de 15 mayores que 0 y \
                 atr_period mayor que 0"
                    .into(),
            );
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
            rule: rule(&raw.rule, "rule")?,
            trend: TrendConfig {
                lookback_months: raw.trend.lookback_months,
                atr_period: raw.trend.atr_period,
                stop_atr: dec(&raw.trend.stop_atr, "trend.stop_atr")?,
            },
            intraday_momentum: IntradayMomentumConfig {
                signal_minutes_after_open: m.signal_minutes_after_open,
                decision_minutes_before_close: m.decision_minutes_before_close,
                atr_period: m.atr_period,
                stop_atr: dec(&m.stop_atr, "intraday_momentum.stop_atr")?,
            },
            rule_round2: rule(&raw.rule_round2, "rule_round2")?,
        })
    }
}

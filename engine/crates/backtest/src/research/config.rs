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

/// Familia V (exposición manejada por volatilidad), ronda 3 (#493).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolManagedConfig {
    /// Peso máximo (veces el capital).
    pub cap: Decimal,
    /// Pares mínimos del entrenamiento para estimar `c`.
    pub min_training_pairs: usize,
}

/// Familia M1 (cascada al cruzar un número redondo), ronda 3 (#493).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundNumberConfig {
    /// Minutos desde el cierre de la vela del cruce hasta la salida.
    pub horizon_minutes: u32,
    pub atr_period: usize,
    pub stop_atr: Decimal,
    /// Paso de los niveles redondos por instrumento (puntos del futuro).
    pub steps: Vec<(String, Decimal)>,
}

impl RoundNumberConfig {
    pub fn step_for(&self, instrument: &str) -> Option<Decimal> {
        self.steps
            .iter()
            .find(|(k, _)| k == instrument)
            .map(|(_, v)| *v)
    }
}

/// Descriptivo de la deriva nocturna, ronda 3 (#493).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvernightDriftConfig {
    pub timezone: chrono_tz::Tz,
    pub from: NaiveTime,
    pub to: NaiveTime,
}

/// Chequeos descriptivos de la revisión externa (#512): fijados antes de mirar, sin voto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewChecksConfig {
    /// Corte entre las dos mitades de la deriva nocturna (la segunda empieza acá).
    pub drift_split_from: NaiveDate,
    /// Días anteriores con los que se mide la caída fuerte.
    pub drop_lookback_days: usize,
    /// Percentil (0–1) bajo el cual el retorno del día es una caída fuerte.
    pub drop_quantile: Decimal,
    /// Peso de comprar y mantener en la cartera mezclada con V.
    pub blend_hold_weight: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchConfig {
    pub orb: OrbConfig,
    pub turn_of_month: TurnOfMonthConfig,
    /// Regla de la ronda 1 (C y D1).
    pub rule: RuleConfig,
    pub trend: TrendConfig,
    pub intraday_momentum: IntradayMomentumConfig,
    /// Regla de la ronda 2 (A y B): la misma de #466 §4.3, con su nivel y su N.
    pub rule_round2: RuleConfig,
    pub vol_managed: VolManagedConfig,
    pub round_number: RoundNumberConfig,
    pub overnight_drift: OvernightDriftConfig,
    /// Regla de la ronda 3 (V y M1), programa 2 (#493).
    pub rule_round3: RuleConfig,
    pub review_checks: ReviewChecksConfig,
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
        struct Vol {
            cap: String,
            min_training_pairs: usize,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Round {
            horizon_minutes: u32,
            atr_period: usize,
            stop_atr: String,
            steps: std::collections::BTreeMap<String, String>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Overnight {
            timezone: String,
            from: String,
            to: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Review {
            drift_split_from: String,
            drop_lookback_days: usize,
            drop_quantile: String,
            blend_hold_weight: String,
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
            vol_managed: Vol,
            round_number: Round,
            overnight_drift: Overnight,
            rule_round3: Rule,
            review_checks: Review,
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
        if raw.vol_managed.min_training_pairs < 2 {
            return Err("vol_managed.min_training_pairs tiene que ser al menos 2".into());
        }
        let rn = &raw.round_number;
        if rn.atr_period == 0 || rn.horizon_minutes == 0 || !rn.horizon_minutes.is_multiple_of(15) {
            return Err(
                "round_number: horizon_minutes múltiplo de 15 mayor que 0 y atr_period mayor que 0"
                    .into(),
            );
        }
        let steps = rn
            .steps
            .iter()
            .map(|(k, v)| Ok((k.clone(), dec(v, &format!("round_number.steps.{k}"))?)))
            .collect::<Result<Vec<_>, String>>()?;
        if steps.is_empty() {
            return Err("round_number.steps está vacío".into());
        }
        let od = &raw.overnight_drift;
        let time = |s: &str, name: &str| {
            NaiveTime::parse_from_str(s, "%H:%M").map_err(|_| format!("{name} no es HH:MM"))
        };
        let overnight_drift = OvernightDriftConfig {
            timezone: od
                .timezone
                .parse()
                .map_err(|_| format!("overnight_drift.timezone desconocida: {}", od.timezone))?,
            from: time(&od.from, "overnight_drift.from")?,
            to: time(&od.to, "overnight_drift.to")?,
        };
        if overnight_drift.to <= overnight_drift.from {
            return Err("overnight_drift: `to` tiene que ser posterior a `from`".into());
        }
        let rc = &raw.review_checks;
        let drop_quantile = dec(&rc.drop_quantile, "review_checks.drop_quantile")?;
        let blend_hold_weight = dec(&rc.blend_hold_weight, "review_checks.blend_hold_weight")?;
        if drop_quantile >= Decimal::ONE || blend_hold_weight > Decimal::ONE {
            return Err(
                "review_checks: drop_quantile tiene que ser menor que 1 y blend_hold_weight no \
                 mayor que 1"
                    .into(),
            );
        }
        if rc.drop_lookback_days < 2 {
            return Err("review_checks.drop_lookback_days tiene que ser al menos 2".into());
        }
        let review_checks = ReviewChecksConfig {
            drift_split_from: NaiveDate::parse_from_str(&rc.drift_split_from, "%Y-%m-%d")
                .map_err(|_| "review_checks.drift_split_from no es AAAA-MM-DD".to_owned())?,
            drop_lookback_days: rc.drop_lookback_days,
            drop_quantile,
            blend_hold_weight,
        };
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
            vol_managed: VolManagedConfig {
                cap: dec(&raw.vol_managed.cap, "vol_managed.cap")?,
                min_training_pairs: raw.vol_managed.min_training_pairs,
            },
            round_number: RoundNumberConfig {
                horizon_minutes: rn.horizon_minutes,
                atr_period: rn.atr_period,
                stop_atr: dec(&rn.stop_atr, "round_number.stop_atr")?,
                steps,
            },
            overnight_drift,
            rule_round3: rule(&raw.rule_round3, "rule_round3")?,
            review_checks,
        })
    }
}

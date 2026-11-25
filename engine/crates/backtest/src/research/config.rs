//! `config/research.toml`, validado al cargar.

use std::str::FromStr;

use chrono::{NaiveDate, NaiveTime, Timelike};
use rust_decimal::Decimal;
use serde::Deserialize;

use super::pre_fomc::PreFomcConfig;

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

/// H2 del programa Nasdaq (#526, pre-registro §4.2): días de CPI y empleo. Congelada; se corre
/// cuando esté cargado el BLS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroDayConfig {
    pub timezone: chrono_tz::Tz,
    /// Hora local de la vela de la entrada el día del dato.
    pub entry_time: NaiveTime,
    /// Spread de la entrada (fuera de la sesión de contado), en puntos.
    pub out_of_session_spread: Decimal,
}

/// Programa 4 (#561, `docs/analysis/2026-11-programa-4-preregistro.md`): estrategia A de
/// Bruni, literal (H4.1) y normalizada (H4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program4Config {
    /// ATR de Wilder de 15m (umbral de H4.2 y riesgo), reiniciado en cada vencimiento.
    pub atr_period: usize,
    /// Velas válidas del color contrario que necesita una zona para romperse (§3).
    pub min_valid_in_zone: usize,
    /// Cuerpo mínimo de H4.1, en puntos.
    pub body_points: Decimal,
    /// k de H4.2 (cuerpo ≥ k × ATR). `None` mientras no se calibró (§5.1): sin k no corre.
    pub body_atr_k: Option<Decimal>,
    /// Objetivo en múltiplos de R.
    pub target_r: Decimal,
    /// La vela de ejecución empieza antes del cierre de la sesión menos estos minutos.
    pub last_entry_minutes_before_close: u32,
    /// Salida al cierre de la última vela que termina antes del cierre menos estos minutos.
    pub exit_minutes_before_close: u32,
    /// Período de la calibración de k: `[desde, hasta)`.
    pub k_reference: (NaiveDate, NaiveDate),
    /// Redondeo de k.
    pub k_step: Decimal,
    /// Comunicados que excluyen el día entero (`fuente/serie` de `official_releases`).
    pub strong_releases: Vec<(String, String)>,
    /// Mínimo de operaciones para pasar.
    pub min_trades: usize,
    pub luck_simulations: usize,
    pub luck_seed: u64,
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
    /// Regla del programa Nasdaq (H1 y H2 en US100, #526).
    pub rule_nasdaq: RuleConfig,
    /// H1 del programa Nasdaq: horas de la ventana antes del FOMC.
    pub pre_fomc: PreFomcConfig,
    pub macro_day: MacroDayConfig,
    /// Regla del programa 4 (#561).
    pub rule_program4: RuleConfig,
    pub program4: Program4Config,
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
            rule_nasdaq: Rule,
            pre_fomc: PreFomc,
            macro_day: MacroDay,
            rule_program4: Rule,
            program4: Program4,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Program4 {
            atr_period: usize,
            min_valid_in_zone: usize,
            body_points: String,
            body_atr_k: Option<String>,
            target_r: String,
            last_entry_minutes_before_close: u32,
            exit_minutes_before_close: u32,
            k_reference_from: String,
            k_reference_to: String,
            k_step: String,
            strong_releases: Vec<String>,
            min_trades: usize,
            luck_simulations: usize,
            luck_seed: u64,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PreFomc {
            timezone: String,
            entry_time: String,
            exit_minutes_before: u32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct MacroDay {
            timezone: String,
            entry_time: String,
            out_of_session_spread_points: String,
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
        let tz = |s: &str, name: &str| -> Result<chrono_tz::Tz, String> {
            s.parse()
                .map_err(|_| format!("{name}: zona horaria desconocida: {s}"))
        };
        let pf = &raw.pre_fomc;
        if pf.exit_minutes_before == 0 || !pf.exit_minutes_before.is_multiple_of(15) {
            return Err("pre_fomc.exit_minutes_before: múltiplo de 15 mayor que 0".into());
        }
        let pre_fomc = PreFomcConfig {
            timezone: tz(&pf.timezone, "pre_fomc.timezone")?,
            entry_time: time(&pf.entry_time, "pre_fomc.entry_time")?,
            exit_minutes_before: pf.exit_minutes_before,
        };
        let md = &raw.macro_day;
        let macro_day = MacroDayConfig {
            timezone: tz(&md.timezone, "macro_day.timezone")?,
            entry_time: time(&md.entry_time, "macro_day.entry_time")?,
            out_of_session_spread: dec(
                &md.out_of_session_spread_points,
                "macro_day.out_of_session_spread_points",
            )?,
        };
        for (t, name) in [
            (pre_fomc.entry_time, "pre_fomc.entry_time"),
            (macro_day.entry_time, "macro_day.entry_time"),
        ] {
            if !t.minute().is_multiple_of(15) || t.second() != 0 {
                return Err(format!("{name} tiene que caer en una vela de 15 minutos"));
            }
        }
        let review_checks = ReviewChecksConfig {
            drift_split_from: NaiveDate::parse_from_str(&rc.drift_split_from, "%Y-%m-%d")
                .map_err(|_| "review_checks.drift_split_from no es AAAA-MM-DD".to_owned())?,
            drop_lookback_days: rc.drop_lookback_days,
            drop_quantile,
            blend_hold_weight,
        };
        let p4 = &raw.program4;
        let date = |s: &str, name: &str| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| format!("{name} no es AAAA-MM-DD"))
        };
        let k_reference = (
            date(&p4.k_reference_from, "program4.k_reference_from")?,
            date(&p4.k_reference_to, "program4.k_reference_to")?,
        );
        if p4.atr_period == 0
            || p4.min_valid_in_zone == 0
            || p4.min_trades == 0
            || p4.luck_simulations == 0
            || k_reference.1 <= k_reference.0
            || !p4.last_entry_minutes_before_close.is_multiple_of(15)
            || !p4.exit_minutes_before_close.is_multiple_of(15)
            || p4.exit_minutes_before_close >= p4.last_entry_minutes_before_close
        {
            return Err(
                "program4: períodos y mínimos mayores que 0, minutos múltiplos de 15 con la \
                 salida después de la última entrada, y k_reference_from < k_reference_to"
                    .into(),
            );
        }
        let strong_releases = p4
            .strong_releases
            .iter()
            .map(|s| {
                s.split_once('/')
                    .filter(|(a, b)| !a.is_empty() && !b.is_empty())
                    .map(|(a, b)| (a.to_owned(), b.to_owned()))
                    .ok_or_else(|| format!("program4.strong_releases: \"{s}\" no es fuente/serie"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let program4 = Program4Config {
            atr_period: p4.atr_period,
            min_valid_in_zone: p4.min_valid_in_zone,
            body_points: dec(&p4.body_points, "program4.body_points")?,
            body_atr_k: p4
                .body_atr_k
                .as_deref()
                .map(|k| dec(k, "program4.body_atr_k"))
                .transpose()?,
            target_r: dec(&p4.target_r, "program4.target_r")?,
            last_entry_minutes_before_close: p4.last_entry_minutes_before_close,
            exit_minutes_before_close: p4.exit_minutes_before_close,
            k_reference,
            k_step: dec(&p4.k_step, "program4.k_step")?,
            strong_releases,
            min_trades: p4.min_trades,
            luck_simulations: p4.luck_simulations,
            luck_seed: p4.luck_seed,
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
            rule_nasdaq: rule(&raw.rule_nasdaq, "rule_nasdaq")?,
            pre_fomc,
            macro_day,
            rule_program4: rule(&raw.rule_program4, "rule_program4")?,
            program4,
        })
    }
}

//! Reglas de una cuenta fondeada (`prop_rules.v1`): el documento del contrato (con valores en
//! blanco en una plantilla), su validación para simular ([`Rules`], todo en dinero y en
//! fracciones) y las plantillas de `config/propsim.toml`.

use chrono::NaiveTime;
use chrono_tz::Tz;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

const HUNDRED: Decimal = Decimal::ONE_HUNDRED;

/// El reglamento publicado de una firma real.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSource {
    pub firm: String,
    pub url: String,
    pub read_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DayCut {
    pub timezone: String,
    pub cutoff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseDoc {
    pub name: String,
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub profit_target_pct: Option<Decimal>,
    #[serde(default)]
    pub min_trading_days: Option<u32>,
    #[serde(default)]
    pub max_trading_days: Option<u32>,
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub consistency_pct: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossBase {
    AccountSize,
    DayStart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyLossDoc {
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub limit_pct: Option<Decimal>,
    pub base: LossBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawdownMode {
    Static,
    TrailingEod,
    TrailingIntraday,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaxDrawdownDoc {
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub limit_pct: Option<Decimal>,
    pub mode: DrawdownMode,
    pub lock_at_initial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundedDoc {
    #[serde(default)]
    pub payout_min_trading_days: Option<u32>,
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub payout_min_profit_pct: Option<Decimal>,
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub profit_split_pct: Option<Decimal>,
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub consistency_pct: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeesDoc {
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub evaluation: Option<Decimal>,
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub activation: Option<Decimal>,
    pub refund_with_first_payout: bool,
}

/// `prop_rules.v1`. En una plantilla los valores pueden faltar (en blanco); en JSON viajan
/// como `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesDoc {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub source: Option<RuleSource>,
    #[serde(default, with = "cfds_core::decimal_str::option")]
    pub account_size: Option<Decimal>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub day: Option<DayCut>,
    pub phases: Vec<PhaseDoc>,
    #[serde(default)]
    pub daily_loss: Option<DailyLossDoc>,
    #[serde(default)]
    pub max_drawdown: Option<MaxDrawdownDoc>,
    #[serde(default)]
    pub funded: Option<FundedDoc>,
    pub fees: FeesDoc,
}

/// Un error de las reglas: `path` es un JSON Pointer dentro de `prop_rules.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleError {
    pub path: String,
    pub message: String,
}

fn err(path: impl Into<String>, message: impl Into<String>) -> RuleError {
    RuleError {
        path: path.into(),
        message: message.into(),
    }
}

/// Una fase de evaluación lista para simular (montos en dinero).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase {
    pub name: String,
    /// Ganancia que hay que alcanzar.
    pub target: Decimal,
    pub min_days: u32,
    pub max_days: Option<u32>,
    /// Mejor día / ganancia total como mucho (fracción).
    pub consistency: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyLoss {
    /// Fracción (0,05 = 5 %).
    pub limit: Decimal,
    pub base: LossBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxDrawdown {
    /// En dinero.
    pub amount: Decimal,
    pub mode: DrawdownMode,
    pub lock_at_initial: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Funded {
    pub min_days: u32,
    /// Ganancia mínima para el primer cobro, en dinero (0 = cualquier ganancia).
    pub min_profit: Decimal,
    /// Parte del trader (fracción).
    pub split: Decimal,
    pub consistency: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fees {
    pub evaluation: Decimal,
    pub activation: Decimal,
    pub refund_with_first_payout: bool,
}

/// Reglas validadas para simular.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    pub account_size: Decimal,
    pub currency: String,
    pub timezone: Tz,
    pub cutoff: NaiveTime,
    pub phases: Vec<Phase>,
    pub daily_loss: Option<DailyLoss>,
    pub max_drawdown: Option<MaxDrawdown>,
    pub funded: Option<Funded>,
    pub fees: Fees,
}

impl Rules {
    /// Reglas que no se pueden simular con operaciones cerradas: se dejan afuera (no se
    /// aproximan) y las probabilidades de pasar quedan como tope superior.
    pub fn unsimulable(&self) -> Vec<String> {
        match self.max_drawdown {
            Some(MaxDrawdown {
                mode: DrawdownMode::TrailingIntraday,
                ..
            }) => vec!["max_drawdown.trailing_intraday".into()],
            _ => Vec::new(),
        }
    }
}

/// Un porcentaje obligatorio en (0, 100] → fracción.
fn pct(v: Option<Decimal>, path: &str, errors: &mut Vec<RuleError>) -> Decimal {
    match v {
        None => {
            errors.push(err(path, "falta este valor para simular"));
            Decimal::ZERO
        }
        Some(p) if p <= Decimal::ZERO || p > HUNDRED => {
            errors.push(err(path, "tiene que estar en (0, 100]"));
            Decimal::ZERO
        }
        Some(p) => p / HUNDRED,
    }
}

/// Un porcentaje opcional en (0, 100] → fracción (`None` = la regla no existe).
fn opt_pct(v: Option<Decimal>, path: &str, errors: &mut Vec<RuleError>) -> Option<Decimal> {
    v.map(|_| pct(v, path, errors))
}

fn money(v: Option<Decimal>, path: &str, errors: &mut Vec<RuleError>) -> Decimal {
    v.unwrap_or_else(|| {
        errors.push(err(path, "falta este valor para simular"));
        Decimal::ZERO
    })
}

fn is_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

impl RulesDoc {
    /// Las reglas listas para simular, o **todos** los errores (valores en blanco, fuera de
    /// rango o incoherentes).
    pub fn validate(&self) -> Result<Rules, Vec<RuleError>> {
        let mut e = Vec::new();
        let size = money(self.account_size, "/account_size", &mut e);
        if self.account_size.is_some() && size <= Decimal::ZERO {
            e.push(err("/account_size", "tiene que ser mayor que 0"));
        }
        let currency = self.currency.clone().unwrap_or_default();
        if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_uppercase()) {
            e.push(err("/currency", "código ISO de 3 letras (por ejemplo USD)"));
        }
        let (timezone, cutoff) = match &self.day {
            None => {
                e.push(err("/day", "falta el corte del día de la firma"));
                (Tz::UTC, NaiveTime::MIN)
            }
            Some(d) => {
                let tz = d.timezone.parse::<Tz>().unwrap_or_else(|_| {
                    e.push(err("/day/timezone", "zona horaria IANA desconocida"));
                    Tz::UTC
                });
                let cut = NaiveTime::parse_from_str(&d.cutoff, "%H:%M").unwrap_or_else(|_| {
                    e.push(err("/day/cutoff", "hora HH:MM"));
                    NaiveTime::MIN
                });
                (tz, cut)
            }
        };
        if self.phases.is_empty() || self.phases.len() > 3 {
            e.push(err("/phases", "de 1 a 3 fases de evaluación"));
        }
        let phases = self
            .phases
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let at = |f: &str| format!("/phases/{k}/{f}");
                if p.name.trim().is_empty() {
                    e.push(err(at("name"), "falta el nombre"));
                }
                let target = pct(p.profit_target_pct, &at("profit_target_pct"), &mut e) * size;
                if p.max_trading_days == Some(0) {
                    e.push(err(at("max_trading_days"), "tiene que ser ≥ 1"));
                } else if let (Some(min), Some(max)) = (p.min_trading_days, p.max_trading_days)
                    && min > max
                {
                    e.push(err(
                        at("max_trading_days"),
                        "tiene que ser ≥ los días mínimos",
                    ));
                }
                Phase {
                    name: p.name.clone(),
                    target,
                    min_days: p.min_trading_days.unwrap_or(0),
                    max_days: p.max_trading_days,
                    consistency: opt_pct(p.consistency_pct, &at("consistency_pct"), &mut e),
                }
            })
            .collect();
        let daily_loss = self.daily_loss.as_ref().map(|d| DailyLoss {
            limit: pct(d.limit_pct, "/daily_loss/limit_pct", &mut e),
            base: d.base,
        });
        let max_drawdown = self.max_drawdown.as_ref().map(|d| MaxDrawdown {
            amount: pct(d.limit_pct, "/max_drawdown/limit_pct", &mut e) * size,
            mode: d.mode,
            lock_at_initial: d.lock_at_initial,
        });
        let funded = self.funded.as_ref().map(|f| Funded {
            min_days: f.payout_min_trading_days.unwrap_or(0),
            min_profit: f.payout_min_profit_pct.map_or(Decimal::ZERO, |p| {
                if p > HUNDRED {
                    e.push(err(
                        "/funded/payout_min_profit_pct",
                        "tiene que estar en [0, 100]",
                    ));
                }
                p / HUNDRED * size
            }),
            split: pct(f.profit_split_pct, "/funded/profit_split_pct", &mut e),
            consistency: opt_pct(f.consistency_pct, "/funded/consistency_pct", &mut e),
        });
        let fees = Fees {
            evaluation: money(self.fees.evaluation, "/fees/evaluation", &mut e),
            activation: self.fees.activation.unwrap_or(Decimal::ZERO),
            refund_with_first_payout: self.fees.refund_with_first_payout,
        };
        if self.fees.refund_with_first_payout && self.funded.is_none() {
            e.push(err(
                "/fees/refund_with_first_payout",
                "la devolución con el primer cobro necesita la etapa fondeada",
            ));
        }
        if let Some(s) = &self.source
            && (s.firm.trim().is_empty() || !s.url.starts_with("https://") || !is_date(&s.read_at))
        {
            e.push(err(
                "/source",
                "firma, enlace https y fecha de lectura AAAA-MM-DD",
            ));
        }
        if e.is_empty() {
            Ok(Rules {
                account_size: size,
                currency,
                timezone,
                cutoff,
                phases,
                daily_loss,
                max_drawdown,
                funded,
                fees,
            })
        } else {
            Err(e)
        }
    }

    /// ¿Trae algún número? Una plantilla con números es de una firma real y necesita fuente.
    pub fn has_values(&self) -> bool {
        let phases = self.phases.iter().any(|p| {
            p.profit_target_pct.is_some()
                || p.min_trading_days.is_some()
                || p.max_trading_days.is_some()
                || p.consistency_pct.is_some()
        });
        let funded = self.funded.as_ref().is_some_and(|f| {
            f.payout_min_trading_days.is_some()
                || f.payout_min_profit_pct.is_some()
                || f.profit_split_pct.is_some()
                || f.consistency_pct.is_some()
        });
        self.account_size.is_some()
            || self.currency.is_some()
            || self.day.is_some()
            || phases
            || self
                .daily_loss
                .as_ref()
                .is_some_and(|d| d.limit_pct.is_some())
            || self
                .max_drawdown
                .as_ref()
                .is_some_and(|d| d.limit_pct.is_some())
            || funded
            || self.fees.evaluation.is_some()
            || self.fees.activation.is_some()
    }
}

/// `config/propsim.toml`, validado al arrancar.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropsimConfig {
    pub limits: Limits,
    pub monte_carlo: crate::montecarlo::McConfig,
    pub templates: Vec<RulesDoc>,
}

/// `[limits]`: lo más que se puede pedir (educativo, PENDIENTE del dueño).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Operaciones de una lista.
    pub max_trades: usize,
    /// Riesgo por operación máximo, en % del tamaño de la cuenta.
    pub max_risk_pct: Decimal,
}

impl PropsimConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let cfg: Self = toml::from_str(text).map_err(|e| format!("propsim.toml: {e}"))?;
        if cfg.limits.max_trades == 0
            || cfg.limits.max_risk_pct <= Decimal::ZERO
            || cfg.limits.max_risk_pct > HUNDRED
        {
            return Err("propsim.toml: [limits] inválido".into());
        }
        cfg.monte_carlo.validate()?;
        if cfg.templates.is_empty() {
            return Err("propsim.toml: hace falta al menos la plantilla genérica".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for t in &cfg.templates {
            if !ids.insert(t.id.as_str()) {
                return Err(format!("propsim.toml: plantilla repetida {}", t.id));
            }
            let valid_id = !t.id.is_empty()
                && t.id.len() <= 48
                && t.id.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'
                });
            if !valid_id || t.name.trim().is_empty() || t.phases.is_empty() || t.phases.len() > 3 {
                return Err(format!("propsim.toml: plantilla {} mal formada", t.id));
            }
            // Una plantilla con números es de una firma real: sin su reglamento citado, no.
            if t.has_values() {
                let ok = t.source.as_ref().is_some_and(|s| {
                    !s.firm.trim().is_empty()
                        && s.url.starts_with("https://")
                        && is_date(&s.read_at)
                });
                if !ok {
                    return Err(format!(
                        "propsim.toml: la plantilla {} trae valores sin `source` (firma, enlace \
                         https y fecha de lectura del reglamento publicado)",
                        t.id
                    ));
                }
                if let Err(errs) = t.validate() {
                    return Err(format!("propsim.toml: plantilla {}: {errs:?}", t.id));
                }
            }
        }
        Ok(cfg)
    }
}

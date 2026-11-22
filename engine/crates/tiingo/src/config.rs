//! `config/tiingo.toml` (ADR 0009 §3), validado al arrancar.

use std::collections::BTreeMap;

use cfds_core::InstrumentId;
use chrono::NaiveTime;
use chrono_tz::Tz;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::TiingoError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TiingoConfig {
    pub base_url: String,
    pub timeout_secs: u64,
    pub max_bytes_per_request: u64,
    pub budget: Budget,
    /// Nuestro instrumento → ETF.
    pub pairs: BTreeMap<InstrumentId, String>,
    pub verify: VerifyConfig,
}

/// Topes propios, contados con ventanas móviles sobre el registro de uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub requests_per_hour: u64,
    pub requests_per_day: u64,
    pub symbols_per_31_days: u64,
    pub bytes_per_31_days: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyConfig {
    pub timezone: Tz,
    pub reference_close: NaiveTime,
    /// Puntos porcentuales.
    pub tolerance_m15_pp: Decimal,
    pub tolerance_daily_pp: Decimal,
    pub max_days: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    api: RawApi,
    plan: RawPlan,
    budget: Budget,
    pairs: BTreeMap<String, String>,
    verify: RawVerify,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApi {
    base_url: String,
    timeout_secs: u64,
    max_bytes_per_request: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlan {
    requests_per_hour: u64,
    requests_per_day: u64,
    symbols_per_month: u64,
    bytes_per_month: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVerify {
    timezone: String,
    reference_close: String,
    tolerance_m15_pp: String,
    tolerance_daily_pp: String,
    max_days: u32,
}

fn err(msg: impl Into<String>) -> TiingoError {
    TiingoError::Config(msg.into())
}

/// Símbolo de un ETF: mayúsculas, dígitos, `.` y `-` (igual que la tabla).
pub(crate) fn valid_symbol(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 16
        && (b[0].is_ascii_uppercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == b'.' || *c == b'-')
}

impl TiingoConfig {
    pub fn from_toml(text: &str) -> Result<Self, TiingoError> {
        let raw: Raw = toml::from_str(text).map_err(|e| err(e.to_string()))?;
        if !raw.api.base_url.starts_with("https://") || raw.api.base_url.ends_with('/') {
            return Err(err("api.base_url tiene que ser https:// y sin / al final"));
        }
        if raw.api.timeout_secs == 0 || raw.api.max_bytes_per_request == 0 {
            return Err(err(
                "api.timeout_secs y api.max_bytes_per_request tienen que ser > 0",
            ));
        }
        let (b, p) = (raw.budget, raw.plan);
        let below = [
            (
                "requests_per_hour",
                b.requests_per_hour,
                p.requests_per_hour,
            ),
            ("requests_per_day", b.requests_per_day, p.requests_per_day),
            (
                "symbols_per_31_days",
                b.symbols_per_31_days,
                p.symbols_per_month,
            ),
            ("bytes_per_31_days", b.bytes_per_31_days, p.bytes_per_month),
        ];
        for (name, own, plan) in below {
            if own == 0 || own >= plan {
                return Err(err(format!(
                    "budget.{name} = {own} tiene que ser > 0 y menor que el límite del plan ({plan})"
                )));
            }
        }
        if raw.api.max_bytes_per_request > b.bytes_per_31_days {
            return Err(err(
                "api.max_bytes_per_request no puede pasar de budget.bytes_per_31_days",
            ));
        }
        let mut pairs = BTreeMap::new();
        for (ours, etf) in raw.pairs {
            let id: InstrumentId = ours
                .parse()
                .map_err(|e| err(format!("pairs: instrumento inválido {ours:?}: {e}")))?;
            if !valid_symbol(&etf) {
                return Err(err(format!("pairs.{ours}: símbolo inválido {etf:?}")));
            }
            pairs.insert(id, etf);
        }
        if pairs.is_empty() {
            return Err(err("pairs está vacío"));
        }
        let v = raw.verify;
        let timezone: Tz = v
            .timezone
            .parse()
            .map_err(|_| err(format!("verify.timezone inválida: {:?}", v.timezone)))?;
        let reference_close = NaiveTime::parse_from_str(&v.reference_close, "%H:%M")
            .map_err(|_| err("verify.reference_close tiene que ser HH:MM"))?;
        let tol = |name: &str, s: &str| -> Result<Decimal, TiingoError> {
            let d: Decimal = s
                .parse()
                .map_err(|_| err(format!("verify.{name} tiene que ser un decimal en texto")))?;
            if d <= Decimal::ZERO || d > Decimal::from(10) {
                return Err(err(format!("verify.{name} tiene que estar en (0, 10]")));
            }
            Ok(d)
        };
        if v.max_days == 0 || v.max_days > 366 {
            return Err(err("verify.max_days tiene que estar en [1, 366]"));
        }
        Ok(Self {
            base_url: raw.api.base_url,
            timeout_secs: raw.api.timeout_secs,
            max_bytes_per_request: raw.api.max_bytes_per_request,
            budget: b,
            pairs,
            verify: VerifyConfig {
                timezone,
                reference_close,
                tolerance_m15_pp: tol("tolerance_m15_pp", &v.tolerance_m15_pp)?,
                tolerance_daily_pp: tol("tolerance_daily_pp", &v.tolerance_daily_pp)?,
                max_days: v.max_days,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../../../config/tiingo.toml");

    #[test]
    fn shipped_config_is_valid_and_below_the_plan() {
        let c = TiingoConfig::from_toml(REAL).unwrap();
        assert_eq!(c.pairs.len(), 3);
        assert!(c.budget.requests_per_hour < 50 && c.budget.requests_per_day < 1000);
        assert!(
            !c.pairs
                .contains_key(&"DE40".parse::<InstrumentId>().unwrap())
        );
    }

    #[test]
    fn caps_at_or_above_the_plan_are_rejected() {
        let bad = REAL.replace("requests_per_hour = 40", "requests_per_hour = 50");
        assert!(
            TiingoConfig::from_toml(&bad)
                .unwrap_err()
                .to_string()
                .contains("requests_per_hour")
        );
        let http = REAL.replace("https://api.tiingo.com", "http://api.tiingo.com");
        assert!(TiingoConfig::from_toml(&http).is_err());
        let sym = REAL.replace("\"SPY\"", "\"spy\"");
        assert!(TiingoConfig::from_toml(&sym).is_err());
        let tol = REAL.replace("\"0.10\"", "\"0\"");
        assert!(TiingoConfig::from_toml(&tol).is_err());
    }
}

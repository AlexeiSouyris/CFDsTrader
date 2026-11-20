//! `config/ig.toml`: entorno, límites de uso y reintentos. Se valida al arrancar.

use std::time::Duration;

use serde::Deserialize;

/// Único host permitido: la cuenta demo (CLAUDE.md §5). Pasar a real requiere un ADR.
pub const DEMO_BASE_URL: &str = "https://demo-api.ig.com/gateway/deal";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IgConfigError {
    #[error("config/ig.toml no es TOML válido: {0}")]
    Parse(String),
    #[error("config/ig.toml, {field}: {problem}")]
    Invalid {
        field: &'static str,
        problem: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgConfig {
    pub base_url: String,
    pub request_timeout: Duration,
    pub requests_per_minute: u32,
    pub historical_points_per_week: u64,
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub page_size: u32,
    pub local_time_margin_hours: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    api: RawApi,
    limits: RawLimits,
    retry: RawRetry,
    history: RawHistory,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawApi {
    environment: String,
    base_url: String,
    request_timeout_secs: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLimits {
    non_trading_requests_per_minute: u32,
    historical_points_per_week: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRetry {
    max_attempts: u32,
    initial_backoff_secs: u64,
    max_backoff_secs: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHistory {
    page_size: u32,
    local_time_margin_hours: u32,
}

fn invalid(field: &'static str, problem: impl Into<String>) -> IgConfigError {
    IgConfigError::Invalid {
        field,
        problem: problem.into(),
    }
}

fn at_least_one<T: PartialOrd + From<u8>>(field: &'static str, v: T) -> Result<T, IgConfigError> {
    if v >= T::from(1) {
        Ok(v)
    } else {
        Err(invalid(field, "tiene que ser al menos 1"))
    }
}

impl IgConfig {
    pub fn from_toml(text: &str) -> Result<Self, IgConfigError> {
        let r: Raw = toml::from_str(text).map_err(|e| IgConfigError::Parse(e.to_string()))?;
        if r.api.environment != "demo" {
            return Err(invalid(
                "api.environment",
                "solo se permite \"demo\" (cambiarlo requiere un ADR)",
            ));
        }
        if r.api.base_url.trim_end_matches('/') != DEMO_BASE_URL {
            return Err(invalid(
                "api.base_url",
                format!("solo se permite {DEMO_BASE_URL}"),
            ));
        }
        let initial = at_least_one("retry.initial_backoff_secs", r.retry.initial_backoff_secs)?;
        let max = at_least_one("retry.max_backoff_secs", r.retry.max_backoff_secs)?;
        if max < initial {
            return Err(invalid(
                "retry.max_backoff_secs",
                "no puede ser menor que initial_backoff_secs",
            ));
        }
        if r.history.local_time_margin_hours > 24 {
            return Err(invalid(
                "history.local_time_margin_hours",
                "más de 24 h gasta cupo sin sentido",
            ));
        }
        Ok(Self {
            base_url: DEMO_BASE_URL.to_owned(),
            request_timeout: Duration::from_secs(at_least_one(
                "api.request_timeout_secs",
                r.api.request_timeout_secs,
            )?),
            requests_per_minute: at_least_one(
                "limits.non_trading_requests_per_minute",
                r.limits.non_trading_requests_per_minute,
            )?,
            historical_points_per_week: at_least_one(
                "limits.historical_points_per_week",
                r.limits.historical_points_per_week,
            )?,
            max_attempts: at_least_one("retry.max_attempts", r.retry.max_attempts)?,
            initial_backoff: Duration::from_secs(initial),
            max_backoff: Duration::from_secs(max),
            page_size: at_least_one("history.page_size", r.history.page_size)?,
            local_time_margin_hours: r.history.local_time_margin_hours,
        })
    }

    /// Espera antes del reintento `attempt` (1 = primer reintento).
    pub fn backoff(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        self.initial_backoff
            .saturating_mul(factor)
            .min(self.max_backoff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../../../config/ig.toml");

    #[test]
    fn repository_config_loads_with_the_published_limits() {
        let c = IgConfig::from_toml(REAL).unwrap();
        assert_eq!(c.base_url, DEMO_BASE_URL);
        assert_eq!(c.requests_per_minute, 30);
        assert_eq!(c.historical_points_per_week, 10_000);
        assert_eq!(c.max_attempts, 4);
    }

    #[test]
    fn only_the_demo_environment_is_accepted() {
        let live = REAL.replace("environment = \"demo\"", "environment = \"live\"");
        assert!(matches!(
            IgConfig::from_toml(&live),
            Err(IgConfigError::Invalid {
                field: "api.environment",
                ..
            })
        ));
        let live_url = REAL.replace("demo-api.ig.com", "api.ig.com");
        assert!(matches!(
            IgConfig::from_toml(&live_url),
            Err(IgConfigError::Invalid {
                field: "api.base_url",
                ..
            })
        ));
        let trailing = REAL.replace("/gateway/deal\"", "/gateway/deal/\"");
        assert!(IgConfig::from_toml(&trailing).is_ok());
    }

    #[test]
    fn invalid_numbers_are_rejected_at_startup() {
        for (from, to, field) in [
            ("max_attempts = 4", "max_attempts = 0", "retry.max_attempts"),
            ("page_size = 500", "page_size = 0", "history.page_size"),
            (
                "max_backoff_secs = 60",
                "max_backoff_secs = 1",
                "retry.max_backoff_secs",
            ),
            (
                "local_time_margin_hours = 1",
                "local_time_margin_hours = 25",
                "history.local_time_margin_hours",
            ),
            (
                "non_trading_requests_per_minute = 30",
                "non_trading_requests_per_minute = 0",
                "limits.non_trading_requests_per_minute",
            ),
        ] {
            let text = REAL.replace(from, to);
            assert_ne!(text, REAL, "{from}");
            assert!(
                matches!(IgConfig::from_toml(&text), Err(IgConfigError::Invalid { field: f, .. }) if f == field),
                "{field}"
            );
        }
        assert!(matches!(
            IgConfig::from_toml(&format!("{REAL}\n[extra]\nx = 1\n")),
            Err(IgConfigError::Parse(_))
        ));
    }

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let c = IgConfig::from_toml(REAL).unwrap();
        let secs: Vec<u64> = (1..=7).map(|a| c.backoff(a).as_secs()).collect();
        assert_eq!(secs, [2, 4, 8, 16, 32, 60, 60]);
        assert_eq!(c.backoff(1000).as_secs(), 60);
    }
}

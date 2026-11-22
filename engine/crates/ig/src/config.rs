//! `config/ig.toml`: entorno, límites de uso y reintentos. Se valida al arrancar.

use std::time::Duration;

use chrono::TimeDelta;

use cfds_core::Timeframe;
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
    /// Zona de la cuenta: límites de las velas diarias (#103).
    pub prices_tz: chrono_tz::Tz,
    /// Demora máxima del inicio de la vela diaria (#103).
    pub daily_start_tolerance: TimeDelta,
    pub live: LiveConfig,
    /// Cada cuánto se graba el sentimiento de clientes de IG (#27).
    pub sentiment_interval: Duration,
    /// Límites antes de una carga histórica (#143).
    pub loads: LoadLimits,
    /// Presupuesto semanal del cupo por uso (#167).
    pub budget: UsageBudget,
}

/// Presupuesto semanal del cupo de históricos por uso (#167, `[budget]`). Suma ≤ cupo semanal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageBudget {
    /// `ig_live` en la máquina que guarda los datos.
    pub live: u64,
    /// `ig_backfill` e `ig_backfill_daily` en la máquina que guarda los datos.
    pub history: u64,
    /// Todo lo pedido desde una máquina sin `CFDS_DATA_MACHINE=1`.
    pub off_data_machine: u64,
    /// Ningún comando deja el cupo restante de la cuenta por debajo de esto.
    pub reserve: u64,
}

/// Límites antes de una carga histórica (#143, `[loads]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadLimits {
    /// Arriba de estos puntos estimados se pide confirmación (salvo `--yes`).
    pub confirm_above_points: u64,
    /// Tope de una carga en una máquina sin `CFDS_DATA_MACHINE=1`.
    pub max_points_off_data_machine: u64,
}

/// Modo en vivo (#68).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveConfig {
    pub timeframes: Vec<Timeframe>,
    pub candle_delay: Duration,
    pub give_up_after: Duration,
    pub startup_lookback: Duration,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    api: RawApi,
    limits: RawLimits,
    retry: RawRetry,
    history: RawHistory,
    live: RawLive,
    sentiment: RawSentiment,
    loads: RawLoads,
    budget: RawBudget,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBudget {
    live: u64,
    history: u64,
    off_data_machine: u64,
    reserve: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLoads {
    confirm_above_points: u64,
    max_points_off_data_machine: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSentiment {
    interval_minutes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLive {
    timeframes: Vec<String>,
    candle_delay_secs: u64,
    give_up_after_secs: u64,
    startup_lookback_hours: u64,
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
    prices_timezone: String,
    daily_start_tolerance_minutes: u32,
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
        let prices_tz: chrono_tz::Tz = r
            .history
            .prices_timezone
            .parse()
            .map_err(|_| invalid("history.prices_timezone", "zona horaria desconocida"))?;
        let mut timeframes = Vec::new();
        for t in &r.live.timeframes {
            let tf: Timeframe = t
                .parse()
                .map_err(|e: String| invalid("live.timeframes", e))?;
            if timeframes.contains(&tf) {
                return Err(invalid("live.timeframes", format!("{t} repetido")));
            }
            timeframes.push(tf);
        }
        if timeframes.is_empty() {
            return Err(invalid("live.timeframes", "no puede estar vacío"));
        }
        if r.live.give_up_after_secs <= r.live.candle_delay_secs {
            return Err(invalid(
                "live.give_up_after_secs",
                "tiene que ser mayor que candle_delay_secs",
            ));
        }
        let live = LiveConfig {
            timeframes,
            candle_delay: Duration::from_secs(r.live.candle_delay_secs),
            give_up_after: Duration::from_secs(r.live.give_up_after_secs),
            startup_lookback: Duration::from_secs(
                at_least_one("live.startup_lookback_hours", r.live.startup_lookback_hours)?
                    .saturating_mul(3600),
            ),
        };
        let minutes = at_least_one("sentiment.interval_minutes", r.sentiment.interval_minutes)?;
        if minutes > 24 * 60 || (24 * 60) % minutes != 0 {
            return Err(invalid(
                "sentiment.interval_minutes",
                "tiene que dividir exacto al día (1, 5, 15, 30, 60...)",
            ));
        }
        let loads = LoadLimits {
            confirm_above_points: at_least_one(
                "loads.confirm_above_points",
                r.loads.confirm_above_points,
            )?,
            max_points_off_data_machine: at_least_one(
                "loads.max_points_off_data_machine",
                r.loads.max_points_off_data_machine,
            )?,
        };
        for (field, v) in [
            ("loads.confirm_above_points", loads.confirm_above_points),
            (
                "loads.max_points_off_data_machine",
                loads.max_points_off_data_machine,
            ),
        ] {
            if v > r.limits.historical_points_per_week {
                return Err(invalid(field, "no puede superar el cupo semanal"));
            }
        }
        let budget = UsageBudget {
            live: at_least_one("budget.live", r.budget.live)?,
            history: at_least_one("budget.history", r.budget.history)?,
            off_data_machine: at_least_one("budget.off_data_machine", r.budget.off_data_machine)?,
            reserve: r.budget.reserve,
        };
        let sum = [
            budget.live,
            budget.history,
            budget.off_data_machine,
            budget.reserve,
        ]
        .iter()
        .try_fold(0u64, |acc, v| acc.checked_add(*v));
        if sum.is_none_or(|s| s > r.limits.historical_points_per_week) {
            return Err(invalid(
                "budget",
                format!(
                    "live + history + off_data_machine + reserve no puede superar el cupo \
                     semanal ({})",
                    r.limits.historical_points_per_week
                ),
            ));
        }
        if loads.max_points_off_data_machine > budget.off_data_machine {
            return Err(invalid(
                "loads.max_points_off_data_machine",
                "no puede superar budget.off_data_machine",
            ));
        }
        Ok(Self {
            budget,
            loads,
            sentiment_interval: Duration::from_secs(minutes * 60),
            live,
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
            prices_tz,
            daily_start_tolerance: {
                let m = r.history.daily_start_tolerance_minutes;
                if m >= 24 * 60 {
                    return Err(invalid(
                        "history.daily_start_tolerance_minutes",
                        "tiene que ser menor que un día",
                    ));
                }
                TimeDelta::minutes(i64::from(m))
            },
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
        assert_eq!(c.prices_tz, chrono_tz::Europe::Madrid);
        assert_eq!(c.daily_start_tolerance, TimeDelta::hours(1));
        assert_eq!(
            c.loads,
            LoadLimits {
                confirm_above_points: 500,
                max_points_off_data_machine: 100,
            }
        );
    }

    #[test]
    fn load_limits_are_validated() {
        for (old, new) in [
            ("confirm_above_points = 500", "confirm_above_points = 0"),
            (
                "max_points_off_data_machine = 100",
                "max_points_off_data_machine = 0",
            ),
            ("confirm_above_points = 500", "confirm_above_points = 10001"),
        ] {
            assert!(
                IgConfig::from_toml(&REAL.replace(old, new)).is_err(),
                "{new}"
            );
        }
    }

    #[test]
    fn usage_budget_is_validated() {
        assert_eq!(
            IgConfig::from_toml(REAL).unwrap().budget,
            UsageBudget {
                live: 3_400,
                history: 5_000,
                off_data_machine: 1_000,
                reserve: 600,
            }
        );
        for (old, new, field) in [
            ("history = 5000", "history = 5001", "budget"),
            ("reserve = 600", "reserve = 18446744073709551615", "budget"),
            ("live = 3400", "live = 0", "budget.live"),
            ("history = 5000", "history = 0", "budget.history"),
            (
                "off_data_machine = 1000",
                "off_data_machine = 99",
                "loads.max_points_off_data_machine",
            ),
        ] {
            let text = REAL.replace(old, new);
            assert_ne!(text, REAL, "{new}");
            assert!(
                matches!(IgConfig::from_toml(&text), Err(IgConfigError::Invalid { field: f, .. }) if f == field),
                "{new}"
            );
        }
        // Sin reserva vale, mientras la suma entre en el cupo.
        assert!(IgConfig::from_toml(&REAL.replace("reserve = 600", "reserve = 0")).is_ok());
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
                "prices_timezone = \"Europe/Madrid\"",
                "prices_timezone = \"Europe/Nowhere\"",
                "history.prices_timezone",
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

    #[test]
    fn live_section_is_validated() {
        let c = IgConfig::from_toml(REAL).unwrap();
        assert_eq!(c.live.timeframes, [Timeframe::M15, Timeframe::H1]);
        assert_eq!(c.live.candle_delay.as_secs(), 20);
        assert_eq!(c.live.give_up_after.as_secs(), 600);
        assert_eq!(c.live.startup_lookback.as_secs(), 24 * 3600);
        for (from, to) in [
            (
                r#"timeframes = ["15m", "1h"]"#,
                r#"timeframes = ["15m", "4h"]"#,
            ),
            (
                r#"timeframes = ["15m", "1h"]"#,
                r#"timeframes = ["15m", "15m"]"#,
            ),
            (r#"timeframes = ["15m", "1h"]"#, r#"timeframes = []"#),
            ("give_up_after_secs = 600", "give_up_after_secs = 20"),
            ("startup_lookback_hours = 24", "startup_lookback_hours = 0"),
        ] {
            let text = REAL.replace(from, to);
            assert_ne!(text, REAL, "{to}");
            assert!(
                matches!(IgConfig::from_toml(&text), Err(IgConfigError::Invalid { field, .. }) if field.starts_with("live.")),
                "{to}"
            );
        }
    }

    #[test]
    fn sentiment_interval_divides_the_day() {
        assert_eq!(
            IgConfig::from_toml(REAL)
                .unwrap()
                .sentiment_interval
                .as_secs(),
            15 * 60
        );
        for bad in [
            "interval_minutes = 0",
            "interval_minutes = 7",
            "interval_minutes = 2880",
        ] {
            let text = REAL.replace("interval_minutes = 15", bad);
            assert_ne!(text, REAL);
            assert!(
                matches!(
                    IgConfig::from_toml(&text),
                    Err(IgConfigError::Invalid {
                        field: "sentiment.interval_minutes",
                        ..
                    })
                ),
                "{bad}"
            );
        }
    }
}

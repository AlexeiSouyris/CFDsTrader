//! `config/terminal.toml`: qué instrumentos se muestran, con qué ritmo se piden datos y qué ventanas.
//! Se valida al arrancar; con algo fuera de rango el programa no se conecta.

use std::time::Duration;

use cfds_core::InstrumentId;
use cfds_ibkr::{BarSize, ContractKind, IbkrConfig};
use cfds_market_data::SessionOffsets;
use chrono::TimeDelta;
use serde::Deserialize;

/// Pausa mínima entre pedidos: con 11 s nunca se llega a 60 pedidos en 10 minutos (límite de IBKR).
pub const MIN_GAP_SECS: u64 = 11;

/// Ventana máxima por pedido según la tabla de duraciones de IBKR, en horas.
pub fn max_window_hours(size: BarSize) -> u32 {
    match size {
        BarSize::M1 => 24,
        BarSize::M15 => 168,
        BarSize::H1 => 720,
        BarSize::D1 => 365 * 24,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalConfigError {
    #[error("config/terminal.toml no es TOML válido: {0}")]
    Parse(String),
    #[error("config/terminal.toml, {field}: {problem}")]
    Invalid {
        field: &'static str,
        problem: String,
    },
}

fn invalid(field: &'static str, problem: impl Into<String>) -> TerminalConfigError {
    TerminalConfigError::Invalid {
        field,
        problem: problem.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalConfig {
    pub client_id: i32,
    pub instruments: Vec<InstrumentId>,
    pub timeframes: Vec<BarSize>,
    pub initial_timeframe: BarSize,
    pub min_request_gap: TimeDelta,
    pub quote_every: TimeDelta,
    pub chart_every: TimeDelta,
    pub daily_every: TimeDelta,
    pub redraw: Duration,
    pub quote_lookback: TimeDelta,
    pub chart_1m: TimeDelta,
    pub chart_15m: TimeDelta,
    pub chart_1h: TimeDelta,
    pub chart_1d: TimeDelta,
    pub daily_lookback: TimeDelta,
    pub stock_session_offset: TimeDelta,
    pub other_session_offset: TimeDelta,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    gateway: RawGateway,
    display: RawDisplay,
    session: RawSession,
    refresh: RawRefresh,
    lookback: RawLookback,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGateway {
    client_id: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDisplay {
    instruments: Vec<String>,
    timeframes: Vec<String>,
    initial_timeframe: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSession {
    stock_offset_hours: i64,
    other_offset_hours: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRefresh {
    min_request_gap_secs: u64,
    quote_secs: u64,
    chart_secs: u64,
    daily_secs: u64,
    redraw_millis: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLookback {
    quote_hours: u32,
    chart_1m_hours: u32,
    chart_15m_hours: u32,
    chart_1h_hours: u32,
    chart_1d_days: u32,
    daily_days: u32,
}

fn secs(field: &'static str, v: u64) -> Result<TimeDelta, TerminalConfigError> {
    if v == 0 || v > 86_400 {
        return Err(invalid(field, "tiene que estar entre 1 y 86400 segundos"));
    }
    Ok(TimeDelta::seconds(v as i64))
}

fn window(
    field: &'static str,
    hours: u32,
    size: BarSize,
) -> Result<TimeDelta, TerminalConfigError> {
    let max = max_window_hours(size);
    if hours == 0 || hours > max {
        return Err(invalid(
            field,
            format!(
                "tiene que estar entre 1 y {max} horas (velas de {})",
                size.as_str()
            ),
        ));
    }
    Ok(TimeDelta::hours(i64::from(hours)))
}

impl TerminalConfig {
    pub fn from_toml(text: &str) -> Result<Self, TerminalConfigError> {
        let raw: Raw =
            toml::from_str(text).map_err(|e| TerminalConfigError::Parse(e.to_string()))?;
        if raw.gateway.client_id < 0 {
            return Err(invalid("gateway.client_id", "no puede ser negativo"));
        }
        let d = raw.display;
        if d.instruments.is_empty() {
            return Err(invalid("display.instruments", "falta al menos uno"));
        }
        let mut instruments: Vec<InstrumentId> = Vec::new();
        for s in &d.instruments {
            let id: InstrumentId = s
                .parse()
                .map_err(|_| invalid("display.instruments", format!("{s:?} no es un id")))?;
            if instruments.contains(&id) {
                return Err(invalid("display.instruments", format!("{s} está repetido")));
            }
            instruments.push(id);
        }
        let mut timeframes: Vec<BarSize> = Vec::new();
        for s in &d.timeframes {
            let b: BarSize = s.parse().map_err(|e| invalid("display.timeframes", e))?;
            if timeframes.contains(&b) {
                return Err(invalid("display.timeframes", format!("{s} está repetido")));
            }
            timeframes.push(b);
        }
        if timeframes.is_empty() {
            return Err(invalid("display.timeframes", "falta al menos uno"));
        }
        let initial_timeframe: BarSize = d
            .initial_timeframe
            .parse()
            .map_err(|e| invalid("display.initial_timeframe", e))?;
        if !timeframes.contains(&initial_timeframe) {
            return Err(invalid(
                "display.initial_timeframe",
                "tiene que estar en display.timeframes",
            ));
        }
        let r = raw.refresh;
        if r.min_request_gap_secs < MIN_GAP_SECS {
            return Err(invalid(
                "refresh.min_request_gap_secs",
                format!(
                    "no puede ser menor que {MIN_GAP_SECS} (límite de 60 pedidos en 10 minutos)"
                ),
            ));
        }
        if !(100..=60_000).contains(&r.redraw_millis) {
            return Err(invalid(
                "refresh.redraw_millis",
                "tiene que estar entre 100 y 60000",
            ));
        }
        // Mismo criterio y validación que el gateway (#551).
        let session = SessionOffsets::from_hours(
            raw.session.stock_offset_hours,
            raw.session.other_offset_hours,
        )
        .map_err(|(field, problem)| invalid(field, problem))?;
        let l = raw.lookback;
        if l.chart_1d_days == 0 || l.chart_1d_days > 365 {
            return Err(invalid(
                "lookback.chart_1d_days",
                "tiene que estar entre 1 y 365",
            ));
        }
        if !(2..=60).contains(&l.daily_days) {
            return Err(invalid(
                "lookback.daily_days",
                "tiene que estar entre 2 y 60",
            ));
        }
        Ok(Self {
            client_id: raw.gateway.client_id,
            instruments,
            timeframes,
            initial_timeframe,
            min_request_gap: secs("refresh.min_request_gap_secs", r.min_request_gap_secs)?,
            quote_every: secs("refresh.quote_secs", r.quote_secs)?,
            chart_every: secs("refresh.chart_secs", r.chart_secs)?,
            daily_every: secs("refresh.daily_secs", r.daily_secs)?,
            redraw: Duration::from_millis(r.redraw_millis),
            quote_lookback: window("lookback.quote_hours", l.quote_hours, BarSize::M1)?,
            chart_1m: window("lookback.chart_1m_hours", l.chart_1m_hours, BarSize::M1)?,
            chart_15m: window("lookback.chart_15m_hours", l.chart_15m_hours, BarSize::M15)?,
            chart_1h: window("lookback.chart_1h_hours", l.chart_1h_hours, BarSize::H1)?,
            chart_1d: TimeDelta::days(i64::from(l.chart_1d_days)),
            daily_lookback: TimeDelta::days(i64::from(l.daily_days)),
            stock_session_offset: session.stock,
            other_session_offset: session.other,
        })
    }

    /// Contra `config/ibkr.toml`: cada instrumento existe allá y el `client_id` es otro (que el de
    /// `[gateway]` y el de `[live]`, #558).
    pub fn check_against(&self, ibkr: &IbkrConfig) -> Result<(), TerminalConfigError> {
        if self.client_id == ibkr.client_id {
            return Err(invalid(
                "gateway.client_id",
                format!(
                    "es el mismo que el de config/ibkr.toml ({}): usá uno propio",
                    ibkr.client_id
                ),
            ));
        }
        if let Some(live) = ibkr.live.as_ref().filter(|l| l.client_id == self.client_id) {
            return Err(invalid(
                "gateway.client_id",
                format!(
                    "es el mismo que [live] de config/ibkr.toml ({}): usá uno propio",
                    live.client_id
                ),
            ));
        }
        for id in &self.instruments {
            if ibkr.instrument(id).is_none() {
                return Err(invalid(
                    "display.instruments",
                    format!("{id} no está en config/ibkr.toml"),
                ));
            }
        }
        Ok(())
    }

    /// Ventana del gráfico para un timeframe.
    pub fn chart_window(&self, size: BarSize) -> TimeDelta {
        match size {
            BarSize::M1 => self.chart_1m,
            BarSize::M15 => self.chart_15m,
            BarSize::H1 => self.chart_1h,
            BarSize::D1 => self.chart_1d,
        }
    }

    /// Corrimiento del día de sesión según el tipo de contrato.
    pub fn session_offset(&self, kind: ContractKind) -> TimeDelta {
        match kind {
            ContractKind::Stock => self.stock_session_offset,
            _ => self.other_session_offset,
        }
    }

    /// Tope de pedidos de velas en 10 minutos que garantiza la pausa mínima.
    pub fn max_requests_per_10_min(&self) -> i64 {
        600 / self.min_request_gap.num_seconds().max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../../../config/terminal.toml");
    const IBKR: &str = include_str!("../../../../config/ibkr.toml");

    #[test]
    fn real_config_is_valid_and_matches_ibkr_toml() {
        let c = TerminalConfig::from_toml(REAL).unwrap();
        let ibkr = IbkrConfig::from_toml(IBKR).unwrap();
        c.check_against(&ibkr).unwrap();
        assert!(c.max_requests_per_10_min() < 60);
        assert!(c.timeframes.contains(&c.initial_timeframe));
    }

    fn with(from: &str, to: &str) -> Result<TerminalConfig, TerminalConfigError> {
        assert!(REAL.contains(from), "{from}");
        TerminalConfig::from_toml(&REAL.replace(from, to))
    }

    #[test]
    fn rejects_gap_below_pacing_floor() {
        let e = with("min_request_gap_secs = 15", "min_request_gap_secs = 5").unwrap_err();
        assert!(e.to_string().contains("min_request_gap_secs"), "{e}");
    }

    #[test]
    fn rejects_windows_outside_ibkr_duration_table() {
        assert!(with("chart_1m_hours = 24", "chart_1m_hours = 25").is_err());
        assert!(with("chart_15m_hours = 72", "chart_15m_hours = 169").is_err());
        assert!(with("chart_1h_hours = 240", "chart_1h_hours = 721").is_err());
        assert!(with("quote_hours = 24", "quote_hours = 0").is_err());
        assert!(with("chart_1d_days = 200", "chart_1d_days = 366").is_err());
    }

    #[test]
    fn rejects_bad_lists() {
        assert!(
            with(
                r#"instruments = ["MNQ", "MES", "QQQ"]"#,
                r#"instruments = []"#
            )
            .is_err()
        );
        assert!(
            with(
                r#"instruments = ["MNQ", "MES", "QQQ"]"#,
                r#"instruments = ["MNQ", "MNQ"]"#
            )
            .is_err()
        );
        assert!(
            with(
                r#"initial_timeframe = "15m""#,
                r#"initial_timeframe = "4h""#
            )
            .is_err()
        );
        assert!(
            with(
                r#"timeframes = ["1m", "15m", "1h", "1d"]"#,
                r#"timeframes = ["1m", "1h"]"#
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_unknown_instrument_and_shared_client_id() {
        let ibkr = IbkrConfig::from_toml(IBKR).unwrap();
        let c = with(
            r#"instruments = ["MNQ", "MES", "QQQ"]"#,
            r#"instruments = ["MNQ", "NOPE"]"#,
        )
        .unwrap();
        assert!(c.check_against(&ibkr).is_err());
        let c = with("client_id = 19", &format!("client_id = {}", ibkr.client_id)).unwrap();
        assert!(c.check_against(&ibkr).is_err());
        // Ni el de la vela en formación (#558).
        let live = ibkr.live.as_ref().expect("[live]").client_id;
        let c = with("client_id = 19", &format!("client_id = {live}")).unwrap();
        assert!(c.check_against(&ibkr).is_err());
        let c = TerminalConfig::from_toml(REAL).unwrap();
        assert!(c.check_against(&ibkr).is_ok());
    }

    #[test]
    fn session_offsets_by_kind() {
        let c = TerminalConfig::from_toml(REAL).unwrap();
        assert_eq!(c.session_offset(ContractKind::Stock), TimeDelta::hours(-5));
        assert_eq!(c.session_offset(ContractKind::Future), TimeDelta::hours(2));
        assert!(with("stock_offset_hours = -5", "stock_offset_hours = -13").is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(with("[refresh]", "[refresh]\nturbo = true").is_err());
    }
}

//! Una corrida de backtest (`contracts/backtest_run.v1`, #200): con qué datos, configuración,
//! costos y noticias se corrió, y el resumen de sus métricas. Sus operaciones son las
//! propuestas con el mismo `run_id` (`proposal.v1`, sin cambios).

use cfds_backtest::{Commission, CostConfig, InstrumentCosts, Metrics};
use cfds_core::InstrumentId;
use cfds_core::candle::utc_z;
use cfds_core::decimal_str;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// `contracts/backtest_run.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BacktestRun {
    pub run_id: String,
    pub instruments: Vec<InstrumentId>,
    /// Timeframe de decisión.
    pub timeframe: RunTimeframe,
    #[serde(with = "utc_z")]
    pub from: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub to: DateTime<Utc>,
    pub config_sha256: String,
    #[serde(with = "decimal_str")]
    pub initial_equity: Decimal,
    pub costs: RunCosts,
    pub news: Option<RunNews>,
    pub metrics: RunMetrics,
    #[serde(with = "utc_z")]
    pub started_at: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub finished_at: DateTime<Utc>,
}

/// Timeframe de decisión de la corrida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunTimeframe {
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "1h")]
    H1,
}

impl RunTimeframe {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::M15 => "15m",
            Self::H1 => "1h",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCosts {
    #[serde(with = "decimal_str")]
    pub slippage_spread_fraction: Decimal,
    #[serde(with = "decimal_str")]
    pub slippage_ticks: Decimal,
    pub instruments: Vec<RunInstrumentCosts>,
    /// De dónde sale el spread (#267). Opcional en el contrato: ausente en las corridas
    /// anteriores, que eran todas con las velas de IG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spread_source: Option<SpreadSource>,
}

/// De dónde sale el spread de la corrida (#267).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpreadSource {
    /// Velas de IG: el filtro de spread §7.3 usa el spread medido de cada vela; los costos, el
    /// spread típico de config.
    Measured,
    /// Velas de futuros: no hay spread medido del CFD para esas fechas. Los costos usan el
    /// spread típico de config (nunca cero) y el filtro §7.3 no se aplica.
    Config,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunInstrumentCosts {
    pub instrument_id: InstrumentId,
    #[serde(with = "decimal_str")]
    pub spread: Decimal,
    #[serde(with = "decimal_str")]
    pub tick: Decimal,
    pub commission: RunCommission,
    #[serde(with = "decimal_str")]
    pub funding_admin_fee_pct: Decimal,
    #[serde(with = "decimal_str")]
    pub funding_day_count: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunCommission {
    None,
    PerShare {
        #[serde(with = "decimal_str")]
        per_share: Decimal,
        #[serde(with = "decimal_str")]
        min: Decimal,
    },
    Percent {
        #[serde(with = "decimal_str")]
        pct: Decimal,
        #[serde(with = "decimal_str")]
        min: Decimal,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunNews {
    pub interpreter_name: String,
    pub interpreter_version: String,
    pub interpreter_model: Option<String>,
    #[serde(with = "utc_z")]
    pub published_from: DateTime<Utc>,
    pub interpretations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunMetrics {
    pub trades: u64,
    #[serde(with = "decimal_str::signed")]
    pub final_equity: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub total_return: Decimal,
    #[serde(with = "decimal_str")]
    pub max_drawdown: Decimal,
    #[serde(with = "decimal_str::signed_option")]
    pub sharpe: Option<Decimal>,
    #[serde(with = "decimal_str::signed_option")]
    pub sortino: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub win_rate: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub profit_factor: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub exposure: Option<Decimal>,
}

impl RunMetrics {
    pub fn new(final_equity: Decimal, m: &Metrics) -> Self {
        Self {
            trades: u64::try_from(m.trades).unwrap_or(u64::MAX),
            final_equity: final_equity.normalize(),
            total_return: m.total_return.normalize(),
            max_drawdown: m.max_drawdown.normalize(),
            sharpe: m.sharpe.map(|d| d.normalize()),
            sortino: m.sortino.map(|d| d.normalize()),
            win_rate: m.win_rate.map(|d| d.normalize()),
            profit_factor: m.profit_factor.map(|d| d.normalize()),
            exposure: m.exposure.map(|d| d.normalize()),
        }
    }
}

impl RunCosts {
    pub fn new(cfg: &CostConfig, instruments: &[(InstrumentId, InstrumentCosts)]) -> Self {
        Self {
            slippage_spread_fraction: cfg.slippage_spread_fraction,
            slippage_ticks: cfg.slippage_ticks,
            instruments: instruments
                .iter()
                .map(|(id, c)| RunInstrumentCosts {
                    instrument_id: id.clone(),
                    spread: c.spread,
                    tick: c.tick,
                    commission: match c.commission {
                        Commission::None => RunCommission::None,
                        Commission::PerShare { per_share, min } => {
                            RunCommission::PerShare { per_share, min }
                        }
                        Commission::Percent { pct, min } => RunCommission::Percent { pct, min },
                    },
                    funding_admin_fee_pct: c.funding_admin_fee_pct,
                    funding_day_count: c.funding_day_count,
                })
                .collect(),
            spread_source: None,
        }
    }

    /// La misma, diciendo de dónde sale el spread (#267).
    pub fn with_spread_source(mut self, source: SpreadSource) -> Self {
        self.spread_source = Some(source);
        self
    }
}

/// Lo que el contrato pide y el JSON Schema no puede expresar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunInvalid {
    #[error("finished_at ({finished}) es anterior a started_at ({started})")]
    FinishedBeforeStarted {
        started: DateTime<Utc>,
        finished: DateTime<Utc>,
    },
    #[error("el período está vacío o invertido: {from} → {to}")]
    EmptyPeriod {
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    },
    #[error("los costos no son de los mismos instrumentos que la corrida")]
    CostsMismatch,
    #[error("instrumento repetido en la corrida: {0}")]
    RepeatedInstrument(InstrumentId),
    #[error("{field} fuera del formato del contrato: {value:?}")]
    Format { field: &'static str, value: String },
}

impl BacktestRun {
    pub fn validate(&self) -> Result<(), RunInvalid> {
        let id_ok = (1..=100).contains(&self.run_id.len())
            && self
                .run_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        if !id_ok {
            return Err(RunInvalid::Format {
                field: "run_id",
                value: self.run_id.clone(),
            });
        }
        let hash_ok = self.config_sha256.len() == 64
            && self
                .config_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !hash_ok {
            return Err(RunInvalid::Format {
                field: "config_sha256",
                value: self.config_sha256.clone(),
            });
        }
        if self.instruments.is_empty() {
            return Err(RunInvalid::CostsMismatch);
        }
        for (i, id) in self.instruments.iter().enumerate() {
            if self.instruments[..i].contains(id) {
                return Err(RunInvalid::RepeatedInstrument(id.clone()));
            }
        }
        if self.finished_at < self.started_at {
            return Err(RunInvalid::FinishedBeforeStarted {
                started: self.started_at,
                finished: self.finished_at,
            });
        }
        if self.to <= self.from {
            return Err(RunInvalid::EmptyPeriod {
                from: self.from,
                to: self.to,
            });
        }
        let mut a: Vec<_> = self.instruments.iter().collect();
        let mut b: Vec<_> = self
            .costs
            .instruments
            .iter()
            .map(|c| &c.instrument_id)
            .collect();
        a.sort();
        b.sort();
        if a != b {
            return Err(RunInvalid::CostsMismatch);
        }
        Ok(())
    }

    /// La misma corrida: todo igual salvo cuándo empezó y terminó.
    pub fn same_run(&self, other: &Self) -> bool {
        let strip = |r: &Self| Self {
            started_at: DateTime::<Utc>::UNIX_EPOCH,
            finished_at: DateTime::<Utc>::UNIX_EPOCH,
            ..r.clone()
        };
        strip(self) == strip(other)
    }
}

/// SHA-256 (hex) de los archivos de configuración usados: cada uno con su nombre, en orden
/// alfabético, para que el orden en que se pasan no cambie el hash.
pub fn config_sha256(files: &[(&str, &str)]) -> String {
    let mut sorted = files.to_vec();
    sorted.sort_by_key(|(name, _)| *name);
    let mut h = Sha256::new();
    for (name, text) in sorted {
        // Largo y contenido: ninguna concatenación de nombres y textos se confunde con otra.
        for part in [name, text] {
            h.update((part.len() as u64).to_be_bytes());
            h.update(part.as_bytes());
        }
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_config_hash_ignores_order_but_not_names_or_content() {
        let a = config_sha256(&[("a.toml", "x = 1"), ("b.toml", "y = 2")]);
        assert_eq!(a.len(), 64);
        assert!(
            a.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert_eq!(
            a,
            config_sha256(&[("b.toml", "y = 2"), ("a.toml", "x = 1")])
        );
        assert_ne!(
            a,
            config_sha256(&[("a.toml", "x = 2"), ("b.toml", "y = 2")])
        );
        assert_ne!(
            a,
            config_sha256(&[("c.toml", "x = 1"), ("b.toml", "y = 2")])
        );
        // Mover texto entre el nombre y el contenido cambia el hash.
        assert_ne!(config_sha256(&[("ab", "c")]), config_sha256(&[("a", "bc")]));
    }
}

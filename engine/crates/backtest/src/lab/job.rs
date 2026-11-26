//! `lab_job_request.v1` y `lab_job_event.v1` (#774): el pedido de una corrida del laboratorio
//! y sus pasos. Lo guarda `cfds_journal::lab` (tablas de solo agregar), lo corre el `lab_worker`
//! y el gateway lo encola y retransmite. Acá solo los tipos, el chequeo de las variantes y el id
//! del trabajo: funciones puras.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::catalog::StrategyCatalog;
use super::spec::{StrategySpec, canonical_json, sha256_hex};
use super::validation::ValidSpec;

/// `lab_job_request.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabJobRequest {
    pub specs: Vec<StrategySpec>,
    #[serde(with = "cfds_core::decimal_str")]
    pub initial_equity: Decimal,
}

impl LabJobRequest {
    /// El pedido con las specs ya validadas (normalizadas) y el capital sin ceros de más: lo que
    /// se guarda y se hashea, así `"2.0"` y `"2"` dan el mismo trabajo.
    pub fn normalized(specs: &[ValidSpec], initial_equity: Decimal) -> Self {
        Self {
            specs: specs.iter().map(|v| v.spec.clone()).collect(),
            initial_equity: initial_equity.normalize(),
        }
    }
}

/// SHA-256 de la forma canónica de un pedido: los hashes de sus specs validadas, en orden, y el
/// capital sin ceros de más.
pub fn request_sha256(specs: &[ValidSpec], initial_equity: Decimal) -> String {
    let hashes: Vec<&str> = specs.iter().map(|v| v.spec_sha256.as_str()).collect();
    let key = json!({
        "specs": hashes,
        "initial_equity": initial_equity.normalize().to_string(),
    });
    sha256_hex(&canonical_json(&key))
}

/// Id del trabajo: SHA-256 del hash del pedido y su intento (1, 2… después de un fallo o una
/// cancelación). Mismo pedido y mismo intento, mismo trabajo.
pub fn job_id(request_sha256: &str, attempt: u32) -> String {
    sha256_hex(&format!("{request_sha256}:{attempt}"))
}

/// Las variantes de un trabajo: de 1 a `max_variants_per_run`, todas de la misma familia y el
/// mismo período (así el PBO compara lo comparable). El error dice qué falla, en español.
pub fn check_variants(specs: &[ValidSpec], catalog: &StrategyCatalog) -> Result<(), String> {
    let max = catalog.limits.max_variants_per_run;
    let Some(first) = specs.first() else {
        return Err("el pedido no tiene specs".into());
    };
    if specs.len() > max {
        return Err(format!("como mucho {max} variantes por corrida"));
    }
    let same = |v: &ValidSpec| {
        v.family_sha256 == first.family_sha256 && v.spec.period == first.spec.period
    };
    if !specs.iter().all(same) {
        return Err(
            "las variantes de una corrida tienen que ser de la misma familia y período".into(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    /// `done`, `failed` o `cancelled`: un trabajo tiene uno solo.
    pub fn is_end(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Queued,
            Self::Running,
            Self::Done,
            Self::Failed,
            Self::Cancelled,
        ]
        .into_iter()
        .find(|x| x.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStage {
    Started,
    LoadingCandles,
    RunningVariants,
    LuckTest,
    BuildingReports,
    Saving,
}

impl JobStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::LoadingCandles => "loading_candles",
            Self::RunningVariants => "running_variants",
            Self::LuckTest => "luck_test",
            Self::BuildingReports => "building_reports",
            Self::Saving => "saving",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Started,
            Self::LoadingCandles,
            Self::RunningVariants,
            Self::LuckTest,
            Self::BuildingReports,
            Self::Saving,
        ]
        .into_iter()
        .find(|x| x.as_str() == s)
    }
}

/// Lo que se agrega a un trabajo (el `seq`, la hora y el total de variantes los pone quien
/// guarda).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStep {
    Queued,
    Running {
        stage: JobStage,
        percent: u8,
        variants_done: u32,
    },
    Done {
        report_ids: Vec<String>,
    },
    Failed {
        error: String,
    },
    Cancelled,
}

impl JobStep {
    pub fn state(&self) -> JobState {
        match self {
            Self::Queued => JobState::Queued,
            Self::Running { .. } => JobState::Running,
            Self::Done { .. } => JobState::Done,
            Self::Failed { .. } => JobState::Failed,
            Self::Cancelled => JobState::Cancelled,
        }
    }
}

/// `lab_job_event.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabJobEvent {
    pub job_id: String,
    pub seq: i64,
    pub state: JobState,
    pub stage: Option<JobStage>,
    pub percent: u8,
    pub variants_done: u32,
    pub variants_total: u32,
    pub report_ids: Vec<String>,
    pub error: Option<String>,
    #[serde(with = "cfds_core::candle::utc_z")]
    pub recorded_at: DateTime<Utc>,
}

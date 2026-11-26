//! `lab_trial.v1`: una variante probada en el laboratorio. Cada corrida agrega una a un archivo
//! JSON Lines de solo agregar (la tabla llega en L8). Las variantes **distintas** de una familia
//! suman al N del Sharpe deflactado del laboratorio; una spec repetida no suma de nuevo. Las
//! exploratorias no suman al N del proyecto (regla #660 §4.1; §9 punto 1 del diseño, PENDIENTE).

use std::collections::BTreeSet;

use cfds_core::{InstrumentId, Timeframe};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::spec::Mode;
use super::validation::ValidSpec;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabTrial {
    pub spec_sha256: String,
    pub family_sha256: String,
    pub instrument_id: InstrumentId,
    pub timeframe: Timeframe,
    pub mode: Mode,
    pub catalog_version: u32,
    pub report_id: String,
    #[serde(with = "cfds_core::candle::utc_z")]
    pub recorded_at: DateTime<Utc>,
}

impl LabTrial {
    pub fn new(spec: &ValidSpec, report_id: &str, recorded_at: DateTime<Utc>) -> Self {
        Self {
            spec_sha256: spec.spec_sha256.clone(),
            family_sha256: spec.family_sha256.clone(),
            instrument_id: spec.spec.market.instrument_id.clone(),
            timeframe: spec.timeframe,
            mode: spec.spec.mode,
            catalog_version: spec.spec.catalog_version,
            report_id: report_id.to_owned(),
            recorded_at,
        }
    }
}

/// Lee un archivo JSON Lines de `lab_trial.v1` (líneas vacías se ignoran). Error con el número
/// de línea si alguna no tiene la forma del contrato.
pub fn read_trials(text: &str) -> Result<Vec<LabTrial>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(k, l)| serde_json::from_str(l).map_err(|e| format!("línea {}: {e}", k + 1)))
        .collect()
}

/// Variantes distintas de `family` entre las probadas y las de esta corrida (`current`, hashes
/// de spec): cada spec cuenta una vez.
pub fn family_variants(trials: &[LabTrial], family: &str, current: &[&str]) -> u32 {
    let past = trials
        .iter()
        .filter(|t| t.family_sha256 == family)
        .map(|t| t.spec_sha256.as_str());
    let distinct: BTreeSet<&str> = past.chain(current.iter().copied()).collect();
    u32::try_from(distinct.len()).unwrap_or(u32::MAX)
}

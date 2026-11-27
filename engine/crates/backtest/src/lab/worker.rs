//! `[worker]` de `config/lab.toml` (#775): lo que necesita el `lab_worker`, el proceso aparte que
//! corre los trabajos de la cola (§5 del diseño, §9 punto 6 recomendado), y el avance que muestra
//! cada etapa. No decide nada de ninguna estrategia.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::job::JobStage;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerConfig {
    /// Cada cuánto mira la cola cuando está vacía, en segundos.
    pub poll_secs: u64,
    /// Duración máxima de un trabajo, en minutos (se revisa entre etapas).
    pub max_minutes: u64,
    /// Desde cuándo se cargan las velas (calentamiento antes del período de la spec), UTC.
    pub load_from: String,
    /// Escenario de financiación de estudio (`ESTR`): el tipo de EE. UU. sigue pendiente.
    pub funding_scenario: String,
    /// Vencimiento de cada stream de Redis (`lab_jobs.v1.<job_id>`), en horas.
    pub stream_ttl_hours: u64,
    /// Entradas que guarda como mucho cada stream.
    pub stream_maxlen: usize,
    /// Puntos que lleva como mucho la curva en vivo de cada variante (#854; 2 a 200, el tope del
    /// contrato).
    pub curve_max_points: usize,
}

/// El tope de puntos de `curve` en `lab_job_event.v1`.
pub const CURVE_MAX_POINTS_LIMIT: usize = 200;

impl WorkerConfig {
    /// Revisa los valores (se llama al leer `config/lab.toml`).
    pub fn validate(&self) -> Result<(), String> {
        if self.poll_secs == 0
            || self.max_minutes == 0
            || self.stream_ttl_hours == 0
            || self.stream_maxlen == 0
        {
            return Err(
                "lab.toml: [worker] poll_secs, max_minutes, stream_ttl_hours y stream_maxlen \
                 tienen que ser > 0"
                    .into(),
            );
        }
        if !(2..=CURVE_MAX_POINTS_LIMIT).contains(&self.curve_max_points) {
            return Err(format!(
                "lab.toml: [worker] curve_max_points tiene que ir de 2 a {CURVE_MAX_POINTS_LIMIT}"
            ));
        }
        self.load_from()?;
        if self.funding_scenario.trim().is_empty() {
            return Err("lab.toml: [worker] funding_scenario vacío".into());
        }
        Ok(())
    }

    pub fn load_from(&self) -> Result<DateTime<Utc>, String> {
        if !self.load_from.ends_with('Z') {
            return Err("lab.toml: [worker] load_from tiene que ser UTC (…Z)".into());
        }
        self.load_from
            .parse()
            .map_err(|_| "lab.toml: [worker] load_from no es una fecha RFC 3339".into())
    }

    pub fn poll(&self) -> Duration {
        Duration::from_secs(self.poll_secs)
    }

    pub fn max_duration(&self) -> Duration {
        Duration::from_secs(self.max_minutes.saturating_mul(60))
    }

    pub fn stream_ttl(&self) -> Duration {
        Duration::from_secs(self.stream_ttl_hours.saturating_mul(3600))
    }

    /// Los flags de estudio de la carga (`--escenario-financiacion`), como el ejemplo
    /// `laboratorio`.
    pub fn study_args(&self) -> Vec<String> {
        vec![
            "--escenario-financiacion".into(),
            self.funding_scenario.clone(),
        ]
    }
}

/// Avance aproximado para mostrar (0 a 99; 100 es `done`): la toma, la carga de velas, las
/// variantes (la mayor parte), los informes y el guardado. `done`/`total` cuentan variantes.
pub fn progress(stage: JobStage, done: u32, total: u32) -> u8 {
    let share = |from: u32, to: u32| {
        let total = total.max(1);
        let span = to - from;
        let k = done.min(total);
        u8::try_from(from + span * k / total).unwrap_or(99)
    };
    match stage {
        JobStage::Started => 0,
        JobStage::LoadingCandles => 5,
        JobStage::RunningVariants => share(10, 60),
        JobStage::LuckTest | JobStage::BuildingReports => share(60, 95),
        JobStage::Saving => 97,
    }
}

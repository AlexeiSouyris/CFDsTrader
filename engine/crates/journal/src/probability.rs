//! Probabilidad mostrada de la spec §13 (#175).
//!
//! Frecuencia histórica de "toca TP1 antes que el stop" (`outcome.hit_tp1`) en las propuestas
//! del backtest con el mismo tipo de gatillo y el mismo rango de puntaje, con su intervalo de
//! Wilson. Con menos de `min_cases` casos en el grupo: "datos insuficientes".
//!
//! Sin look-ahead y sin tocar el tramo fuera de muestra: solo cuentan los pares cuyo resultado
//! terminó (su última salida) a más tardar en `cutoff`, el inicio del tramo fuera de muestra
//! (`DataSplit::out_of_sample.start`). Las salidas por tiempo o forzadas cuentan como "no tocó
//! TP1". Funciones puras, decimales exactos y configuración en `config/probability.toml`.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, MathematicalOps};
use serde::Deserialize;

use crate::{Outcome, OutcomeKind, Proposal, Signal, Source};

/// Los cuatro gatillos de la spec §6 (los únicos valores válidos en `trigger_priority`).
const TRIGGERS: [Signal; 4] = [
    Signal::CandlePattern,
    Signal::DoubleBreak,
    Signal::LevelRejection,
    Signal::Sweep,
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProbabilityError {
    #[error("config/probability.toml: {0}")]
    Config(String),
    #[error("el resultado {outcome} no corresponde a la propuesta {proposal}")]
    Mismatch { proposal: String, outcome: String },
    #[error("la propuesta {0} no viene del backtest o su resultado no es simulado")]
    NotBacktest(String),
}

/// `config/probability.toml`, validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbabilityConfig {
    pub min_cases: u32,
    pub z: Decimal,
    /// Límite inferior de cada rango, estrictamente creciente.
    pub score_bands: Vec<i32>,
    pub trigger_priority: Vec<Signal>,
}

impl ProbabilityConfig {
    pub fn from_toml(text: &str) -> Result<Self, ProbabilityError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            min_cases: u32,
            z: String,
            score_bands: Vec<i32>,
            trigger_priority: Vec<Signal>,
        }
        let bad = |m: &str| ProbabilityError::Config(m.to_string());
        let raw: Raw = toml::from_str(text).map_err(|e| ProbabilityError::Config(e.to_string()))?;
        if raw.min_cases == 0 {
            return Err(bad("min_cases tiene que ser mayor que 0"));
        }
        let z: Decimal = raw
            .z
            .parse()
            .map_err(|_| bad("z tiene que ser un decimal"))?;
        if z <= Decimal::ZERO || z > Decimal::from(5) {
            return Err(bad("z tiene que estar en (0, 5]"));
        }
        if raw.score_bands.is_empty() || raw.score_bands.windows(2).any(|w| w[0] >= w[1]) {
            return Err(bad("score_bands: al menos uno y estrictamente creciente"));
        }
        let mut seen = raw.trigger_priority.clone();
        seen.sort_by_key(|s| TRIGGERS.iter().position(|t| t == s));
        seen.dedup();
        if raw.trigger_priority.len() != TRIGGERS.len()
            || seen.len() != TRIGGERS.len()
            || !raw.trigger_priority.iter().all(|s| TRIGGERS.contains(s))
        {
            return Err(bad(
                "trigger_priority: los cuatro gatillos (sweep, double_break, level_rejection, candle_pattern), una vez cada uno",
            ));
        }
        Ok(Self {
            min_cases: raw.min_cases,
            z,
            score_bands: raw.score_bands,
            trigger_priority: raw.trigger_priority,
        })
    }

    /// Grupo de una propuesta: su gatillo de mayor preferencia y su rango de puntaje.
    /// `None` si no tiene gatillos o su puntaje está por debajo del primer rango.
    pub fn group(&self, p: &Proposal) -> Option<Group> {
        let trigger = *self
            .trigger_priority
            .iter()
            .find(|t| p.triggers.contains(t))?;
        let band = self.score_bands.iter().rposition(|&lo| p.score >= lo)?;
        Some(Group { trigger, band })
    }
}

/// Tipo de gatillo y rango de puntaje (índice en `score_bands`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Group {
    pub trigger: Signal,
    pub band: usize,
}

/// Lo que se muestra al lado de una propuesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// Frecuencia `hits / cases` con su intervalo, todo en [0, 1] y redondeado a 4 decimales.
    Estimate {
        probability: Decimal,
        low: Decimal,
        high: Decimal,
        hits: u32,
        cases: u32,
    },
    /// Menos de `needed` casos en el grupo (spec §13).
    Insufficient { cases: u32, needed: u32 },
}

/// Intervalo de Wilson de `hits / n` con el `z` dado, en [0, 1]. `n` tiene que ser > 0.
pub fn wilson(hits: u32, n: u32, z: Decimal) -> (Decimal, Decimal) {
    debug_assert!(n > 0 && hits <= n);
    let n_d = Decimal::from(n);
    let p = Decimal::from(hits) / n_d;
    let z2 = z * z;
    let denom = Decimal::ONE + z2 / n_d;
    let center = p + z2 / (Decimal::TWO * n_d);
    let radicand = p * (Decimal::ONE - p) / n_d + z2 / (Decimal::from(4) * n_d * n_d);
    let half = z * radicand.sqrt().unwrap_or(Decimal::ZERO);
    let low = ((center - half) / denom).max(Decimal::ZERO);
    let high = ((center + half) / denom).min(Decimal::ONE);
    (low, high)
}

/// Tabla de frecuencias por grupo, armada con el backtest de entrenamiento y validación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbabilityTable {
    cfg: ProbabilityConfig,
    /// (aciertos, casos) por grupo.
    cells: BTreeMap<Group, (u32, u32)>,
}

impl ProbabilityTable {
    /// Arma la tabla con los pares propuesta + resultado simulado del backtest. Ignora los que
    /// terminaron después de `cutoff` (inicio del tramo fuera de muestra) y los que no tienen
    /// grupo. Falla si un resultado no corresponde a su propuesta o no es del backtest.
    pub fn build(
        pairs: &[(Proposal, Outcome)],
        cutoff: DateTime<Utc>,
        cfg: &ProbabilityConfig,
    ) -> Result<Self, ProbabilityError> {
        let mut cells: BTreeMap<Group, (u32, u32)> = BTreeMap::new();
        for (p, o) in pairs {
            if o.proposal_id != p.id {
                return Err(ProbabilityError::Mismatch {
                    proposal: p.id.clone(),
                    outcome: o.proposal_id.clone(),
                });
            }
            if p.source != Source::Backtest || o.kind != OutcomeKind::Simulated {
                return Err(ProbabilityError::NotBacktest(p.id.clone()));
            }
            let resolved = o.exits.iter().map(|e| e.at).max().unwrap_or(o.entry_at);
            if resolved > cutoff {
                continue;
            }
            let Some(g) = cfg.group(p) else { continue };
            let cell = cells.entry(g).or_insert((0, 0));
            cell.0 += u32::from(o.hit_tp1);
            cell.1 += 1;
        }
        Ok(Self {
            cfg: cfg.clone(),
            cells,
        })
    }

    /// Aciertos y casos de un grupo.
    pub fn counts(&self, g: Group) -> (u32, u32) {
        self.cells.get(&g).copied().unwrap_or((0, 0))
    }

    /// Los grupos con al menos un caso, en orden, con (aciertos, casos).
    pub fn groups(&self) -> impl Iterator<Item = (Group, (u32, u32))> + '_ {
        self.cells.iter().map(|(g, c)| (*g, *c))
    }

    /// La probabilidad que se muestra para `p`. Sin grupo (sin gatillos o puntaje por debajo
    /// del primer rango) es "datos insuficientes" con 0 casos.
    pub fn shown(&self, p: &Proposal) -> Shown {
        match self.cfg.group(p) {
            Some(g) => self.shown_for(g),
            None => Shown::Insufficient {
                cases: 0,
                needed: self.cfg.min_cases,
            },
        }
    }

    /// La probabilidad que se muestra para un grupo.
    pub fn shown_for(&self, g: Group) -> Shown {
        let needed = self.cfg.min_cases;
        let (hits, cases) = self.counts(g);
        if cases < needed {
            return Shown::Insufficient { cases, needed };
        }
        let (low, high) = wilson(hits, cases, self.cfg.z);
        Shown::Estimate {
            probability: (Decimal::from(hits) / Decimal::from(cases)).round_dp(4),
            low: low.round_dp(4),
            high: high.round_dp(4),
            hits,
            cases,
        }
    }
}

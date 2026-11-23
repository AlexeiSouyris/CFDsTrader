//! Propuestas de cambio y votos (fase 8b, spec §15b y §1.2, #327): `change_proposal.v1` y
//! `change_vote.v1`.
//!
//! ```text
//! informe → propuestas de cambio ("pendiente_de_voto") → votos (1 por miembro)
//!   aprobada:  `majority` a favor dentro de `window_hours` desde la propuesta
//!   rechazada: ya no puede llegar a la mayoría, o pasó el plazo sin llegar
//!   pendiente: ninguna de las dos
//! ```
//! Contar votos **nunca aplica** un cambio: un parámetro solo cambia a mano en `config/`, con el
//! voto registrado. Funciones puras; el guardado es de solo agregar (`pg`).

use cfds_core::candle::utc_z;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::learning::{ChangeKind, LearningReport, PENDING_VOTE};

/// `change_proposal.v1`: una propuesta de cambio guardada, como nació en su informe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredChange {
    pub change_id: String,
    pub report_id: String,
    pub kind: ChangeKind,
    pub text: String,
    pub evidence: String,
    /// Siempre `pendiente_de_voto`: el estado actual sale de los votos ([`tally`]).
    pub status: String,
    #[serde(with = "utc_z")]
    pub created_at: DateTime<Utc>,
}

/// Las propuestas de un informe, listas para guardar.
pub fn changes_of(report: &LearningReport) -> Vec<StoredChange> {
    report
        .change_proposals
        .iter()
        .map(|c| StoredChange {
            change_id: c.change_id.clone(),
            report_id: report.report_id.clone(),
            kind: c.kind,
            text: c.text.clone(),
            evidence: c.evidence.clone(),
            status: PENDING_VOTE.into(),
            created_at: report.created_at,
        })
        .collect()
}

/// `change_vote.v1`: el voto de un miembro sobre una propuesta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vote {
    pub vote_id: String,
    pub change_id: String,
    pub member_id: String,
    pub in_favor: bool,
    #[serde(with = "utc_z")]
    pub voted_at: DateTime<Utc>,
}

/// `[rule_change]` de `config/votes.toml` (spec §1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoteRules {
    pub members: u32,
    pub majority: u32,
    pub window: TimeDelta,
}

impl VoteRules {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            rule_change: RawRule,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawRule {
            members: u32,
            majority: u32,
            window_hours: u32,
        }
        let r = toml::from_str::<Raw>(text)
            .map_err(|e| format!("config/votes.toml: {e}"))?
            .rule_change;
        // Mayoría de verdad: más de la mitad y no más que los miembros.
        if r.members == 0 || r.majority * 2 <= r.members || r.majority > r.members {
            return Err("rule_change: majority tiene que ser más de la mitad de members".into());
        }
        if r.window_hours == 0 {
            return Err("rule_change.window_hours > 0".into());
        }
        Ok(Self {
            members: r.members,
            majority: r.majority,
            window: TimeDelta::hours(i64::from(r.window_hours)),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum VoteStatus {
    PendienteDeVoto { in_favor: u32, against: u32 },
    Aprobada { in_favor: u32, against: u32 },
    Rechazada { in_favor: u32, against: u32 },
}

/// El estado de una propuesta en `now`, con sus votos. Solo cuentan los votos de esa propuesta,
/// uno por miembro (el primero) y dentro del plazo. Pura: no aplica nada.
pub fn tally(
    change: &StoredChange,
    votes: &[Vote],
    rules: &VoteRules,
    now: DateTime<Utc>,
) -> VoteStatus {
    let deadline = change.created_at + rules.window;
    let mut seen: Vec<&str> = Vec::new();
    let (mut in_favor, mut against) = (0u32, 0u32);
    let mut valid: Vec<&Vote> = votes
        .iter()
        .filter(|v| {
            v.change_id == change.change_id
                && v.voted_at >= change.created_at
                && v.voted_at <= deadline
                && v.voted_at <= now
        })
        .collect();
    valid.sort_by_key(|v| v.voted_at);
    for v in valid {
        if seen.contains(&v.member_id.as_str()) {
            continue;
        }
        seen.push(&v.member_id);
        if v.in_favor {
            in_favor += 1;
        } else {
            against += 1;
        }
    }
    if in_favor >= rules.majority {
        VoteStatus::Aprobada { in_favor, against }
    } else if against > rules.members - rules.majority || now > deadline {
        VoteStatus::Rechazada { in_favor, against }
    } else {
        VoteStatus::PendienteDeVoto { in_favor, against }
    }
}

/// `change_vote_status.v1` (#331): el estado de una propuesta en un momento, para mostrarlo.
/// Solo lectura: sale de [`tally`] y no aplica nada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeStatus {
    pub change_id: String,
    #[serde(flatten)]
    pub status: VoteStatus,
}

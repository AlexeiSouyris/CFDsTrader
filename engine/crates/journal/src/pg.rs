//! Escritura y lectura del diario en PostgreSQL (`infra/migrations/0002_journal.sql`).
//!
//! Idempotente, como las velas: volver a guardar la misma entrada no duplica nada. Si ya hay
//! una entrada con el mismo id y **otro** contenido, es un error (`Conflict`): el diario nunca
//! se reescribe en silencio.

use rust_decimal::Decimal;
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Postgres, Row, Transaction};

use crate::{Outcome, OutcomeKind, Proposal};

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("PostgreSQL: {0}")]
    Db(#[from] sqlx::Error),
    #[error("JSON del diario: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ya hay una entrada {id} con otro contenido: el diario no se reescribe")]
    Conflict { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    Inserted,
    /// Ya estaba, idéntica.
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct PgJournal {
    pool: PgPool,
}

/// Texto de un enum del contrato (`"backtest"`, `"long"`...), sin las comillas del JSON.
fn name<T: serde::Serialize>(v: &T) -> Result<String, JournalError> {
    match serde_json::to_value(v)? {
        serde_json::Value::String(s) => Ok(s),
        other => Ok(other.to_string()),
    }
}

fn parse_name<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, JournalError> {
    Ok(serde_json::from_value(serde_json::Value::String(
        s.to_string(),
    ))?)
}

fn row_to_proposal(r: &PgRow) -> Result<Proposal, JournalError> {
    Ok(Proposal {
        id: r.try_get("id")?,
        source: parse_name(r.try_get::<&str, _>("source")?)?,
        run_id: r.try_get("run_id")?,
        instrument_id: serde_json::from_value(serde_json::Value::String(
            r.try_get("instrument_id")?,
        ))?,
        side: parse_name(r.try_get::<&str, _>("side")?)?,
        signal_at: r.try_get("signal_at")?,
        reference: r.try_get::<Decimal, _>("reference")?,
        stop: r.try_get("stop")?,
        tp1: r.try_get("tp1")?,
        size: r.try_get("size")?,
        leverage: r.try_get("leverage")?,
        score: r.try_get("score")?,
        breakdown: serde_json::from_str(r.try_get("breakdown")?)?,
        triggers: serde_json::from_str(r.try_get("triggers")?)?,
    })
}

fn row_to_outcome(r: &PgRow) -> Result<Outcome, JournalError> {
    Ok(Outcome {
        proposal_id: r.try_get("proposal_id")?,
        kind: parse_name(r.try_get::<&str, _>("kind")?)?,
        entry_at: r.try_get("entry_at")?,
        entry_price: r.try_get("entry_price")?,
        exits: serde_json::from_str(r.try_get("exits")?)?,
        commission: r.try_get("commission")?,
        funding: r.try_get("funding")?,
        gross_pnl: r.try_get("gross_pnl")?,
        net_pnl: r.try_get("net_pnl")?,
        hit_tp1: r.try_get("hit_tp1")?,
        hit_stop: r.try_get("hit_stop")?,
        exit_reason: parse_name(r.try_get::<&str, _>("exit_reason")?)?,
    })
}

/// Columnas en el orden que leen `row_to_*`. Macros para armar SQL fijo con `concat!`: sqlx
/// exige consultas estáticas, que no se puedan inyectar (igual que en `cfds-storage`).
macro_rules! select_proposal {
    () => {
        concat!(
            "SELECT id, source, run_id, instrument_id, side, signal_at, reference, stop, tp1, ",
            "size, leverage, score, breakdown::text AS breakdown, triggers::text AS triggers ",
            "FROM proposals WHERE id = $1"
        )
    };
}

macro_rules! select_outcome {
    () => {
        concat!(
            "SELECT proposal_id, kind, entry_at, entry_price, exits::text AS exits, commission, ",
            "funding, gross_pnl, net_pnl, hit_tp1, hit_stop, exit_reason ",
            "FROM outcomes WHERE proposal_id = $1 AND kind = $2"
        )
    };
}

impl PgJournal {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Guarda la propuesta y, si hay, su resultado, en una sola transacción.
    pub async fn record(
        &self,
        proposal: &Proposal,
        outcome: Option<&Outcome>,
    ) -> Result<Recorded, JournalError> {
        let mut tx = self.pool.begin().await?;
        let mut inserted = insert_proposal(&mut tx, proposal).await?;
        if let Some(o) = outcome {
            if o.proposal_id != proposal.id {
                return Err(JournalError::Conflict {
                    id: o.proposal_id.clone(),
                });
            }
            inserted |= insert_outcome(&mut tx, o).await?;
        }
        tx.commit().await?;
        Ok(if inserted {
            Recorded::Inserted
        } else {
            Recorded::Unchanged
        })
    }

    pub async fn proposal(&self, id: &str) -> Result<Option<Proposal>, JournalError> {
        sqlx::query(select_proposal!())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| row_to_proposal(&r))
            .transpose()
    }

    pub async fn outcome(
        &self,
        proposal_id: &str,
        kind: OutcomeKind,
    ) -> Result<Option<Outcome>, JournalError> {
        sqlx::query(select_outcome!())
            .bind(proposal_id)
            .bind(name(&kind)?)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| row_to_outcome(&r))
            .transpose()
    }
}

/// `true` si era nueva; si ya estaba, tiene que ser idéntica.
async fn insert_proposal(
    tx: &mut Transaction<'_, Postgres>,
    p: &Proposal,
) -> Result<bool, JournalError> {
    let done = sqlx::query(
        "INSERT INTO proposals (id, source, run_id, instrument_id, side, signal_at, reference,
           stop, tp1, size, leverage, score, breakdown, triggers)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13::jsonb, $14::jsonb)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&p.id)
    .bind(name(&p.source)?)
    .bind(&p.run_id)
    .bind(p.instrument_id.as_str())
    .bind(name(&p.side)?)
    .bind(p.signal_at)
    .bind(p.reference)
    .bind(p.stop)
    .bind(p.tp1)
    .bind(p.size)
    .bind(p.leverage)
    .bind(p.score)
    .bind(serde_json::to_string(&p.breakdown)?)
    .bind(serde_json::to_string(&p.triggers)?)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 1 {
        return Ok(true);
    }
    let stored = sqlx::query(select_proposal!())
        .bind(&p.id)
        .fetch_one(&mut **tx)
        .await?;
    if row_to_proposal(&stored)? != *p {
        return Err(JournalError::Conflict { id: p.id.clone() });
    }
    Ok(false)
}

async fn insert_outcome(
    tx: &mut Transaction<'_, Postgres>,
    o: &Outcome,
) -> Result<bool, JournalError> {
    let done = sqlx::query(
        "INSERT INTO outcomes (proposal_id, kind, entry_at, entry_price, exits, commission,
           funding, gross_pnl, net_pnl, hit_tp1, hit_stop, exit_reason)
         VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (proposal_id, kind) DO NOTHING",
    )
    .bind(&o.proposal_id)
    .bind(name(&o.kind)?)
    .bind(o.entry_at)
    .bind(o.entry_price)
    .bind(serde_json::to_string(&o.exits)?)
    .bind(o.commission)
    .bind(o.funding)
    .bind(o.gross_pnl)
    .bind(o.net_pnl)
    .bind(o.hit_tp1)
    .bind(o.hit_stop)
    .bind(name(&o.exit_reason)?)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 1 {
        return Ok(true);
    }
    let stored = sqlx::query(select_outcome!())
        .bind(&o.proposal_id)
        .bind(name(&o.kind)?)
        .fetch_one(&mut **tx)
        .await?;
    if row_to_outcome(&stored)? != *o {
        return Err(JournalError::Conflict {
            id: o.proposal_id.clone(),
        });
    }
    Ok(false)
}

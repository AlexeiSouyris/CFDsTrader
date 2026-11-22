//! Historial del fondo en PostgreSQL (`infra/migrations/0010_fund_movements.sql`, #179).
//!
//! Idempotente: volver a guardar el mismo movimiento no duplica nada. Si ya hay un movimiento
//! con el mismo id y **otro** contenido, es un error (`Conflict`): el historial nunca se
//! reescribe. La tabla es de solo agregar (un trigger impide modificar o borrar).

use rust_decimal::Decimal;
use sqlx::Row;
use sqlx::postgres::{PgPool, PgRow};

use crate::{MoveKind, Record};

#[derive(Debug, thiserror::Error)]
pub enum FundStoreError {
    #[error("PostgreSQL: {0}")]
    Db(#[from] sqlx::Error),
    #[error("ya hay un movimiento {id} con otro contenido: el historial no se reescribe")]
    Conflict { id: String },
    #[error("tipo de movimiento desconocido en la base: {0}")]
    UnknownKind(String),
}

#[derive(Debug, Clone)]
pub struct PgFund {
    pool: PgPool,
}

fn kind_name(k: MoveKind) -> &'static str {
    match k {
        MoveKind::Contribution => "contribution",
        MoveKind::Withdrawal => "withdrawal",
    }
}

fn row_to_record(r: &PgRow) -> Result<Record, FundStoreError> {
    let kind: String = r.try_get("kind")?;
    Ok(Record {
        id: r.try_get("id")?,
        settled_at: r.try_get("settled_at")?,
        member_id: r.try_get("member_id")?,
        kind: match kind.as_str() {
            "contribution" => MoveKind::Contribution,
            "withdrawal" => MoveKind::Withdrawal,
            _ => return Err(FundStoreError::UnknownKind(kind)),
        },
        amount: r.try_get::<Decimal, _>("amount")?,
        units: r.try_get("units")?,
        nav: r.try_get("nav")?,
        currency: r.try_get("currency")?,
    })
}

impl PgFund {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Guarda los movimientos de una liquidación en una sola transacción: o entran todos o
    /// ninguno. Devuelve cuántos eran nuevos.
    pub async fn record(&self, records: &[Record]) -> Result<usize, FundStoreError> {
        let mut tx = self.pool.begin().await?;
        let mut inserted = 0;
        for r in records {
            let done = sqlx::query(
                "INSERT INTO fund_movements \
                 (id, settled_at, member_id, kind, amount, units, nav, currency) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (id) DO NOTHING",
            )
            .bind(&r.id)
            .bind(r.settled_at)
            .bind(&r.member_id)
            .bind(kind_name(r.kind))
            .bind(r.amount)
            .bind(r.units)
            .bind(r.nav)
            .bind(&r.currency)
            .execute(&mut *tx)
            .await?;
            if done.rows_affected() == 1 {
                inserted += 1;
                continue;
            }
            let row = sqlx::query(
                "SELECT id, settled_at, member_id, kind, amount, units, nav, currency \
                 FROM fund_movements WHERE id = $1",
            )
            .bind(&r.id)
            .fetch_one(&mut *tx)
            .await?;
            if row_to_record(&row)? != *r {
                return Err(FundStoreError::Conflict { id: r.id.clone() });
            }
        }
        tx.commit().await?;
        Ok(inserted)
    }

    /// Todo el historial, en orden de liquidación.
    pub async fn load(&self) -> Result<Vec<Record>, FundStoreError> {
        let rows = sqlx::query(
            "SELECT id, settled_at, member_id, kind, amount, units, nav, currency \
             FROM fund_movements ORDER BY settled_at, id",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_record).collect()
    }
}

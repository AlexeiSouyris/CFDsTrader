//! Puntos de control del broker simulado (#463,
//! `infra/migrations/0043_paper_broker_checkpoints.sql`). Solo agregar; la verificación (cadena
//! de hashes, registro y config) es de `cfds_broker::checkpoint`.

use crate::{PgStore, StorageError};

/// Un punto de control guardado, tal cual (el texto exacto que se hashea).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointRow {
    pub after_orders: u64,
    pub state: String,
    pub prev_hash: Option<String>,
    pub hash: String,
}

impl PgStore {
    /// Agrega un punto de control, en su propia transacción de escritura (el gateway abre la
    /// sesión en solo lectura, como con el registro de órdenes).
    pub async fn insert_paper_checkpoint(&self, row: &CheckpointRow) -> Result<(), StorageError> {
        let after = i64::try_from(row.after_orders)
            .map_err(|_| StorageError::CorruptRow("after_orders fuera de rango".into()))?;
        let mut tx = self.pool().begin().await?;
        sqlx::query("SET TRANSACTION READ WRITE")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO paper_broker_checkpoints (after_orders, state, prev_hash, hash)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(after)
        .bind(&row.state)
        .bind(row.prev_hash.as_deref())
        .bind(&row.hash)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Todos los puntos de control, en el orden en que se agregaron.
    pub async fn paper_checkpoints(&self) -> Result<Vec<CheckpointRow>, StorageError> {
        let rows: Vec<(i64, String, Option<String>, String)> = sqlx::query_as(
            "SELECT after_orders, state, prev_hash, hash FROM paper_broker_checkpoints
             ORDER BY seq",
        )
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|(after, state, prev_hash, hash)| {
                Ok(CheckpointRow {
                    after_orders: u64::try_from(after).map_err(|_| {
                        StorageError::CorruptRow(format!("after_orders negativo: {after}"))
                    })?,
                    state,
                    prev_hash,
                    hash,
                })
            })
            .collect()
    }
}

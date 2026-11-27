//! Órdenes pendientes del broker simulado, guardadas (#479,
//! `infra/migrations/0044_paper_pending_orders.sql`).
//!
//! Una orden que el broker deja pendiente de su vela del fill (#471) se guarda acá
//! (`order.v1` entero). Cuando se decide entra al registro (`insert_paper_order`), que en la
//! misma transacción la marca `settled`. Si al arrancar no se puede retomar, se marca `dropped`
//! con el motivo. Al arrancar, el broker retoma las que no tienen ninguna de las dos.

use cfds_core::Order;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{PgStore, StorageError};

impl PgStore {
    /// Guarda una orden pendiente. `true` si era nueva, `false` si ya estaba idéntica. Otro
    /// contenido para el mismo `order_id`, o un `order_id` que ya está en el registro de
    /// órdenes decididas, es un error y no se guarda.
    pub async fn insert_paper_pending(&self, order: &Order) -> Result<bool, StorageError> {
        let doc = serde_json::to_value(order)?;
        let mut tx = self.pool().begin().await?;
        // Como el registro: el gateway abre la sesión en solo lectura y declara de escritura
        // solo sus transacciones del broker simulado.
        sqlx::query("SET TRANSACTION READ WRITE")
            .execute(&mut *tx)
            .await?;
        let decided: Option<(String,)> =
            sqlx::query_as("SELECT order_id FROM paper_orders WHERE order_id = $1")
                .bind(order.order_id.as_str())
                .fetch_optional(&mut *tx)
                .await?;
        if decided.is_some() {
            tx.rollback().await?;
            return Err(StorageError::PaperOrderConflict {
                order_id: order.order_id.to_string(),
            });
        }
        let existing: Option<(Value,)> =
            sqlx::query_as("SELECT document FROM paper_pending_orders WHERE order_id = $1")
                .bind(order.order_id.as_str())
                .fetch_optional(&mut *tx)
                .await?;
        if let Some((d,)) = existing {
            tx.rollback().await?;
            return if d == doc {
                Ok(false)
            } else {
                Err(StorageError::PaperOrderConflict {
                    order_id: order.order_id.to_string(),
                })
            };
        }
        sqlx::query(
            "INSERT INTO paper_pending_orders (order_id, instrument_id, created_at, document)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(order.order_id.as_str())
        .bind(order.instrument_id.as_str())
        .bind(order.created_at)
        .bind(&doc)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Las órdenes pendientes que todavía no se decidieron ni se descartaron, en orden de
    /// llegada. Cada documento se lee con su contrato (`order.v1`): una fila que no lo cumple es
    /// un error, no se ignora.
    pub async fn paper_pending(&self) -> Result<Vec<Order>, StorageError> {
        let rows: Vec<(Value,)> = sqlx::query_as(
            "SELECT p.document FROM paper_pending_orders p
             WHERE NOT EXISTS (SELECT 1 FROM paper_pending_outcomes o WHERE o.order_id = p.order_id)
             ORDER BY p.seq",
        )
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|(d,)| Ok(serde_json::from_value(d)?))
            .collect()
    }

    /// Marca como descartada una orden pendiente que no se pudo retomar, con el motivo. Si ya
    /// tenía un final guardado, no cambia nada (`false`).
    pub async fn drop_paper_pending(
        &self,
        order_id: &str,
        reason: &str,
        at: DateTime<Utc>,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("SET TRANSACTION READ WRITE")
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query(
            "INSERT INTO paper_pending_outcomes (order_id, outcome, reason, at)
             VALUES ($1, 'dropped', $2, $3)
             ON CONFLICT (order_id) DO NOTHING",
        )
        .bind(order_id)
        .bind(reason)
        .bind(at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(done.rows_affected() == 1)
    }

    /// Cómo terminó cada orden pendiente que ya terminó: `(order_id, outcome, reason)`, en orden
    /// de llegada de la orden. Para revisar y para los tests.
    pub async fn paper_pending_outcomes(
        &self,
    ) -> Result<Vec<(String, String, Option<String>)>, StorageError> {
        Ok(sqlx::query_as(
            "SELECT o.order_id, o.outcome, o.reason FROM paper_pending_outcomes o
             JOIN paper_pending_orders p ON p.order_id = o.order_id
             ORDER BY p.seq",
        )
        .fetch_all(self.pool())
        .await?)
    }
}

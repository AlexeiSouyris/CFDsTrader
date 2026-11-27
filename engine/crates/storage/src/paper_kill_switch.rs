//! Kill switch del broker simulado, guardado (#441, `infra/migrations/0042_paper_kill_switch.sql`).
//!
//! Cada cambio es una fila, en orden, con cuántas órdenes del registro había (`after_orders`).
//! El motor solo escribe activaciones; la liberación (votación de la spec §1.2) la agrega una
//! persona con el gateway apagado.

use chrono::{DateTime, Utc};

use crate::{PgStore, StorageError};

/// Un cambio guardado del kill switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillSwitchRow {
    /// `true` si es una activación; `false` si es una liberación.
    pub activated: bool,
    pub after_orders: u64,
    pub reason: String,
    pub at: DateTime<Utc>,
}

impl PgStore {
    /// Guarda una activación del kill switch, en su propia transacción de escritura (como el
    /// registro de órdenes: el gateway abre la sesión en solo lectura).
    pub async fn insert_kill_switch_activation(
        &self,
        after_orders: u64,
        reason: &str,
        at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let after = i64::try_from(after_orders)
            .map_err(|_| StorageError::CorruptRow("after_orders fuera de rango".into()))?;
        let mut tx = self.pool().begin().await?;
        sqlx::query("SET TRANSACTION READ WRITE")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO paper_kill_switch (change, after_orders, reason, at)
             VALUES ('activated', $1, $2, $3)",
        )
        .bind(after)
        .bind(reason)
        .bind(at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Todos los cambios guardados del kill switch, en orden.
    pub async fn kill_switch_changes(&self) -> Result<Vec<KillSwitchRow>, StorageError> {
        let rows: Vec<(String, i64, String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT change, after_orders, reason, at FROM paper_kill_switch ORDER BY seq",
        )
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|(change, after, reason, at)| {
                let activated = match change.as_str() {
                    "activated" => true,
                    "released" => false,
                    other => {
                        return Err(StorageError::CorruptRow(format!(
                            "cambio de kill switch desconocido: {other}"
                        )));
                    }
                };
                let after_orders = u64::try_from(after).map_err(|_| {
                    StorageError::CorruptRow(format!("after_orders negativo: {after}"))
                })?;
                Ok(KillSwitchRow {
                    activated,
                    after_orders,
                    reason,
                    at,
                })
            })
            .collect()
    }
}

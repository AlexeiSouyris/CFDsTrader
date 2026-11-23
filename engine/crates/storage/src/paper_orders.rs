//! Registro de órdenes del broker simulado (fase 7, #278, `infra/migrations/0028_paper_orders.sql`).
//!
//! Cada orden (`order.v1`) se guarda una sola vez con su informe (`order_report.v1`), en una
//! transacción y en orden de llegada. Guardar otra vez lo mismo no cambia nada (idempotente); un
//! contenido o un informe distinto para el mismo `order_id` es repintado: error y no se guarda.
//! Con este registro el broker se reconstruye al arrancar (`cfds_broker::restore_and_reconcile`).

use cfds_core::{Order, OrderReport};
use serde_json::Value;

use crate::{PgStore, StorageError};

fn status(r: &OrderReport) -> Result<String, StorageError> {
    match serde_json::to_value(r.status)? {
        Value::String(s) => Ok(s),
        other => Err(StorageError::CorruptRow(format!(
            "estado inesperado: {other}"
        ))),
    }
}

impl PgStore {
    /// Guarda la orden y su informe. `true` si era nueva, `false` si ya estaba idéntica.
    pub async fn insert_paper_order(
        &self,
        order: &Order,
        report: &OrderReport,
    ) -> Result<bool, StorageError> {
        if report.order_id != order.order_id {
            return Err(StorageError::CorruptRow(format!(
                "el informe {} no es de la orden {}",
                report.order_id, order.order_id
            )));
        }
        let order_doc = serde_json::to_value(order)?;
        let report_doc = serde_json::to_value(report)?;
        let mut tx = self.pool().begin().await?;
        // El gateway abre la sesión en solo lectura (`default_transaction_read_only`). Esta
        // transacción, y solo esta, se declara de escritura: es la única escritura del gateway y
        // toca solo las tablas del registro del broker simulado. Con una sesión normal no cambia
        // nada.
        sqlx::query("SET TRANSACTION READ WRITE")
            .execute(&mut *tx)
            .await?;
        let existing: Option<(Value, Option<Value>)> = sqlx::query_as(
            "SELECT o.document, r.document FROM paper_orders o
             LEFT JOIN paper_order_reports r ON r.order_id = o.order_id
             WHERE o.order_id = $1",
        )
        .bind(order.order_id.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((o, r)) = existing {
            tx.rollback().await?;
            return if o == order_doc && r.as_ref() == Some(&report_doc) {
                Ok(false)
            } else {
                Err(StorageError::PaperOrderConflict {
                    order_id: order.order_id.to_string(),
                })
            };
        }
        sqlx::query(
            "INSERT INTO paper_orders (order_id, instrument_id, created_at, document)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(order.order_id.as_str())
        .bind(order.instrument_id.as_str())
        .bind(order.created_at)
        .bind(&order_doc)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO paper_order_reports (order_id, status, at, document)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(report.order_id.as_str())
        .bind(status(report)?)
        .bind(report.at)
        .bind(&report_doc)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Todas las órdenes guardadas con su informe, en orden de llegada. Cada documento se lee
    /// con su tipo de contrato: una fila que no lo cumple es un error, no se ignora.
    pub async fn paper_journal(&self) -> Result<Vec<(Order, OrderReport)>, StorageError> {
        let rows: Vec<(Value, Option<Value>)> = sqlx::query_as(
            "SELECT o.document, r.document FROM paper_orders o
             LEFT JOIN paper_order_reports r ON r.order_id = o.order_id
             ORDER BY o.seq",
        )
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|(o, r)| {
                let order: Order = serde_json::from_value(o)?;
                let report = r.ok_or_else(|| {
                    StorageError::CorruptRow(format!(
                        "la orden {} no tiene informe",
                        order.order_id
                    ))
                })?;
                Ok((order, serde_json::from_value(report)?))
            })
            .collect()
    }
}

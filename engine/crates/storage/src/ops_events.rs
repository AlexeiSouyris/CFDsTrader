//! `ops_events` (`ops_event.v1`, migración 0035, #337): los eventos de la operación del gateway,
//! **ya redactados** por el `Logger` de `cfds-obs`.
//!
//! - [`PgStore::insert_ops_events`]: un lote en una transacción que se declara de escritura (el
//!   gateway abre la sesión en solo lectura). Lo llama la tarea en segundo plano del gateway.
//! - [`PgStore::latest_ops_events`]: los últimos `limit`, el más nuevo primero (a igual hora,
//!   el último escrito primero).
//! - [`PgStore::purge_ops_events_before`]: la retención. Es lo único que puede borrar: lo pide al
//!   trigger con `SET LOCAL cfds.ops_events_retention = 'on'` en su propia transacción.

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use crate::{PgStore, StorageError};

/// Un evento tal como se guarda (`ops_event.v1`, con la hora ya leída).
#[derive(Debug, Clone, PartialEq)]
pub struct OpsEventRecord {
    pub ts: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub event: String,
    pub fields: Map<String, Value>,
}

type Row = (DateTime<Utc>, String, String, String, Value);

impl PgStore {
    /// Guarda los eventos en orden, en una sola transacción. Devuelve cuántos guardó.
    pub async fn insert_ops_events(
        &self,
        events: &[OpsEventRecord],
    ) -> Result<usize, StorageError> {
        if events.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool().begin().await?;
        // La sesión del gateway es de solo lectura: esta transacción se declara de escritura y
        // toca solo `ops_events`. Con una sesión normal no cambia nada.
        sqlx::query("SET TRANSACTION READ WRITE")
            .execute(&mut *tx)
            .await?;
        for e in events {
            sqlx::query(
                "INSERT INTO ops_events (ts, level, target, event, fields)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(e.ts)
            .bind(&e.level)
            .bind(&e.target)
            .bind(&e.event)
            .bind(Value::Object(e.fields.clone()))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(events.len())
    }

    /// Los últimos `limit` eventos, el más nuevo primero.
    pub async fn latest_ops_events(&self, limit: u32) -> Result<Vec<OpsEventRecord>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT ts, level, target, event, fields FROM ops_events
             ORDER BY ts DESC, id DESC LIMIT $1",
        )
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|(ts, level, target, event, fields)| match fields {
                Value::Object(fields) => Ok(OpsEventRecord {
                    ts,
                    level,
                    target,
                    event,
                    fields,
                }),
                other => Err(StorageError::CorruptRow(format!(
                    "ops_events.fields no es un objeto: {other}"
                ))),
            })
            .collect()
    }

    /// Retención: borra los eventos anteriores a `cutoff`. Devuelve cuántos borró. Necesita una
    /// sesión con permiso de escritura (el job de retención, no el gateway).
    pub async fn purge_ops_events_before(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, StorageError> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("SET LOCAL cfds.ops_events_retention = 'on'")
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM ops_events WHERE ts < $1")
            .bind(cutoff)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(done.rows_affected())
    }
}

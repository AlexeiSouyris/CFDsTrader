//! `emotional_states` (`emotional_state.v1`, migración 0033, #220): de solo agregar.
//!
//! - El motor guarda los estados de precio; Python, los del índice (mismo formato).
//! - Guardar lo mismo otra vez no hace nada; el mismo momento con otros estados es un error
//!   (sería repintado) y no se guarda nada de ese lote.
//! - [`PgStore::latest_emotional_states`] da el último de cada instrumento, fuente y base que ya se
//!   conocía en un momento (`as_of <= at`): nunca mira el futuro.

use cfds_core::{EmotionalState, InstrumentId};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

use crate::{InsertReport, PgStore, StorageError};

type Row = (String, String, String, DateTime<Utc>, Value);

fn row_to_state(row: Row) -> Result<EmotionalState, StorageError> {
    let (instrument, source, basis, as_of, states) = row;
    let doc = serde_json::json!({
        "instrument_id": instrument,
        "source": source,
        "basis": basis,
        "as_of": as_of.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        "states": states,
    });
    serde_json::from_value(doc).map_err(StorageError::from)
}

fn name<T: serde::Serialize>(v: &T) -> Result<String, StorageError> {
    match serde_json::to_value(v)? {
        Value::String(s) => Ok(s),
        other => Err(StorageError::CorruptRow(format!(
            "se esperaba texto: {other}"
        ))),
    }
}

impl PgStore {
    /// Guarda los estados en una transacción: o entran todos o ninguno.
    pub async fn insert_emotional_states(
        &self,
        rows: &[EmotionalState],
    ) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool().begin().await?;
        let mut report = InsertReport::default();
        for r in rows {
            let (source, basis) = (name(&r.source)?, name(&r.basis)?);
            let states = serde_json::to_value(&r.states)?;
            let inserted = sqlx::query(
                "INSERT INTO emotional_states (instrument_id, source, basis, as_of, states)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT ON CONSTRAINT emotional_states_key DO NOTHING",
            )
            .bind(r.instrument_id.as_str())
            .bind(&source)
            .bind(&basis)
            .bind(r.as_of)
            .bind(&states)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.inserted += 1;
                continue;
            }
            let stored: Value = sqlx::query_scalar(
                "SELECT states FROM emotional_states
                 WHERE instrument_id = $1 AND source = $2 AND basis = $3 AND as_of = $4",
            )
            .bind(r.instrument_id.as_str())
            .bind(&source)
            .bind(&basis)
            .bind(r.as_of)
            .fetch_one(&mut *tx)
            .await?;
            if stored != states {
                return Err(StorageError::EmotionalStateConflict {
                    instrument: r.instrument_id.to_string(),
                    basis,
                    as_of: r.as_of,
                });
            }
            report.unchanged += 1;
        }
        tx.commit().await?;
        Ok(report)
    }

    /// El último registro de cada instrumento, fuente y base conocido en `at`.
    pub async fn latest_emotional_states(
        &self,
        at: DateTime<Utc>,
    ) -> Result<Vec<EmotionalState>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT DISTINCT ON (instrument_id, source, basis)
                    instrument_id, source, basis, as_of, states
             FROM emotional_states WHERE as_of <= $1
             ORDER BY instrument_id, source, basis, as_of DESC",
        )
        .bind(at)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(row_to_state).collect()
    }

    /// Todos los registros de un instrumento en `[from, to)`, en orden de `as_of` (para leerlos
    /// en el arnés de hipótesis o en un backtest).
    pub async fn emotional_states(
        &self,
        id: &InstrumentId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<EmotionalState>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT instrument_id, source, basis, as_of, states FROM emotional_states
             WHERE instrument_id = $1 AND as_of >= $2 AND as_of < $3
             ORDER BY as_of, source, basis",
        )
        .bind(id.as_str())
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(row_to_state).collect()
    }
}

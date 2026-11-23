//! Mapa de liquidez en PostgreSQL (#231, `infra/migrations/0020_liquidity_levels.sql`).
//!
//! - `liquidity_levels`: el nivel (no cambia nunca).
//! - `liquidity_level_events`: a lo sumo un evento por nivel (`swept`, `broken` o `expired`).
//!
//! Las dos tablas son de solo agregar: guardar otra vez lo mismo no cambia nada, un evento nuevo
//! se agrega una sola vez, y un evento distinto del guardado (o un nivel que ya tenía evento y
//! ahora no) es repintado: error y no se guarda nada del lote.

use cfds_core::{InstrumentId, LiquidityLevel, LiquidityLevelState};
use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;

use crate::{PgStore, StorageError};

/// Resultado de guardar niveles: niveles nuevos, eventos nuevos (de niveles nuevos o ya
/// guardados) y niveles que ya estaban idénticos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevelsReport {
    pub new_levels: usize,
    pub new_events: usize,
    pub unchanged: usize,
}

/// Evento de un nivel: cuándo dejó de estar activo y por qué.
fn event_of(l: &LiquidityLevel) -> Option<(&'static str, DateTime<Utc>)> {
    match (l.state, l.state_changed_at, l.expired_at) {
        (LiquidityLevelState::Swept, Some(t), _) => Some(("swept", t)),
        (LiquidityLevelState::Broken, Some(t), _) => Some(("broken", t)),
        (LiquidityLevelState::Intact, None, Some(t)) => Some(("expired", t)),
        _ => None,
    }
}

fn text<T: serde::Serialize>(v: T) -> Result<String, StorageError> {
    match serde_json::to_value(v)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(StorageError::CorruptRow(format!(
            "se esperaba texto: {other}"
        ))),
    }
}

/// Fila leída: tipo, sesión, precio, exists_from, evento y su hora.
type LevelRow = (
    String,
    Option<String>,
    Decimal,
    DateTime<Utc>,
    Option<String>,
    Option<DateTime<Utc>>,
);

impl PgStore {
    /// Guarda niveles del mapa de liquidez (`liquidity_level.v1`) en **una sola transacción**.
    pub async fn insert_liquidity_levels(
        &self,
        levels: &[LiquidityLevel],
    ) -> Result<LevelsReport, StorageError> {
        let mut tx = self.pool().begin().await?;
        let mut report = LevelsReport::default();
        for l in levels {
            let kind = text(l.kind)?;
            let session = l.session.map(text).transpose()?;
            let inserted: Option<i64> = sqlx::query_scalar(
                "INSERT INTO liquidity_levels (instrument_id, kind, session, price, exists_from)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT ON CONSTRAINT liquidity_levels_key DO NOTHING
                 RETURNING id",
            )
            .bind(l.instrument_id.as_str())
            .bind(&kind)
            .bind(&session)
            .bind(l.price)
            .bind(l.exists_from)
            .fetch_optional(&mut *tx)
            .await?;
            let (level_id, stored) = match inserted {
                Some(id) => {
                    report.new_levels += 1;
                    (id, None)
                }
                None => {
                    let id: i64 = sqlx::query_scalar(
                        "SELECT id FROM liquidity_levels
                         WHERE instrument_id = $1 AND kind = $2 AND session IS NOT DISTINCT FROM $3
                           AND price = $4 AND exists_from = $5",
                    )
                    .bind(l.instrument_id.as_str())
                    .bind(&kind)
                    .bind(&session)
                    .bind(l.price)
                    .bind(l.exists_from)
                    .fetch_one(&mut *tx)
                    .await?;
                    let stored: Option<(String, DateTime<Utc>)> = sqlx::query_as(
                        "SELECT event, at FROM liquidity_level_events WHERE level_id = $1",
                    )
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?;
                    (id, stored)
                }
            };
            let computed = event_of(l);
            match (stored, computed) {
                (None, None) => {
                    if inserted.is_none() {
                        report.unchanged += 1;
                    }
                }
                (None, Some((event, at))) => {
                    sqlx::query(
                        "INSERT INTO liquidity_level_events (level_id, event, at)
                         VALUES ($1, $2, $3)",
                    )
                    .bind(level_id)
                    .bind(event)
                    .bind(at)
                    .execute(&mut *tx)
                    .await?;
                    report.new_events += 1;
                }
                (Some((s, t)), Some((event, at))) if s == event && t == at => {
                    report.unchanged += 1;
                }
                (Some(_), _) => {
                    return Err(StorageError::LiquidityLevelConflict {
                        instrument: l.instrument_id.to_string(),
                        kind,
                        exists_from: l.exists_from,
                    });
                }
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Niveles de un instrumento con `exists_from` en `[from, as_of]`, **tal como se conocían
    /// en `as_of`**: un evento posterior a `as_of` no se ve (el nivel sigue intacto y activo).
    /// Así un backtest no mira el futuro. Ordenados por `exists_from`, tipo, sesión y precio.
    pub async fn liquidity_levels(
        &self,
        instrument: &InstrumentId,
        from: DateTime<Utc>,
        as_of: DateTime<Utc>,
    ) -> Result<Vec<LiquidityLevel>, StorageError> {
        let rows: Vec<LevelRow> = sqlx::query_as(
            "SELECT l.kind, l.session, l.price, l.exists_from, e.event, e.at
             FROM liquidity_levels l
             LEFT JOIN liquidity_level_events e ON e.level_id = l.id AND e.at <= $3
             WHERE l.instrument_id = $1 AND l.exists_from >= $2 AND l.exists_from <= $3
             ORDER BY l.exists_from, l.kind, l.session NULLS FIRST, l.price",
        )
        .bind(instrument.as_str())
        .bind(from)
        .bind(as_of)
        .fetch_all(self.pool())
        .await?;
        let utc = |t: DateTime<Utc>| t.to_rfc3339_opts(SecondsFormat::AutoSi, true);
        rows.into_iter()
            .map(|(kind, session, price, exists_from, event, at)| {
                let (state, changed, expired) = match event.as_deref() {
                    None => ("intact", None, None),
                    Some("expired") => ("intact", None, at.map(utc)),
                    Some(e) => (e, at.map(utc), None),
                };
                // Se arma el documento del contrato y se valida como cualquier mensaje.
                let doc = serde_json::json!({
                    "instrument_id": instrument.as_str(),
                    "kind": kind,
                    "session": session,
                    "price": price.to_string(),
                    "exists_from": utc(exists_from),
                    "state": state,
                    "state_changed_at": changed,
                    "expired_at": expired,
                });
                serde_json::from_value(doc).map_err(StorageError::from)
            })
            .collect()
    }
}

//! Velas cerradas de IBKR como fuente de datos (`ibkr_bars`, `ibkr_bar.v1`, #536), aparte de las
//! de IG y de Databento. Igual que [`PgStore::insert_candles`]: una vela ya guardada e idéntica
//! cuenta como `unchanged`; con otros valores es un conflicto (sería repintado) y no se guarda
//! nada del lote.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{InsertReport, PgStore, StorageError};

/// Una fila de `ibkr_bars`. Los textos ya validados por quien la arma (la base los vuelve a
/// revisar con sus CHECK).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrBarRow {
    pub source: String,
    pub instrument_id: String,
    pub kind: String,
    pub contract: String,
    pub bar_size: String,
    pub what_to_show: String,
    pub start: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
}

/// Qué serie: todas las velas de un contrato, un tamaño y un precio de una fuente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrSeries<'a> {
    pub source: &'a str,
    pub instrument_id: &'a str,
    pub contract: &'a str,
    pub bar_size: &'a str,
    pub what_to_show: &'a str,
}

type Stored = (
    String,
    DateTime<Utc>,
    Decimal,
    Decimal,
    Decimal,
    Decimal,
    Option<Decimal>,
);

impl PgStore {
    /// Guarda velas de IBKR en una sola transacción: o entran todas, o ninguna.
    pub async fn insert_ibkr_bars(
        &self,
        rows: &[IbkrBarRow],
    ) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool().begin().await?;
        let mut report = InsertReport::default();
        for r in rows {
            let inserted = sqlx::query(
                "INSERT INTO ibkr_bars (source, instrument_id, kind, contract, bar_size,
                     what_to_show, start_at, open, high, low, close, volume)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                 ON CONFLICT ON CONSTRAINT ibkr_bars_key DO NOTHING",
            )
            .bind(&r.source)
            .bind(&r.instrument_id)
            .bind(&r.kind)
            .bind(&r.contract)
            .bind(&r.bar_size)
            .bind(&r.what_to_show)
            .bind(r.start)
            .bind(r.open)
            .bind(r.high)
            .bind(r.low)
            .bind(r.close)
            .bind(r.volume)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.inserted += 1;
                continue;
            }
            let stored: Stored = sqlx::query_as(
                "SELECT kind, start_at, open, high, low, close, volume FROM ibkr_bars
                 WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
                   AND what_to_show = $5 AND start_at = $6",
            )
            .bind(&r.source)
            .bind(&r.instrument_id)
            .bind(&r.contract)
            .bind(&r.bar_size)
            .bind(&r.what_to_show)
            .bind(r.start)
            .fetch_one(&mut *tx)
            .await?;
            let mine = (
                r.kind.clone(),
                r.start,
                r.open,
                r.high,
                r.low,
                r.close,
                r.volume,
            );
            if stored == mine {
                report.unchanged += 1;
            } else {
                return Err(StorageError::Conflict {
                    instrument: format!("{} ({})", r.instrument_id, r.contract),
                    timeframe: r.bar_size.clone(),
                    start: r.start,
                });
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Inicio de la última vela guardada de la serie, si hay alguna (para seguir desde ahí).
    pub async fn last_ibkr_bar_start(
        &self,
        s: &IbkrSeries<'_>,
    ) -> Result<Option<DateTime<Utc>>, StorageError> {
        let last: Option<DateTime<Utc>> = sqlx::query_scalar(
            "SELECT max(start_at) FROM ibkr_bars
             WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
               AND what_to_show = $5",
        )
        .bind(s.source)
        .bind(s.instrument_id)
        .bind(s.contract)
        .bind(s.bar_size)
        .bind(s.what_to_show)
        .fetch_one(self.pool())
        .await?;
        Ok(last)
    }
}

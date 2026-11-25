//! Velas cerradas de IBKR como fuente de datos (`ibkr_bars`, `ibkr_bar.v1`, #536), aparte de las
//! de IG y de Databento. Igual que [`PgStore::insert_candles`]: una vela ya guardada e idéntica
//! cuenta como `unchanged`; con otros valores es un conflicto (sería repintado) y no se guarda
//! nada del lote.
//!
//! Lecturas para el gateway (#551): solo velas **cerradas** a un instante (`start + tamaño <=
//! at`), de una sola serie (un contrato, un tamaño y un precio): nunca se mezclan contratos.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{InsertReport, PgStore, StorageError};

/// Columnas de [`IbkrBarRow`], en orden.
macro_rules! columns {
    () => {
        "source, instrument_id, kind, contract, bar_size, what_to_show, start_at, \
         open, high, low, close, volume"
    };
}

/// Cierre de la vela (inicio + tamaño).
macro_rules! end_at {
    () => {
        "(start_at + CASE bar_size WHEN '1m' THEN interval '1 minute' \
         WHEN '15m' THEN interval '15 minutes' ELSE interval '1 hour' END)"
    };
}

/// Orden de los tamaños, del más chico al más grande.
macro_rules! size_rank {
    () => {
        "CASE bar_size WHEN '1m' THEN 1 WHEN '15m' THEN 2 ELSE 3 END"
    };
}

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

/// Una serie con sus textos propios (para listar las series guardadas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrSeriesKey {
    pub source: String,
    pub instrument_id: String,
    pub contract: String,
    pub bar_size: String,
    pub what_to_show: String,
}

impl IbkrSeriesKey {
    pub fn as_series(&self) -> IbkrSeries<'_> {
        IbkrSeries {
            source: &self.source,
            instrument_id: &self.instrument_id,
            contract: &self.contract,
            bar_size: &self.bar_size,
            what_to_show: &self.what_to_show,
        }
    }
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

    /// La última vela **cerrada** a `at` de un instrumento y una fuente, de cualquier contrato y
    /// precio; con `bar_size`, solo de ese tamaño. Entre velas que cierran a la vez gana la más
    /// chica y, después, la cargada más tarde (la del contrato vigente).
    pub async fn latest_closed_ibkr_bar(
        &self,
        source: &str,
        instrument_id: &str,
        bar_size: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, StorageError> {
        let row: Option<Row> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            " FROM ibkr_bars
             WHERE source = $1 AND instrument_id = $2 AND ($3::text IS NULL OR bar_size = $3)
               AND ",
            end_at!(),
            " <= $4
             ORDER BY ",
            end_at!(),
            " DESC, ",
            size_rank!(),
            " ASC, inserted_at DESC
             LIMIT 1"
        ))
        .bind(source)
        .bind(instrument_id)
        .bind(bar_size)
        .bind(at)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(into_row))
    }

    /// Las últimas `limit` velas de la serie **cerradas** a `at`, de la más vieja a la más nueva.
    pub async fn ibkr_bars_closed_by(
        &self,
        s: &IbkrSeries<'_>,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<IbkrBarRow>, StorageError> {
        let mut rows: Vec<Row> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            " FROM ibkr_bars
             WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
               AND what_to_show = $5 AND ",
            end_at!(),
            " <= $6
             ORDER BY start_at DESC
             LIMIT $7"
        ))
        .bind(s.source)
        .bind(s.instrument_id)
        .bind(s.contract)
        .bind(s.bar_size)
        .bind(s.what_to_show)
        .bind(at)
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?;
        rows.reverse();
        Ok(rows.into_iter().map(into_row).collect())
    }

    /// Las velas de la serie que empiezan en `[from, to)`, en orden (#563: revisar una vela de 15m
    /// o 1h contra sus velas de 1m). Solo lectura.
    pub async fn ibkr_bars_between(
        &self,
        s: &IbkrSeries<'_>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<IbkrBarRow>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            " FROM ibkr_bars
             WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
               AND what_to_show = $5 AND start_at >= $6 AND start_at < $7
             ORDER BY start_at"
        ))
        .bind(s.source)
        .bind(s.instrument_id)
        .bind(s.contract)
        .bind(s.bar_size)
        .bind(s.what_to_show)
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(into_row).collect())
    }

    /// Todas las series guardadas, ordenadas. Solo lectura.
    pub async fn ibkr_series(&self) -> Result<Vec<IbkrSeriesKey>, StorageError> {
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT DISTINCT source, instrument_id, contract, bar_size, what_to_show
             FROM ibkr_bars ORDER BY 1, 2, 3, 4, 5",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(source, instrument_id, contract, bar_size, what_to_show)| IbkrSeriesKey {
                    source,
                    instrument_id,
                    contract,
                    bar_size,
                    what_to_show,
                },
            )
            .collect())
    }

    /// La última vela de la serie que **empieza** antes de `before` (para el cierre de una sesión
    /// anterior: `before` es el primer instante de la sesión actual).
    pub async fn last_ibkr_bar_starting_before(
        &self,
        s: &IbkrSeries<'_>,
        before: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, StorageError> {
        let row: Option<Row> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            " FROM ibkr_bars
             WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
               AND what_to_show = $5 AND start_at < $6
             ORDER BY start_at DESC
             LIMIT 1"
        ))
        .bind(s.source)
        .bind(s.instrument_id)
        .bind(s.contract)
        .bind(s.bar_size)
        .bind(s.what_to_show)
        .bind(before)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(into_row))
    }
}

type Row = (
    String,
    String,
    String,
    String,
    String,
    String,
    DateTime<Utc>,
    Decimal,
    Decimal,
    Decimal,
    Decimal,
    Option<Decimal>,
);

fn into_row(r: Row) -> IbkrBarRow {
    IbkrBarRow {
        source: r.0,
        instrument_id: r.1,
        kind: r.2,
        contract: r.3,
        bar_size: r.4,
        what_to_show: r.5,
        start: r.6,
        open: r.7,
        high: r.8,
        low: r.9,
        close: r.10,
        volume: r.11,
    }
}

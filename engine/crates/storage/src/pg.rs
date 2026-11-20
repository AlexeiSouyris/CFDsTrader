//! Histórico en PostgreSQL.

use cfds_core::{Candle, Instrument, InstrumentId, Timeframe};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Postgres, Row, Transaction};

use crate::StorageError;

/// Resultado de una carga: cuántas velas eran nuevas y cuántas ya estaban, idénticas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InsertReport {
    pub inserted: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
}

/// Columnas de `candles` en el orden que lee [`row_to_candle`]. Es una macro para armar SQL fijo
/// con `concat!`: sqlx exige consultas estáticas, que no se puedan inyectar.
macro_rules! candle_columns {
    () => {
        "instrument_id, timeframe, start_at, open, high, low, close, volume"
    };
}

impl PgStore {
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        Ok(Self {
            pool: PgPool::connect(database_url).await?,
        })
    }

    /// El pool de conexiones, para los módulos que escriben sus propias tablas (por ejemplo el
    /// diario) sobre la misma base y las mismas migraciones.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Aplica las migraciones de `infra/migrations/` que falten. Se puede correr varias veces.
    pub async fn migrate(&self) -> Result<(), StorageError> {
        sqlx::migrate!("../../../infra/migrations")
            .run(&self.pool)
            .await?;
        Ok(())
    }

    /// Crea el instrumento o actualiza sus datos descriptivos.
    pub async fn upsert_instrument(&self, i: &Instrument) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO instruments (id, name, asset_class, currency, exchange_timezone)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name,
               asset_class = EXCLUDED.asset_class, currency = EXCLUDED.currency,
               exchange_timezone = EXCLUDED.exchange_timezone",
        )
        .bind(i.id.as_str())
        .bind(&i.name)
        .bind(i.asset_class.as_str())
        .bind(i.currency.as_str())
        .bind(&i.exchange_timezone)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Guarda velas en **una sola transacción**: o entran todas, o ninguna.
    /// Una vela ya guardada e idéntica se cuenta como `unchanged`; con otros valores es
    /// [`StorageError::Conflict`] y no se guarda nada del lote.
    pub async fn insert_candles(&self, candles: &[Candle]) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool.begin().await?;
        let mut report = InsertReport::default();
        for c in candles {
            let inserted = sqlx::query(concat!(
                "INSERT INTO candles (",
                candle_columns!(),
                ") VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (instrument_id, timeframe, start_at) DO NOTHING"
            ))
            .bind(c.instrument_id.as_str())
            .bind(c.timeframe.as_str())
            .bind(c.start)
            .bind(c.open)
            .bind(c.high)
            .bind(c.low)
            .bind(c.close)
            .bind(c.volume)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.inserted += 1;
            } else if existing(&mut tx, c).await?.as_ref() == Some(c) {
                report.unchanged += 1;
            } else {
                return Err(StorageError::Conflict {
                    instrument: c.instrument_id.to_string(),
                    timeframe: c.timeframe.to_string(),
                    start: c.start,
                });
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Velas de un instrumento y timeframe con inicio en `[from, to)`, en orden.
    pub async fn candles(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, StorageError> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            candle_columns!(),
            " FROM candles
             WHERE instrument_id = $1 AND timeframe = $2 AND start_at >= $3 AND start_at < $4
             ORDER BY start_at"
        ))
        .bind(instrument.as_str())
        .bind(timeframe.as_str())
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_candle).collect()
    }
}

async fn existing(
    tx: &mut Transaction<'_, Postgres>,
    c: &Candle,
) -> Result<Option<Candle>, StorageError> {
    let row = sqlx::query(concat!(
        "SELECT ",
        candle_columns!(),
        " FROM candles
         WHERE instrument_id = $1 AND timeframe = $2 AND start_at = $3"
    ))
    .bind(c.instrument_id.as_str())
    .bind(c.timeframe.as_str())
    .bind(c.start)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref().map(row_to_candle).transpose()
}

fn row_to_candle(row: &PgRow) -> Result<Candle, StorageError> {
    let corrupt = |e: String| StorageError::CorruptRow(e);
    let id: String = row.try_get("instrument_id")?;
    let tf: String = row.try_get("timeframe")?;
    Ok(Candle {
        instrument_id: id.parse().map_err(|e| corrupt(format!("{e}")))?,
        timeframe: tf.parse().map_err(corrupt)?,
        start: row.try_get("start_at")?,
        open: row.try_get::<Decimal, _>("open")?,
        high: row.try_get::<Decimal, _>("high")?,
        low: row.try_get::<Decimal, _>("low")?,
        close: row.try_get::<Decimal, _>("close")?,
        volume: row.try_get::<Option<Decimal>, _>("volume")?,
    })
}

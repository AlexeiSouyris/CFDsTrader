//! Histórico en PostgreSQL.

use cfds_core::{
    Candle, CandleSpread, DailyCandle, DailyIndicator, Instrument, InstrumentId, ReferenceRate,
    SentimentObservation, Timeframe,
};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Postgres, Row, Transaction};

use crate::StorageError;

/// Fila de `daily_indicators`: día, publicación, ATR y volatilidad realizada.
type IndicatorRow = (NaiveDate, DateTime<Utc>, Option<Decimal>, Option<Decimal>);

/// Resultado de una carga: cuántas velas eran nuevas y cuántas ya estaban, idénticas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InsertReport {
    pub inserted: usize,
    pub unchanged: usize,
}

/// Fila de `ig_allowance_log`: observed_at, points_returned, remaining, total, resets_at.
type AllowanceRow = (DateTime<Utc>, i64, i64, i64, DateTime<Utc>);

/// Una respuesta de `GET /prices` de IG para `ig_allowance_log` (#140).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowanceRecord {
    pub observed_at: DateTime<Utc>,
    pub points_returned: u64,
    pub remaining: u64,
    pub total: u64,
    pub resets_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
}

/// Columnas de `candles` en el orden que lee [`row_to_candle`]. Es una macro para armar SQL fijo
/// con `concat!`: sqlx exige consultas estáticas, que no se puedan inyectar.
macro_rules! daily_columns {
    () => {
        "instrument_id, day, start_at, end_at, open, high, low, close, volume"
    };
}

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

    /// Guarda observaciones de sentimiento (#27) en `sentiment_observations`, que es de solo
    /// agregar. Repetir la misma observación no cambia nada; la misma clave con otro valor es un
    /// error (no se pisa). Todo o nada: una transacción.
    pub async fn insert_sentiment(
        &self,
        observations: &[SentimentObservation],
    ) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool.begin().await?;
        let mut report = InsertReport::default();
        for o in observations {
            let instrument = o.instrument_id.as_ref().map(InstrumentId::as_str);
            let inserted = sqlx::query(
                "INSERT INTO sentiment_observations
                     (source, series, instrument_id, observed_for, published_at, value)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT ON CONSTRAINT sentiment_observations_key DO NOTHING",
            )
            .bind(&o.source)
            .bind(&o.series)
            .bind(instrument)
            .bind(o.observed_for)
            .bind(o.published_at)
            .bind(o.value)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.inserted += 1;
                continue;
            }
            let stored: Decimal = sqlx::query_scalar(
                "SELECT value FROM sentiment_observations
                 WHERE source = $1 AND series = $2 AND instrument_id IS NOT DISTINCT FROM $3
                   AND observed_for = $4 AND published_at = $5",
            )
            .bind(&o.source)
            .bind(&o.series)
            .bind(instrument)
            .bind(o.observed_for)
            .bind(o.published_at)
            .fetch_one(&mut *tx)
            .await?;
            if stored == o.value {
                report.unchanged += 1;
            } else {
                return Err(StorageError::SentimentConflict {
                    feed: o.source.clone(),
                    series: o.series.clone(),
                    instrument: instrument.unwrap_or("-").to_owned(),
                    observed_for: o.observed_for,
                    published_at: o.published_at,
                });
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Guarda el spread de cierre de cada vela (#95) en **una sola transacción**. La vela tiene
    /// que estar guardada antes. El mismo valor se cuenta como `unchanged`; otro valor (o
    /// otra fuente) para la misma vela es [`StorageError::Conflict`] y no se guarda nada.
    pub async fn insert_spreads(
        &self,
        spreads: &[CandleSpread],
    ) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool.begin().await?;
        let mut report = InsertReport::default();
        for x in spreads {
            let inserted = sqlx::query(
                "INSERT INTO candle_spreads (instrument_id, timeframe, start_at, spread, source)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (instrument_id, timeframe, start_at) DO NOTHING",
            )
            .bind(x.instrument_id.as_str())
            .bind(x.timeframe.as_str())
            .bind(x.start)
            .bind(x.spread)
            .bind(&x.source)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.inserted += 1;
                continue;
            }
            let (spread, source): (Decimal, String) = sqlx::query_as(
                "SELECT spread, source FROM candle_spreads
                 WHERE instrument_id = $1 AND timeframe = $2 AND start_at = $3",
            )
            .bind(x.instrument_id.as_str())
            .bind(x.timeframe.as_str())
            .bind(x.start)
            .fetch_one(&mut *tx)
            .await?;
            if spread == x.spread && source == x.source {
                report.unchanged += 1;
            } else {
                return Err(StorageError::Conflict {
                    instrument: x.instrument_id.to_string(),
                    timeframe: x.timeframe.to_string(),
                    start: x.start,
                });
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Agrega las observaciones del cupo de IG de una corrida de `command` (#140), en una sola
    /// transacción. La tabla es de solo agregar.
    pub async fn insert_allowance_log(
        &self,
        command: &str,
        records: &[AllowanceRecord],
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        for r in records {
            sqlx::query(
                "INSERT INTO ig_allowance_log
                    (observed_at, command, points_returned, remaining, total, resets_at)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(r.observed_at)
            .bind(command)
            .bind(to_i64(r.points_returned)?)
            .bind(to_i64(r.remaining)?)
            .bind(to_i64(r.total)?)
            .bind(r.resets_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// La última observación del cupo de IG guardada, de cualquier comando.
    pub async fn last_allowance(&self) -> Result<Option<AllowanceRecord>, StorageError> {
        let row: Option<AllowanceRow> = sqlx::query_as(
            "SELECT observed_at, points_returned, remaining, total, resets_at
             FROM ig_allowance_log ORDER BY observed_at DESC, id DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        row.map(|(observed_at, points, remaining, total, resets_at)| {
            Ok(AllowanceRecord {
                observed_at,
                points_returned: to_u64(points)?,
                remaining: to_u64(remaining)?,
                total: to_u64(total)?,
                resets_at,
            })
        })
        .transpose()
    }

    /// Spreads de cierre de un instrumento y timeframe con inicio en `[from, to)`, en orden.
    pub async fn spreads(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<CandleSpread>, StorageError> {
        let rows: Vec<(DateTime<Utc>, Decimal, String)> = sqlx::query_as(
            "SELECT start_at, spread, source FROM candle_spreads
             WHERE instrument_id = $1 AND timeframe = $2 AND start_at >= $3 AND start_at < $4
             ORDER BY start_at",
        )
        .bind(instrument.as_str())
        .bind(timeframe.as_str())
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(start, spread, source)| CandleSpread {
                instrument_id: instrument.clone(),
                timeframe,
                start,
                spread,
                source,
            })
            .collect())
    }

    /// Guarda indicadores diarios (#148) en **una sola transacción**. Uno idéntico ya guardado
    /// cuenta como `unchanged`; otro valor para la misma clave es
    /// [`StorageError::DailyIndicatorConflict`] (sería repintado) y no se guarda nada.
    pub async fn insert_daily_indicators(
        &self,
        rows: &[DailyIndicator],
    ) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool.begin().await?;
        let mut report = InsertReport::default();
        for r in rows {
            let atr_period = i32::try_from(r.atr_period).unwrap_or(i32::MAX);
            let window = i32::try_from(r.realized_vol_window).unwrap_or(i32::MAX);
            let inserted = sqlx::query(
                "INSERT INTO daily_indicators (instrument_id, day, published_at, atr_period, atr,
                     realized_vol_window, realized_vol)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (instrument_id, day, atr_period, realized_vol_window) DO NOTHING",
            )
            .bind(r.instrument_id.as_str())
            .bind(r.day)
            .bind(r.published_at)
            .bind(atr_period)
            .bind(r.atr)
            .bind(window)
            .bind(r.realized_vol)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.inserted += 1;
                continue;
            }
            let stored: (DateTime<Utc>, Option<Decimal>, Option<Decimal>) = sqlx::query_as(
                "SELECT published_at, atr, realized_vol FROM daily_indicators
                 WHERE instrument_id = $1 AND day = $2 AND atr_period = $3
                   AND realized_vol_window = $4",
            )
            .bind(r.instrument_id.as_str())
            .bind(r.day)
            .bind(atr_period)
            .bind(window)
            .fetch_one(&mut *tx)
            .await?;
            if stored == (r.published_at, r.atr, r.realized_vol) {
                report.unchanged += 1;
            } else {
                return Err(StorageError::DailyIndicatorConflict {
                    instrument: r.instrument_id.to_string(),
                    day: r.day,
                });
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Indicadores diarios de un instrumento con día en `[from, to)` y los parámetros dados,
    /// en orden.
    pub async fn daily_indicators(
        &self,
        instrument: &InstrumentId,
        atr_period: u32,
        realized_vol_window: u32,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<DailyIndicator>, StorageError> {
        let rows: Vec<IndicatorRow> = sqlx::query_as(
            "SELECT day, published_at, atr, realized_vol FROM daily_indicators
                 WHERE instrument_id = $1 AND atr_period = $2 AND realized_vol_window = $3
                   AND day >= $4 AND day < $5
                 ORDER BY day",
        )
        .bind(instrument.as_str())
        .bind(i32::try_from(atr_period).unwrap_or(i32::MAX))
        .bind(i32::try_from(realized_vol_window).unwrap_or(i32::MAX))
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(day, published_at, atr, realized_vol)| DailyIndicator {
                instrument_id: instrument.clone(),
                day,
                published_at,
                atr_period,
                atr,
                realized_vol_window,
                realized_vol,
            })
            .collect())
    }

    /// Valores de un tipo de referencia (#79, `reference_rate.v1`) publicados hasta `until`, en
    /// orden de publicación. Incluye las correcciones (otra fila con otra `published_at`).
    pub async fn reference_rates(
        &self,
        series: &str,
        until: DateTime<Utc>,
    ) -> Result<Vec<ReferenceRate>, StorageError> {
        let rows: Vec<(NaiveDate, DateTime<Utc>, Decimal)> = sqlx::query_as(
            "SELECT ref_date, published_at, rate_pct FROM reference_rates
             WHERE series = $1 AND published_at <= $2
             ORDER BY published_at, ref_date",
        )
        .bind(series)
        .bind(until)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(ref_date, published_at, rate_pct)| ReferenceRate {
                series: series.to_string(),
                ref_date,
                published_at,
                rate_pct,
            })
            .collect())
    }

    /// Guarda velas diarias (#103) en **una sola transacción**. Una vela idéntica ya guardada
    /// cuenta como `unchanged`; otro valor para el mismo día es [`StorageError::DailyConflict`]
    /// y no se guarda nada del lote.
    pub async fn insert_daily(
        &self,
        candles: &[DailyCandle],
    ) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool.begin().await?;
        let mut report = InsertReport::default();
        for c in candles {
            let inserted = sqlx::query(concat!(
                "INSERT INTO daily_candles (",
                daily_columns!(),
                ") VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT (instrument_id, day) DO NOTHING"
            ))
            .bind(c.instrument_id.as_str())
            .bind(c.day)
            .bind(c.start)
            .bind(c.end)
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
                continue;
            }
            let row = sqlx::query(concat!(
                "SELECT ",
                daily_columns!(),
                " FROM daily_candles WHERE instrument_id = $1 AND day = $2"
            ))
            .bind(c.instrument_id.as_str())
            .bind(c.day)
            .fetch_one(&mut *tx)
            .await?;
            if row_to_daily(&row)? == *c {
                report.unchanged += 1;
            } else {
                return Err(StorageError::DailyConflict {
                    instrument: c.instrument_id.to_string(),
                    day: c.day,
                });
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Velas diarias de un instrumento con día en `[from, to)`, en orden.
    pub async fn daily_candles(
        &self,
        instrument: &InstrumentId,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<DailyCandle>, StorageError> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            daily_columns!(),
            " FROM daily_candles WHERE instrument_id = $1 AND day >= $2 AND day < $3
             ORDER BY day"
        ))
        .bind(instrument.as_str())
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_daily).collect()
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

fn row_to_daily(row: &PgRow) -> Result<DailyCandle, StorageError> {
    let id: String = row.try_get("instrument_id")?;
    Ok(DailyCandle {
        instrument_id: id
            .parse()
            .map_err(|e| StorageError::CorruptRow(format!("{e}")))?,
        day: row.try_get("day")?,
        start: row.try_get("start_at")?,
        end: row.try_get("end_at")?,
        open: row.try_get::<Decimal, _>("open")?,
        high: row.try_get::<Decimal, _>("high")?,
        low: row.try_get::<Decimal, _>("low")?,
        close: row.try_get::<Decimal, _>("close")?,
        volume: row.try_get::<Option<Decimal>, _>("volume")?,
    })
}

fn to_i64(v: u64) -> Result<i64, StorageError> {
    i64::try_from(v).map_err(|_| StorageError::CorruptRow(format!("{v} no entra en BIGINT")))
}

fn to_u64(v: i64) -> Result<u64, StorageError> {
    u64::try_from(v).map_err(|_| StorageError::CorruptRow(format!("{v} negativo en el cupo de IG")))
}

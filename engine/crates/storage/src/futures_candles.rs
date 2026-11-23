//! Velas de 15m y 1h de futuros (`futures_candles`, #259), aparte de las velas de IG.
//!
//! Las arma `cfds_market_data::futures_candles` desde `futures_bars_1m` y las guarda el ejemplo
//! `futures_candles` de `cfds-ingest`. Igual que [`PgStore::insert_candles`]: una vela ya
//! guardada e idéntica cuenta como `unchanged`, y con otros valores es un conflicto (sería
//! repintado) y no se guarda nada del lote.

use cfds_core::{Candle, FuturesCandle, InstrumentId, Timeframe};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{InsertReport, PgStore, StorageError};

/// Fila: contrato, inicio y OHLCV.
type CandleRow = (
    i64,
    DateTime<Utc>,
    Decimal,
    Decimal,
    Decimal,
    Decimal,
    Decimal,
);

impl PgStore {
    /// Guarda velas armadas de un futuro (de `source`, `dataset` y `symbol`) en una sola
    /// transacción.
    pub async fn insert_futures_candles(
        &self,
        source: &str,
        dataset: &str,
        symbol: &str,
        candles: &[FuturesCandle],
    ) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool().begin().await?;
        let mut report = InsertReport::default();
        for fc in candles {
            let c = &fc.candle;
            let contract = i64::try_from(fc.contract_id)
                .map_err(|_| StorageError::CorruptRow(format!("contract_id {}", fc.contract_id)))?;
            let volume = c.volume.unwrap_or_default();
            let inserted = sqlx::query(
                "INSERT INTO futures_candles (source, dataset, symbol, contract_id, instrument_id,
                     timeframe, start_at, open, high, low, close, volume)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                 ON CONFLICT ON CONSTRAINT futures_candles_key DO NOTHING",
            )
            .bind(source)
            .bind(dataset)
            .bind(symbol)
            .bind(contract)
            .bind(c.instrument_id.as_str())
            .bind(c.timeframe.as_str())
            .bind(c.start)
            .bind(c.open)
            .bind(c.high)
            .bind(c.low)
            .bind(c.close)
            .bind(volume)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.inserted += 1;
                continue;
            }
            let stored: CandleRow = sqlx::query_as(
                "SELECT contract_id, start_at, open, high, low, close, volume FROM futures_candles
                 WHERE source = $1 AND instrument_id = $2 AND timeframe = $3 AND start_at = $4",
            )
            .bind(source)
            .bind(c.instrument_id.as_str())
            .bind(c.timeframe.as_str())
            .bind(c.start)
            .fetch_one(&mut *tx)
            .await?;
            if stored == (contract, c.start, c.open, c.high, c.low, c.close, volume) {
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

    /// Velas armadas de `source` para el instrumento (CFD) `instrument`, con inicio en
    /// `[from, to)`, en orden. Tienen la forma de `candle.v1`: lo que las distingue de las de IG
    /// es de qué tabla salen, nunca un campo que se pueda olvidar.
    pub async fn futures_candles(
        &self,
        source: &str,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, StorageError> {
        let rows: Vec<CandleRow> = sqlx::query_as(
            "SELECT contract_id, start_at, open, high, low, close, volume FROM futures_candles
             WHERE source = $1 AND instrument_id = $2 AND timeframe = $3
               AND start_at >= $4 AND start_at < $5
             ORDER BY start_at",
        )
        .bind(source)
        .bind(instrument.as_str())
        .bind(timeframe.as_str())
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(_, start, open, high, low, close, volume)| Candle {
                instrument_id: instrument.clone(),
                timeframe,
                start,
                open,
                high,
                low,
                close,
                volume: Some(volume),
            })
            .collect())
    }

    /// Inicios de la primera vela de cada contrato nuevo (vencimientos, #267) entre las velas de
    /// `source` de `instrument` con inicio en `[from, to)`, en orden. La primera vela del período
    /// no cuenta como cambio.
    pub async fn futures_contract_changes(
        &self,
        source: &str,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, StorageError> {
        Ok(sqlx::query_scalar(
            "SELECT start_at FROM (
                 SELECT start_at, contract_id,
                        lag(contract_id) OVER (ORDER BY start_at) AS previous
                 FROM futures_candles
                 WHERE source = $1 AND instrument_id = $2 AND timeframe = $3
                   AND start_at >= $4 AND start_at < $5
             ) c
             WHERE previous IS NOT NULL AND contract_id <> previous
             ORDER BY start_at",
        )
        .bind(source)
        .bind(instrument.as_str())
        .bind(timeframe.as_str())
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?)
    }

    /// Desde y hasta cuándo (`[desde, hasta)`) están bajadas las barras de 1 minuto del futuro
    /// de `instrument` (`futures_bar_loads`). `None` si no se bajó nada.
    pub async fn futures_loaded_range(
        &self,
        source: &str,
        instrument: &InstrumentId,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, StorageError> {
        let row: (Option<DateTime<Utc>>, Option<DateTime<Utc>>) = sqlx::query_as(
            "SELECT min(l.from_at), max(l.to_at) FROM futures_bar_loads l
             WHERE l.source = $1 AND EXISTS (
                 SELECT 1 FROM futures_bars_1m b
                 WHERE b.source = l.source AND b.dataset = l.dataset AND b.symbol = l.symbol
                   AND b.instrument_id = $2)",
        )
        .bind(source)
        .bind(instrument.as_str())
        .fetch_one(self.pool())
        .await?;
        Ok(row.0.zip(row.1))
    }

    /// Velas de 15m o 1h de la fuente `feed` para el CFD `base`, con inicio en `[from, to)`, en
    /// orden, con `out` como instrumento (el id derivado de la fuente, para que lo que se calcule
    /// con ellas se guarde aparte). Con [`CandleFeed::Ig`] son las de `candles`.
    pub async fn candles_from(
        &self,
        feed: CandleFeed<'_>,
        base: &InstrumentId,
        out: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, StorageError> {
        let mut candles = match feed {
            CandleFeed::Ig => self.candles(base, timeframe, from, to).await?,
            CandleFeed::Futures(source) => {
                self.futures_candles(source, base, timeframe, from, to)
                    .await?
            }
        };
        for c in &mut candles {
            c.instrument_id = out.clone();
        }
        Ok(candles)
    }

    /// Inicio de la última vela de la fuente `feed` que empezó hasta `before`.
    pub async fn last_candle_start_from(
        &self,
        feed: CandleFeed<'_>,
        base: &InstrumentId,
        timeframe: Timeframe,
        before: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, StorageError> {
        match feed {
            CandleFeed::Ig => self.last_candle_start(base, timeframe, before).await,
            CandleFeed::Futures(source) => Ok(sqlx::query_scalar(
                "SELECT max(start_at) FROM futures_candles
                 WHERE source = $1 AND instrument_id = $2 AND timeframe = $3 AND start_at <= $4",
            )
            .bind(source)
            .bind(base.as_str())
            .bind(timeframe.as_str())
            .bind(before)
            .fetch_one(self.pool())
            .await?),
        }
    }
}

/// De qué tabla salen las velas (#259): las del CFD de IG o las armadas desde los futuros de
/// una fuente (por ejemplo `databento`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleFeed<'a> {
    Ig,
    Futures(&'a str),
}

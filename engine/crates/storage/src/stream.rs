//! Publicación de velas cerradas en Redis Streams.
//!
//! - Stream: `candles.v1.<instrumento>.<timeframe>` (por ejemplo `candles.v1.US100.15m`).
//! - Cada entrada tiene un campo `data` con el JSON del contrato `candle.v1`.
//! - El ID de la entrada es `<inicio de la vela en ms>-0`, así Redis mismo garantiza el orden
//!   y que una vela no se publique dos veces.

use cfds_core::{Candle, InstrumentId, Timeframe};
use chrono::{DateTime, Utc};
use redis::aio::MultiplexedConnection;

use crate::StorageError;

pub fn stream_key(instrument: &InstrumentId, timeframe: Timeframe) -> String {
    format!("candles.v1.{instrument}.{timeframe}")
}

pub fn stream_id(start: DateTime<Utc>) -> String {
    format!("{}-0", start.timestamp_millis())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    New {
        stream: String,
        id: String,
    },
    /// La vela ya estaba publicada (por ejemplo, después de un reintento): no se duplica.
    AlreadyPublished {
        stream: String,
        id: String,
    },
}

pub struct CandlePublisher {
    conn: MultiplexedConnection,
}

impl CandlePublisher {
    pub async fn connect(redis_url: &str) -> Result<Self, StorageError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self {
            conn: client.get_multiplexed_async_connection().await?,
        })
    }

    /// Publica una vela **cerrada** a las `now`. Una vela en curso es un error: publicarla
    /// dejaría que alguien decida con información que todavía no existe.
    pub async fn publish(
        &mut self,
        candle: &Candle,
        now: DateTime<Utc>,
    ) -> Result<Published, StorageError> {
        if !candle.is_closed_at(now) {
            return Err(StorageError::NotClosed {
                start: candle.start,
                now,
            });
        }
        let stream = stream_key(&candle.instrument_id, candle.timeframe);
        let id = stream_id(candle.start);
        let data = serde_json::to_string(candle)?;
        let added: Result<String, redis::RedisError> = redis::cmd("XADD")
            .arg(&stream)
            .arg(&id)
            .arg("data")
            .arg(&data)
            .query_async(&mut self.conn)
            .await;
        match added {
            Ok(id) => Ok(Published::New { stream, id }),
            // Redis rechaza un ID menor o igual al último. Puede ser la misma vela (reintento)
            // o una vela vieja que llega tarde: se distingue mirando si ese ID existe.
            Err(e) if e.code() == Some("ERR") => {
                let found: Vec<(String, Vec<(String, String)>)> = redis::cmd("XRANGE")
                    .arg(&stream)
                    .arg(&id)
                    .arg(&id)
                    .query_async(&mut self.conn)
                    .await?;
                if found.is_empty() {
                    Err(StorageError::OutOfOrder {
                        stream,
                        start: candle.start,
                    })
                } else {
                    Ok(Published::AlreadyPublished { stream, id })
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_names_and_ids_follow_the_convention() {
        let id: InstrumentId = "US100".parse().unwrap();
        assert_eq!(stream_key(&id, Timeframe::M15), "candles.v1.US100.15m");
        assert_eq!(stream_key(&id, Timeframe::H1), "candles.v1.US100.1h");
        let start: DateTime<Utc> = "2026-11-19T15:30:00Z".parse().unwrap();
        assert_eq!(stream_id(start), "1795102200000-0");
    }
}

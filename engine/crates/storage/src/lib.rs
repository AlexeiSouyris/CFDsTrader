//! Almacenamiento (Issue #6): histórico en PostgreSQL y velas cerradas en Redis Streams.
//!
//! - [`PgStore`]: escritura **idempotente** (recargar el mismo período no duplica nada) y que
//!   **no pisa datos**: si llega una vela con la misma clave pero otros precios, es un error.
//! - [`CandlePublisher`]: publica cada vela **cerrada** en `candles.v1.<instrumento>.<timeframe>`,
//!   con el JSON del contrato `candle.v1`.
//!
//! Las migraciones viven en `infra/migrations/` y se aplican con [`PgStore::migrate`].

mod pg;
mod stream;

pub use pg::{InsertReport, PgStore};
pub use stream::{CandlePublisher, Published, stream_id, stream_key};

use chrono::{DateTime, Utc};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("PostgreSQL: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migraciones: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("Redis: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "ya existe {instrument} {timeframe} {start} con otros valores: no se pisa (revisar la fuente)"
    )]
    Conflict {
        instrument: String,
        timeframe: String,
        start: DateTime<Utc>,
    },
    #[error("fila inválida en la base: {0}")]
    CorruptRow(String),
    #[error("la vela de {start} todavía no cerró a las {now}: no se publica")]
    NotClosed {
        start: DateTime<Utc>,
        now: DateTime<Utc>,
    },
    #[error("la vela de {start} llega después de una más nueva ya publicada en {stream}")]
    OutOfOrder {
        stream: String,
        start: DateTime<Utc>,
    },
}

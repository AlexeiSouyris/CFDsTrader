//! Almacenamiento (Issue #6): histórico en PostgreSQL y velas cerradas en Redis Streams.
//!
//! - [`PgStore`]: escritura **idempotente** (recargar el mismo período no duplica nada) y que
//!   **no pisa datos**: si llega una vela con la misma clave pero otros precios, es un error.
//! - [`CandlePublisher`]: publica cada vela **cerrada** en `candles.v1.<instrumento>.<timeframe>`,
//!   con el JSON del contrato `candle.v1`.
//! - [`NewsInterpretationConsumer`]: lee las interpretaciones de noticias que publica Python en
//!   `news_interpretations.v1`, sin perder ni repetir entradas entre reinicios (#197).
//! - Calendario económico (`economic_event.v1`, #243): [`PgStore::economic_events`] y
//!   [`PgStore::economic_event_sightings`], solo lectura (lo escribe Python).
//! - Mapa de liquidez (`liquidity_level.v1`, #231): [`PgStore::insert_liquidity_levels`] y
//!   [`PgStore::liquidity_levels`], de solo agregar y sin repintado.
//! - Estadísticas de liquidez (#231): eventos de FVG, gaps de apertura, desequilibrio y
//!   agregados por corrida ([`PgStore::insert_fvg_events`] y siguientes), de solo agregar.
//!
//! Las migraciones viven en `infra/migrations/` y se aplican con [`PgStore::migrate`].

mod economic_events;
mod futures_bars;
mod liquidity_levels;
mod liquidity_stats;
mod news_stream;
mod pg;
mod stream;

pub use liquidity_levels::LevelsReport;
pub use liquidity_stats::{StatsReport, StatsRunReport};
pub use news_stream::{
    Delivery, NEWS_INTERPRETATIONS_STREAM, NewsBatch, NewsInterpretationConsumer, Rejected, StartAt,
};
pub use pg::{AllowanceRecord, InsertReport, PgStore};
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
        "ya existe la observación {feed}/{series} de {instrument} para {observed_for} \
         publicada {published_at} con otro valor: no se pisa (revisar la fuente)"
    )]
    SentimentConflict {
        feed: String,
        series: String,
        instrument: String,
        observed_for: chrono::DateTime<chrono::Utc>,
        published_at: chrono::DateTime<chrono::Utc>,
    },
    #[error(
        "ya existe la vela diaria {instrument} {day} con otros valores: no se pisa (revisar la fuente)"
    )]
    DailyConflict {
        instrument: String,
        day: chrono::NaiveDate,
    },
    #[error(
        "el indicador diario de {instrument} {day} ya está guardado con otro valor: sería repintado (revisar el cálculo o las velas)"
    )]
    DailyIndicatorConflict {
        instrument: String,
        day: chrono::NaiveDate,
    },
    #[error(
        "ya existe {instrument} {timeframe} {start} con otros valores: no se pisa (revisar la fuente)"
    )]
    Conflict {
        instrument: String,
        timeframe: String,
        start: DateTime<Utc>,
    },
    #[error(
        "el nivel de liquidez {kind} de {instrument} que existe desde {exists_from} ya está guardado con otro estado: sería repintado (revisar el cálculo o las velas)"
    )]
    LiquidityLevelConflict {
        instrument: String,
        kind: String,
        exists_from: DateTime<Utc>,
    },
    #[error(
        "{what} de {instrument} ({at}) ya está guardado con otros valores o eventos: sería repintado (revisar el cálculo, la config o las velas)"
    )]
    LiquidityStatsConflict {
        what: String,
        instrument: String,
        at: DateTime<Utc>,
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

//! Lectura del contexto de mercado que escribe Python (#294): `sentiment_index` (`sentiment.v1`,
//! migración 0017) y `analog_reports` (`analog_report.v1`, migración 0029). Solo lectura: cada
//! fila se arma como documento del contrato y se lee con su tipo (una fila que no lo cumple es
//! un error, no se ignora).

use cfds_core::{AnalogReport, InstrumentId, SentimentIndex};
use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::{PgStore, StorageError};

type SentimentRow = (
    String,
    DateTime<Utc>,
    DateTime<Utc>,
    Decimal,
    bool,
    i32,
    Value,
);

fn sentiment(row: SentimentRow) -> Result<SentimentIndex, StorageError> {
    let (instrument, as_of, published_at, value, warmup, min, components) = row;
    let utc = |t: DateTime<Utc>| t.to_rfc3339_opts(SecondsFormat::AutoSi, true);
    let doc = serde_json::json!({
        "instrument_id": instrument,
        "as_of": utc(as_of),
        "published_at": utc(published_at),
        "value": value.normalize().to_string(),
        "warmup_complete": warmup,
        "min_components": min,
        "components": components,
    });
    serde_json::from_value(doc).map_err(StorageError::from)
}

const SENTIMENT_COLUMNS: &str =
    "instrument_id, as_of, published_at, round(value, 28) AS value, warmup_complete, min_components,
     components";

impl PgStore {
    /// El último índice de sentimiento publicado hasta `at` de cada instrumento.
    pub async fn latest_sentiment(
        &self,
        at: DateTime<Utc>,
    ) -> Result<Vec<SentimentIndex>, StorageError> {
        let rows: Vec<SentimentRow> = sqlx::query_as(
            "SELECT DISTINCT ON (instrument_id)
               instrument_id, as_of, published_at, round(value, 28) AS value, warmup_complete,
               min_components, components
             FROM sentiment_index WHERE published_at <= $1
             ORDER BY instrument_id, published_at DESC",
        )
        .bind(at)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(sentiment).collect()
    }

    /// Los últimos `limit` valores del índice de un instrumento publicados hasta `at`, del más
    /// viejo al más nuevo.
    pub async fn sentiment_history(
        &self,
        instrument: &InstrumentId,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<SentimentIndex>, StorageError> {
        let sql = format!(
            "SELECT * FROM (
               SELECT {SENTIMENT_COLUMNS} FROM sentiment_index
               WHERE instrument_id = $1 AND published_at <= $2
               ORDER BY published_at DESC LIMIT $3
             ) last ORDER BY published_at"
        );
        let rows: Vec<SentimentRow> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(instrument.as_str())
            .bind(at)
            .bind(i64::from(limit))
            .fetch_all(self.pool())
            .await?;
        rows.into_iter().map(sentiment).collect()
    }

    /// El último informe de análogos de cada instrumento y fuente, calculado hasta `at`.
    pub async fn latest_analog_reports(
        &self,
        at: DateTime<Utc>,
    ) -> Result<Vec<AnalogReport>, StorageError> {
        let docs: Vec<Value> = sqlx::query_scalar(
            "SELECT DISTINCT ON (instrument_id, source) document FROM analog_reports
             WHERE computed_at <= $1
             ORDER BY instrument_id, source, as_of DESC, id DESC",
        )
        .bind(at)
        .fetch_all(self.pool())
        .await?;
        docs.into_iter()
            .map(|d| serde_json::from_value(d).map_err(StorageError::from))
            .collect()
    }
}

//! Lectura del calendario económico guardado (`economic_events`, `economic_event.v1`, #243).
//!
//! La tabla la escribe Python (`cfds_intel.calendar`, #239). El motor solo la lee, sin mirar el
//! futuro: nada publicado después de `published_until` sale de acá. El filtro §7.2
//! (`cfds_market_data::news_flags`) además exige `published_at ≤ cierre` en cada vela.

use cfds_core::EconomicEvent;
use chrono::{DateTime, SecondsFormat, Utc};

use crate::{PgStore, StorageError};

/// Fila: fuente, evento, país, instrumento, impacto, hora y publicación.
type EventRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    DateTime<Utc>,
    DateTime<Utc>,
);

impl PgStore {
    /// Eventos de todas las fuentes con hora en `[scheduled_from, scheduled_to]` y publicados
    /// hasta `published_until`, en orden de hora. Cada fila se valida con el contrato.
    pub async fn economic_events(
        &self,
        scheduled_from: DateTime<Utc>,
        scheduled_to: DateTime<Utc>,
        published_until: DateTime<Utc>,
    ) -> Result<Vec<EconomicEvent>, StorageError> {
        let rows: Vec<EventRow> = sqlx::query_as(
            "SELECT source, event, country, instrument_id, impact, scheduled_at, published_at
             FROM economic_events
             WHERE scheduled_at >= $1 AND scheduled_at <= $2 AND published_at <= $3
             ORDER BY scheduled_at, published_at, source, event",
        )
        .bind(scheduled_from)
        .bind(scheduled_to)
        .bind(published_until)
        .fetch_all(self.pool())
        .await?;
        let utc = |t: DateTime<Utc>| t.to_rfc3339_opts(SecondsFormat::AutoSi, true);
        rows.into_iter()
            .map(
                |(source, event, country, instrument, impact, scheduled, published)| {
                    let doc = serde_json::json!({
                        "source": source,
                        "event": event,
                        "country": country,
                        "instrument_id": instrument,
                        "impact": impact,
                        "scheduled_at": utc(scheduled),
                        "published_at": utc(published),
                    });
                    serde_json::from_value(doc).map_err(StorageError::from)
                },
            )
            .collect()
    }

    /// Las horas distintas en que la ingesta vio eventos nuevos (`published_at`), hasta `until`
    /// y en orden. Con ellas se sabe qué tramos tienen calendario
    /// (`cfds_market_data::calendar_coverage`).
    pub async fn economic_event_sightings(
        &self,
        until: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, StorageError> {
        Ok(sqlx::query_scalar(
            "SELECT DISTINCT published_at FROM economic_events
             WHERE published_at <= $1 ORDER BY published_at",
        )
        .bind(until)
        .fetch_all(self.pool())
        .await?)
    }
}

//! Lectura del calendario histórico oficial (`official_releases`, `official_release.v1`,
//! ADR 0016, #520). La tabla la escribe Python (`cfds_intel.calendar.official`). El motor solo
//! la lee, sin mirar el futuro: nada publicado después de `published_until` sale de acá, y cada
//! estudio además exige `published_at` ≤ la decisión (programa Nasdaq, #526).

use chrono::{DateTime, Utc};

use crate::{PgStore, StorageError};

/// Una fila de `official_release.v1`, con lo que usan los estudios.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialReleaseRow {
    pub source: String,
    pub series: String,
    pub reference_period: String,
    pub status: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub published_at: DateTime<Utc>,
    pub time_rule: String,
    pub with_press_conference: Option<bool>,
    /// Página oficial de la que sale la fila.
    pub source_url: String,
}

type Row = (
    String,
    String,
    String,
    String,
    Option<DateTime<Utc>>,
    DateTime<Utc>,
    String,
    Option<bool>,
    String,
);

impl PgStore {
    /// Filas de una serie (`source`, `series`) publicadas hasta `published_until`, en orden de
    /// hora programada y de publicación.
    pub async fn official_releases(
        &self,
        source: &str,
        series: &str,
        published_until: DateTime<Utc>,
    ) -> Result<Vec<OfficialReleaseRow>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT source, series, reference_period, status, scheduled_at, published_at,
                    time_rule, with_press_conference, source_url
             FROM official_releases
             WHERE source = $1 AND series = $2 AND published_at <= $3
             ORDER BY scheduled_at NULLS LAST, published_at, reference_period, status",
        )
        .bind(source)
        .bind(series)
        .bind(published_until)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(to_row).collect())
    }

    /// Filas de todas las series con hora programada en `[scheduled_from, scheduled_to]` y
    /// publicadas hasta `published_until`, en orden de hora (#600, el calendario de la UI). Las
    /// canceladas no tienen hora y no entran.
    pub async fn official_releases_between(
        &self,
        scheduled_from: DateTime<Utc>,
        scheduled_to: DateTime<Utc>,
        published_until: DateTime<Utc>,
    ) -> Result<Vec<OfficialReleaseRow>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT source, series, reference_period, status, scheduled_at, published_at,
                    time_rule, with_press_conference, source_url
             FROM official_releases
             WHERE scheduled_at >= $1 AND scheduled_at <= $2 AND published_at <= $3
             ORDER BY scheduled_at, published_at, source, series, reference_period, status",
        )
        .bind(scheduled_from)
        .bind(scheduled_to)
        .bind(published_until)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(to_row).collect())
    }
}

fn to_row(
    (
        source,
        series,
        reference_period,
        status,
        scheduled_at,
        published_at,
        time_rule,
        with_press_conference,
        source_url,
    ): Row,
) -> OfficialReleaseRow {
    OfficialReleaseRow {
        source,
        series,
        reference_period,
        status,
        scheduled_at,
        published_at,
        time_rule,
        with_press_conference,
        source_url,
    }
}

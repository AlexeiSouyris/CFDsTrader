//! Lectura de las noticias guardadas (`news_items`, `news_item.v1`) y de sus interpretaciones
//! (`news_interpretations`, `news_interpretation.v1`) para la vista Noticias de la UI (#600).
//!
//! Las dos tablas las escribe Python (`cfds_intel.news`). El motor solo las lee, sin mirar el
//! futuro: una noticia sale recién desde su `published_at` y una interpretación desde su
//! `interpreted_at` (CLAUDE.md §5b).

use cfds_core::NewsInterpretation;
use chrono::{DateTime, Utc};

use crate::pg::{NewsInterpretationRow, row_to_news_interpretation};
use crate::{PgStore, StorageError};

/// Una fila de `news_item.v1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsItemRow {
    pub source: String,
    pub feed: String,
    pub item_id: String,
    pub title: String,
    pub url: String,
    pub published_at: DateTime<Utc>,
    pub retrieved_at: DateTime<Utc>,
}

type Row = (
    String,
    String,
    String,
    String,
    String,
    DateTime<Utc>,
    DateTime<Utc>,
);

impl PgStore {
    /// Una página de noticias, de la más nueva a la más vieja: publicadas hasta `published_until`,
    /// antes de `before` (si viene; sin incluirlo) y del `feed` pedido (si viene).
    ///
    /// Son `limit` como máximo, salvo que la última comparta `published_at` con otras: entonces
    /// vienen todas las de esa hora, así la página siguiente (`before` = esa hora) no saltea
    /// ninguna. Con `limit` = 0 no viene nada.
    pub async fn news_page(
        &self,
        feed: Option<&str>,
        before: Option<DateTime<Utc>>,
        published_until: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<NewsItemRow>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(
            "WITH page AS (
                 SELECT published_at FROM news_items
                 WHERE published_at <= $1
                   AND ($2::timestamptz IS NULL OR published_at < $2)
                   AND ($3::text IS NULL OR feed = $3)
                 ORDER BY published_at DESC
                 LIMIT $4
             )
             SELECT source, feed, item_id, title, url, published_at, retrieved_at
             FROM news_items
             WHERE published_at <= $1
               AND ($2::timestamptz IS NULL OR published_at < $2)
               AND ($3::text IS NULL OR feed = $3)
               AND published_at >= (SELECT min(published_at) FROM page)
             ORDER BY published_at DESC, source, item_id",
        )
        .bind(published_until)
        .bind(before)
        .bind(feed)
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(source, feed, item_id, title, url, published_at, retrieved_at)| NewsItemRow {
                    source,
                    feed,
                    item_id,
                    title,
                    url,
                    published_at,
                    retrieved_at,
                },
            )
            .collect())
    }

    /// Las interpretaciones de esas noticias (`(source, item_id)`) hechas hasta
    /// `interpreted_until`, de todos los intérpretes, en orden de `interpreted_at`. Cada fila se
    /// valida con el contrato, como en [`PgStore::news_interpretations`].
    pub async fn news_interpretations_of(
        &self,
        items: &[(String, String)],
        interpreted_until: DateTime<Utc>,
    ) -> Result<Vec<NewsInterpretation>, StorageError> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let (sources, ids): (Vec<String>, Vec<String>) = items.iter().cloned().unzip();
        let rows: Vec<NewsInterpretationRow> = sqlx::query_as(
            "SELECT i.interpretation_id, i.source, i.item_id, i.published_at, i.interpreted_at,
                    i.interpreter_name, i.interpreter_version, i.interpreter_model,
                    i.interpreter_prompt_sha256, i.summary_es, i.impacts::text
             FROM news_interpretations i
             JOIN unnest($1::text[], $2::text[]) AS k(source, item_id)
               ON i.source = k.source AND i.item_id = k.item_id
             WHERE i.interpreted_at <= $3
             ORDER BY i.interpreted_at, i.interpretation_id",
        )
        .bind(sources)
        .bind(ids)
        .bind(interpreted_until)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(row_to_news_interpretation).collect()
    }
}

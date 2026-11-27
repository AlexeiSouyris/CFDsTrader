//! Noticias y calendario para la vista Noticias de la UI (#600), **solo lectura**:
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/news[?limit=<n>][&before=<UTC>][&feed=<feed>]` | `gateway_news.v1` |
//! | `GET /api/v1/calendar[?from=<UTC>][&to=<UTC>]` | `gateway_calendar.v1` |
//!
//! Lee lo que guardó Python: `news_items` con `news_interpretations`, y `economic_events` con
//! `official_releases`. Sin mirar el futuro: una noticia sale desde su `published_at`, una
//! interpretación desde su `interpreted_at` y una fila del calendario desde su `published_at`
//! (CLAUDE.md §5b). La interpretación se pasa tal cual la guardó `intelligence`: el gateway no
//! calcula nada, no envía órdenes ni escribe en la base.

use std::future::Future;

use cfds_core::{EconomicEvent, NewsInterpretation};
use cfds_storage::{NewsItemRow, OfficialReleaseRow, PgStore};
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::Reply;

const NEWS: &str = "/api/v1/news";
const CALENDAR: &str = "/api/v1/calendar";

/// `[news]` y `[calendar]` de `config/gateway.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewsConfig {
    /// Noticias que devuelve `/api/v1/news` sin `?limit`.
    pub default_limit: u32,
    /// El máximo que acepta `?limit`.
    pub max_limit: u32,
    /// Sin `?from`, el calendario empieza esto antes de ahora.
    pub calendar_past: TimeDelta,
    /// Sin `?to`, el calendario termina esto después de `from` o de ahora.
    pub calendar_future: TimeDelta,
    /// El rango más largo que acepta el calendario (`to - from`).
    pub calendar_max: TimeDelta,
}

impl NewsConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            news: RawNews,
            calendar: RawCalendar,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawNews {
            default_limit: u32,
            max_limit: u32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawCalendar {
            past_days: u32,
            future_days: u32,
            max_days: u32,
        }
        let r = toml::from_str::<Raw>(text).map_err(|e| e.to_string())?;
        let n = r.news;
        if n.default_limit == 0 || n.max_limit > 1000 || n.default_limit > n.max_limit {
            return Err("news: 0 < default_limit <= max_limit <= 1000".into());
        }
        let c = r.calendar;
        if c.max_days == 0 || c.max_days > 3660 {
            return Err("calendar.max_days: entre 1 y 3660".into());
        }
        if c.future_days == 0 || c.past_days + c.future_days > c.max_days {
            return Err("calendar: 0 < future_days y past_days + future_days <= max_days".into());
        }
        let days = |d: u32| TimeDelta::days(i64::from(d));
        Ok(Self {
            default_limit: n.default_limit,
            max_limit: n.max_limit,
            calendar_past: days(c.past_days),
            calendar_future: days(c.future_days),
            calendar_max: days(c.max_days),
        })
    }
}

/// De dónde salen las noticias y el calendario. En producción, PostgreSQL.
pub trait NewsSource: Sync {
    /// Ver [`PgStore::news_page`].
    fn news_page(
        &self,
        feed: Option<&str>,
        before: Option<DateTime<Utc>>,
        until: DateTime<Utc>,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<NewsItemRow>, String>> + Send;
    /// Ver [`PgStore::news_interpretations_of`].
    fn interpretations_of(
        &self,
        items: &[(String, String)],
        until: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<NewsInterpretation>, String>> + Send;
    /// Ver [`PgStore::economic_events`].
    fn economic_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<EconomicEvent>, String>> + Send;
    /// Ver [`PgStore::official_releases_between`].
    fn official_releases(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<OfficialReleaseRow>, String>> + Send;
}

impl NewsSource for PgStore {
    async fn news_page(
        &self,
        feed: Option<&str>,
        before: Option<DateTime<Utc>>,
        until: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<NewsItemRow>, String> {
        PgStore::news_page(self, feed, before, until, limit)
            .await
            .map_err(|e| e.to_string())
    }
    async fn interpretations_of(
        &self,
        items: &[(String, String)],
        until: DateTime<Utc>,
    ) -> Result<Vec<NewsInterpretation>, String> {
        self.news_interpretations_of(items, until)
            .await
            .map_err(|e| e.to_string())
    }
    async fn economic_events(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<EconomicEvent>, String> {
        PgStore::economic_events(self, from, to, until)
            .await
            .map_err(|e| e.to_string())
    }
    async fn official_releases(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<OfficialReleaseRow>, String> {
        self.official_releases_between(from, to, until)
            .await
            .map_err(|e| e.to_string())
    }
}

fn utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

/// Una noticia como `contracts/news_item.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewsItemOut {
    pub source: String,
    pub feed: String,
    pub item_id: String,
    pub title: String,
    pub url: String,
    pub published_at: String,
    pub retrieved_at: String,
}

impl From<NewsItemRow> for NewsItemOut {
    fn from(r: NewsItemRow) -> Self {
        Self {
            published_at: utc(r.published_at),
            retrieved_at: utc(r.retrieved_at),
            source: r.source,
            feed: r.feed,
            item_id: r.item_id,
            title: r.title,
            url: r.url,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewsEntry {
    pub item: NewsItemOut,
    pub interpretations: Vec<NewsInterpretation>,
}

/// `contracts/gateway_news.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewsReply {
    pub as_of: String,
    pub generated_at: String,
    pub feed: Option<String>,
    pub items: Vec<NewsEntry>,
    pub next_before: Option<String>,
}

/// Una publicación oficial como `contracts/official_release.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OfficialReleaseOut {
    pub source: String,
    pub series: String,
    pub reference_period: String,
    pub status: String,
    pub scheduled_at: Option<String>,
    pub published_at: String,
    pub time_rule: String,
    pub with_press_conference: Option<bool>,
    pub source_url: String,
}

impl From<OfficialReleaseRow> for OfficialReleaseOut {
    fn from(r: OfficialReleaseRow) -> Self {
        Self {
            scheduled_at: r.scheduled_at.map(utc),
            published_at: utc(r.published_at),
            source: r.source,
            series: r.series,
            reference_period: r.reference_period,
            status: r.status,
            time_rule: r.time_rule,
            with_press_conference: r.with_press_conference,
            source_url: r.source_url,
        }
    }
}

/// `contracts/gateway_calendar.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CalendarReply {
    pub from: String,
    pub to: String,
    pub as_of: String,
    pub generated_at: String,
    pub economic_events: Vec<EconomicEvent>,
    pub official_releases: Vec<OfficialReleaseOut>,
}

/// Junta cada noticia con sus interpretaciones (en el orden en que llegan). Función pura.
pub fn attach(items: Vec<NewsItemRow>, interpretations: Vec<NewsInterpretation>) -> Vec<NewsEntry> {
    items
        .into_iter()
        .map(|item| {
            let mine = interpretations
                .iter()
                .filter(|i| i.source == item.source && i.item_id == item.item_id)
                .cloned()
                .collect();
            NewsEntry {
                item: item.into(),
                interpretations: mine,
            }
        })
        .collect()
}

/// `true` si la ruta es de este módulo.
pub fn is_news_route(path: &str) -> bool {
    matches!(path, NEWS | CALENDAR)
}

/// Un nombre de feed como en `news_item.v1` (`^[a-z][a-z0-9_]{0,31}$`).
fn valid_feed(s: &str) -> bool {
    s.len() <= 32
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

fn json<T: Serialize>(v: &T) -> Reply {
    match serde_json::to_string(v) {
        Ok(body) => Reply {
            status: 200,
            content_type: "application/json",
            body,
        },
        Err(e) => {
            eprintln!("gateway: no se pudieron serializar las noticias: {e}");
            text(500, "error interno")
        }
    }
}

fn internal(e: String) -> Reply {
    eprintln!("gateway: noticias: {e}");
    text(500, "error interno")
}

/// Una fecha de la query (`before`, `from`, `to`): RFC 3339 en UTC (`…Z`).
fn time_param(query: Option<&str>, name: &str) -> Result<Option<DateTime<Utc>>, Reply> {
    match crate::param(query, name) {
        None => Ok(None),
        Some(s) => crate::parse_before(s)
            .map(Some)
            .ok_or_else(|| text(400, &format!("{name}: fecha RFC 3339 en UTC (…Z)"))),
    }
}

/// Atiende las rutas de noticias y calendario. Solo `GET`.
pub async fn respond_news<S: NewsSource>(
    store: &S,
    cfg: &NewsConfig,
    method: &str,
    path: &str,
    query: Option<&str>,
    now: DateTime<Utc>,
) -> Reply {
    if !is_news_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(
            405,
            "solo GET: las noticias y el calendario son de solo lectura",
        );
    }
    if path == CALENDAR {
        return calendar(store, cfg, query, now).await;
    }
    let limit = match crate::param(query, "limit") {
        None => cfg.default_limit,
        Some(s) => match s.parse::<u32>() {
            Ok(n) if (1..=cfg.max_limit).contains(&n) => n,
            _ => return text(400, &format!("limit: entero entre 1 y {}", cfg.max_limit)),
        },
    };
    let before = match time_param(query, "before") {
        Ok(b) => b,
        Err(r) => return r,
    };
    let feed = match crate::param(query, "feed") {
        None => None,
        Some(f) if valid_feed(f) => Some(f),
        Some(_) => return text(400, "feed: minúsculas, dígitos o _ (como news_item.v1)"),
    };
    let items = match store.news_page(feed, before, now, limit).await {
        Ok(i) => i,
        Err(e) => return internal(e),
    };
    // ¿Hay más viejas? Una sola fila antes de la última alcanza para saberlo.
    let next_before = match items.last() {
        None => None,
        Some(last) => match store.news_page(feed, Some(last.published_at), now, 1).await {
            Ok(more) if more.is_empty() => None,
            Ok(_) => Some(utc(last.published_at)),
            Err(e) => return internal(e),
        },
    };
    let keys: Vec<(String, String)> = items
        .iter()
        .map(|i| (i.source.clone(), i.item_id.clone()))
        .collect();
    let interpretations = match store.interpretations_of(&keys, now).await {
        Ok(v) => v,
        Err(e) => return internal(e),
    };
    json(&NewsReply {
        as_of: utc(now),
        generated_at: utc(now),
        feed: feed.map(str::to_owned),
        items: attach(items, interpretations),
        next_before,
    })
}

async fn calendar<S: NewsSource>(
    store: &S,
    cfg: &NewsConfig,
    query: Option<&str>,
    now: DateTime<Utc>,
) -> Reply {
    let (from, to) = match (time_param(query, "from"), time_param(query, "to")) {
        (Err(r), _) | (_, Err(r)) => return r,
        (Ok(f), Ok(t)) => {
            let from = f.unwrap_or(now - cfg.calendar_past);
            (from, t.unwrap_or(from.max(now) + cfg.calendar_future))
        }
    };
    if to < from {
        return text(400, "to tiene que ser posterior a from");
    }
    if to - from > cfg.calendar_max {
        return text(
            400,
            &format!(
                "el rango from..to es de {} días como máximo",
                cfg.calendar_max.num_days()
            ),
        );
    }
    let economic_events = match store.economic_events(from, to, now).await {
        Ok(v) => v,
        Err(e) => return internal(e),
    };
    let official_releases = match store.official_releases(from, to, now).await {
        Ok(v) => v.into_iter().map(OfficialReleaseOut::from).collect(),
        Err(e) => return internal(e),
    };
    json(&CalendarReply {
        from: utc(from),
        to: utc(to),
        as_of: utc(now),
        generated_at: utc(now),
        economic_events,
        official_releases,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GATEWAY: &str = include_str!("../../../../config/gateway.toml");

    #[test]
    fn reads_the_repo_config() {
        let c = NewsConfig::from_toml(GATEWAY).unwrap();
        assert!(c.default_limit >= 1 && c.default_limit <= c.max_limit);
        assert!(c.calendar_past + c.calendar_future <= c.calendar_max);
    }

    #[test]
    fn rejects_bad_config() {
        for (from, to) in [
            ("default_limit = 50", "default_limit = 0"),
            ("default_limit = 50", "default_limit = 500"),
            ("max_limit = 200", "max_limit = 5000"),
            ("max_days = 400", "max_days = 0"),
            ("future_days = 14", "future_days = 0"),
            ("future_days = 14", "future_days = 400"),
            ("max_limit = 200", "max_limit = 200\nextra = 1"),
        ] {
            let bad = GATEWAY.replace(from, to);
            assert_ne!(bad, GATEWAY, "{from}");
            assert!(NewsConfig::from_toml(&bad).is_err(), "{to}");
        }
    }

    #[test]
    fn feed_names_follow_the_contract() {
        for ok in ["press", "press_monetary", "a1"] {
            assert!(valid_feed(ok), "{ok}");
        }
        for bad in [
            "",
            "Press",
            "1press",
            "press-all",
            "press%20x",
            &"a".repeat(33),
        ] {
            assert!(!valid_feed(bad), "{bad}");
        }
    }
}

//! Rutas de solo lectura del contexto de mercado para el Panel (#294). Lo calcula Python; el
//! gateway lo lee de PostgreSQL tal como se publicó y no recalcula nada.
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/sentiment` | `sentiment.v1[]`: el último de cada instrumento |
//! | `GET /api/v1/sentiment?instrument=<id>` | `sentiment.v1[]`: la historia (las últimas `[context] sentiment_history`, de la más vieja a la más nueva) |
//! | `GET /api/v1/analogs` | `analog_report.v1[]`: el último de cada instrumento y fuente |
//! | `GET /api/v1/analogs?instrument=<id>` | lo mismo, solo de ese instrumento |
//!
//! Los análogos son una distribución de resultados pasados, nunca un pronóstico.

use std::future::Future;

use cfds_core::{AnalogReport, InstrumentId, SentimentIndex};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::Reply;

/// `[context]` de `config/gateway.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextConfig {
    pub sentiment_history: u32,
}

impl ContextConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            context: RawContext,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawContext {
            sentiment_history: u32,
        }
        let r = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .context;
        if r.sentiment_history == 0 {
            return Err("context.sentiment_history tiene que ser > 0".into());
        }
        Ok(Self {
            sentiment_history: r.sentiment_history,
        })
    }
}

/// De dónde sale el contexto. En producción, PostgreSQL.
pub trait ContextSource: Sync {
    fn latest_sentiment(
        &self,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<SentimentIndex>, String>> + Send;
    fn sentiment_history(
        &self,
        id: &InstrumentId,
        at: DateTime<Utc>,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<SentimentIndex>, String>> + Send;
    fn latest_analogs(
        &self,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<AnalogReport>, String>> + Send;
}

impl ContextSource for PgStore {
    async fn latest_sentiment(&self, at: DateTime<Utc>) -> Result<Vec<SentimentIndex>, String> {
        PgStore::latest_sentiment(self, at)
            .await
            .map_err(|e| e.to_string())
    }
    async fn sentiment_history(
        &self,
        id: &InstrumentId,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<SentimentIndex>, String> {
        PgStore::sentiment_history(self, id, at, limit)
            .await
            .map_err(|e| e.to_string())
    }
    async fn latest_analogs(&self, at: DateTime<Utc>) -> Result<Vec<AnalogReport>, String> {
        self.latest_analog_reports(at)
            .await
            .map_err(|e| e.to_string())
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_context_route(path: &str) -> bool {
    matches!(path, "/api/v1/sentiment" | "/api/v1/analogs")
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

fn json<T: serde::Serialize>(v: &T) -> Reply {
    match serde_json::to_string(v) {
        Ok(body) => Reply {
            status: 200,
            content_type: "application/json",
            body,
        },
        Err(e) => {
            eprintln!("gateway: no se pudo serializar el contexto: {e}");
            text(500, "error interno")
        }
    }
}

fn internal(e: String) -> Reply {
    eprintln!("gateway: contexto: {e}");
    text(500, "error interno")
}

/// Atiende las rutas del contexto. Solo `GET`.
pub async fn respond_context<C: ContextSource>(
    source: &C,
    cfg: &ContextConfig,
    method: &str,
    path: &str,
    query: Option<&str>,
    now: DateTime<Utc>,
) -> Reply {
    if !is_context_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: el contexto es de solo lectura");
    }
    let instrument = match crate::param(query, "instrument") {
        None => None,
        Some(s) => match s.parse::<InstrumentId>() {
            Ok(id) => Some(id),
            Err(_) => return text(400, "instrument inválido"),
        },
    };
    match (path, instrument) {
        ("/api/v1/sentiment", None) => source
            .latest_sentiment(now)
            .await
            .map_or_else(internal, |v| json(&v)),
        ("/api/v1/sentiment", Some(id)) => source
            .sentiment_history(&id, now, cfg.sentiment_history)
            .await
            .map_or_else(internal, |v| json(&v)),
        (_, id) => match source.latest_analogs(now).await {
            Ok(all) => {
                let v: Vec<AnalogReport> = match id {
                    Some(id) => all.into_iter().filter(|r| r.instrument_id == id).collect(),
                    None => all,
                };
                json(&v)
            }
            Err(e) => internal(e),
        },
    }
}

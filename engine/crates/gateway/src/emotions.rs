//! Estados emocionales de la spec §9.1 para el Panel (#220), **solo lectura**:
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/emotions` | `emotional_state.v1[]`: el último de cada instrumento, fuente y base |
//! | `GET /api/v1/emotions?instrument=<id>` | lo mismo, solo de ese instrumento |
//!
//! Los calcula el motor (precio) y Python (índice) y los guarda en `emotional_states`; el
//! gateway los lee tal cual, conocidos hasta ahora (`as_of <= now`), y no recalcula nada. Son
//! estados con los valores de su regla, nunca un pronóstico.

use std::future::Future;

use cfds_core::{EmotionalState, InstrumentId};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};

use crate::Reply;

const PATH: &str = "/api/v1/emotions";

/// De dónde salen los estados. En producción, PostgreSQL.
pub trait EmotionStore: Sync {
    fn latest(
        &self,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<EmotionalState>, String>> + Send;
}

impl EmotionStore for PgStore {
    async fn latest(&self, at: DateTime<Utc>) -> Result<Vec<EmotionalState>, String> {
        self.latest_emotional_states(at)
            .await
            .map_err(|e| e.to_string())
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_emotions_route(path: &str) -> bool {
    path == PATH
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

/// Atiende `GET /api/v1/emotions`. Solo `GET`.
pub async fn respond_emotions<S: EmotionStore>(
    store: &S,
    method: &str,
    path: &str,
    query: Option<&str>,
    now: DateTime<Utc>,
) -> Reply {
    if !is_emotions_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: los estados emocionales son de solo lectura");
    }
    let instrument = match crate::param(query, "instrument") {
        None => None,
        Some(s) => match s.parse::<InstrumentId>() {
            Ok(id) => Some(id),
            Err(_) => return text(400, "instrument inválido"),
        },
    };
    let all = match store.latest(now).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("gateway: estados emocionales: {e}");
            return text(500, "error interno");
        }
    };
    let rows: Vec<EmotionalState> = match instrument {
        Some(id) => all.into_iter().filter(|s| s.instrument_id == id).collect(),
        None => all,
    };
    match serde_json::to_string(&rows) {
        Ok(body) => Reply {
            status: 200,
            content_type: "application/json",
            body,
        },
        Err(e) => {
            eprintln!("gateway: no se pudieron serializar los estados: {e}");
            text(500, "error interno")
        }
    }
}

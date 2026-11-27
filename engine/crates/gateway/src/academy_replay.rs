//! Rutas del replay de exámenes de la Academia (#723, docs/design/2026-11-academia.md §5). Las
//! sesiones viven en memoria (como mucho `[replay] max_sessions`; al llenarse se cierra la más
//! vieja) y todo lo calcula `cfds_academy::replay` y `cfds_academy::grade`.
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `POST /api/v1/academy/replay` | `replay_session.v1#/$defs/request` → `replay_session.v1` |
//! | `POST /api/v1/academy/replay/{id}/step` | `replay_step.v1#/$defs/request` → `replay_step.v1` |
//! | `POST /api/v1/academy/replay/{id}/marks` | `replay_mark.v1#/$defs/request` → `replay_mark.v1` |
//! | `POST /api/v1/academy/replay/{id}/orders` | `replay_order.v1#/$defs/request` → `replay_order.v1` |
//! | `POST /api/v1/academy/replay/{id}/finish` | → `exam_result.v1` (y la sesión se cierra) |
//!
//! Todas son `POST` con los chequeos de escritura del gateway ([`crate::paper::write_guard`]):
//! crean o cambian una sesión. Ninguna devuelve una vela posterior al cursor ni la fecha real
//! antes de terminar. La única que escribe en la base es `finish` de una sesión **de una lección**
//! (#759): guarda el intento (`academy_event.v1` con `replay`) **antes** de responder; si no se
//! puede guardar, no hay nota y la sesión sigue abierta para volver a terminarla. Una práctica no
//! escribe nada.

use std::collections::BTreeMap;
use std::sync::Mutex;

use cfds_academy::grade::{GradeConfig, ReplayCosts, grade};
use cfds_academy::replay::{
    BarRange, BarStore, MarkRequest, OrderRequest, ProcessRules, ReplayError, ReplaySession,
    SessionRequest, StepRequest, open_session,
};
use cfds_academy::{AcademyConfig, EventBody, ExamKind};
use cfds_backtest::research::program4::SplitMix64;
use cfds_backtest::{BacktestConfig, Commission, market_terms_from_catalog};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{Catalog, SplitConfig};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;

use crate::Reply;
use crate::academy::{AcademyRoutes, AcademyStore};
use crate::paper::{HttpIn, write_guard};

const PREFIX: &str = "/api/v1/academy/replay";

/// Las velas de los futuros guardadas (`futures_candles` de `source`).
pub struct PgBars<'a> {
    pub store: &'a PgStore,
    pub source: &'a str,
}

impl BarStore for PgBars<'_> {
    async fn range(&self, id: &InstrumentId, tf: Timeframe) -> Result<Option<BarRange>, String> {
        self.store
            .futures_candle_range(self.source, id, tf)
            .await
            .map_err(|e| e.to_string())
    }

    async fn bars(
        &self,
        id: &InstrumentId,
        tf: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, String> {
        self.store
            .futures_candles(self.source, id, tf, from, to)
            .await
            .map_err(|e| e.to_string())
    }
}

/// Lo que el replay lee de config, validado al arrancar.
pub struct ReplayDeps {
    pub split: SplitConfig,
    pub patterns: PatternsConfig,
    pub rules: ProcessRules,
    /// Costos del CFD de cada instrumento de `[replay] instruments`.
    pub costs: BTreeMap<InstrumentId, ReplayCosts>,
}

impl ReplayDeps {
    /// Arma la config del replay al arrancar. Cada instrumento de `[replay]` tiene que estar en el
    /// catálogo, con sus costos completos y sin comisión por ejecución.
    pub fn from_config(
        academy: &AcademyConfig,
        catalog: &Catalog,
        risk: &RiskConfig,
        backtest: &BacktestConfig,
        data_split: &str,
        patterns: &str,
    ) -> Result<Self, String> {
        let mut costs = BTreeMap::new();
        for id in &academy.replay.instruments {
            let terms =
                market_terms_from_catalog(catalog, id).map_err(|e| format!("replay: {id}: {e}"))?;
            if terms.costs.commission != Commission::None {
                return Err(format!(
                    "replay: {id} tiene comisión por ejecución (solo índices)"
                ));
            }
            let c = ReplayCosts {
                costs: terms.costs,
                execution: backtest.costs,
                funding_cutoff: terms.funding_cutoff,
                funding_tz: terms.funding_tz,
            };
            costs.insert(id.clone(), c);
        }
        Ok(Self {
            split: SplitConfig::from_toml(data_split).map_err(|e| e.to_string())?,
            patterns: PatternsConfig::from_toml(patterns)?,
            rules: ProcessRules {
                stop_min_atr: risk.min_stop_atr,
                stop_max_atr: risk.max_stop_atr,
                max_risk_pct: risk.risk_per_trade_pct,
                provisional: academy.catalog.thresholds.provisional,
            },
            costs,
        })
    }
}

/// Cuántas sesiones se crearon, y cada sesión abierta con su número de creación.
type Open = (u64, BTreeMap<String, (u64, ReplaySession)>);

/// Las sesiones abiertas, con su orden de creación.
#[derive(Default)]
pub struct ReplaySessions(Mutex<Open>);

impl ReplaySessions {
    fn insert(&self, s: ReplaySession, max: usize) {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let (counter, map) = &mut *guard;
        while map.len() >= max {
            let oldest = map
                .iter()
                .min_by_key(|(_, (n, _))| *n)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(k) => map.remove(&k),
                None => break,
            };
        }
        *counter += 1;
        map.insert(s.id.clone(), (*counter, s));
    }

    /// Corre `f` sobre la sesión `id`; `None` si no existe.
    fn with<T>(&self, id: &str, f: impl FnOnce(&mut ReplaySession) -> T) -> Option<T> {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.1.get_mut(id).map(|(_, s)| f(s))
    }

    /// Saca la sesión `id` (con su número de creación): nadie más la puede terminar a la vez.
    fn take(&self, id: &str) -> Option<(u64, ReplaySession)> {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.1.remove(id)
    }

    /// Devuelve una sesión que se sacó con [`Self::take`].
    fn restore(&self, n: u64, s: ReplaySession) {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        guard.1.insert(s.id.clone(), (n, s));
    }
}

pub fn is_replay_route(path: &str) -> bool {
    path == PREFIX || path.starts_with("/api/v1/academy/replay/")
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
            eprintln!("gateway: replay: {e}");
            text(500, "error interno")
        }
    }
}

fn internal(e: String) -> Reply {
    eprintln!("gateway: replay: {e}");
    text(500, "error interno")
}

fn error(e: ReplayError) -> Reply {
    match e {
        ReplayError::Store(msg) => {
            eprintln!("gateway: replay: {msg}");
            text(500, "error interno")
        }
        ReplayError::NoData(_) | ReplayError::NoWindow(_) => text(503, &e.to_string()),
        ReplayError::Finished | ReplayError::AfterCursor { .. } => text(409, &e.to_string()),
        ReplayError::Bad(_) => text(400, &e.to_string()),
    }
}

fn parse<T: DeserializeOwned>(body: &[u8], what: &str) -> Result<T, Reply> {
    serde_json::from_slice(body).map_err(|e| text(400, &format!("no es un {what} válido: {e}")))
}

/// Atiende las rutas del replay. `store` guarda el intento de una sesión de una lección.
#[allow(clippy::too_many_arguments)]
pub async fn respond_replay<B: BarStore, S: AcademyStore>(
    bars: &B,
    store: &S,
    sessions: &ReplaySessions,
    deps: &ReplayDeps,
    academy: &AcademyConfig,
    routes: &AcademyRoutes,
    token: Option<&str>,
    req: HttpIn<'_>,
    now: DateTime<Utc>,
) -> Reply {
    let rest = req.path.strip_prefix(PREFIX).unwrap_or_default();
    let action = rest.strip_prefix('/').and_then(|r| r.split_once('/'));
    if !rest.is_empty() && action.is_none() {
        return text(404, "ruta desconocida");
    }
    if req.method != "POST" {
        return text(405, "solo POST");
    }
    if let Some(refused) = write_guard(token, &req, routes.max_body_bytes) {
        return refused;
    }
    let cfg = &academy.replay;
    let Some((id, action)) = action else {
        return open(bars, sessions, deps, academy, req.body, now).await;
    };
    let found = match action {
        "step" => match parse::<StepRequest>(req.body, "pedido de paso") {
            Ok(r) => sessions.with(id, |s| s.step(r.bars, cfg.max_step_bars).map(|v| json(&v))),
            Err(reply) => return reply,
        },
        "marks" => match parse::<MarkRequest>(req.body, "replay_mark") {
            Ok(r) => sessions.with(id, |s| s.mark(r).map(|v| json(&v))),
            Err(reply) => return reply,
        },
        "orders" => match parse::<OrderRequest>(req.body, "replay_order") {
            Ok(r) => sessions.with(id, |s| s.order(r).map(|v| json(&v))),
            Err(reply) => return reply,
        },
        "finish" => {
            let member = routes.member_id.as_str();
            return finish(store, member, sessions, deps, academy, id, now).await;
        }
        _ => return text(404, "ruta desconocida"),
    };
    match found {
        Some(Ok(reply)) => reply,
        Some(Err(e)) => error(e),
        None => text(404, "no hay una sesión abierta con ese id"),
    }
}

/// Termina y califica la sesión `id`. Si es el examen de una lección, guarda el intento **antes**
/// de responder (#759): si no se puede, no hay nota y la sesión vuelve tal como estaba (sin
/// terminar), para pedir `finish` otra vez.
async fn finish<S: AcademyStore>(
    store: &S,
    member: &str,
    sessions: &ReplaySessions,
    deps: &ReplayDeps,
    academy: &AcademyConfig,
    id: &str,
    now: DateTime<Utc>,
) -> Reply {
    // Afuera del mapa mientras se termina: dos `finish` a la vez no guardan dos intentos.
    let Some((n, open)) = sessions.take(id) else {
        return text(404, "no hay una sesión abierta con ese id");
    };
    let mut done = open.clone();
    let graded = done.finish().and_then(|()| {
        let costs = deps.costs.get(&done.draw.instrument_id).ok_or_else(|| {
            ReplayError::Bad(format!(
                "{} sin costos en el catálogo",
                done.draw.instrument_id
            ))
        })?;
        let g = GradeConfig {
            replay: &academy.replay,
            rules: deps.rules,
            patterns: &deps.patterns,
            pass_pct: academy.catalog.thresholds.quiz_pass_pct,
            catalog_version: &academy.catalog.catalog_version,
        };
        grade(&done, g, costs, now)
    });
    let result = match graded {
        Ok(r) => r,
        Err(e) => return error(e),
    };
    if let Some(lesson_id) = result.lesson_id.clone() {
        let attempt = EventBody::ReplayAttempt {
            lesson_id,
            replay: Box::new(result.clone()),
        };
        let version = &academy.catalog.catalog_version;
        if let Err(e) = store.append(member, attempt, version, now).await {
            sessions.restore(n, open);
            return internal(e);
        }
    }
    json(&result)
}

async fn open<B: BarStore>(
    bars: &B,
    sessions: &ReplaySessions,
    deps: &ReplayDeps,
    academy: &AcademyConfig,
    body: &[u8],
    now: DateTime<Utc>,
) -> Reply {
    let r: SessionRequest = match parse(body, "pedido de sesión") {
        Ok(r) => r,
        Err(reply) => return reply,
    };
    // Un examen de una lección solo si esa lección tiene examen de replay, y de su tipo (#759).
    let replay_exam = |l: &String| {
        academy
            .lesson(l)
            .is_some_and(|l| l.exam.kind == ExamKind::Replay && l.exam.replay_kind == Some(r.kind))
    };
    if r.lesson_id.as_ref().is_some_and(|l| !replay_exam(l)) {
        return text(400, "esa lección no tiene examen de replay de ese tipo");
    }
    let nanos = u64::try_from(now.timestamp_nanos_opt().unwrap_or_default()).unwrap_or_default();
    let seed = match r.seed() {
        Ok(s) => s.unwrap_or_else(|| SplitMix64::new(nanos).next_u64()),
        Err(e) => return error(e),
    };
    let cfg = &academy.replay;
    match open_session(bars, cfg, &deps.split, r.kind, r.lesson_id, seed, nanos).await {
        Ok(s) => {
            let view = json(&s.view(cfg, deps.rules));
            sessions.insert(s, cfg.max_sessions);
            view
        }
        Err(e) => error(e),
    }
}

//! Laboratorio de estrategias (#776, `docs/design/2026-11-laboratorio.md` §1.1 y §5). El gateway
//! **nunca corre un backtest**: arma el catálogo del motor, valida specs, encola trabajos, los
//! cancela, lee su estado y sus informes y retransmite el progreso. Los corre el `lab_worker`.
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/lab/catalog` | `strategy_catalog.v1` |
//! | `POST /api/v1/lab/validate` | `strategy_spec.v1` → `spec_validation.v1` |
//! | `POST /api/v1/lab/jobs` | `lab_job_request.v1` → `lab_job_event.v1` (202 nuevo; 200 el mismo pedido) |
//! | `GET /api/v1/lab/jobs/{job_id}` | `lab_job_event.v1` (el último) |
//! | `DELETE /api/v1/lab/jobs/{job_id}` | `lab_job_event.v1` (cancelado, o el final si ya terminó) |
//! | `GET /api/v1/lab/jobs/{job_id}/events` | SSE de `lab_job_event.v1` (`id` = `seq`, `Last-Event-ID`) |
//! | `GET /api/v1/lab/reports/{report_id}` | `lab_report.v1` |
//! | `POST /api/v1/lab/risk_preview` | `risk_preview.v1#/$defs/request` → `risk_preview.v1` (sin efectos) |
//!
//! Escritura (encolar, cancelar): el mismo token anti-CSRF que paper y la Academia, solo en las
//! tablas del laboratorio (de solo agregar). La separación de datos sale de la hora de la última
//! vela de futuros guardada, como en el worker: un período que llega al fuera de muestra se
//! rechaza.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use cfds_backtest::lab::{
    LabConfig, LabJobEvent, LabJobRequest, StrategyCatalog, StrategySpec, ValidSpec, catalog,
    check_variants, request_sha256, validate_spec, validate_spec_json,
};
use cfds_backtest::{CostConfig, FxRates};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_journal::lab::{Appended, Enqueued, PgLab};
use cfds_market_data::{CandleSources, Catalog, DataSplit, Period, SplitConfig, plan};
use cfds_risk::RiskConfig;
use cfds_storage::{CandleFeed, LabStream, PgStore};
use chrono::{DateTime, Utc};
use hyper::body::Bytes;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::Reply;
use crate::ibkr_live::SseBody;
use crate::lab_risk::{PreviewInputs, PreviewRequest, closed_at, preview};
use crate::paper::{HttpIn, token_guard, write_guard};

const PREFIX: &str = "/api/v1/lab/";

/// `[lab]` de `config/gateway.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabRoutes {
    pub max_body_bytes: usize,
    pub sse_max_clients: usize,
    pub sse_heartbeat: Duration,
    /// Sin Redis, cada cuánto se miran los eventos en PostgreSQL.
    pub sse_poll: Duration,
    /// Días de velas que lee la vista previa de riesgo (para el ATR).
    pub risk_preview_days: u32,
}

impl LabRoutes {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            lab: RawLab,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawLab {
            max_body_bytes: usize,
            sse_max_clients: usize,
            sse_heartbeat_secs: u64,
            sse_poll_secs: u64,
            risk_preview_days: u32,
        }
        let r = toml::from_str::<Raw>(text).map_err(|e| e.to_string())?.lab;
        if r.max_body_bytes == 0
            || r.sse_max_clients == 0
            || r.sse_heartbeat_secs == 0
            || r.sse_poll_secs == 0
            || r.risk_preview_days == 0
        {
            return Err("lab: todos los valores tienen que ser > 0".into());
        }
        Ok(Self {
            max_body_bytes: r.max_body_bytes,
            sse_max_clients: r.sse_max_clients,
            sse_heartbeat: Duration::from_secs(r.sse_heartbeat_secs),
            sse_poll: Duration::from_secs(r.sse_poll_secs),
            risk_preview_days: r.risk_preview_days,
        })
    }
}

/// Lo que el gateway necesita del laboratorio, validado al arrancar.
#[derive(Debug, Clone)]
pub struct LabDeps {
    pub catalog: StrategyCatalog,
    pub split_cfg: SplitConfig,
    /// Desde cuándo carga velas el worker (`[worker] load_from`): define la separación.
    pub load_from: DateTime<Utc>,
    pub routes: LabRoutes,
    /// Redis de los streams de progreso (`REDIS_URL`); `None`: el SSE lee PostgreSQL.
    pub redis_url: Option<String>,
    /// Vencimiento y tope de los streams (`[worker]` de `config/lab.toml`, como el worker).
    pub stream_ttl: Duration,
    pub stream_maxlen: usize,
    /// Vista previa de riesgo (#777): los instrumentos y la cuenta del catálogo, el riesgo de
    /// `config/risk.toml` y el slippage del backtest.
    pub instruments: Catalog,
    pub risk: RiskConfig,
    pub costs: CostConfig,
}

impl LabDeps {
    #[allow(
        clippy::too_many_arguments,
        reason = "cada config del laboratorio, validada al arrancar"
    )]
    pub fn from_config(
        gateway: &str,
        lab: &str,
        risk: &RiskConfig,
        sources: &CandleSources,
        data_split: &str,
        redis_url: Option<String>,
        instruments: &Catalog,
        costs: CostConfig,
    ) -> Result<Self, String> {
        let lab = LabConfig::from_toml(lab)?;
        Ok(Self {
            catalog: catalog(&lab, risk, sources.roll.keys().cloned())?,
            split_cfg: SplitConfig::from_toml(data_split).map_err(|e| e.to_string())?,
            load_from: lab.worker.load_from()?,
            routes: LabRoutes::from_toml(gateway)?,
            redis_url,
            stream_ttl: lab.worker.stream_ttl(),
            stream_maxlen: lab.worker.stream_maxlen,
            instruments: instruments.clone(),
            risk: risk.clone(),
            costs,
        })
    }
}

/// Dónde están la cola y las velas. En producción, PostgreSQL ([`PgLabStore`]).
pub trait LabStore: Sync {
    /// Inicio de la última vela de 15m de futuros guardada de `id`.
    fn last_candle(
        &self,
        id: &InstrumentId,
    ) -> impl Future<Output = Result<Option<DateTime<Utc>>, String>> + Send;
    fn enqueue(
        &self,
        request: &LabJobRequest,
        request_sha256: &str,
        family_sha256: &str,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<Enqueued, String>> + Send;
    fn cancel(
        &self,
        job_id: &str,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<Option<Appended>, String>> + Send;
    fn last_event(
        &self,
        job_id: &str,
    ) -> impl Future<Output = Result<Option<LabJobEvent>, String>> + Send;
    fn events_after(
        &self,
        job_id: &str,
        after: i64,
    ) -> impl Future<Output = Result<Vec<LabJobEvent>, String>> + Send;
    fn report(&self, report_id: &str)
    -> impl Future<Output = Result<Option<Value>, String>> + Send;
    /// Velas de futuros de `id` en `[from, to)` (para la vista previa de riesgo).
    fn candles(
        &self,
        id: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Candle>, String>> + Send;
    /// El tipo de cambio de `pair` (por ejemplo `EURUSD`) vigente en `at`, si hay uno publicado.
    fn fx_rate(
        &self,
        pair: &str,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<Option<Decimal>, String>> + Send;
}

/// La cola en PostgreSQL (`cfds_journal::lab`) y las velas de futuros guardadas.
#[derive(Debug, Clone)]
pub struct PgLabStore {
    pub lab: PgLab,
    pub store: PgStore,
    pub futures_source: String,
}

impl LabStore for PgLabStore {
    async fn last_candle(&self, id: &InstrumentId) -> Result<Option<DateTime<Utc>>, String> {
        let feed = CandleFeed::Futures(&self.futures_source);
        self.store
            .last_candle_start_from(feed, id, Timeframe::M15, Utc::now())
            .await
            .map_err(|e| e.to_string())
    }

    async fn enqueue(
        &self,
        request: &LabJobRequest,
        request_sha256: &str,
        family_sha256: &str,
        now: DateTime<Utc>,
    ) -> Result<Enqueued, String> {
        self.lab
            .enqueue(request, request_sha256, family_sha256, now)
            .await
            .map_err(|e| e.to_string())
    }

    async fn cancel(&self, job_id: &str, now: DateTime<Utc>) -> Result<Option<Appended>, String> {
        self.lab
            .cancel(job_id, now)
            .await
            .map_err(|e| e.to_string())
    }

    async fn last_event(&self, job_id: &str) -> Result<Option<LabJobEvent>, String> {
        self.lab.last_event(job_id).await.map_err(|e| e.to_string())
    }

    async fn events_after(&self, job_id: &str, after: i64) -> Result<Vec<LabJobEvent>, String> {
        self.lab
            .events_after(job_id, after)
            .await
            .map_err(|e| e.to_string())
    }

    async fn report(&self, report_id: &str) -> Result<Option<Value>, String> {
        self.lab.report(report_id).await.map_err(|e| e.to_string())
    }

    async fn candles(
        &self,
        id: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, String> {
        let feed = CandleFeed::Futures(&self.futures_source);
        self.store
            .candles_from(feed, id, id, timeframe, from, to)
            .await
            .map_err(|e| e.to_string())
    }

    async fn fx_rate(&self, pair: &str, at: DateTime<Utc>) -> Result<Option<Decimal>, String> {
        let rates = self
            .store
            .fx_rates(pair, at)
            .await
            .map_err(|e| e.to_string())?;
        Ok(FxRates::new(rates).and_then(|r| r.at(at)))
    }
}

/// `true` si la ruta es de este módulo.
pub fn is_lab_route(path: &str) -> bool {
    path.starts_with(PREFIX)
}

/// El `job_id` de `/api/v1/lab/jobs/{job_id}/events`, si la ruta es esa (el SSE).
pub fn lab_events_job(path: &str) -> Option<&str> {
    path.strip_prefix(PREFIX)?
        .strip_prefix("jobs/")?
        .strip_suffix("/events")
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

fn json_with<T: serde::Serialize>(status: u16, v: &T) -> Reply {
    match serde_json::to_string(v) {
        Ok(body) => Reply {
            status,
            content_type: "application/json",
            body,
        },
        Err(e) => internal(e.to_string()),
    }
}

fn json<T: serde::Serialize>(v: &T) -> Reply {
    json_with(200, v)
}

fn internal(e: String) -> Reply {
    eprintln!("gateway: laboratorio: {e}");
    text(500, "error interno")
}

/// La separación vigente para las specs de `id`: con la última vela de futuros guardada. Un
/// instrumento fuera del catálogo no tiene velas que mirar: la validación lo rechaza igual
/// (`unknown_market`), así que el período no se juzga.
async fn split_for<S: LabStore>(
    store: &S,
    deps: &LabDeps,
    id: &InstrumentId,
) -> Result<DataSplit, Reply> {
    if !deps.catalog.markets.iter().any(|m| &m.instrument_id == id) {
        let never = DateTime::<Utc>::MAX_UTC;
        return Ok(DataSplit {
            windows: Vec::new(),
            out_of_sample: Period {
                start: never,
                end: never,
            },
        });
    }
    let last = match store.last_candle(id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return Err(text(
                409,
                &format!("sin velas de futuros guardadas de {id}"),
            ));
        }
        Err(e) => return Err(internal(e)),
    };
    plan(deps.load_from, last, &deps.split_cfg)
        .map_err(|e| text(409, &format!("separación de datos de {id}: {e}")))
}

/// Atiende las rutas del laboratorio salvo el SSE. `token` es el token local esperado (`None`:
/// escritura apagada).
pub async fn respond_lab<S: LabStore>(
    store: &S,
    deps: &LabDeps,
    token: Option<&str>,
    req: HttpIn<'_>,
    now: DateTime<Utc>,
) -> Reply {
    let rest = req.path.strip_prefix(PREFIX).unwrap_or_default();
    match (req.method, rest) {
        ("GET", "catalog") => return json(&deps.catalog),
        ("POST", "validate") => return validate(store, deps, token, &req).await,
        ("POST", "jobs") => return submit(store, deps, token, &req, now).await,
        ("POST", "risk_preview") => return risk_preview(store, deps, token, &req, now).await,
        (_, "catalog" | "validate" | "jobs" | "risk_preview") => {
            return text(405, "método no permitido");
        }
        _ => {}
    }
    if let Some(id) = rest.strip_prefix("reports/") {
        if req.method != "GET" {
            return text(405, "solo GET");
        }
        if !is_sha256(id) {
            return text(400, "report_id: SHA-256 en hexadecimal");
        }
        return match store.report(id).await {
            Ok(Some(r)) => json(&r),
            Ok(None) => text(404, "no existe ese informe"),
            Err(e) => internal(e),
        };
    }
    let Some(id) = rest.strip_prefix("jobs/") else {
        return text(404, "ruta desconocida");
    };
    if !is_sha256(id) {
        return text(404, "ruta desconocida");
    }
    match req.method {
        "GET" => match store.last_event(id).await {
            Ok(Some(e)) => json(&e),
            Ok(None) => text(404, "no existe ese trabajo"),
            Err(e) => internal(e),
        },
        "DELETE" => {
            if let Some(refused) = token_guard(token, &req) {
                return refused;
            }
            match store.cancel(id, now).await {
                Ok(Some(Appended::Added(e))) => {
                    // Los SSE abiertos lo ven al instante (si no, en la próxima mirada).
                    publish_event(deps, &e).await;
                    json(&e)
                }
                Ok(Some(Appended::AlreadyEnded(e))) => json(&e),
                Ok(None) => text(404, "no existe ese trabajo"),
                Err(e) => internal(e),
            }
        }
        _ => text(405, "solo GET o DELETE"),
    }
}

async fn validate<S: LabStore>(
    store: &S,
    deps: &LabDeps,
    token: Option<&str>,
    req: &HttpIn<'_>,
) -> Reply {
    if let Some(refused) = write_guard(token, req, deps.routes.max_body_bytes) {
        return refused;
    }
    let Ok(text_body) = std::str::from_utf8(req.body) else {
        return text(400, "el body no es UTF-8");
    };
    // Una spec mal formada da su `spec_validation.v1` con el error `malformed`.
    let id = serde_json::from_str::<StrategySpec>(text_body)
        .ok()
        .map(|s| s.market.instrument_id);
    let split = match &id {
        Some(id) => match split_for(store, deps, id).await {
            Ok(s) => s,
            Err(reply) => return reply,
        },
        None => DataSplit {
            windows: Vec::new(),
            out_of_sample: Period {
                start: DateTime::<Utc>::MAX_UTC,
                end: DateTime::<Utc>::MAX_UTC,
            },
        },
    };
    let (validation, _) = validate_spec_json(text_body, &deps.catalog, &split);
    json(&validation)
}

async fn submit<S: LabStore>(
    store: &S,
    deps: &LabDeps,
    token: Option<&str>,
    req: &HttpIn<'_>,
    now: DateTime<Utc>,
) -> Reply {
    if let Some(refused) = write_guard(token, req, deps.routes.max_body_bytes) {
        return refused;
    }
    let request: LabJobRequest = match serde_json::from_slice(req.body) {
        Ok(r) => r,
        Err(e) => return text(400, &format!("no es un lab_job_request.v1 válido: {e}")),
    };
    let Some(first) = request.specs.first() else {
        return text(422, "el pedido no tiene specs");
    };
    if request.specs.len() > deps.catalog.limits.max_variants_per_run {
        let max = deps.catalog.limits.max_variants_per_run;
        return text(422, &format!("como mucho {max} variantes por corrida"));
    }
    let split = match split_for(store, deps, &first.market.instrument_id).await {
        Ok(s) => s,
        Err(reply) => return reply,
    };
    let mut specs: Vec<ValidSpec> = Vec::new();
    let mut problems = Vec::new();
    for (k, s) in request.specs.iter().enumerate() {
        match validate_spec(s, &deps.catalog, &split) {
            Ok(v) => specs.push(v),
            Err(errors) => {
                let msgs: Vec<String> = errors
                    .iter()
                    .map(|e| format!("{} ({})", e.message, e.path))
                    .collect();
                problems.push(format!("variante {}: {}", k + 1, msgs.join("; ")));
            }
        }
    }
    if !problems.is_empty() {
        return text(422, &problems.join("\n"));
    }
    if let Err(e) = check_variants(&specs, &deps.catalog) {
        return text(422, &e);
    }
    let normalized = LabJobRequest::normalized(&specs, request.initial_equity);
    let hash = request_sha256(&specs, request.initial_equity);
    match store
        .enqueue(&normalized, &hash, &specs[0].family_sha256, now)
        .await
    {
        Ok(Enqueued::New(e)) => json_with(202, &e),
        Ok(Enqueued::Existing(e)) => json(&e),
        Err(e) => internal(e),
    }
}

/// La vista previa de riesgo (#777): las velas cerradas recientes de futuros, el instrumento del
/// catálogo y el tipo de cambio vigente → `lab_risk::preview` (el mismo `cfds_risk::check`).
async fn risk_preview<S: LabStore>(
    store: &S,
    deps: &LabDeps,
    token: Option<&str>,
    req: &HttpIn<'_>,
    now: DateTime<Utc>,
) -> Reply {
    if let Some(refused) = write_guard(token, req, deps.routes.max_body_bytes) {
        return refused;
    }
    let request: PreviewRequest = match serde_json::from_slice(req.body) {
        Ok(r) => r,
        Err(e) => return text(400, &format!("no es un pedido de vista previa válido: {e}")),
    };
    let timeframe = match request.timeframe.as_str() {
        "15m" => Timeframe::M15,
        "1h" => Timeframe::H1,
        _ => return text(400, "timeframe: 15m o 1h"),
    };
    if !(1..=200).contains(&request.atr_period) {
        return text(400, "atr_period: de 1 a 200");
    }
    let id = &request.instrument_id;
    let in_lab = deps.catalog.markets.iter().any(|m| &m.instrument_id == id);
    let Some(spec) = deps.instruments.instruments.get(id).filter(|_| in_lab) else {
        return text(
            404,
            "ese instrumento no está en el catálogo del laboratorio",
        );
    };
    let Some(account) = deps.instruments.account_currency.value() else {
        return internal("el catálogo no tiene la moneda de la cuenta".into());
    };
    let from = now - chrono::Duration::days(i64::from(deps.routes.risk_preview_days));
    let candles = match store.candles(id, timeframe, from, now).await {
        Ok(c) => closed_at(c, now),
        Err(e) => return internal(e),
    };
    let fx = if spec.instrument.currency == *account {
        Some(Decimal::ONE)
    } else {
        let pair = format!("{account}{}", spec.instrument.currency);
        match store.fx_rate(&pair, now).await {
            Ok(r) => r,
            Err(e) => return internal(e),
        }
    };
    json(&preview(
        &request,
        &PreviewInputs {
            candles: &candles,
            spec,
            risk: &deps.risk,
            costs: &deps.costs,
            account_currency: account.as_str(),
            fx,
        },
    ))
}

/// Conexiones SSE abiertas del laboratorio.
#[derive(Debug, Default)]
pub struct LabSseClients(AtomicUsize);

struct ClientGuard(Arc<LabSseClients>);

impl Drop for ClientGuard {
    fn drop(&mut self) {
        (self.0).0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Respuesta del SSE.
pub enum LabEventsReply {
    Error { status: u16, body: String },
    Stream(SseBody),
}

fn sse_event(e: &LabJobEvent) -> Bytes {
    let data = serde_json::to_string(e).unwrap_or_else(|_| "{}".into());
    Bytes::from(format!("id: {}\ndata: {data}\n\n", e.seq))
}

/// Atiende `GET /api/v1/lab/jobs/{job_id}/events`: primero los eventos guardados después de
/// `Last-Event-ID` (o todos) y después los nuevos, hasta el final del trabajo. Con `redis_url`
/// espera los nuevos en el Redis Stream del trabajo; sin Redis (o si falla) mira PostgreSQL cada
/// `sse_poll`. Lo que se manda sale siempre en orden de `seq` y sin repetir.
pub async fn respond_lab_events<S: LabStore + Clone + Send + 'static>(
    store: &S,
    deps: &LabDeps,
    clients: &Arc<LabSseClients>,
    method: &str,
    path: &str,
    last_event_id: Option<&str>,
) -> LabEventsReply {
    let err = |status: u16, body: &str| LabEventsReply::Error {
        status,
        body: body.to_owned(),
    };
    if method != "GET" {
        return err(405, "solo GET");
    }
    let Some(job) = lab_events_job(path).filter(|j| is_sha256(j)) else {
        return err(404, "ruta desconocida");
    };
    let last = match store.last_event(job).await {
        Ok(Some(e)) => e,
        Ok(None) => return err(404, "no existe ese trabajo"),
        Err(e) => {
            eprintln!("gateway: laboratorio: {e}");
            return err(500, "error interno");
        }
    };
    let after = last_event_id
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0);
    // Ya mandó el final: 204 le dice al navegador que no vuelva a conectar.
    if last.state.is_end() && last.seq <= after {
        return err(204, "");
    }
    if clients.0.fetch_add(1, Ordering::SeqCst) >= deps.routes.sse_max_clients {
        clients.0.fetch_sub(1, Ordering::SeqCst);
        return err(503, "demasiadas conexiones de progreso del laboratorio");
    }
    let (tx, rx) = mpsc::channel(16);
    let guard = ClientGuard(clients.clone());
    let s = SseLoop {
        store: store.clone(),
        job: job.to_owned(),
        last: after,
        routes: deps.routes,
        redis_url: deps.redis_url.clone(),
    };
    tokio::spawn(s.serve(guard, tx));
    LabEventsReply::Stream(SseBody::new(rx))
}

struct SseLoop<S> {
    store: S,
    job: String,
    last: i64,
    routes: LabRoutes,
    redis_url: Option<String>,
}

impl<S: LabStore> SseLoop<S> {
    /// Manda los eventos guardados después de `last`. `Ok(true)` si llegó el final.
    async fn catch_up(&mut self, tx: &mpsc::Sender<Bytes>) -> Result<bool, ()> {
        let events = self.store.events_after(&self.job, self.last).await;
        let events = events.map_err(|e| eprintln!("gateway: laboratorio: {e}"))?;
        for e in events {
            tx.send(sse_event(&e)).await.map_err(|_| ())?;
            self.last = e.seq;
            if e.state.is_end() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn serve(mut self, _guard: ClientGuard, tx: mpsc::Sender<Bytes>) {
        // Si se corta, el navegador vuelve a conectar a los 3 s con Last-Event-ID.
        if tx
            .send(Bytes::from_static(b"retry: 3000\n\n"))
            .await
            .is_err()
        {
            return;
        }
        let mut redis = match &self.redis_url {
            Some(url) => LabStream::connect(url).await.ok(),
            None => None,
        };
        let mut since_ping = Duration::ZERO;
        loop {
            match self.catch_up(&tx).await {
                Ok(true) | Err(()) => return,
                Ok(false) => {}
            }
            // Esperar algo nuevo: el stream de Redis avisa al instante; si no, PostgreSQL.
            let waited = match redis.as_mut() {
                Some(r) => match r
                    .read_after(&self.job, self.last, self.routes.sse_heartbeat)
                    .await
                {
                    Ok(new) if !new.is_empty() => continue,
                    Ok(_) => self.routes.sse_heartbeat,
                    Err(e) => {
                        eprintln!("gateway: laboratorio: Redis: {e} (sigue con PostgreSQL)");
                        redis = None;
                        Duration::ZERO
                    }
                },
                None => {
                    tokio::time::sleep(self.routes.sse_poll).await;
                    self.routes.sse_poll
                }
            };
            since_ping += waited;
            if since_ping >= self.routes.sse_heartbeat {
                since_ping = Duration::ZERO;
                if tx.send(Bytes::from_static(b": ping\n\n")).await.is_err() {
                    return;
                }
            }
        }
    }
}

/// Publica en el Redis Stream del trabajo un evento que agregó el gateway (cancelar), para que
/// los SSE abiertos lo vean al instante. Sin Redis no pasa nada: el SSE lo lee de PostgreSQL.
async fn publish_event(deps: &LabDeps, e: &LabJobEvent) {
    let Some(url) = deps.redis_url.as_deref() else {
        return;
    };
    let Ok(data) = serde_json::to_string(e) else {
        return;
    };
    let (ttl, maxlen) = (deps.stream_ttl, deps.stream_maxlen);
    let sent = async {
        let mut r = LabStream::connect(url).await?;
        r.publish(&e.job_id, e.seq, &data, ttl, maxlen).await
    };
    if let Err(err) = sent.await {
        eprintln!("gateway: laboratorio: Redis: {err}");
    }
}

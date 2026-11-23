//! Rutas del broker simulado y de las corridas guardadas (fase 7 y 10, #278 y #275).
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `POST /api/v1/paper/preview` | `order.v1` → `ticket_verdict.v1` (sin efectos) |
//! | `POST /api/v1/paper/orders` | `order.v1` → `order_report.v1` (idempotente, se guarda) |
//! | `GET /api/v1/paper/positions` | `position.v1[]` |
//! | `GET /api/v1/paper/account` | `paper_account.v1` |
//! | `GET /api/v1/backtest_runs` | `backtest_run.v1[]` (las más nuevas primero) |
//! | `GET /api/v1/backtest_runs/{run_id}` | `backtest_run_detail.v1` (corrida, curva y operaciones, #299) |
//!
//! Seguridad (además de escuchar solo en 127.0.0.1):
//! - **Solo paper:** `order.v1` solo admite `mode = paper`, el broker real no existe y la tabla
//!   del registro rechaza cualquier otra cosa.
//! - **Anti-CSRF:** los `POST` exigen la cabecera `x-cfds-token` igual al token local (variable
//!   de entorno de `[paper] token_env`) y `content-type: application/json`. Sin token
//!   configurado, la escritura queda **apagada**. Otra web abierta en el navegador no puede mandar
//!   esa cabecera sin un preflight de CORS, que el gateway no contesta, ni conoce el token.
//! - **Tamaño:** body de hasta `[paper] max_body_bytes`.
//! - **Hora de la orden:** `created_at` tiene que estar a menos de `[paper] max_order_age_secs`
//!   de la hora del gateway (nunca en el futuro): nadie elige a qué precio viejo entra.
//! - **Arranque y parada (fase 9, #297):** `POST /api/v1/paper/orders` responde 503 mientras la
//!   reconciliación al arrancar no pasó limpia o el gateway se está apagando (ver [`crate::ops`]).

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use cfds_backtest::{DailyRates, FundingRates, market_terms_from_catalog};
use cfds_broker::{
    Broker, BrokerConfig, Flush, FundingSetup, MarketSetup, PaperBroker, ReconcileSchedule,
    flush_journal, reconcile_periodically, restore_and_reconcile, submit_and_record,
};
use cfds_core::{
    InstrumentId, Order, OrderReport, PaperAccount, Position, TicketVerdict, Timeframe,
};
use cfds_journal::pg::PgJournal;
use cfds_journal::{BacktestRun, RunDetail};
use cfds_market_data::Catalog;
use cfds_obs::{AlertKind, Alerter, Level, Logger, ReqwestPost};
use cfds_storage::PgStore;
use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::Reply;
use crate::ops::{Gate, GateState, alert_events, gate_after_reconcile, log_events, report_failure};
use crate::status::{ReconcileKind, ReconcileStatus};

/// `[paper]` de `config/gateway.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperConfig {
    /// Variable de entorno con el token local de escritura.
    pub token_env: String,
    pub max_body_bytes: usize,
    pub max_order_age: TimeDelta,
    /// Velas de 15m que el broker carga hacia atrás al arrancar (para el ATR y las salidas).
    pub candles_days: u32,
    pub runs_limit: u32,
}

impl PaperConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            paper: RawPaper,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawPaper {
            token_env: String,
            max_body_bytes: usize,
            max_order_age_secs: u32,
            candles_days: u32,
            runs_limit: u32,
        }
        let r = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .paper;
        let ok_env = !r.token_env.is_empty()
            && r.token_env
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
        if !ok_env {
            return Err("paper.token_env: nombre de variable inválido".into());
        }
        if r.max_body_bytes == 0
            || r.max_order_age_secs == 0
            || r.candles_days == 0
            || r.runs_limit == 0
        {
            return Err(
                "paper: max_body_bytes, max_order_age_secs, candles_days y runs_limit > 0".into(),
            );
        }
        Ok(Self {
            token_env: r.token_env,
            max_body_bytes: r.max_body_bytes,
            max_order_age: TimeDelta::seconds(i64::from(r.max_order_age_secs)),
            candles_days: r.candles_days,
            runs_limit: r.runs_limit,
        })
    }
}

/// Lo que las rutas necesitan del broker y del diario. En producción, [`PgPaper`].
pub trait PaperService: Sync {
    fn preview(
        &self,
        order: Order,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<TicketVerdict, String>> + Send;
    fn submit(&self, order: Order) -> impl Future<Output = Result<OrderReport, String>> + Send;
    fn positions(
        &self,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Position>, String>> + Send;
    fn account(
        &self,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<PaperAccount, String>> + Send;
    fn runs(&self, limit: u32) -> impl Future<Output = Result<Vec<BacktestRun>, String>> + Send;
    /// El detalle de una corrida guardada, o `None` si no existe (#299). Solo lectura.
    fn run_detail(
        &self,
        run_id: &str,
    ) -> impl Future<Output = Result<Option<RunDetail>, String>> + Send;
    /// La etapa del broker (fase 9): solo en `Ready` se aceptan órdenes.
    fn gate(&self) -> GateState {
        GateState::Ready
    }
}

/// Un pedido HTTP ya leído (el body, con el límite de tamaño aplicado).
#[derive(Debug, Clone, Copy)]
pub struct HttpIn<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub token: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub body: &'a [u8],
}

/// `true` si la ruta es de este módulo.
pub fn is_paper_route(path: &str) -> bool {
    path.starts_with("/api/v1/paper/")
        || path == "/api/v1/backtest_runs"
        || path.starts_with(RUN_DETAIL)
}

const RUN_DETAIL: &str = "/api/v1/backtest_runs/";

/// El `run_id` de `/api/v1/backtest_runs/{run_id}`, con el mismo formato que `backtest_run.v1`
/// (`^[A-Za-z0-9._-]{1,100}$`). `None` si la ruta no es esa; `Some(Err)` si el id no vale.
fn run_detail_id(path: &str) -> Option<Result<&str, ()>> {
    let id = path.strip_prefix(RUN_DETAIL)?;
    let ok = (1..=100).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && id != "."
        && id != "..";
    Some(if ok { Ok(id) } else { Err(()) })
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
            eprintln!("gateway: no se pudo serializar: {e}");
            text(500, "error interno")
        }
    }
}

fn internal(e: String) -> Reply {
    eprintln!("gateway: {e}");
    text(500, "error interno")
}

/// Comparación en tiempo constante (el token no se filtra por el tiempo de respuesta).
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Atiende las rutas del broker simulado. `paper` es `None` si el broker no arrancó; `token` es
/// el token local esperado (`None`: escritura apagada).
pub async fn respond_paper<P: PaperService>(
    paper: Option<&P>,
    cfg: &PaperConfig,
    token: Option<&str>,
    req: HttpIn<'_>,
    now: DateTime<Utc>,
) -> Reply {
    let route = (req.method, req.path);
    if let Some(id) = run_detail_id(req.path) {
        if req.method != "GET" {
            return text(405, "solo GET");
        }
        let Ok(id) = id else {
            return text(400, "run_id inválido");
        };
        let Some(paper) = paper else {
            return text(503, "el broker simulado no está disponible");
        };
        return match paper.run_detail(id).await {
            Ok(Some(d)) => json(&d),
            Ok(None) => text(404, "no hay una corrida con ese run_id"),
            Err(e) => internal(e),
        };
    }
    let write = matches!(req.path, "/api/v1/paper/preview" | "/api/v1/paper/orders");
    let read = matches!(
        req.path,
        "/api/v1/paper/positions" | "/api/v1/paper/account" | "/api/v1/backtest_runs"
    );
    if !write && !read {
        return text(404, "ruta desconocida");
    }
    if (write && req.method != "POST") || (read && req.method != "GET") {
        return text(405, if write { "solo POST" } else { "solo GET" });
    }
    let Some(paper) = paper else {
        return text(503, "el broker simulado no está disponible");
    };
    if read {
        return match route.1 {
            "/api/v1/paper/positions" => paper
                .positions(now)
                .await
                .map_or_else(internal, |p| json(&p)),
            "/api/v1/paper/account" => paper.account(now).await.map_or_else(internal, |a| json(&a)),
            _ => paper
                .runs(cfg.runs_limit)
                .await
                .map_or_else(internal, |r| json(&r)),
        };
    }
    // Escritura: token anti-CSRF, tipo de contenido y tamaño, en ese orden.
    let Some(expected) = token else {
        return text(403, "escritura apagada: falta el token local del gateway");
    };
    if !req.token.is_some_and(|t| same_token(t, expected)) {
        return text(403, "falta la cabecera x-cfds-token o no coincide");
    }
    if !req.content_type.is_some_and(|c| {
        c.split(';')
            .next()
            .is_some_and(|m| m.trim() == "application/json")
    }) {
        return text(415, "el body tiene que ser application/json");
    }
    if req.body.len() > cfg.max_body_bytes {
        return text(413, "body demasiado grande");
    }
    if req.path == "/api/v1/paper/orders" {
        match paper.gate() {
            GateState::Ready => {}
            GateState::Starting => {
                return text(
                    503,
                    "el broker simulado no acepta órdenes: la reconciliación al arrancar no pasó",
                );
            }
            GateState::Stopping => {
                return text(503, "el gateway se está apagando: no acepta órdenes nuevas");
            }
        }
    }
    let order: Order = match serde_json::from_slice(req.body) {
        Ok(o) => o,
        Err(e) => return text(400, &format!("no es un order.v1 válido: {e}")),
    };
    if order.created_at > now || now - order.created_at > cfg.max_order_age {
        return text(400, "created_at fuera de tiempo: tiene que ser de ahora");
    }
    let result = if req.path == "/api/v1/paper/preview" {
        paper.preview(order, now).await.map(|v| json(&v))
    } else {
        paper.submit(order).await.map(|r| json(&r))
    };
    result.unwrap_or_else(internal)
}

/// Logs y alertas del gateway.
pub struct Ops {
    pub log: Arc<Logger>,
    pub alerts: Alerter<ReqwestPost>,
}

/// El broker simulado del gateway: en memoria, con su registro en PostgreSQL.
pub struct PgPaper {
    pub(crate) state: Mutex<(PaperBroker, ReconcileSchedule)>,
    /// Sesión de solo lectura salvo la transacción del registro (`insert_paper_order`).
    pub(crate) store: PgStore,
    pub(crate) journal: PgJournal,
    pub(crate) gate: Gate,
    pub(crate) ops: Arc<Ops>,
    /// La última reconciliación (al arrancar o periódica), para `GET /api/v1/ops/status` (#314).
    last_reconcile: std::sync::Mutex<Option<ReconcileStatus>>,
}

impl PgPaper {
    /// Arma los mercados desde el catálogo y las velas de 15m guardadas, reconstruye el broker
    /// desde el registro y lo reconcilia (kill switch si algo no coincide). Los instrumentos con
    /// datos del broker pendientes en el catálogo no se operan: se devuelven aparte.
    ///
    /// Solo con la reconciliación limpia queda en [`GateState::Ready`]; si no, se queda en
    /// `Starting` y las órdenes responden 503.
    pub async fn start(
        store: PgStore,
        catalog: &Catalog,
        cfg: BrokerConfig,
        paper: &PaperConfig,
        ops: Arc<Ops>,
        now: DateTime<Utc>,
    ) -> Result<(Self, Vec<String>), String> {
        let mut markets = BTreeMap::new();
        let mut skipped = Vec::new();
        let from = now - TimeDelta::days(i64::from(paper.candles_days));
        for id in catalog.instruments.keys() {
            let terms = match market_terms_from_catalog(catalog, id) {
                Ok(t) => t,
                Err(e) => {
                    skipped.push(format!("{id}: {e}"));
                    continue;
                }
            };
            // Financiación con la serie diaria guardada (#79) si existe; si no, la constante
            // documentada del catálogo (`[funding_benchmarks]`).
            let mut rates = terms.funding_rates.clone();
            if let Some(series) = catalog.instruments[id].terms.funding_benchmark.value() {
                let saved = store
                    .reference_rates(series, now)
                    .await
                    .map_err(|e| e.to_string())?;
                if let Some(daily) = DailyRates::new(saved) {
                    rates = FundingRates::Daily(daily);
                }
            }
            let candles = store
                .candles(id, Timeframe::M15, from, now)
                .await
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|c| c.is_closed_at(now))
                .collect();
            markets.insert(
                id.clone(),
                MarketSetup {
                    limits: terms.limits,
                    costs: terms.costs,
                    account_to_instrument: terms.account_to_instrument,
                    candles,
                    funding: FundingSetup {
                        cutoff: terms.funding_cutoff,
                        tz: terms.funding_tz,
                        rates,
                    },
                },
            );
        }
        let journal = store.paper_journal().await.map_err(|e| e.to_string())?;
        // En paper, el "broker" es el mismo simulador: se reconstruye y se exige que cada
        // informe salga igual al guardado (drift = repintado → kill switch).
        let (probe, _) = restore_and_reconcile(cfg.clone(), markets.clone(), &journal, &[], now);
        let mut probe = probe;
        let positions = probe.positions(now);
        let (mut broker, rec) = restore_and_reconcile(cfg, markets, &journal, &positions, now);
        let gate = Gate::default();
        gate.set(gate_after_reconcile(&rec));
        ops.log.log(
            if rec.is_clean() {
                Level::Info
            } else {
                Level::Error
            },
            "gateway",
            "startup_reconcile",
            json!({
                "clean": rec.is_clean(),
                "matched": rec.matched,
                "mismatches": rec.mismatches.len(),
                "drifts": rec.drifts.len(),
                "orders_accepted": gate.state() == GateState::Ready,
            }),
        );
        let events = broker.take_events();
        log_events(&ops.log, &events);
        alert_events(&ops.alerts, &events, now).await;
        let journal_db = PgJournal::new(store.pool().clone());
        Ok((
            Self {
                state: Mutex::new((broker, ReconcileSchedule::default())),
                store,
                journal: journal_db,
                gate,
                ops,
                last_reconcile: std::sync::Mutex::new(Some(ReconcileStatus::new(
                    ReconcileKind::Startup,
                    now,
                    &rec,
                ))),
            },
            skipped,
        ))
    }

    /// Parada segura: deja de aceptar órdenes, espera a la que esté en curso (el candado),
    /// guarda en el registro lo que falte y verifica que no quede ninguna orden sin guardar.
    pub async fn shutdown(&self, now: DateTime<Utc>) -> Result<Flush, String> {
        self.gate.set(GateState::Stopping);
        let guard = self.state.lock().await;
        let flush = flush_journal(&guard.0, &self.store).await;
        match &flush {
            Ok(f) => {
                self.ops.log.log(
                    if f.is_clean() {
                        Level::Info
                    } else {
                        Level::Error
                    },
                    "gateway",
                    "shutdown",
                    json!({
                        "clean": f.is_clean(),
                        "saved_now": f.saved_now,
                        "orphans": f.orphans,
                    }),
                );
                if !f.is_clean() {
                    self.ops
                        .alerts
                        .raise(
                            AlertKind::Disconnect,
                            &format!("apagado con {} órdenes sin guardar", f.orphans.len()),
                            now,
                        )
                        .await;
                }
            }
            Err(e) => {
                self.fail(AlertKind::Disconnect, "shutdown_failed", e, now)
                    .await;
            }
        }
        flush
    }

    pub(crate) async fn fail(&self, kind: AlertKind, event: &str, error: &str, now: DateTime<Utc>) {
        report_failure(&self.ops.log, &self.ops.alerts, kind, event, error, now).await;
    }

    /// Después de cada operación: los eventos del broker al log y a las alertas.
    pub(crate) async fn drain(&self, broker: &mut PaperBroker, now: DateTime<Utc>) {
        let events = broker.take_events();
        log_events(&self.ops.log, &events);
        alert_events(&self.ops.alerts, &events, now).await;
    }

    /// `refresh` con sus errores logueados y alertados.
    pub(crate) async fn refresh_checked(
        &self,
        broker: &mut PaperBroker,
        sched: &mut ReconcileSchedule,
        now: DateTime<Utc>,
    ) -> Result<(), String> {
        let r = self.refresh(broker, sched, now).await;
        if let Err(e) = &r {
            self.fail(AlertKind::Disconnect, "store_failed", e, now)
                .await;
        }
        self.drain(broker, now).await;
        r
    }

    /// Antes de cada operación: velas nuevas cerradas y, si toca, la reconciliación periódica.
    async fn refresh(
        &self,
        broker: &mut PaperBroker,
        sched: &mut ReconcileSchedule,
        now: DateTime<Utc>,
    ) -> Result<(), String> {
        let ids: Vec<InstrumentId> = broker.markets().keys().cloned().collect();
        for id in ids {
            let from = broker.markets()[&id]
                .candles
                .last()
                .map_or(now - TimeDelta::days(1), |c| {
                    c.start + TimeDelta::seconds(1)
                });
            let fresh = self
                .store
                .candles(&id, Timeframe::M15, from, now)
                .await
                .map_err(|e| e.to_string())?;
            for c in fresh.into_iter().filter(|c| c.is_closed_at(now)) {
                // Una vela fuera de orden no se agrega (la validación de datos es de la ingesta),
                // pero se avisa: es un error de datos.
                let start = c.start;
                if let Err(e) = broker.push_candle(c) {
                    self.fail(
                        AlertKind::DataError,
                        "candle_rejected",
                        &format!("{id} {start}: {e}"),
                        now,
                    )
                    .await;
                }
            }
        }
        let markets = broker.markets().clone();
        let rec = reconcile_periodically(&self.store, broker, markets, sched, now)
            .await
            .map_err(|e| e.to_string())?;
        if let (Some(rec), Ok(mut last)) = (rec, self.last_reconcile.lock()) {
            *last = Some(ReconcileStatus::new(ReconcileKind::Periodic, now, &rec));
        }
        Ok(())
    }

    /// La última reconciliación (al arrancar o periódica).
    pub fn last_reconcile(&self) -> Option<ReconcileStatus> {
        self.last_reconcile.lock().ok().and_then(|l| l.clone())
    }
}

impl PaperService for PgPaper {
    async fn preview(&self, order: Order, now: DateTime<Utc>) -> Result<TicketVerdict, String> {
        let mut guard = self.state.lock().await;
        let (broker, sched) = &mut *guard;
        self.refresh_checked(broker, sched, now).await?;
        Ok(broker.preview(&order, now))
    }

    async fn submit(&self, order: Order) -> Result<OrderReport, String> {
        let mut guard = self.state.lock().await;
        let (broker, sched) = &mut *guard;
        // Una orden que esperaba el candado mientras empezaba la parada no se procesa.
        if self.gate.state() != GateState::Ready {
            return Err("el broker simulado no acepta órdenes ahora".into());
        }
        let now = order.created_at;
        self.refresh_checked(broker, sched, now).await?;
        let r = submit_and_record(broker, &self.store, &order)
            .await
            .map_err(|e| e.to_string());
        if let Err(e) = &r {
            // El informe quedó en el broker: la parada segura lo guarda si la base vuelve.
            self.fail(AlertKind::Disconnect, "journal_write_failed", e, now)
                .await;
        }
        self.drain(broker, now).await;
        r
    }

    async fn positions(&self, now: DateTime<Utc>) -> Result<Vec<Position>, String> {
        let mut guard = self.state.lock().await;
        let (broker, sched) = &mut *guard;
        self.refresh_checked(broker, sched, now).await?;
        Ok(broker.positions(now))
    }

    async fn account(&self, now: DateTime<Utc>) -> Result<PaperAccount, String> {
        let mut guard = self.state.lock().await;
        let (broker, sched) = &mut *guard;
        self.refresh_checked(broker, sched, now).await?;
        Ok(broker.account(now))
    }

    async fn runs(&self, limit: u32) -> Result<Vec<BacktestRun>, String> {
        self.journal.runs(limit).await.map_err(|e| e.to_string())
    }
    async fn run_detail(&self, run_id: &str) -> Result<Option<RunDetail>, String> {
        self.journal
            .run_detail(run_id)
            .await
            .map_err(|e| e.to_string())
    }

    fn gate(&self) -> GateState {
        self.gate.state()
    }
}

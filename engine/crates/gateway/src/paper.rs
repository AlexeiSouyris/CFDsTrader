//! Rutas del broker simulado y de las corridas guardadas (fase 7 y 10, #278 y #275).
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `POST /api/v1/paper/preview` | `order.v1` → `ticket_verdict.v1` (sin efectos) |
//! | `POST /api/v1/paper/orders` | `order.v1` → `order_report.v1` (idempotente, se guarda) |
//! | `GET /api/v1/paper/positions` | `position.v1[]` |
//! | `GET /api/v1/paper/account` | `paper_account.v1` |
//! | `GET /api/v1/backtest_runs` | `backtest_run.v1[]` (las más nuevas primero) |
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

use std::collections::BTreeMap;
use std::future::Future;

use cfds_backtest::{DailyRates, FundingRates, market_terms_from_catalog};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, MarketSetup, PaperBroker, ReconcileSchedule,
    reconcile_periodically, restore_and_reconcile, submit_and_record,
};
use cfds_core::{
    InstrumentId, Order, OrderReport, PaperAccount, Position, TicketVerdict, Timeframe,
};
use cfds_journal::BacktestRun;
use cfds_journal::pg::PgJournal;
use cfds_market_data::Catalog;
use cfds_storage::PgStore;
use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::Reply;

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
    path.starts_with("/api/v1/paper/") || path == "/api/v1/backtest_runs"
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

/// El broker simulado del gateway: en memoria, con su registro en PostgreSQL.
pub struct PgPaper {
    state: Mutex<(PaperBroker, ReconcileSchedule)>,
    /// Sesión de solo lectura salvo la transacción del registro (`insert_paper_order`).
    store: PgStore,
    journal: PgJournal,
}

impl PgPaper {
    /// Arma los mercados desde el catálogo y las velas de 15m guardadas, reconstruye el broker
    /// desde el registro y lo reconcilia (kill switch si algo no coincide). Los instrumentos con
    /// datos del broker pendientes en el catálogo no se operan: se devuelven aparte.
    pub async fn start(
        store: PgStore,
        catalog: &Catalog,
        cfg: BrokerConfig,
        paper: &PaperConfig,
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
        let (broker, _) = restore_and_reconcile(cfg, markets, &journal, &positions, now);
        let journal_db = PgJournal::new(store.pool().clone());
        Ok((
            Self {
                state: Mutex::new((broker, ReconcileSchedule::default())),
                store,
                journal: journal_db,
            },
            skipped,
        ))
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
                // Una vela fuera de orden no se agrega (la validación de datos es de la ingesta).
                let _ = broker.push_candle(c);
            }
        }
        let markets = broker.markets().clone();
        reconcile_periodically(&self.store, broker, markets, sched, now)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl PaperService for PgPaper {
    async fn preview(&self, order: Order, now: DateTime<Utc>) -> Result<TicketVerdict, String> {
        let mut guard = self.state.lock().await;
        let (broker, sched) = &mut *guard;
        self.refresh(broker, sched, now).await?;
        Ok(broker.preview(&order, now))
    }

    async fn submit(&self, order: Order) -> Result<OrderReport, String> {
        let mut guard = self.state.lock().await;
        let (broker, sched) = &mut *guard;
        self.refresh(broker, sched, order.created_at).await?;
        submit_and_record(broker, &self.store, &order)
            .await
            .map_err(|e| e.to_string())
    }

    async fn positions(&self, now: DateTime<Utc>) -> Result<Vec<Position>, String> {
        let mut guard = self.state.lock().await;
        let (broker, sched) = &mut *guard;
        self.refresh(broker, sched, now).await?;
        Ok(broker.positions(now))
    }

    async fn account(&self, now: DateTime<Utc>) -> Result<PaperAccount, String> {
        let mut guard = self.state.lock().await;
        let (broker, sched) = &mut *guard;
        self.refresh(broker, sched, now).await?;
        Ok(broker.account(now))
    }

    async fn runs(&self, limit: u32) -> Result<Vec<BacktestRun>, String> {
        self.journal.runs(limit).await.map_err(|e| e.to_string())
    }
}

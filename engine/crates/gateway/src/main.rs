//! Servidor del gateway (#231, #278): API para la UI. Lectura, más las rutas del broker simulado
//! (`/api/v1/paper/*`, solo paper; ver `cfds_gateway::paper`).
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-gateway
//! ```
//! Escucha en `[server] bind` de `config/gateway.toml` (por defecto solo 127.0.0.1). No corre
//! migraciones. La sesión de PostgreSQL es de solo lectura; la única escritura es el registro del
//! broker simulado, en una transacción que se declara de escritura (`insert_paper_order`). La
//! escritura exige el token local de `[paper] token_env`; sin él queda apagada. La UI llega con el
//! proxy de Vite (`/api` → este servidor).
//!
//! Fase 9 (#297): logs en líneas JSON por la salida de errores (sin secretos: el token y la
//! contraseña de la base se registran para taparlos), alertas de `config/alerts.toml` y parada
//! segura con SIGTERM o Ctrl-C (deja de aceptar órdenes, guarda el registro, verifica).
//!
//! Fase 8 (#305): con `[runner] enabled = true` en `config/runner.toml`, corre además el runner de
//! paper trading sobre el mismo broker simulado (`cfds_gateway::paper_runner`). Nunca llama a IG.

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;

use cfds_backtest::BacktestConfig;
use cfds_broker::BrokerConfig;
use cfds_core::Instrument;
use cfds_fund::FundConfig;
use cfds_fund::pg::PgFund;
use cfds_gateway::context::{ContextConfig, is_context_route, respond_context};
use cfds_gateway::emotions::{is_emotions_route, respond_emotions};
use cfds_gateway::fund::{is_fund_route, respond_fund};
use cfds_gateway::learning::{NoStoredReports, is_learning_route, respond_learning};
use cfds_gateway::ops::url_password;
use cfds_gateway::paper::{HttpIn, Ops, PaperConfig, PgPaper, is_paper_route, respond_paper};
use cfds_gateway::paper_runner::RunnerDeps;
use cfds_gateway::status::{
    Feed, FeedWriter, StatusConfig, StatusDeps, is_status_route, respond_status,
};
use cfds_gateway::walk_forward::{is_walk_forward_route, respond_walk_forward};
use cfds_gateway::{GatewayConfig, PgSource, Reply, read_only_url, respond};
use cfds_liquidity::StatsConfig;
use cfds_market_data::{CandleSources, Catalog};
use cfds_obs::{AlertConfig, AlertKind, Alerter, Level, Logger, ReqwestPost, TelegramSink};
use cfds_risk::RiskConfig;
use cfds_runner::RunnerConfig;
use cfds_storage::PgStore;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use serde_json::json;
use tokio::net::TcpListener;

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const LIQUIDITY: &str = include_str!("../../../../config/liquidity.toml");
const SOURCES: &str = include_str!("../../../../config/candle_sources.toml");
const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("../../../../config/broker.toml");
const ALERTS: &str = include_str!("../../../../config/alerts.toml");
const FUND: &str = include_str!("../../../../config/fund.toml");
const RUNNER: &str = include_str!("../../../../config/runner.toml");

/// `config/` del repo (sesiones y feriados del calendario se leen de ahí).
fn config_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config")
}

struct State {
    source: PgSource,
    catalog: Vec<Instrument>,
    paper: Option<PgPaper>,
    paper_cfg: PaperConfig,
    /// Token local de escritura; `None`: escritura apagada.
    token: Option<String>,
    context_cfg: ContextConfig,
    /// Fondo del grupo (#312): solo lectura.
    fund: PgFund,
    fund_cfg: FundConfig,
    /// Estado de la operación (#314): eventos en memoria, límite de pérdida diaria y el
    /// `Logger` (para tapar secretos). Solo lectura.
    feed: Arc<Feed>,
    daily_loss_limit_pct: rust_decimal::Decimal,
    log: Arc<Logger>,
    /// Reportes de walk-forward (#316): solo lectura del diario.
    walk_forward: cfds_journal::pg::PgJournal,
}

fn header<'a>(req: &'a Request<Incoming>, name: &str) -> Option<&'a str> {
    req.headers().get(name).and_then(|v| v.to_str().ok())
}

async fn paper_reply(state: &State, req: Request<Incoming>) -> Reply {
    let method = req.method().as_str().to_owned();
    let path = req.uri().path().to_owned();
    let token = header(&req, "x-cfds-token").map(str::to_owned);
    let content_type = header(&req, "content-type").map(str::to_owned);
    // El body se lee con límite: uno más grande se corta sin leerlo entero.
    let body = match Limited::new(req.into_body(), state.paper_cfg.max_body_bytes)
        .collect()
        .await
    {
        Ok(b) => b.to_bytes(),
        Err(_) => {
            return Reply {
                status: 413,
                content_type: "text/plain; charset=utf-8",
                body: "body demasiado grande".into(),
            };
        }
    };
    respond_paper(
        state.paper.as_ref(),
        &state.paper_cfg,
        state.token.as_deref(),
        HttpIn {
            method: &method,
            path: &path,
            token: token.as_deref(),
            content_type: content_type.as_deref(),
            body: &body,
        },
        chrono::Utc::now(),
    )
    .await
}

async fn handle(
    state: Arc<State>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let reply = if is_paper_route(req.uri().path()) {
        paper_reply(&state, req).await
    } else if is_status_route(req.uri().path()) {
        let scrub = |t: &str| state.log.scrub(t);
        let deps = StatusDeps {
            feed: &state.feed,
            daily_loss_limit_pct: state.daily_loss_limit_pct,
            scrub: &scrub,
        };
        respond_status(
            state.paper.as_ref(),
            &deps,
            req.method().as_str(),
            req.uri().path(),
            chrono::Utc::now(),
        )
        .await
    } else if is_fund_route(req.uri().path()) {
        respond_fund(
            &state.fund,
            &state.fund_cfg,
            req.method().as_str(),
            req.uri().path(),
        )
        .await
    } else if is_learning_route(req.uri().path()) {
        // Fases 8 y 8b (#331): comparaciones del diario; el informe guardado llega con #330.
        respond_learning(
            &state.walk_forward,
            &NoStoredReports,
            req.method().as_str(),
            req.uri().path(),
        )
        .await
    } else if is_walk_forward_route(req.uri().path()) {
        respond_walk_forward(
            &state.walk_forward,
            req.method().as_str(),
            req.uri().path(),
            req.uri().query(),
        )
        .await
    } else if is_emotions_route(req.uri().path()) {
        respond_emotions(
            &state.source.store,
            req.method().as_str(),
            req.uri().path(),
            req.uri().query(),
            chrono::Utc::now(),
        )
        .await
    } else if is_context_route(req.uri().path()) {
        respond_context(
            &state.source.store,
            &state.context_cfg,
            req.method().as_str(),
            req.uri().path(),
            req.uri().query(),
            chrono::Utc::now(),
        )
        .await
    } else {
        liquidity_reply(&state, &req).await
    };
    let mut res = Response::new(Full::new(Bytes::from(reply.body)));
    *res.status_mut() = hyper::StatusCode::from_u16(reply.status)
        .unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR);
    res.headers_mut().insert(
        CONTENT_TYPE,
        hyper::header::HeaderValue::from_static(reply.content_type),
    );
    res.headers_mut().insert(
        CACHE_CONTROL,
        hyper::header::HeaderValue::from_static("no-store"),
    );
    Ok(res)
}

async fn liquidity_reply(state: &State, req: &Request<Incoming>) -> Reply {
    let uri = req.uri();
    respond(
        &state.source,
        &state.source.config,
        &state.source.sources,
        &state.catalog,
        req.method().as_str(),
        uri.path(),
        uri.query(),
        chrono::Utc::now(),
    )
    .await
}

/// Espera SIGTERM (o Ctrl-C).
async fn shutdown_signal() {
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    tokio::select! {
        () = term => {}
        _ = tokio::signal::ctrl_c() => {}
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Toda la configuración se valida al arrancar (CLAUDE.md §5b).
    let cfg = GatewayConfig::from_toml(GATEWAY)?;
    let catalog = Catalog::from_toml(CATALOG)?;
    let stats = StatsConfig::from_toml(LIQUIDITY)?;
    let paper_cfg = PaperConfig::from_toml(GATEWAY)?;
    let alert_cfg = AlertConfig::from_toml(ALERTS)?;
    let runner_cfg = RunnerConfig::from_toml(RUNNER)?;
    // Si el runner está prendido, toda su config se valida antes de abrir nada.
    let runner = if runner_cfg.enabled {
        Some(RunnerDeps::from_repo_config(&catalog, &config_dir())?)
    } else {
        None
    };
    let risk = RiskConfig::from_toml(RISK)?;
    // Logs sin secretos: el token local y la base se registran antes del primer log. Las líneas
    // ya redactadas del broker y de las alertas quedan además en memoria para la UI (#314).
    let feed = Feed::new(StatusConfig::from_toml(GATEWAY)?);
    let log = Arc::new(Logger::new(Box::new(FeedWriter::new(
        feed.clone(),
        Box::new(std::io::stderr()),
    ))));
    let database_url = std::env::var("DATABASE_URL")?;
    log.register_secret(&database_url);
    if let Some(pass) = url_password(&database_url) {
        log.register_secret(pass);
    }
    let token = std::env::var(&paper_cfg.token_env)
        .ok()
        .filter(|t| t.len() >= 16);
    if let Some(t) = &token {
        log.register_secret(t);
    }
    let telegram = TelegramSink::from_env(
        &alert_cfg,
        ReqwestPost::new(alert_cfg.telegram_timeout)?,
        |name| std::env::var(name).ok(),
    );
    let alerts = Alerter::new(alert_cfg, log.clone(), telegram);
    log.log(
        Level::Info,
        "gateway",
        "alerts",
        json!({ "telegram": alerts.telegram_enabled() }),
    );
    let ops = Arc::new(Ops {
        log: log.clone(),
        alerts,
    });
    // La sesión de PostgreSQL se abre en solo lectura: aunque hubiera un error, no puede escribir.
    let store = match PgStore::connect(&read_only_url(&database_url)).await {
        Ok(s) => s,
        Err(e) => {
            ops.alerts
                .raise(
                    AlertKind::Disconnect,
                    &format!("sin base al arrancar: {e}"),
                    chrono::Utc::now(),
                )
                .await;
            return Err(log.scrub(&e.to_string()).into());
        }
    };
    let backtest = BacktestConfig::from_toml(BACKTEST)?;
    let broker_cfg = BrokerConfig::from_toml(
        BROKER,
        risk,
        backtest.costs,
        backtest.exits,
        backtest.day_tz,
    )?;
    let paper = match PgPaper::start(
        store.clone(),
        &catalog,
        broker_cfg,
        &paper_cfg,
        ops.clone(),
        chrono::Utc::now(),
    )
    .await
    {
        Ok((p, skipped)) => {
            for s in skipped {
                log.log(
                    Level::Warn,
                    "gateway",
                    "instrument_skipped",
                    json!({ "detail": s }),
                );
            }
            Some(p)
        }
        Err(e) => {
            // Las rutas /api/v1/paper responden 503.
            ops.alerts
                .raise(
                    AlertKind::Disconnect,
                    &format!("el broker simulado no arrancó: {e}"),
                    chrono::Utc::now(),
                )
                .await;
            None
        }
    };
    if token.is_none() {
        log.log(
            Level::Warn,
            "gateway",
            "writes_disabled",
            json!({ "missing_env": paper_cfg.token_env }),
        );
    }
    let fund_cfg = FundConfig::from_toml(FUND)?;
    let fund = PgFund::new(store.pool().clone());
    let state = Arc::new(State {
        feed,
        daily_loss_limit_pct: risk.max_daily_loss_pct,
        log: log.clone(),
        walk_forward: cfds_journal::pg::PgJournal::new(store.pool().clone()),
        fund,
        fund_cfg,
        paper,
        paper_cfg,
        token,
        context_cfg: ContextConfig::from_toml(GATEWAY)?,
        source: PgSource {
            store,
            config: cfg.clone(),
            reaction_bars: stats.reaction_bars,
            sources: CandleSources::from_toml(SOURCES)?,
        },
        catalog: catalog
            .instruments
            .values()
            .map(|s| s.instrument.clone())
            .collect(),
    });
    match (runner, &state.paper) {
        (Some(deps), Some(_)) => {
            log.log(
                Level::Info,
                "runner",
                "started",
                json!({ "poll_secs": runner_cfg.poll.as_secs(), "mode": "paper" }),
            );
            let state = state.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(runner_cfg.poll);
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tick.tick().await;
                    let Some(paper) = &state.paper else { break };
                    // Los errores ya quedan en el log y en las alertas; se reintenta en el
                    // próximo tick.
                    let _ = paper.run_once(&deps, chrono::Utc::now()).await;
                }
            });
        }
        (Some(_), None) => log.log(
            Level::Error,
            "runner",
            "not_started",
            json!({ "reason": "el broker simulado no arrancó" }),
        ),
        (None, _) => {}
    }
    let listener = TcpListener::bind(cfg.bind).await?;
    log.log(
        Level::Info,
        "gateway",
        "listening",
        json!({ "bind": cfg.bind.to_string(), "mode": "paper" }),
    );
    let stop = shutdown_signal();
    tokio::pin!(stop);
    loop {
        let (stream, _) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(a) => a,
                Err(e) => {
                    ops.alerts
                        .raise(
                            AlertKind::Disconnect,
                            &format!("el gateway no acepta conexiones: {e}"),
                            chrono::Utc::now(),
                        )
                        .await;
                    continue;
                }
            },
            () = &mut stop => break,
        };
        let state = state.clone();
        let log = log.clone();
        tokio::spawn(async move {
            let service = service_fn(move |req| handle(state.clone(), req));
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                log.log(
                    Level::Warn,
                    "gateway",
                    "connection_error",
                    json!({ "error": e.to_string() }),
                );
            }
        });
    }
    // Parada segura: sin órdenes nuevas, registro guardado y verificado.
    log.log(Level::Info, "gateway", "shutdown_requested", json!({}));
    if let Some(paper) = &state.paper {
        let flush = paper.shutdown(chrono::Utc::now()).await?;
        if !flush.is_clean() {
            return Err(format!("{} órdenes sin guardar al apagar", flush.orphans.len()).into());
        }
    }
    Ok(())
}

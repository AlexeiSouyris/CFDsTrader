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
use cfds_gateway::ibkr::{IbkrViewConfig, is_ibkr_route, respond_ibkr};
use cfds_gateway::ibkr_live::{
    HubEvent, LiveHub, LiveReply, is_ibkr_live_route, respond_ibkr_live,
};
use cfds_gateway::learning::{PgLearning, is_learning_route, respond_learning};
use cfds_gateway::ml::{is_ml_route, respond_ml};
use cfds_gateway::ops::{connection_end, url_password};
use cfds_gateway::ops_store::{persistence, run_persister};
use cfds_gateway::paper::{HttpIn, Ops, PaperConfig, PgPaper, is_paper_route, respond_paper};
use cfds_gateway::paper_runner::RunnerDeps;
use cfds_gateway::status::{
    Feed, FeedWriter, StatusConfig, StatusDeps, is_status_route, respond_status,
};
use cfds_gateway::walk_forward::{is_walk_forward_route, respond_walk_forward};
use cfds_gateway::{GatewayConfig, PgSource, Reply, ViewAnalysis, read_only_url, respond};
use cfds_ibkr::IbkrConfig;
use cfds_ibkr::live_feed::FeedStatus;
use cfds_journal::votes::VoteRules;
use cfds_liquidity::StatsConfig;
use cfds_market_data::{CandleSources, Catalog};
use cfds_obs::{AlertConfig, AlertKind, Alerter, Level, Logger, ReqwestPost, TelegramSink};
use cfds_risk::RiskConfig;
use cfds_runner::RunnerConfig;
use cfds_storage::PgStore;
use http_body_util::combinators::BoxBody;
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
const VOTES: &str = include_str!("../../../../config/votes.toml");
const RUNNER: &str = include_str!("../../../../config/runner.toml");
const ANALYSIS: &str = include_str!("../../../../config/analysis.toml");
const PATTERNS: &str = include_str!("../../../../config/patterns.toml");
const TERMINAL: &str = include_str!("../../../../config/terminal.toml");
const IBKR: &str = include_str!("../../../../config/ibkr.toml");

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
    /// `[ops] events_limit` y `group_scan` (#576).
    ops_events_limit: usize,
    ops_group_scan: usize,
    daily_loss_limit_pct: rust_decimal::Decimal,
    log: Arc<Logger>,
    /// Reportes de walk-forward (#316) y experimentos de ML (#426): solo lectura del diario.
    walk_forward: cfds_journal::pg::PgJournal,
    /// Informe de aprendizaje y estado de sus propuestas (#331), solo lectura.
    learning: PgLearning,
    /// Velas de IBKR para la terminal de la app (#551), solo lectura.
    ibkr_cfg: IbkrViewConfig,
    /// Vela en formación de IBKR (#558), solo visual.
    live_hub: Arc<LiveHub>,
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

type Body = BoxBody<Bytes, Infallible>;

fn no_store(res: &mut Response<Body>, content_type: &'static str) {
    res.headers_mut().insert(
        CONTENT_TYPE,
        hyper::header::HeaderValue::from_static(content_type),
    );
    res.headers_mut().insert(
        CACHE_CONTROL,
        hyper::header::HeaderValue::from_static("no-store"),
    );
}

async fn handle(state: Arc<State>, req: Request<Incoming>) -> Result<Response<Body>, Infallible> {
    if is_ibkr_live_route(req.uri().path()) {
        // Vela en formación (#558): SSE, un mensaje por segundo como mucho.
        let mut res = match respond_ibkr_live(
            &state.live_hub,
            req.method().as_str(),
            req.uri().query(),
            state.ibkr_cfg.live_max_clients,
            state.ibkr_cfg.live_heartbeat,
        ) {
            LiveReply::Stream(body) => {
                let mut res = Response::new(body.boxed());
                no_store(&mut res, "text/event-stream");
                res
            }
            LiveReply::Error { status, body } => {
                let mut res = Response::new(Full::new(Bytes::from(body)).boxed());
                *res.status_mut() = hyper::StatusCode::from_u16(status)
                    .unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR);
                no_store(&mut res, "text/plain; charset=utf-8");
                res
            }
        };
        res.headers_mut().insert(
            "x-accel-buffering",
            hyper::header::HeaderValue::from_static("no"),
        );
        return Ok(res);
    }
    let reply = if is_paper_route(req.uri().path()) {
        paper_reply(&state, req).await
    } else if is_status_route(req.uri().path()) {
        let scrub = |t: &str| state.log.scrub(t);
        let deps = StatusDeps {
            feed: &state.feed,
            events: Some(&state.source.store),
            events_limit: state.ops_events_limit,
            group_scan: state.ops_group_scan,
            daily_loss_limit_pct: state.daily_loss_limit_pct,
            scrub: &scrub,
        };
        respond_status(
            state.paper.as_ref(),
            &deps,
            req.method().as_str(),
            req.uri().path(),
            req.uri().query(),
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
        // Fases 8 y 8b (#331): comparaciones, el último informe y el estado de sus propuestas.
        respond_learning(
            &state.walk_forward,
            &state.learning,
            req.method().as_str(),
            req.uri().path(),
            chrono::Utc::now(),
        )
        .await
    } else if is_ml_route(req.uri().path()) {
        // Experimentos de ML (#426): solo lectura, el modelo es informativo.
        respond_ml(
            &state.walk_forward,
            req.method().as_str(),
            req.uri().path(),
            req.uri().query(),
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
    } else if is_ibkr_route(req.uri().path()) {
        respond_ibkr(
            &state.source.store,
            &state.ibkr_cfg,
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
    let mut res = Response::new(Full::new(Bytes::from(reply.body)).boxed());
    *res.status_mut() = hyper::StatusCode::from_u16(reply.status)
        .unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR);
    no_store(&mut res, reply.content_type);
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
    let ibkr_cfg = IbkrViewConfig::from_toml(GATEWAY, TERMINAL)?;
    let ibkr_conn = IbkrConfig::from_toml(IBKR)?;
    if ibkr_cfg.live && ibkr_conn.live.is_none() {
        return Err(
            "[ibkr] live = true en config/gateway.toml pero config/ibkr.toml no tiene [live]"
                .into(),
        );
    }
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
    // ya redactadas del broker y de las alertas quedan además en memoria para la UI (#314) y se
    // guardan en `ops_events` en segundo plano (#337): la cola existe desde ya y la tarea que la
    // vacía arranca cuando hay base. El broker nunca espera a la base.
    let status_cfg = StatusConfig::from_toml(GATEWAY)?;
    let feed = Feed::new(status_cfg);
    let (sink, events_rx) = persistence(status_cfg.persist_queue);
    feed.attach(sink);
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
    {
        // Si la base no responde, avisa con la alerta de desconexión de siempre (con su intervalo
        // mínimo) y reintenta; el feed en memoria sigue.
        let ops = ops.clone();
        tokio::spawn(run_persister(
            events_rx,
            Arc::new(store.clone()),
            status_cfg.persist_queue,
            status_cfg.retry,
            move |msg: String| {
                let ops = ops.clone();
                async move {
                    ops.alerts
                        .raise(AlertKind::Disconnect, &msg, chrono::Utc::now())
                        .await;
                }
            },
        ));
    }
    let backtest = BacktestConfig::from_toml(BACKTEST)?;
    // Con la sesión de cada instrumento: el cierre de fin de semana de la spec §8 (#334).
    let calendar = cfds_market_data::ExchangeCalendar::load(&config_dir(), catalog.exchange_map())?;
    let broker_cfg = BrokerConfig::from_toml(
        BROKER,
        risk,
        backtest.costs,
        backtest.exits,
        backtest.day_tz,
    )?
    .with_calendar(&calendar, catalog.instruments.keys())?;
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
    // Vela en formación (#558): la conexión de streaming solo con [ibkr] live = true. Si IB Gateway
    // está cerrado, el gateway sigue y la ruta informa "disconnected" (reintenta solo).
    let live_hub = {
        let (instruments, timeframes) = ibkr_conn
            .live
            .as_ref()
            .map(|l| (l.instruments.clone(), l.timeframes.clone()))
            .unwrap_or_default();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let status = if ibkr_cfg.live {
            FeedStatus::Connecting
        } else {
            FeedStatus::Disabled
        };
        let hub = Arc::new(LiveHub::new(
            &ibkr_cfg.source,
            instruments,
            timeframes,
            status,
            Some(tx),
        ));
        let (log, ops) = (log.clone(), ops.clone());
        tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                match ev {
                    HubEvent::Status(s) => log.log(
                        if s.reason().is_some() {
                            Level::Warn
                        } else {
                            Level::Info
                        },
                        "ibkr_live",
                        s.as_str(),
                        json!({ "reason": s.reason() }),
                    ),
                    HubEvent::Trip(t) => {
                        ops.alerts
                            .raise(
                                t.kind,
                                &format!("IBKR vela en formación: {}", t.reason),
                                t.at,
                            )
                            .await;
                    }
                }
            }
        });
        if ibkr_cfg.live {
            tokio::spawn(cfds_ibkr::live_feed::run(
                ibkr_conn,
                |k| std::env::var(k).ok(),
                hub.clone(),
            ));
        }
        hub
    };
    let fund_cfg = FundConfig::from_toml(FUND)?;
    let fund = PgFund::new(store.pool().clone());
    let state = Arc::new(State {
        feed,
        ops_events_limit: status_cfg.events_limit,
        ops_group_scan: status_cfg.group_scan,
        daily_loss_limit_pct: risk.max_daily_loss_pct,
        log: log.clone(),
        walk_forward: cfds_journal::pg::PgJournal::new(store.pool().clone()),
        learning: PgLearning {
            journal: cfds_journal::pg::PgJournal::new(store.pool().clone()),
            rules: VoteRules::from_toml(VOTES)?,
        },
        fund,
        fund_cfg,
        paper,
        paper_cfg,
        token,
        context_cfg: ContextConfig::from_toml(GATEWAY)?,
        ibkr_cfg,
        live_hub,
        source: PgSource {
            store,
            config: cfg.clone(),
            reaction_bars: stats.reaction_bars,
            sources: CandleSources::from_toml(SOURCES)?,
            analysis: Some(ViewAnalysis::from_toml(ANALYSIS, PATTERNS)?),
            cache: Default::default(),
        },
        catalog: catalog
            .instruments
            .values()
            .map(|s| s.instrument.clone())
            .collect(),
    });
    // La probabilidad mostrada de las propuestas de paper (§13, #333), si está configurada.
    let runner = match runner {
        Some(mut deps) => {
            let journal = cfds_journal::pg::PgJournal::new(state.source.store.pool().clone());
            match deps.load_probability(&journal).await {
                Ok(()) => Some(deps),
                Err(e) => {
                    log.log(
                        Level::Error,
                        "runner",
                        "not_started",
                        json!({ "reason": format!("probabilidad mostrada: {e}") }),
                    );
                    None
                }
            }
        }
        None => None,
    };
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
                // Que el cliente cierre (recarga, cambio de vista, fin de un SSE) es lo
                // esperable: `info`, fuera del feed y de la base (#576).
                let (level, event) = connection_end(&e);
                log.log(level, "gateway", event, json!({ "error": e.to_string() }));
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

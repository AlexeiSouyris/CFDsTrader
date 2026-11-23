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

use std::convert::Infallible;
use std::sync::Arc;

use cfds_backtest::BacktestConfig;
use cfds_broker::BrokerConfig;
use cfds_core::Instrument;
use cfds_gateway::paper::{HttpIn, PaperConfig, PgPaper, is_paper_route, respond_paper};
use cfds_gateway::{GatewayConfig, PgSource, Reply, read_only_url, respond};
use cfds_liquidity::StatsConfig;
use cfds_market_data::{CandleSources, Catalog};
use cfds_risk::RiskConfig;
use cfds_storage::PgStore;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const LIQUIDITY: &str = include_str!("../../../../config/liquidity.toml");
const SOURCES: &str = include_str!("../../../../config/candle_sources.toml");
const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("../../../../config/broker.toml");

struct State {
    source: PgSource,
    catalog: Vec<Instrument>,
    paper: Option<PgPaper>,
    paper_cfg: PaperConfig,
    /// Token local de escritura; `None`: escritura apagada.
    token: Option<String>,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Toda la configuración se valida al arrancar (CLAUDE.md §5b).
    let cfg = GatewayConfig::from_toml(GATEWAY)?;
    let catalog = Catalog::from_toml(CATALOG)?;
    let stats = StatsConfig::from_toml(LIQUIDITY)?;
    // La sesión de PostgreSQL se abre en solo lectura: aunque hubiera un error, no puede escribir.
    let store = PgStore::connect(&read_only_url(&std::env::var("DATABASE_URL")?)).await?;
    let paper_cfg = PaperConfig::from_toml(GATEWAY)?;
    let backtest = BacktestConfig::from_toml(BACKTEST)?;
    let broker_cfg = BrokerConfig::from_toml(
        BROKER,
        RiskConfig::from_toml(RISK)?,
        backtest.costs,
        backtest.exits,
        backtest.day_tz,
    )?;
    let token = std::env::var(&paper_cfg.token_env)
        .ok()
        .filter(|t| t.len() >= 16);
    let paper = match PgPaper::start(
        store.clone(),
        &catalog,
        broker_cfg,
        &paper_cfg,
        chrono::Utc::now(),
    )
    .await
    {
        Ok((p, skipped)) => {
            for s in skipped {
                println!("broker simulado: {s} (no se opera)");
            }
            Some(p)
        }
        Err(e) => {
            eprintln!("broker simulado: no arrancó ({e}); las rutas /api/v1/paper responden 503");
            None
        }
    };
    if token.is_none() {
        println!(
            "broker simulado: sin {} (16 caracteres o más), la escritura queda apagada",
            paper_cfg.token_env
        );
    }
    let state = Arc::new(State {
        paper,
        paper_cfg,
        token,
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
    let listener = TcpListener::bind(cfg.bind).await?;
    println!(
        "gateway escuchando en http://{} (lectura; broker simulado solo paper)",
        cfg.bind
    );
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let service = service_fn(move |req| handle(state.clone(), req));
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                eprintln!("gateway: conexión: {e}");
            }
        });
    }
}

//! Servidor del gateway (#231): API de solo lectura para la UI.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-gateway
//! ```
//! Escucha en `[server] bind` de `config/gateway.toml` (por defecto solo 127.0.0.1). No corre
//! migraciones ni escribe nada (la sesión de PostgreSQL es de solo lectura): lee lo que guardaron `ig_live`, `liquidity_levels` y las
//! estadísticas. La UI llega con el proxy de Vite (`/api` → este servidor).

use std::convert::Infallible;
use std::sync::Arc;

use cfds_core::Instrument;
use cfds_gateway::{GatewayConfig, PgSource, read_only_url, respond};
use cfds_liquidity::StatsConfig;
use cfds_market_data::Catalog;
use cfds_storage::PgStore;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const LIQUIDITY: &str = include_str!("../../../../config/liquidity.toml");

struct State {
    source: PgSource,
    catalog: Vec<Instrument>,
}

async fn handle(
    state: Arc<State>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let uri = req.uri();
    let reply = respond(
        &state.source,
        &state.source.config,
        &state.catalog,
        req.method().as_str(),
        uri.path(),
        uri.query(),
        chrono::Utc::now(),
    )
    .await;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Toda la configuración se valida al arrancar (CLAUDE.md §5b).
    let cfg = GatewayConfig::from_toml(GATEWAY)?;
    let catalog = Catalog::from_toml(CATALOG)?;
    let stats = StatsConfig::from_toml(LIQUIDITY)?;
    // La sesión de PostgreSQL se abre en solo lectura: aunque hubiera un error, no puede escribir.
    let store = PgStore::connect(&read_only_url(&std::env::var("DATABASE_URL")?)).await?;
    let state = Arc::new(State {
        source: PgSource {
            store,
            config: cfg.clone(),
            reaction_bars: stats.reaction_bars,
        },
        catalog: catalog
            .instruments
            .values()
            .map(|s| s.instrument.clone())
            .collect(),
    });
    let listener = TcpListener::bind(cfg.bind).await?;
    println!("gateway escuchando en http://{} (solo lectura)", cfg.bind);
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

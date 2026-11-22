//! Proveedor de datos de IG, cuenta **demo** (#66, ADR 0002).
//!
//! - [`IgProvider`] implementa [`cfds_market_data::MarketDataProvider`]: histórico de velas
//!   de 15m y 1h con precio medio exacto y en UTC. Las velas entran al sistema solo por el
//!   circuito de ingesta (`cfds-ingest`).
//! - Lee las condiciones del contrato y el spread al cierre de cada vela, para completar
//!   `config/instruments.toml` (#67).
//! - Modo en vivo (#68): [`live`] decide qué velas cerradas pedir y [`Recent`] las trae
//!   gastando 1 punto por vela.
//! - Sentimiento de clientes de IG (#27): [`IgProvider::client_sentiment`] y [`sentiment`].
//! - Respeta los límites de IG (`config/ig.toml`): pedidos por minuto, cupo semanal de
//!   históricos y reintentos con backoff.
//! - Credenciales solo por variables de entorno; nunca aparecen en `Debug` ni en errores.
//! - Solo lectura: no existe ningún endpoint de órdenes en este crate.

pub mod allowance;
pub mod config;
pub mod credentials;
pub mod live;
pub mod loads;
pub mod model;
pub mod provider;
pub mod sentiment;
pub mod transport;

pub use allowance::{AllowanceObservation, AllowanceSummary, summarize};
pub use config::{DEMO_BASE_URL, IgConfig, IgConfigError, LiveConfig, LoadLimits};
pub use credentials::{Credentials, MissingEnv, Secret, api_key_from_env};
pub use loads::{
    DATA_MACHINE_ENV, LoadDecision, LoadFlags, decide, estimate_points, is_data_machine,
    parse_flags,
};
pub use model::{DailyHistory, MarketTerms, median};
pub use provider::{IgProvider, Recent, epics_from_catalog};
pub use transport::{HttpRequest, HttpResponse, Method, ReqwestTransport, Transport};

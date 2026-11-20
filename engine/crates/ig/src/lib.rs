//! Proveedor de datos de IG, cuenta **demo** (#66, ADR 0002).
//!
//! - [`IgProvider`] implementa [`cfds_market_data::MarketDataProvider`]: histórico de velas
//!   de 15m y 1h con precio medio exacto y en UTC. Las velas entran al sistema solo por el
//!   circuito de ingesta (`cfds-ingest`).
//! - Respeta los límites de IG (`config/ig.toml`): pedidos por minuto, cupo semanal de
//!   históricos y reintentos con backoff.
//! - Credenciales solo por variables de entorno; nunca aparecen en `Debug` ni en errores.
//! - Solo lectura: no existe ningún endpoint de órdenes en este crate.

pub mod config;
pub mod credentials;
pub mod model;
pub mod provider;
pub mod transport;

pub use config::{DEMO_BASE_URL, IgConfig, IgConfigError};
pub use credentials::{Credentials, MissingEnv, Secret, api_key_from_env};
pub use provider::{IgProvider, epics_from_catalog};
pub use transport::{HttpRequest, HttpResponse, Method, ReqwestTransport, Transport};

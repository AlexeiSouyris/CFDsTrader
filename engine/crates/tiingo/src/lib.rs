//! Tiingo, plan gratis, solo para **verificación transitoria** (ADR 0009, opción B; #206).
//!
//! Compara en memoria las variaciones diarias de nuestras velas (US500, US100, US30) con las de un
//! ETF que sigue el mismo índice (SPY, QQQ, DIA) y guarda **solo el veredicto**: coincide, no
//! coincide o sin dato. Los términos del plan prohíben guardar datos de Tiingo y sus productos
//! derivados (§1.6(a) y (c)): ningún precio ni número de Tiingo se escribe en la base, en archivos,
//! en caché ni en logs.
//!
//! - [`TiingoConfig`]: `config/tiingo.toml`, validado al arrancar.
//! - [`Usage`] y [`check_budget`]: topes propios con ventanas móviles (60 min, 24 h y 31 días).
//! - [`Client`]: el pedido HTTP, con el token en el encabezado y un tope de bytes.
//! - [`compare`]: la comparación, función pura.
//! - [`TiingoStore`]: registro de uso y veredictos en PostgreSQL (tablas de solo agregar).
//!
//! Kill switch: cualquier [`TiingoError`] corta la corrida sin reintentar.

mod budget;
mod client;
mod config;
mod eod;
mod store;
mod verify;

pub use budget::{Usage, check_budget};
pub use client::{
    ApiKey, Client, Fetched, HttpTransport, ReqwestTransport, TOKEN_ENV, TransportError,
};
pub use config::{Budget, TiingoConfig, VerifyConfig};
pub use eod::{EodRow, parse_eod};
pub use store::{TiingoStore, UsageRecord};
pub use verify::{DayVerdict, Method, OurClose, Verdict, compare, our_closes};

#[derive(Debug, thiserror::Error)]
pub enum TiingoError {
    #[error("config/tiingo.toml: {0}")]
    Config(String),
    #[error("kill switch: {0}")]
    Budget(String),
    #[error("kill switch: Tiingo respondió {status} a {symbol}{hint}")]
    Http {
        symbol: String,
        status: u16,
        hint: &'static str,
    },
    #[error(
        "kill switch: la respuesta de Tiingo para {symbol} no tiene la forma esperada: {reason}"
    )]
    Shape { symbol: String, reason: String },
    #[error("kill switch: la respuesta de Tiingo para {symbol} pasa de {max} bytes")]
    TooLarge { symbol: String, max: u64 },
    #[error("kill switch: error de red con Tiingo ({0})")]
    Network(String),
    #[error("falta {0} en el entorno (.env)")]
    MissingToken(&'static str),
    #[error("PostgreSQL: {0}")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Storage(#[from] cfds_storage::StorageError),
}

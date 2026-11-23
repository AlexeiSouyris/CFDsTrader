//! El pedido HTTP a Tiingo (ADR 0009 §3).
//!
//! - El token va en el encabezado `Authorization: Token …` (documentación oficial), nunca en la
//!   URL: así no aparece en mensajes de error ni en logs. `ApiKey` no se puede imprimir.
//! - Antes de pedir, controla los topes con el registro de uso; después, registra el pedido
//!   (fecha, símbolo, código HTTP y bytes) aunque haya fallado. Nunca el cuerpo.
//! - Sin caché en disco ni redirecciones, y con un tope de bytes por respuesta.
//! - Cualquier problema es un [`TiingoError`]: kill switch, sin reintentos.

use std::fmt;
use std::future::Future;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};

use crate::TiingoError;
use crate::budget::check_budget;
use crate::config::{TiingoConfig, valid_symbol};
use crate::eod::{EodRow, parse_eod};
use crate::store::{TiingoStore, UsageRecord};

pub const TOKEN_ENV: &str = "TIINGO_API_KEY";
const ENDPOINT: &str = "daily_prices";

/// Token de Tiingo. `Debug` y `Display` no lo muestran.
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(token: String) -> Result<Self, TiingoError> {
        if token.trim().is_empty() {
            return Err(TiingoError::MissingToken(TOKEN_ENV));
        }
        Ok(Self(token))
    }

    pub fn from_env() -> Result<Self, TiingoError> {
        Self::new(std::env::var(TOKEN_ENV).map_err(|_| TiingoError::MissingToken(TOKEN_ENV))?)
    }

    pub(crate) fn header(&self) -> String {
        format!("Token {}", self.0)
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(***)")
    }
}

/// Lo que volvió de un pedido. `body` vive solo en memoria.
pub struct Fetched {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Fallo de transporte, con los bytes que llegaron a leerse (para el registro de uso).
#[derive(Debug)]
pub enum TransportError {
    Network(String),
    TooLarge { bytes: u64 },
}

/// GET con el token en el encabezado. Se reemplaza en los tests (sin red).
pub trait HttpTransport {
    fn get(
        &self,
        url: &str,
        key: &ApiKey,
        max_bytes: u64,
    ) -> impl Future<Output = Result<Fetched, TransportError>> + Send;
}

pub struct ReqwestTransport {
    http: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new(timeout_secs: u64) -> Result<Self, TiingoError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| TiingoError::Network(e.to_string()))?;
        Ok(Self { http })
    }
}

impl HttpTransport for ReqwestTransport {
    async fn get(
        &self,
        url: &str,
        key: &ApiKey,
        max_bytes: u64,
    ) -> Result<Fetched, TransportError> {
        let mut resp = self
            .http
            .get(url)
            .header("Authorization", key.header())
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| TransportError::Network(e.without_url().to_string()))?;
        let status = resp.status().as_u16();
        let mut body = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| TransportError::Network(e.without_url().to_string()))?
        {
            body.extend_from_slice(&chunk);
            if body.len() as u64 > max_bytes {
                return Err(TransportError::TooLarge {
                    bytes: body.len() as u64,
                });
            }
        }
        Ok(Fetched { status, body })
    }
}

pub struct Client<T> {
    cfg: TiingoConfig,
    key: ApiKey,
    transport: T,
}

impl<T: HttpTransport> Client<T> {
    pub fn new(cfg: TiingoConfig, key: ApiKey, transport: T) -> Self {
        Self {
            cfg,
            key,
            transport,
        }
    }

    pub fn config(&self) -> &TiingoConfig {
        &self.cfg
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Diarias de `symbol` entre `from` y `to` (inclusive), en memoria. Controla los topes antes
    /// y registra el uso después, pase lo que pase.
    pub async fn daily_prices(
        &self,
        store: &TiingoStore,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<Vec<EodRow>, TiingoError> {
        if !valid_symbol(symbol) {
            return Err(TiingoError::Config(format!("símbolo inválido: {symbol:?}")));
        }
        let max = self.cfg.max_bytes_per_request;
        check_budget(&store.usage(now).await?, symbol, max, &self.cfg.budget)?;
        let url = format!(
            "{}/tiingo/daily/{symbol}/prices?startDate={from}&endDate={to}",
            self.cfg.base_url
        );
        let got = self.transport.get(&url, &self.key, max).await;
        let (http_status, bytes) = match &got {
            Ok(f) => (Some(f.status), f.body.len() as u64),
            Err(TransportError::TooLarge { bytes }) => (None, *bytes),
            Err(TransportError::Network(_)) => (None, 0),
        };
        store
            .record_usage(&UsageRecord {
                requested_at: now,
                endpoint: ENDPOINT,
                symbol: symbol.to_owned(),
                http_status,
                bytes,
            })
            .await?;
        let fetched = match got {
            Ok(f) => f,
            Err(TransportError::Network(e)) => return Err(TiingoError::Network(e)),
            Err(TransportError::TooLarge { .. }) => {
                return Err(TiingoError::TooLarge {
                    symbol: symbol.to_owned(),
                    max,
                });
            }
        };
        if fetched.status != 200 {
            let hint = match fetched.status {
                401 | 403 => " (token inválido o sin permiso)",
                404 => " (símbolo no encontrado)",
                429 => " (límite de pedidos de Tiingo)",
                _ => "",
            };
            return Err(TiingoError::Http {
                symbol: symbol.to_owned(),
                status: fetched.status,
                hint,
            });
        }
        parse_eod(symbol, &fetched.body)
    }
}

//! HTTP detrás de un trait: en producción `reqwest`, en los tests respuestas grabadas.

use std::future::Future;

/// Un pedido a la API, relativo a `base_url` (por ejemplo `/prices/X?resolution=HOUR`).
#[derive(Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    /// Versión del endpoint (header `Version`).
    pub version: u8,
    /// Headers extra (tokens de sesión). Nunca se muestran: ver el `Debug` de abajo.
    pub headers: Vec<(&'static str, String)>,
    pub body: Option<String>,
}

impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Sin headers ni body: llevan tokens y la contraseña.
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Delete,
}

#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    /// Nombres en minúscula.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

impl std::fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Los headers de la sesión traen los tokens y el body del login trae datos de la cuenta.
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

pub trait Transport: Send + Sync {
    /// Error de red o de transporte (no un status HTTP): se puede reintentar.
    fn send(&self, req: HttpRequest) -> impl Future<Output = Result<HttpResponse, String>> + Send;
}

/// Transporte real. Usa el proxy de `HTTPS_PROXY` y los certificados del sistema.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<crate::Secret>,
}

impl ReqwestTransport {
    pub fn new(cfg: &crate::IgConfig, api_key: Option<crate::Secret>) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(cfg.request_timeout)
            .build()
            .map_err(|e| format!("no se pudo crear el cliente HTTP: {e}"))?;
        Ok(Self {
            client,
            base_url: cfg.base_url.clone(),
            api_key,
        })
    }
}

impl Transport for ReqwestTransport {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
        let url = format!("{}{}", self.base_url, req.path);
        let method = match req.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut b = self
            .client
            .request(method, &url)
            .header("Accept", "application/json; charset=UTF-8")
            .header("Content-Type", "application/json")
            .header("Version", req.version.to_string());
        if let Some(key) = &self.api_key {
            b = b.header("X-IG-API-KEY", key.expose());
        }
        for (k, v) in req.headers {
            b = b.header(k, v);
        }
        if let Some(body) = req.body {
            b = b.body(body);
        }
        // `without_url`: el error no repite la URL (no tiene secretos, pero así queda corto).
        let resp = b.send().await.map_err(|e| e.without_url().to_string())?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
            .collect();
        let body = resp.text().await.map_err(|e| e.without_url().to_string())?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

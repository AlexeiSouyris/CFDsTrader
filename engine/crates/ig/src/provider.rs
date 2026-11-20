//! [`IgProvider`]: histórico de velas de IG detrás de [`MarketDataProvider`].
//!
//! Endpoints (labs.ig.com/reference, consultado el 2026-11-20):
//! - `POST /session` v2: tokens `CST` y `X-SECURITY-TOKEN` en los headers; `timezoneOffset`.
//! - `GET /prices/{epic}` v3: `resolution`, `from`, `to`, `pageSize`, `pageNumber`.
//! - `DELETE /session` v1.
//!
//! Solo lectura: no hay ningún endpoint de órdenes (CLAUDE.md §5).

use std::collections::{BTreeMap, VecDeque};

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{Catalog, MarketDataProvider, ProviderError, Sourced};
use chrono::{DateTime, TimeDelta, Utc};
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant, sleep, sleep_until};

use crate::model::{self, AllowanceBody, PricesBody, SessionBody};
use crate::transport::{HttpRequest, HttpResponse, Method, Transport};
use crate::{Credentials, IgConfig, Secret};

const MINUTE: Duration = Duration::from_secs(60);

/// Códigos 403 de límite **por minuto**: se reintentan con backoff.
const PER_MINUTE_LIMITS: [&str; 2] = [
    "error.public-api.exceeded-account-allowance",
    "error.public-api.exceeded-api-key-allowance",
];
/// Límite **semanal** de históricos: no se reintenta.
const WEEKLY_HISTORY_LIMIT: &str = "error.public-api.exceeded-account-historical-data-allowance";

struct Session {
    cst: Secret,
    security_token: Secret,
    /// Hora local de la cuenta − UTC.
    offset: TimeDelta,
}

/// Cupo semanal de puntos históricos, según la última respuesta de IG.
#[derive(Debug, Clone, Copy)]
struct Allowance {
    remaining: u64,
    resets_at: Instant,
}

/// Estado compartido entre pedidos. Un solo `Mutex`: los pedidos van de a uno, que además
/// es lo que piden los límites de IG.
struct State {
    session: Option<Session>,
    sent: VecDeque<Instant>,
    allowance: Option<Allowance>,
}

pub struct IgProvider<T> {
    transport: T,
    cfg: IgConfig,
    credentials: Credentials,
    epics: BTreeMap<InstrumentId, String>,
    state: Mutex<State>,
}

/// Epics de `config/instruments.toml` que ya tienen valor con fuente.
pub fn epics_from_catalog(catalog: &Catalog) -> BTreeMap<InstrumentId, String> {
    catalog
        .instruments
        .iter()
        .filter_map(|(id, spec)| match &spec.terms.epic {
            Sourced::Known { value, .. } => Some((id.clone(), value.clone())),
            Sourced::Pending { .. } => None,
        })
        .collect()
}

/// Qué hacer con una respuesta.
enum Outcome {
    Done(HttpResponse),
    Retry(ProviderError),
    SessionExpired,
    Fail(ProviderError),
}

impl<T: Transport> IgProvider<T> {
    pub fn new(
        transport: T,
        cfg: IgConfig,
        credentials: Credentials,
        epics: BTreeMap<InstrumentId, String>,
    ) -> Self {
        Self {
            transport,
            cfg,
            credentials,
            epics,
            state: Mutex::new(State {
                session: None,
                sent: VecDeque::new(),
                allowance: None,
            }),
        }
    }

    /// Puntos que quedan del cupo semanal y en cuánto se renueva, si ya se conocen.
    pub async fn allowance(&self) -> Option<(u64, Duration)> {
        let st = self.state.lock().await;
        st.allowance.map(|a| {
            (
                a.remaining,
                a.resets_at.saturating_duration_since(Instant::now()),
            )
        })
    }

    /// Cierra la sesión en IG (`DELETE /session`). Sin sesión abierta no hace nada.
    pub async fn logout(&self) -> Result<(), ProviderError> {
        let mut st = self.state.lock().await;
        let Some(s) = st.session.take() else {
            return Ok(());
        };
        self.throttle(&mut st).await;
        let req = authed(Method::Delete, "/session".into(), 1, &s);
        let resp = self
            .transport
            .send(req)
            .await
            .map_err(ProviderError::Transport)?;
        match resp.status {
            200..=299 | 401 => Ok(()),
            s => Err(ProviderError::InvalidResponse(format!("logout: HTTP {s}"))),
        }
    }

    /// Respeta `requests_per_minute` con una ventana deslizante de 60 s.
    async fn throttle(&self, st: &mut State) {
        let limit = usize::try_from(self.cfg.requests_per_minute).unwrap_or(usize::MAX);
        loop {
            let now = Instant::now();
            while st
                .sent
                .front()
                .is_some_and(|t| now.duration_since(*t) >= MINUTE)
            {
                st.sent.pop_front();
            }
            match st.sent.front() {
                Some(oldest) if st.sent.len() >= limit => sleep_until(*oldest + MINUTE).await,
                _ => break,
            }
        }
        st.sent.push_back(Instant::now());
    }

    async fn login(&self, st: &mut State) -> Result<(), ProviderError> {
        let body = serde_json::json!({
            "identifier": self.credentials.username.expose(),
            "password": self.credentials.password.expose(),
        })
        .to_string();
        let req = HttpRequest {
            method: Method::Post,
            path: "/session".into(),
            version: 2,
            headers: Vec::new(),
            body: Some(body),
        };
        let mut attempt = 1;
        let resp = loop {
            self.throttle(st).await;
            // Un login rechazado (401/403 de credenciales) NUNCA se reintenta: IG bloquea la
            // cuenta tras varios intentos fallidos (error.security.too-many-failed-attempts).
            match classify(self.transport.send(req.clone()).await, None) {
                Outcome::Done(r) => break r,
                Outcome::SessionExpired => return Err(ProviderError::Unauthorized),
                Outcome::Fail(e) => return Err(e),
                Outcome::Retry(e) if attempt >= self.cfg.max_attempts => return Err(e),
                Outcome::Retry(_) => {
                    sleep(self.cfg.backoff(attempt)).await;
                    attempt += 1;
                }
            }
        };
        let token = |name: &str| {
            resp.header(name)
                .map(Secret::new)
                .ok_or_else(|| ProviderError::InvalidResponse(format!("login sin header {name}")))
        };
        let cst = token("cst")?;
        let security_token = token("x-security-token")?;
        let offset = serde_json::from_str::<SessionBody>(&resp.body)
            .map_err(|_| ProviderError::InvalidResponse("login: cuerpo inesperado".into()))?
            .timezone_offset()
            .map_err(ProviderError::InvalidResponse)?;
        st.session = Some(Session {
            cst,
            security_token,
            offset,
        });
        Ok(())
    }

    /// GET autenticado con reintentos. `path` recibe el offset horario de la sesión vigente,
    /// porque un login nuevo puede cambiarlo.
    async fn get(
        &self,
        st: &mut State,
        path: impl Fn(TimeDelta) -> String,
        version: u8,
        not_found: &str,
    ) -> Result<HttpResponse, ProviderError> {
        let mut attempt = 1;
        let mut relogged = false;
        loop {
            if st.session.is_none() {
                self.login(st).await?;
            }
            let session = st.session.as_ref().ok_or(ProviderError::Unauthorized)?;
            let req = authed(Method::Get, path(session.offset), version, session);
            self.throttle(st).await;
            match classify(self.transport.send(req).await, Some(not_found)) {
                Outcome::Done(r) => return Ok(r),
                Outcome::Fail(ProviderError::RateLimited { .. }) => {
                    // Límite semanal: se informa cuándo se renueva, si se sabe.
                    let retry_after_secs = st.allowance.map(|a| {
                        a.resets_at
                            .saturating_duration_since(Instant::now())
                            .as_secs()
                    });
                    return Err(ProviderError::RateLimited { retry_after_secs });
                }
                Outcome::Fail(e) => return Err(e),
                // Sesión vencida: un solo login nuevo por pedido.
                Outcome::SessionExpired if !relogged => {
                    st.session = None;
                    relogged = true;
                }
                Outcome::SessionExpired => return Err(ProviderError::Unauthorized),
                Outcome::Retry(e) if attempt >= self.cfg.max_attempts => return Err(e),
                Outcome::Retry(_) => {
                    sleep(self.cfg.backoff(attempt)).await;
                    attempt += 1;
                }
            }
        }
    }

    fn update_allowance(st: &mut State, a: AllowanceBody) {
        st.allowance = Some(Allowance {
            remaining: a.remaining,
            resets_at: Instant::now() + Duration::from_secs(a.expiry_secs),
        });
    }
}

fn authed(method: Method, path: String, version: u8, s: &Session) -> HttpRequest {
    HttpRequest {
        method,
        path,
        version,
        headers: vec![
            ("CST", s.cst.expose().to_owned()),
            ("X-SECURITY-TOKEN", s.security_token.expose().to_owned()),
        ],
        body: None,
    }
}

/// Clasifica una respuesta. Los errores llevan solo el status y el `errorCode` de IG, nunca el
/// cuerpo (el del login trae datos de la cuenta).
fn classify(sent: Result<HttpResponse, String>, not_found: Option<&str>) -> Outcome {
    let resp = match sent {
        Ok(r) => r,
        Err(e) => return Outcome::Retry(ProviderError::Transport(e)),
    };
    let code = || model::error_code(&resp.body).unwrap_or_default();
    match resp.status {
        200..=299 => Outcome::Done(resp),
        401 => Outcome::SessionExpired,
        403 => {
            let c = code();
            if PER_MINUTE_LIMITS.contains(&c.as_str()) {
                Outcome::Retry(ProviderError::RateLimited {
                    retry_after_secs: Some(60),
                })
            } else if c == WEEKLY_HISTORY_LIMIT {
                Outcome::Fail(ProviderError::RateLimited {
                    retry_after_secs: None,
                })
            } else {
                Outcome::Fail(ProviderError::Unauthorized)
            }
        }
        // IG responde 400 error.unsupported.epic y 404 ...epic.unavailable a un epic que no existe.
        400 | 404
            if not_found.is_some()
                && (resp.status == 404 || code() == "error.unsupported.epic") =>
        {
            Outcome::Fail(ProviderError::NotFound(
                not_found.unwrap_or_default().to_owned(),
            ))
        }
        500..=599 => Outcome::Retry(ProviderError::Transport(format!(
            "HTTP {} {}",
            resp.status,
            code()
        ))),
        s => Outcome::Fail(ProviderError::InvalidResponse(format!(
            "HTTP {s} {}",
            code()
        ))),
    }
}

impl<T: Transport> MarketDataProvider for IgProvider<T> {
    async fn history(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, ProviderError> {
        let epic = self.epics.get(instrument).ok_or_else(|| {
            ProviderError::NotFound(format!("{instrument}: sin epic en config/instruments.toml"))
        })?;
        if from >= to {
            return Ok(Vec::new());
        }
        let margin = TimeDelta::hours(i64::from(self.cfg.local_time_margin_hours));
        let needed = model::estimated_points(from, to, timeframe, margin);
        let mut st = self.state.lock().await;
        // Si el pedido no entra en el cupo semanal, ni se hace: gastaría el cupo a medias.
        let remaining = match st.allowance {
            Some(a) if Instant::now() < a.resets_at => a.remaining,
            _ => self.cfg.historical_points_per_week,
        };
        if needed > remaining {
            let retry_after_secs = st.allowance.map(|a| {
                a.resets_at
                    .saturating_duration_since(Instant::now())
                    .as_secs()
            });
            return Err(ProviderError::RateLimited { retry_after_secs });
        }

        let mut candles = Vec::new();
        let mut page = 1u32;
        loop {
            let path = |offset: TimeDelta| {
                let (f, t) = model::local_query_range(from, to, offset, margin);
                format!(
                    "/prices/{epic}?resolution={}&from={f}&to={t}&pageSize={}&pageNumber={page}",
                    model::resolution(timeframe),
                    self.cfg.page_size
                )
            };
            let resp = self.get(&mut st, path, 3, epic).await?;
            let body: PricesBody = serde_json::from_str(&resp.body)
                .map_err(|e| ProviderError::InvalidResponse(format!("GET /prices: {e}")))?;
            Self::update_allowance(&mut st, body.metadata.allowance);
            candles.extend(
                model::to_candles(&body.prices, instrument, timeframe, from, to)
                    .map_err(ProviderError::InvalidResponse)?,
            );
            let pages = &body.metadata.page_data;
            if pages.page_number != page {
                return Err(ProviderError::InvalidResponse(format!(
                    "GET /prices: pedí la página {page} y llegó la {}",
                    pages.page_number
                )));
            }
            if pages.page_number >= pages.total_pages {
                return Ok(candles);
            }
            page = pages.page_number + 1;
        }
    }
}

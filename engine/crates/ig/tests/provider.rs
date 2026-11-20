//! `IgProvider` con respuestas grabadas de la demo (tests/fixtures): sin red.
//! El tiempo de tokio está pausado: los backoffs y la ventana por minuto no esperan de verdad.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cfds_core::{InstrumentId, Timeframe};
use cfds_ig::{
    Credentials, HttpRequest, HttpResponse, IgConfig, IgProvider, Method, Secret, Transport,
};
use cfds_ingest::{IngestRequest, prepare};
use cfds_market_data::{ExchangeCalendar, MarketDataProvider, ProviderError, ValidationConfig};
use chrono::{DateTime, Utc};
use rust_decimal::dec;
use tokio::time::Instant;

const CONFIG: &str = include_str!("../../../../config/ig.toml");
const SESSION: &str = include_str!("fixtures/session_v2.json");
const DE40_15M: &str = include_str!("fixtures/prices_de40_15m.json");
const US100_1H: [&str; 3] = [
    include_str!("fixtures/prices_us100_1h_p1.json"),
    include_str!("fixtures/prices_us100_1h_p2.json"),
    include_str!("fixtures/prices_us100_1h_p3.json"),
];

const FAKE_CST: &str = "cst-falso-123";
const FAKE_XST: &str = "xst-falso-456";
const FAKE_USER: &str = "usuario-falso";
const FAKE_PASSWORD: &str = "clave-falsa";

/// Transporte de prueba: devuelve respuestas en orden y guarda lo que se pidió.
#[derive(Clone, Default)]
struct Recorded {
    replies: Arc<Mutex<VecDeque<Result<HttpResponse, String>>>>,
    sent: Arc<Mutex<Vec<(HttpRequest, Instant)>>>,
}

impl Recorded {
    fn reply(self, r: Result<HttpResponse, String>) -> Self {
        self.replies.lock().unwrap().push_back(r);
        self
    }
    fn ok(self, body: &str) -> Self {
        self.reply(Ok(resp(200, body)))
    }
    fn login(self) -> Self {
        self.reply(Ok(HttpResponse {
            status: 200,
            headers: vec![
                ("cst".into(), FAKE_CST.into()),
                ("x-security-token".into(), FAKE_XST.into()),
            ],
            body: SESSION.into(),
        }))
    }
    fn error(self, status: u16, code: &str) -> Self {
        self.reply(Ok(resp(status, &format!(r#"{{"errorCode":"{code}"}}"#))))
    }
    fn sent(&self) -> Vec<HttpRequest> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .map(|(r, _)| r.clone())
            .collect()
    }
    fn times(&self) -> Vec<Instant> {
        self.sent.lock().unwrap().iter().map(|(_, t)| *t).collect()
    }
    fn pending(&self) -> usize {
        self.replies.lock().unwrap().len()
    }
}

impl Transport for Recorded {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
        self.sent.lock().unwrap().push((req, Instant::now()));
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("el test no preparó una respuesta para este pedido")
    }
}

fn resp(status: u16, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        headers: Vec::new(),
        body: body.into(),
    }
}

fn cfg() -> IgConfig {
    IgConfig::from_toml(CONFIG).unwrap()
}

fn de40() -> InstrumentId {
    "DE40".parse().unwrap()
}
fn us100() -> InstrumentId {
    "US100".parse().unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn provider(t: &Recorded, cfg: IgConfig) -> IgProvider<Recorded> {
    let creds = Credentials {
        username: Secret::new(FAKE_USER),
        password: Secret::new(FAKE_PASSWORD),
    };
    let epics = BTreeMap::from([
        (de40(), "IX.D.DAX.IFE.IP".to_string()),
        (us100(), "IX.D.NASDAQ.IFE.IP".to_string()),
    ]);
    IgProvider::new(t.clone(), cfg, creds, epics)
}

async fn de40_hour(ig: &IgProvider<Recorded>) -> Result<Vec<cfds_core::Candle>, ProviderError> {
    ig.history(
        &de40(),
        Timeframe::M15,
        at("2026-11-20T09:00:00Z"),
        at("2026-11-20T10:00:00Z"),
    )
    .await
}

fn header<'a>(r: &'a HttpRequest, name: &str) -> Option<&'a str> {
    r.headers
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.as_str())
}

#[tokio::test(start_paused = true)]
async fn recorded_15m_candles_become_exact_mid_prices_in_utc() {
    let t = Recorded::default().login().ok(DE40_15M);
    let candles = de40_hour(&provider(&t, cfg())).await.unwrap();

    // IG devolvió 5 velas (to inclusivo); solo quedan las de [09:00, 10:00) UTC.
    let starts: Vec<_> = candles.iter().map(|c| c.start).collect();
    assert_eq!(
        starts,
        [
            at("2026-11-20T09:00:00Z"),
            at("2026-11-20T09:15:00Z"),
            at("2026-11-20T09:30:00Z"),
            at("2026-11-20T09:45:00Z"),
        ]
    );
    let c = &candles[0];
    // (bid + ask) / 2 exacto: open 25060.5/25061.9, high 25079.0/25080.4, low 25005.2/25006.6,
    // close 25058.4/25059.8.
    assert_eq!(
        (c.open, c.high, c.low, c.close),
        (dec!(25061.2), dec!(25079.7), dec!(25005.9), dec!(25059.1))
    );
    assert_eq!(c.instrument_id, de40());
    assert_eq!(c.timeframe, Timeframe::M15);
    assert_eq!(
        c.volume, None,
        "lastTradedVolume de un CFD no es volumen de mercado"
    );
}

#[tokio::test(start_paused = true)]
async fn login_then_prices_request_in_account_local_time_with_margin() {
    let t = Recorded::default().login().ok(DE40_15M);
    de40_hour(&provider(&t, cfg())).await.unwrap();

    let sent = t.sent();
    assert_eq!(sent.len(), 2);
    let (login, prices) = (&sent[0], &sent[1]);
    assert_eq!(
        (login.method, login.path.as_str(), login.version),
        (Method::Post, "/session", 2)
    );
    let body = login.body.as_deref().unwrap();
    assert!(body.contains(FAKE_USER) && body.contains(FAKE_PASSWORD));

    // UTC 09:00–10:00, cuenta en UTC+1 (timezoneOffset = 1), margen de 1 h a cada lado.
    assert_eq!(prices.method, Method::Get);
    assert_eq!(prices.version, 3);
    assert_eq!(
        prices.path,
        "/prices/IX.D.DAX.IFE.IP?resolution=MINUTE_15&from=2026-11-20T09:00:00\
         &to=2026-11-20T12:00:00&pageSize=500&pageNumber=1"
    );
    assert_eq!(header(prices, "CST"), Some(FAKE_CST));
    assert_eq!(header(prices, "X-SECURITY-TOKEN"), Some(FAKE_XST));
    assert_eq!(header(login, "CST"), None);
}

#[tokio::test(start_paused = true)]
async fn hourly_history_follows_every_page_and_reuses_the_session() {
    let t = Recorded::default()
        .login()
        .ok(US100_1H[0])
        .ok(US100_1H[1])
        .ok(US100_1H[2]);
    let mut c = cfg();
    c.page_size = 2;
    let ig = provider(&t, c);
    let candles = ig
        .history(
            &us100(),
            Timeframe::H1,
            at("2026-11-19T14:00:00Z"),
            at("2026-11-19T18:00:00Z"),
        )
        .await
        .unwrap();

    let hours: Vec<_> = candles
        .iter()
        .map(|c| c.start.format("%H").to_string())
        .collect();
    assert_eq!(
        hours,
        ["14", "15", "16", "17"],
        "la vela de las 18:00 queda fuera de [from, to)"
    );
    let pages: Vec<_> = t.sent()[1..].iter().map(|r| r.path.clone()).collect();
    assert_eq!(pages.len(), 3, "un solo login para las tres páginas");
    for (i, p) in pages.iter().enumerate() {
        assert!(
            p.ends_with(&format!("pageSize=2&pageNumber={}", i + 1)),
            "{p}"
        );
    }
    // El cupo se toma de la última respuesta: 9990 puntos, renovación en 604631 s.
    let (left, resets) = ig.allowance().await.unwrap();
    assert_eq!(left, 9990);
    assert_eq!(resets.as_secs(), 604_631);
}

#[tokio::test(start_paused = true)]
async fn expired_session_logs_in_again_once() {
    let t = Recorded::default()
        .login()
        .error(401, "error.security.invalid-session-token")
        .login()
        .ok(DE40_15M);
    assert_eq!(de40_hour(&provider(&t, cfg())).await.unwrap().len(), 4);
    let paths: Vec<_> = t.sent().iter().map(|r| r.path.clone()).collect();
    assert_eq!(paths[0], "/session");
    assert_eq!(paths[2], "/session");

    // Si después del login nuevo vuelve a dar 401, no se insiste.
    let t = Recorded::default()
        .login()
        .error(401, "error.security.invalid-session-token")
        .login()
        .error(401, "error.security.invalid-session-token");
    let err = de40_hour(&provider(&t, cfg())).await.unwrap_err();
    assert_eq!(err, ProviderError::Unauthorized);
    assert_eq!(t.pending(), 0);
}

#[tokio::test(start_paused = true)]
async fn rejected_login_is_never_retried() {
    // IG bloquea la cuenta tras varios logins fallidos: un 401 en el login corta en seco.
    let t = Recorded::default().error(401, "error.security.invalid-details");
    let err = de40_hour(&provider(&t, cfg())).await.unwrap_err();
    assert_eq!(err, ProviderError::Unauthorized);
    assert_eq!(t.sent().len(), 1);

    let t = Recorded::default().error(403, "error.security.too-many-failed-attempts");
    assert_eq!(
        de40_hour(&provider(&t, cfg())).await.unwrap_err(),
        ProviderError::Unauthorized
    );
    assert_eq!(t.sent().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn server_errors_and_network_failures_are_retried_with_growing_backoff() {
    let t = Recorded::default()
        .login()
        .error(503, "system.error")
        .reply(Err("conexión reiniciada".into()))
        .ok(DE40_15M);
    assert_eq!(de40_hour(&provider(&t, cfg())).await.unwrap().len(), 4);

    // Esperas entre los intentos de /prices: 2 s y 4 s (config/ig.toml: inicial 2, ×2).
    let times = t.times();
    assert_eq!((times[2] - times[1]).as_secs(), 2);
    assert_eq!((times[3] - times[2]).as_secs(), 4);
}

#[tokio::test(start_paused = true)]
async fn retries_stop_at_max_attempts() {
    let mut c = cfg();
    c.max_attempts = 3;
    let t = Recorded::default()
        .login()
        .error(500, "system.error")
        .error(502, "")
        .error(503, "");
    let err = de40_hour(&provider(&t, c)).await.unwrap_err();
    assert!(
        matches!(err, ProviderError::Transport(ref m) if m.contains("503")),
        "{err:?}"
    );
    assert_eq!(t.sent().len(), 4, "1 login + 3 intentos");
}

#[tokio::test(start_paused = true)]
async fn login_server_errors_are_retried_up_to_max_attempts() {
    let mut c = cfg();
    c.max_attempts = 2;
    let t = Recorded::default()
        .error(503, "system.error")
        .error(503, "system.error");
    let err = de40_hour(&provider(&t, c.clone())).await.unwrap_err();
    assert!(matches!(err, ProviderError::Transport(_)), "{err:?}");
    assert_eq!(t.sent().len(), 2);

    let t = Recorded::default()
        .error(503, "system.error")
        .login()
        .ok(DE40_15M);
    assert_eq!(de40_hour(&provider(&t, c)).await.unwrap().len(), 4);
}

#[tokio::test(start_paused = true)]
async fn per_minute_limit_is_retried_but_weekly_history_limit_is_not() {
    let t = Recorded::default()
        .login()
        .error(403, "error.public-api.exceeded-account-allowance")
        .error(403, "error.public-api.exceeded-api-key-allowance")
        .ok(DE40_15M);
    assert_eq!(de40_hour(&provider(&t, cfg())).await.unwrap().len(), 4);

    let t = Recorded::default().login().error(
        403,
        "error.public-api.exceeded-account-historical-data-allowance",
    );
    let err = de40_hour(&provider(&t, cfg())).await.unwrap_err();
    assert_eq!(
        err,
        ProviderError::RateLimited {
            retry_after_secs: None
        }
    );
    assert_eq!(t.sent().len(), 2, "sin reintentos");
}

#[tokio::test(start_paused = true)]
async fn weekly_limit_reports_when_the_allowance_resets() {
    let t = Recorded::default().login().ok(DE40_15M).error(
        403,
        "error.public-api.exceeded-account-historical-data-allowance",
    );
    let ig = provider(&t, cfg());
    de40_hour(&ig).await.unwrap();
    let err = de40_hour(&ig).await.unwrap_err();
    // allowanceExpiry de la respuesta grabada.
    let expiry = 604_799;
    assert!(
        matches!(err, ProviderError::RateLimited { retry_after_secs: Some(s) } if s <= expiry && s + 5 >= expiry),
        "{err:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_request_that_does_not_fit_the_weekly_allowance_is_not_sent() {
    let t = Recorded::default().login().ok(DE40_15M);
    let ig = provider(&t, cfg());
    de40_hour(&ig).await.unwrap(); // deja el cupo en 9995 puntos

    // 105 días de velas de 15m ≈ 10.080 puntos: no entra.
    let err = ig
        .history(
            &de40(),
            Timeframe::M15,
            at("2026-06-01T00:00:00Z"),
            at("2026-09-14T00:00:00Z"),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ProviderError::RateLimited {
            retry_after_secs: Some(_)
        }
    ));
    assert_eq!(t.sent().len(), 2, "no se mandó ningún pedido nuevo");

    // Sin datos del cupo todavía, el tope es el semanal de config (10.000 puntos).
    let t = Recorded::default();
    let err = provider(&t, cfg())
        .history(
            &de40(),
            Timeframe::M15,
            at("2026-06-01T00:00:00Z"),
            at("2026-09-14T00:00:00Z"),
        )
        .await
        .unwrap_err();
    assert_eq!(
        err,
        ProviderError::RateLimited {
            retry_after_secs: None
        }
    );
    assert!(t.sent().is_empty());
}

#[tokio::test(start_paused = true)]
async fn requests_per_minute_are_capped() {
    let mut c = cfg();
    c.requests_per_minute = 2;
    let t = Recorded::default().login().ok(DE40_15M).ok(DE40_15M);
    let ig = provider(&t, c);
    let start = Instant::now();
    de40_hour(&ig).await.unwrap();
    de40_hour(&ig).await.unwrap();

    let times = t.times();
    // Login + primer pedido en el primer minuto; el segundo pedido espera a que se libere uno.
    assert_eq!(times[1] - start, std::time::Duration::ZERO);
    assert!(times[2] - times[0] >= std::time::Duration::from_secs(60));
}

#[tokio::test(start_paused = true)]
async fn unknown_instruments_and_epics_are_not_found() {
    let t = Recorded::default();
    let err = provider(&t, cfg())
        .history(
            &"US500".parse().unwrap(),
            Timeframe::H1,
            at("2026-11-20T09:00:00Z"),
            at("2026-11-20T10:00:00Z"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ProviderError::NotFound(ref m) if m.contains("US500")));
    assert!(t.sent().is_empty());

    // Epic inexistente: IG responde 400 error.unsupported.epic (medido en la demo).
    let t = Recorded::default()
        .login()
        .error(400, "error.unsupported.epic");
    let err = de40_hour(&provider(&t, cfg())).await.unwrap_err();
    assert_eq!(err, ProviderError::NotFound("IX.D.DAX.IFE.IP".into()));
}

#[tokio::test(start_paused = true)]
async fn broken_responses_are_errors_not_guesses() {
    let missing_ask = DE40_15M.replacen("\"ask\": 25061.9", "\"ask\": null", 1);
    let misaligned = DE40_15M.replacen("2026-11-20T09:15:00", "2026-11-20T09:16:00", 1);
    let wrong_page = DE40_15M.replacen("\"pageNumber\": 1", "\"pageNumber\": 2", 1);
    for (body, expected) in [
        (missing_ask.as_str(), "sin bid o ask"),
        (misaligned.as_str(), "no alineada"),
        (wrong_page.as_str(), "página"),
        ("{\"prices\": []}", "GET /prices"),
    ] {
        assert_ne!(body, DE40_15M);
        let t = Recorded::default().login().ok(body);
        let err = de40_hour(&provider(&t, cfg())).await.unwrap_err();
        assert!(
            matches!(err, ProviderError::InvalidResponse(ref m) if m.contains(expected)),
            "{expected}: {err:?}"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn no_secret_appears_in_debug_or_errors() {
    let t = Recorded::default().login().error(418, "algo.raro");
    let ig = provider(&t, cfg());
    let err = de40_hour(&ig).await.unwrap_err();
    let mut shown = format!("{err} {err:?}");
    for r in t.sent() {
        shown.push_str(&format!("{r:?}"));
    }
    shown.push_str(&format!(
        "{:?}",
        Credentials {
            username: Secret::new(FAKE_USER),
            password: Secret::new(FAKE_PASSWORD)
        }
    ));
    for secret in [FAKE_CST, FAKE_XST, FAKE_USER, FAKE_PASSWORD, "ABC12"] {
        assert!(!shown.contains(secret), "se filtró {secret}: {shown}");
    }
}

#[tokio::test(start_paused = true)]
async fn logout_closes_the_session_once() {
    let t = Recorded::default().login().ok(DE40_15M).ok("");
    let ig = provider(&t, cfg());
    ig.logout().await.unwrap(); // sin sesión: no hace nada
    assert!(t.sent().is_empty());
    de40_hour(&ig).await.unwrap();
    ig.logout().await.unwrap();
    ig.logout().await.unwrap();
    let last = t.sent().pop().unwrap();
    assert_eq!(
        (last.method, last.path.as_str()),
        (Method::Delete, "/session")
    );
    assert_eq!(header(&last, "CST"), Some(FAKE_CST));
    assert_eq!(t.sent().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn ig_candles_pass_the_ingest_circuit() {
    // Las velas de IG entran por `cfds-ingest` como las de cualquier proveedor: el viernes
    // 20-11-2026 de 09:00 a 10:00 UTC Xetra está abierto y la serie es válida.
    let t = Recorded::default().login().ok(DE40_15M);
    let req = IngestRequest {
        instrument: de40(),
        timeframe: Timeframe::M15,
        from: at("2026-11-20T09:00:00Z"),
        to: at("2026-11-20T10:00:00Z"),
    };
    let candles = provider(&t, cfg())
        .history(&req.instrument, req.timeframe, req.from, req.to)
        .await
        .unwrap();
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar =
        ExchangeCalendar::load(&config_dir, HashMap::from([(de40(), "XETR".to_string())])).unwrap();
    let validation =
        ValidationConfig::from_toml(include_str!("../../../../config/data_validation.toml"))
            .unwrap();
    let (ready, skipped_open) = prepare(
        &req,
        candles,
        &calendar,
        &validation,
        at("2026-11-20T12:00:00Z"),
    )
    .unwrap();
    assert_eq!((ready.len(), skipped_open), (4, 0));
}

const MARKET_US100: &str = include_str!("fixtures/market_us100_ife.json");

#[tokio::test(start_paused = true)]
async fn market_terms_come_from_the_recorded_market_details() {
    let t = Recorded::default().login().ok(MARKET_US100);
    let terms = provider(&t, cfg()).market_terms(&us100()).await.unwrap();
    assert_eq!(
        terms,
        cfds_ig::MarketTerms {
            epic: "IX.D.NASDAQ.IFE.IP".into(),
            name: "EEUU Tech 100 al contado (1€)".into(),
            instrument_type: "INDICES".into(),
            currency: "EUR".into(),
            value_per_point: dec!(1),
            min_deal_size: dec!(0.5),
            margin_pct: Some(dec!(5)),
        }
    );
    let req = &t.sent()[1];
    assert_eq!(
        (req.path.as_str(), req.version),
        ("/markets/IX.D.NASDAQ.IFE.IP", 3)
    );
    assert_eq!(header(req, "CST"), Some(FAKE_CST));
}

#[tokio::test(start_paused = true)]
async fn market_terms_reject_broken_details() {
    for (from, to, expected) in [
        ("\"isDefault\": true", "\"isDefault\": false", "isDefault"),
        (
            "\"valueOfOnePip\": \"1.00\"",
            "\"valueOfOnePip\": \"uno\"",
            "valueOfOnePip",
        ),
        ("\"minDealSize\"", "\"minDealSizeX\"", "GET /markets"),
    ] {
        let body = MARKET_US100.replacen(from, to, 1);
        assert_ne!(body, MARKET_US100, "{from}");
        let t = Recorded::default().login().ok(&body);
        let err = provider(&t, cfg())
            .market_terms(&us100())
            .await
            .unwrap_err();
        assert!(
            matches!(err, ProviderError::InvalidResponse(ref m) if m.contains(expected)),
            "{expected}: {err:?}"
        );
    }
    // Un margen que no está en porcentaje no se interpreta.
    let points = MARKET_US100.replacen(
        "\"marginFactorUnit\": \"PERCENTAGE\"",
        "\"marginFactorUnit\": \"POINTS\"",
        1,
    );
    assert_ne!(points, MARKET_US100);
    let t = Recorded::default().login().ok(&points);
    let terms = provider(&t, cfg()).market_terms(&us100()).await.unwrap();
    assert_eq!(terms.margin_pct, None);
}

#[tokio::test(start_paused = true)]
async fn close_spreads_are_ask_minus_bid_at_each_close() {
    let t = Recorded::default().login().ok(DE40_15M);
    let spreads = provider(&t, cfg())
        .close_spreads(
            &de40(),
            Timeframe::M15,
            at("2026-11-20T09:00:00Z"),
            at("2026-11-20T10:00:00Z"),
        )
        .await
        .unwrap();
    // closePrice de la respuesta grabada: 25059.8 − 25058.4, etc. La vela de las 10:00 queda fuera.
    assert_eq!(spreads.len(), 4);
    assert_eq!(spreads[0], (at("2026-11-20T09:00:00Z"), dec!(1.4)));
    assert!(spreads.iter().all(|(_, s)| *s == dec!(1.4)));

    let missing = DE40_15M.replacen("\"bid\": 25058.4", "\"bid\": null", 1);
    assert_ne!(missing, DE40_15M);
    let t = Recorded::default().login().ok(&missing);
    let err = provider(&t, cfg())
        .close_spreads(
            &de40(),
            Timeframe::M15,
            at("2026-11-20T09:00:00Z"),
            at("2026-11-20T10:00:00Z"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ProviderError::InvalidResponse(ref m) if m.contains("closePrice")));
}

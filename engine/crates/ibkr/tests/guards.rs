//! Guardas del ADR 0017 §2, sin red real: un cliente falso que implementa `IbkrApi` y, para ver
//! que no se abre ningún socket, un servidor TCP falso en 127.0.0.1.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use cfds_core::{InstrumentId, KillSwitch, Timeframe};
use cfds_ibkr::{
    ApiError, BarSize, BarsRequest, BarsResponse, ConnectError, ContractChoice, ContractKind,
    IbkrApi, IbkrConfig, IbkrConfigError, PaperAccountError, PaperAccountId, RawBar,
    ReadOnlySession, SessionError, SummaryItem, WhatToShow, connect,
};
use cfds_obs::AlertKind;
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal::dec;
use tokio::net::TcpListener;

const CONFIG: &str = include_str!("../../../../config/ibkr.toml");
const PAPER: &str = "DU0000001";
const ENV: &str = "IBKR_PAPER_ACCOUNT_ID";

fn cfg() -> IbkrConfig {
    IbkrConfig::from_toml(CONFIG).expect("config/ibkr.toml válida")
}

fn paper() -> PaperAccountId {
    PaperAccountId::from_env(ENV, |_| Some(PAPER.into())).unwrap()
}

fn t(h: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 25, h, 0, 0).unwrap()
}

#[derive(Default)]
struct Fake {
    disconnected: AtomicBool,
    disconnects: AtomicUsize,
    accounts: Vec<String>,
    summary: Vec<SummaryItem>,
    bars: Mutex<Option<Result<BarsResponse, ApiError>>>,
    last: Mutex<Option<BarsRequest>>,
}

impl Fake {
    fn with_accounts(a: &[&str]) -> Self {
        Self {
            accounts: a.iter().map(|s| s.to_string()).collect(),
            ..Self::default()
        }
    }
}

impl IbkrApi for &Fake {
    fn is_connected(&self) -> bool {
        !self.disconnected.load(Ordering::SeqCst)
    }
    async fn managed_accounts(&self) -> Result<Vec<String>, ApiError> {
        Ok(self.accounts.clone())
    }
    async fn account_summary(&self) -> Result<Vec<SummaryItem>, ApiError> {
        Ok(self.summary.clone())
    }
    async fn bars(&self, req: &BarsRequest) -> Result<BarsResponse, ApiError> {
        *self.last.lock().unwrap() = Some(req.clone());
        self.bars
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(Err(ApiError::Rejected("sin velas".into())))
    }
    async fn head_timestamp(
        &self,
        _req: &BarsRequest,
    ) -> Result<(String, DateTime<Utc>), ApiError> {
        Ok(("MNQZ6".into(), t(0)))
    }
    async fn disconnect(&self) {
        self.disconnects.fetch_add(1, Ordering::SeqCst);
        self.disconnected.store(true, Ordering::SeqCst);
    }
}

// ---- Guarda 1: puertos de sesión real ----

fn with_port(port: u16, forbidden: &str) -> String {
    CONFIG
        .replace("port = 4002", &format!("port = {port}"))
        .replace(
            "forbidden_ports = [7496, 4001]",
            &format!("forbidden_ports = {forbidden}"),
        )
}

#[test]
fn config_rejects_real_session_ports() {
    for port in [7496, 4001] {
        let err = IbkrConfig::from_toml(&with_port(port, "[7496, 4001]")).unwrap_err();
        assert!(matches!(
            err,
            IbkrConfigError::Invalid {
                field: "gateway.port",
                ..
            }
        ));
    }
    // Un puerto agregado a la lista también se rechaza.
    let err = IbkrConfig::from_toml(&with_port(4999, "[7496, 4001, 4999]")).unwrap_err();
    assert!(matches!(
        err,
        IbkrConfigError::Invalid {
            field: "gateway.port",
            ..
        }
    ));
}

#[test]
fn config_requires_both_real_ports_in_the_list() {
    for list in ["[4001]", "[7496]", "[]"] {
        let err = IbkrConfig::from_toml(&with_port(4002, list)).unwrap_err();
        assert!(
            matches!(
                err,
                IbkrConfigError::Invalid {
                    field: "gateway.forbidden_ports",
                    ..
                }
            ),
            "{list}"
        );
    }
}

#[test]
fn paper_ports_are_allowed() {
    assert_eq!(cfg().port, 4002);
    assert!(IbkrConfig::from_toml(&with_port(7497, "[7496, 4001]")).is_ok());
}

#[test]
fn config_rejects_non_loopback_host() {
    let text = CONFIG.replace("host = \"127.0.0.1\"", "host = \"192.168.1.10\"");
    let err = IbkrConfig::from_toml(&text).unwrap_err();
    assert!(matches!(
        err,
        IbkrConfigError::Invalid {
            field: "gateway.host",
            ..
        }
    ));
}

/// Un servidor falso escucha en un puerto que está en la lista de prohibidos: `connect` se niega
/// sin abrir el socket (el servidor no recibe ninguna conexión).
#[tokio::test]
async fn connect_never_opens_a_socket_to_a_forbidden_port() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut c = cfg();
    c.port = port;
    c.forbidden_ports.push(port);
    let err = connect(c, |_| Some(PAPER.into())).await.err().unwrap();
    assert!(matches!(err, ConnectError::Config(_)), "{err}");
    let accepted = tokio::time::timeout(Duration::from_millis(200), listener.accept()).await;
    assert!(accepted.is_err(), "no tenía que haber ninguna conexión");
}

/// Aunque la lista de la configuración se vacíe a mano, 7496 y 4001 siguen prohibidos.
#[tokio::test]
async fn real_ports_stay_forbidden_even_if_removed_from_the_list() {
    for port in [7496, 4001] {
        let mut c = cfg();
        c.forbidden_ports.clear();
        c.port = port;
        let err = connect(c, |_| Some(PAPER.into())).await.err().unwrap();
        assert!(matches!(err, ConnectError::Config(_)), "{err}");
    }
}

// ---- Guarda 2: cuenta paper de .env ----

#[tokio::test]
async fn without_paper_account_in_env_it_does_not_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut c = cfg();
    c.port = listener.local_addr().unwrap().port();
    for value in [None, Some(String::new()), Some("   ".into())] {
        let err = connect(c.clone(), |_| value.clone()).await.err().unwrap();
        assert!(
            matches!(
                err,
                ConnectError::PaperAccount(PaperAccountError::Missing(_))
            ),
            "{err}"
        );
    }
    let err = connect(c, |_| Some("DU 1;drop".into()))
        .await
        .err()
        .unwrap();
    assert!(matches!(
        err,
        ConnectError::PaperAccount(PaperAccountError::Malformed(_))
    ));
    let accepted = tokio::time::timeout(Duration::from_millis(200), listener.accept()).await;
    assert!(accepted.is_err(), "no tenía que haber ninguna conexión");
}

#[tokio::test]
async fn raw_recording_env_blocks_the_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut c = cfg();
    c.port = listener.local_addr().unwrap().port();
    let env = |k: &str| match k {
        ENV => Some(PAPER.to_owned()),
        "IBAPI_RECORDING_DIR" => Some("/tmp/x".to_owned()),
        _ => None,
    };
    let err = connect(c, env).await.err().unwrap();
    assert!(matches!(
        err,
        ConnectError::RecordingEnabled("IBAPI_RECORDING_DIR")
    ));
    let accepted = tokio::time::timeout(Duration::from_millis(200), listener.accept()).await;
    assert!(accepted.is_err());
}

#[tokio::test]
async fn session_with_only_the_paper_account_opens() {
    let fake = Fake::with_accounts(&[PAPER]);
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    assert_eq!(s.kill_switch(), KillSwitch::Inactive);
    assert_eq!(fake.disconnects.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn other_account_in_session_disconnects_and_trips() {
    for accounts in [vec!["U1234567"], vec![PAPER, "U1234567"], vec![]] {
        let fake = Fake::with_accounts(&accounts);
        let trip = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
            .await
            .err()
            .expect("tenía que rechazar");
        assert_eq!(trip.kind, AlertKind::KillSwitch);
        assert_eq!(trip.at, t(10));
        assert_eq!(fake.disconnects.load(Ordering::SeqCst), 1, "{accounts:?}");
        // El ID de la otra cuenta nunca aparece entero en el motivo.
        assert!(!trip.reason.contains("U1234567"), "{}", trip.reason);
    }
}

#[tokio::test]
async fn summary_with_another_account_trips() {
    let mut fake = Fake::with_accounts(&[PAPER]);
    fake.summary = vec![SummaryItem {
        account: "U7654321".into(),
        tag: "NetLiquidation".into(),
        value: "1000".into(),
        currency: "USD".into(),
    }];
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    let err = s.account_summary(t(11)).await.unwrap_err();
    assert!(matches!(err, SessionError::KillSwitch(ref tr) if tr.kind == AlertKind::KillSwitch));
    assert!(matches!(s.kill_switch(), KillSwitch::Active { since, .. } if since == t(11)));
}

#[tokio::test]
async fn summary_values_are_exact_decimals() {
    let mut fake = Fake::with_accounts(&[PAPER]);
    let item = |tag: &str, value: &str| SummaryItem {
        account: PAPER.into(),
        tag: tag.into(),
        value: value.into(),
        currency: "USD".into(),
    };
    fake.summary = vec![
        item("NetLiquidation", "1000000.10"),
        item("TotalCashValue", "999999.99"),
    ];
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    let sum = s.account_summary(t(11)).await.unwrap();
    assert_eq!(sum.net_liquidation, Some(dec!(1000000.10)));
    assert_eq!(sum.total_cash, Some(dec!(999999.99)));
    assert_eq!(sum.currency.as_deref(), Some("USD"));
    assert_eq!(sum.account, "***01");
}

// ---- Guarda 4: desconexión ----

#[tokio::test]
async fn disconnect_trips_the_kill_switch_and_stays_tripped() {
    let fake = Fake::with_accounts(&[PAPER]);
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    s.check_connection(t(10)).await.unwrap();
    fake.disconnected.store(true, Ordering::SeqCst);
    let err = s.check_connection(t(12)).await.unwrap_err();
    assert!(matches!(err, SessionError::KillSwitch(ref tr) if tr.kind == AlertKind::Disconnect));
    assert!(matches!(s.kill_switch(), KillSwitch::Active { since, .. } if since == t(12)));
    // Aunque la conexión "vuelva", la sesión no lee nada más: el kill switch no se libera acá.
    fake.disconnected.store(false, Ordering::SeqCst);
    let id: InstrumentId = "MNQ".parse().unwrap();
    let err = s
        .closed_candles(&id, Timeframe::H1, t(0), t(13), t(13))
        .await
        .unwrap_err();
    assert!(matches!(err, SessionError::KillSwitch(ref tr) if tr.at == t(12)));
}

#[tokio::test]
async fn connection_lost_during_a_read_trips() {
    let fake = Fake::with_accounts(&[PAPER]);
    *fake.bars.lock().unwrap() = Some(Err(ApiError::ConnectionLost("EOF".into())));
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    let id: InstrumentId = "MNQ".parse().unwrap();
    let err = s
        .closed_candles(&id, Timeframe::H1, t(0), t(10), t(10))
        .await
        .unwrap_err();
    assert!(matches!(err, SessionError::KillSwitch(ref tr) if tr.kind == AlertKind::Disconnect));
    assert!(fake.disconnects.load(Ordering::SeqCst) >= 1);
}

// ---- Velas: solo cerradas, con fuente; error de datos → kill switch ----

fn bar(h: u32, close: Decimal) -> RawBar {
    RawBar {
        start: t(h),
        open: close,
        high: close + dec!(1),
        low: close - dec!(1),
        close,
        volume: Some(dec!(10)),
    }
}

#[tokio::test]
async fn only_closed_candles_with_their_source() {
    let fake = Fake::with_accounts(&[PAPER]);
    *fake.bars.lock().unwrap() = Some(Ok(BarsResponse {
        contract: "MNQZ6".into(),
        bars: vec![
            bar(8, dec!(21000.25)),
            bar(9, dec!(21001.5)),
            bar(10, dec!(21002)),
        ],
    }));
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    let id: InstrumentId = "MNQ".parse().unwrap();
    // A las 10:30 la vela de las 10:00 (1 h) sigue abierta: no sale.
    let now = t(10) + chrono::TimeDelta::minutes(30);
    let got = s
        .closed_candles(&id, Timeframe::H1, t(0), t(11), now)
        .await
        .unwrap();
    assert_eq!(got.source, "ibkr_paper");
    assert_eq!(got.contract, "MNQZ6");
    let starts: Vec<_> = got.candles.iter().map(|c| c.start).collect();
    assert_eq!(starts, vec![t(8), t(9)]);
    assert_eq!(got.candles[1].close, dec!(21001.5));
    assert_eq!(s.kill_switch(), KillSwitch::Inactive);
}

#[tokio::test]
async fn bad_bar_is_a_data_error_and_trips() {
    let fake = Fake::with_accounts(&[PAPER]);
    let mut b = bar(8, dec!(21000));
    b.high = dec!(20000);
    *fake.bars.lock().unwrap() = Some(Ok(BarsResponse {
        contract: "MNQZ6".into(),
        bars: vec![b],
    }));
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    let id: InstrumentId = "MNQ".parse().unwrap();
    let err = s
        .closed_candles(&id, Timeframe::H1, t(0), t(10), t(10))
        .await
        .unwrap_err();
    assert!(matches!(err, SessionError::KillSwitch(ref tr) if tr.kind == AlertKind::DataError));
}

#[tokio::test]
async fn rejected_request_does_not_trip() {
    let fake = Fake::with_accounts(&[PAPER]);
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    let id: InstrumentId = "MES".parse().unwrap();
    let err = s
        .closed_candles(&id, Timeframe::H1, t(0), t(10), t(10))
        .await
        .unwrap_err();
    assert!(matches!(err, SessionError::Rejected(_)));
    assert_eq!(s.kill_switch(), KillSwitch::Inactive);
    let unknown: InstrumentId = "US100".parse().unwrap();
    let err = s
        .closed_candles(&unknown, Timeframe::H1, t(0), t(10), t(10))
        .await
        .unwrap_err();
    assert!(matches!(err, SessionError::UnknownInstrument(_)));
}

#[test]
fn paper_account_is_masked_in_debug_and_display() {
    let p = paper();
    assert_eq!(p.to_string(), "***01");
    assert!(!format!("{p:?}").contains(PAPER));
}

// ---- Otros tamaños de vela, tipos de contrato y la primera vela (#536) ----

#[tokio::test]
async fn one_minute_bars_only_closed_and_forex_asks_midpoint() {
    let fake = Fake::with_accounts(&[PAPER]);
    let m = |min: i64| t(10) + chrono::TimeDelta::minutes(min);
    let mut bars: Vec<RawBar> = (0..3).map(|_| bar(10, dec!(1.1))).collect();
    for (i, b) in bars.iter_mut().enumerate() {
        b.start = m(i as i64);
    }
    *fake.bars.lock().unwrap() = Some(Ok(BarsResponse {
        contract: "EUR.USD".into(),
        bars,
    }));
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    let id: InstrumentId = "EURUSD".parse().unwrap();
    // A las 10:02:30 la vela de las 10:02 sigue abierta.
    let now = m(2) + chrono::TimeDelta::seconds(30);
    let got = s
        .closed_bars(&id, ContractChoice::Front, BarSize::M1, t(10), t(11), now)
        .await
        .unwrap();
    assert_eq!(got.bar_size, BarSize::M1);
    assert_eq!(
        got.bars.iter().map(|b| b.start).collect::<Vec<_>>(),
        vec![m(0), m(1)]
    );
    let req = fake.last.lock().unwrap().clone().unwrap();
    assert_eq!(req.spec.kind, ContractKind::Forex);
    assert_eq!(req.what_to_show, WhatToShow::Midpoint);
    assert_eq!(req.bar_size, BarSize::M1);
}

#[tokio::test]
async fn an_expired_month_reaches_the_client() {
    let fake = Fake::with_accounts(&[PAPER]);
    *fake.bars.lock().unwrap() = Some(Ok(BarsResponse {
        contract: "MNQU6".into(),
        bars: vec![bar(8, dec!(21000))],
    }));
    let s = ReadOnlySession::open(&fake, cfg(), paper(), t(10))
        .await
        .unwrap();
    let id: InstrumentId = "MNQ".parse().unwrap();
    let month = ContractChoice::Month("202609".into());
    let got = s
        .closed_bars(&id, month.clone(), BarSize::H1, t(0), t(10), t(10))
        .await
        .unwrap();
    assert_eq!(got.contract, "MNQU6");
    let req = fake.last.lock().unwrap().clone().unwrap();
    assert_eq!(req.contract, month);
    assert_eq!(req.what_to_show, WhatToShow::Trades);
    let (c, first) = s
        .head_timestamp(&id, ContractChoice::Front, t(10))
        .await
        .unwrap();
    assert_eq!((c.as_str(), first), ("MNQZ6", t(0)));
}

#[test]
fn bar_sizes_and_contract_kinds_from_text() {
    for b in BarSize::ALL {
        assert_eq!(b.as_str().parse::<BarSize>().unwrap(), b);
    }
    assert!("4h".parse::<BarSize>().is_err());
    assert_eq!(BarSize::from(Timeframe::M15), BarSize::M15);
    let c = cfg();
    let kind = |id: &str| c.instrument(&id.parse().unwrap()).unwrap().kind;
    assert_eq!(kind("MNQ"), ContractKind::Future);
    assert_eq!(kind("MNQ.C"), ContractKind::ContinuousFuture);
    assert_eq!(kind("QQQ"), ContractKind::Stock);
    assert_eq!(kind("EURUSD"), ContractKind::Forex);
    let bad = CONFIG.replace("kind = \"stock\"", "kind = \"bond\"");
    assert!(matches!(
        IbkrConfig::from_toml(&bad),
        Err(IbkrConfigError::Invalid {
            field: "instruments.kind",
            ..
        })
    ));
}

// ---- [live]: vela en formación (#558) ----

#[test]
fn live_config_is_validated() {
    let live = cfg().live.expect("config/ibkr.toml tiene [live]");
    assert_ne!(live.client_id, cfg().client_id);
    assert_eq!(live.client_id, 21);
    let bad = |from: &str, to: &str| {
        let text = CONFIG.replace(from, to);
        assert_ne!(text, CONFIG, "{from:?} no está en config/ibkr.toml");
        IbkrConfig::from_toml(&text).unwrap_err()
    };
    // El mismo client_id que [gateway] (el del cron) no se acepta.
    assert!(matches!(
        bad("client_id = 21", "client_id = 17"),
        IbkrConfigError::Invalid {
            field: "live.client_id",
            ..
        }
    ));
    // EURUSD (IDEALPRO) no tiene último operado; un id que no está, tampoco.
    for to in [
        r#"instruments = ["MNQ", "EURUSD"]"#,
        r#"instruments = ["NOPE"]"#,
    ] {
        assert!(matches!(
            bad(r#"instruments = ["MNQ", "MES", "QQQ"]"#, to),
            IbkrConfigError::Invalid {
                field: "live.instruments",
                ..
            }
        ));
    }
    assert!(matches!(
        bad(
            r#"timeframes = ["1m", "5m", "15m", "1h"]"#,
            r#"timeframes = ["1m", "1d"]"#
        ),
        IbkrConfigError::Invalid {
            field: "live.timeframes",
            ..
        }
    ));
    assert!(matches!(
        bad(r#"first_try = "live""#, r#"first_try = "frozen""#),
        IbkrConfigError::Invalid {
            field: "live.first_try",
            ..
        }
    ));
    assert!(matches!(
        bad("reconnect_max_secs = 300", "reconnect_max_secs = 2"),
        IbkrConfigError::Invalid {
            field: "live.reconnect_max_secs",
            ..
        }
    ));
    // Sin [live], el resto sigue andando (la vela en formación queda apagada).
    let start = CONFIG.find("[live]").unwrap();
    let end = start + CONFIG[start..].find("[[instruments]]").unwrap();
    let without = format!("{}{}", &CONFIG[..start], &CONFIG[end..]);
    assert_eq!(IbkrConfig::from_toml(&without).unwrap().live, None);
}

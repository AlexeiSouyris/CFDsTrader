//! Logs sin secretos y alertas (fase 9, #297). Los "secretos" de acá son valores falsos.

use std::io::Write;
use std::sync::{Arc, Mutex};

use cfds_obs::{
    AlertConfig, AlertKind, Alerter, HttpPost, Level, Logger, TelegramSink, redact_url,
};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};

const ALERTS: &str = include_str!("../../../../config/alerts.toml");
const FAKE_TOKEN: &str = "123456:FAKE-bot-token-for-tests";
const FAKE_DB: &str = "postgres://cfds:fake-db-password@127.0.0.1:5432/cfdstrader";
const FAKE_GATEWAY: &str = "fake-gateway-token-0123456789";

/// Un `Write` que guarda todo, para leer el log en los tests.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

fn logger() -> (Arc<Logger>, Captured) {
    let out = Captured::default();
    (Arc::new(Logger::new(Box::new(out.clone()))), out)
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

#[test]
fn every_line_is_json_in_utc_with_level_target_and_event() {
    let (log, out) = logger();
    log.log(
        Level::Info,
        "broker",
        "order_report",
        json!({ "order_id": "o1", "status": "filled" }),
    );
    let line = out.text();
    assert_eq!(line.lines().count(), 1);
    let v: Value = serde_json::from_str(line.trim()).unwrap();
    assert!(v["ts"].as_str().unwrap().ends_with('Z'));
    assert_eq!(v["level"], "info");
    assert_eq!(v["target"], "broker");
    assert_eq!(v["event"], "order_report");
    assert_eq!(v["order_id"], "o1");
}

#[test]
fn secrets_never_reach_the_log() {
    let (log, out) = logger();
    log.register_secret(FAKE_GATEWAY);
    log.log(
        Level::Error,
        "gateway",
        "startup",
        json!({
            "database_url": FAKE_DB,
            "token": FAKE_TOKEN,
            "api_key": "fake-api-key-value",
            "nested": { "authorization": "Bearer fake", "password": "fake-pass" },
            // Un secreto en un campo de nombre inocente, o dentro de un mensaje.
            "error": format!("no se pudo conectar a {FAKE_DB}"),
            "detail": format!("token recibido: {FAKE_GATEWAY}"),
        }),
    );
    let text = out.text();
    for secret in [
        "fake-db-password",
        FAKE_TOKEN,
        "fake-api-key-value",
        "Bearer fake",
        "fake-pass",
        FAKE_GATEWAY,
    ] {
        assert!(!text.contains(secret), "el log tiene un secreto: {secret}");
    }
    // Lo que no es secreto sigue legible.
    assert!(text.contains("cfds:***@127.0.0.1:5432/cfdstrader"));
}

#[test]
fn url_redaction_keeps_urls_without_credentials() {
    assert_eq!(
        redact_url("http://127.0.0.1:8787/api"),
        "http://127.0.0.1:8787/api"
    );
    assert_eq!(
        redact_url("redis://127.0.0.1:6379"),
        "redis://127.0.0.1:6379"
    );
    assert_eq!(
        redact_url("a postgres://u:p@h/db y b postgres://v:q@h/db"),
        "a postgres://u:***@h/db y b postgres://v:***@h/db"
    );
}

/// Un `POST` falso que guarda lo que se mandó.
#[derive(Default)]
struct FakePost {
    sent: Mutex<Vec<(String, String)>>,
    status: u16,
}

impl HttpPost for &FakePost {
    async fn post_json(&self, url: &str, body: String) -> Result<u16, String> {
        self.sent.lock().unwrap().push((url.to_owned(), body));
        Ok(self.status)
    }
}

fn cfg() -> AlertConfig {
    AlertConfig::from_toml(ALERTS).unwrap()
}

fn env_with(
    token: Option<&'static str>,
    chat: Option<&'static str>,
) -> impl Fn(&str) -> Option<String> {
    move |name: &str| match name {
        "TELEGRAM_BOT_TOKEN" => token.map(str::to_owned),
        "TELEGRAM_CHAT_ID" => chat.map(str::to_owned),
        _ => None,
    }
}

#[test]
fn telegram_is_off_unless_both_variables_are_set() {
    let http = FakePost {
        status: 200,
        ..Default::default()
    };
    assert!(TelegramSink::from_env(&cfg(), &http, env_with(None, None)).is_none());
    assert!(TelegramSink::from_env(&cfg(), &http, env_with(Some(FAKE_TOKEN), None)).is_none());
    assert!(TelegramSink::from_env(&cfg(), &http, env_with(None, Some("42"))).is_none());
    assert!(TelegramSink::from_env(&cfg(), &http, env_with(Some(" "), Some("42"))).is_none());
    let on = TelegramSink::from_env(&cfg(), &http, env_with(Some(FAKE_TOKEN), Some("42"))).unwrap();
    assert!(
        !format!("{on:?}").contains(FAKE_TOKEN),
        "el Debug no muestra el token"
    );
}

#[tokio::test]
async fn telegram_uses_only_send_message_and_the_token_stays_out_of_the_log() {
    let http = FakePost {
        status: 200,
        ..Default::default()
    };
    let (log, out) = logger();
    let tg = TelegramSink::from_env(&cfg(), &http, env_with(Some(FAKE_TOKEN), Some("42")));
    let alerter = Alerter::new(cfg(), log, tg);
    assert!(alerter.telegram_enabled());
    assert!(
        alerter
            .raise(
                AlertKind::KillSwitch,
                "drawdown máximo",
                at("2026-11-23T10:00:00Z")
            )
            .await
    );
    let sent = http.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0,
        format!("https://api.telegram.org/bot{FAKE_TOKEN}/sendMessage")
    );
    let body: Value = serde_json::from_str(&sent[0].1).unwrap();
    assert_eq!(body["chat_id"], "42");
    assert!(body["text"].as_str().unwrap().contains("kill_switch"));
    assert!(body["text"].as_str().unwrap().contains("paper"));
    // El log tiene la alerta y no tiene el token.
    let text = out.text();
    assert!(text.contains("\"event\":\"alert\""));
    assert!(!text.contains(FAKE_TOKEN));
}

#[tokio::test]
async fn a_failing_telegram_is_logged_without_the_url() {
    let http = FakePost {
        status: 401,
        ..Default::default()
    };
    let (log, out) = logger();
    let tg = TelegramSink::from_env(&cfg(), &http, env_with(Some(FAKE_TOKEN), Some("42")));
    let alerter = Alerter::new(cfg(), log, tg);
    alerter
        .raise(
            AlertKind::Disconnect,
            "sin base",
            at("2026-11-23T10:00:00Z"),
        )
        .await;
    let text = out.text();
    assert!(text.contains("alert_sink_failed"));
    assert!(text.contains("401"));
    assert!(!text.contains(FAKE_TOKEN));
}

#[tokio::test]
async fn each_kind_is_sent_at_most_once_per_interval() {
    let (log, _) = logger();
    let alerter = Alerter::<&FakePost>::new(cfg(), log, None);
    assert!(!alerter.telegram_enabled());
    let t0 = at("2026-11-23T10:00:00Z");
    assert!(alerter.raise(AlertKind::DataError, "a", t0).await);
    assert!(
        !alerter
            .raise(AlertKind::DataError, "b", t0 + TimeDelta::seconds(10))
            .await
    );
    // Otro tipo no espera.
    assert!(
        alerter
            .raise(AlertKind::Disconnect, "c", t0 + TimeDelta::seconds(10))
            .await
    );
    // Pasado el intervalo, vuelve a mandar.
    assert!(
        alerter
            .raise(AlertKind::DataError, "d", t0 + cfg().min_interval)
            .await
    );
}

#[tokio::test]
async fn a_streak_of_rejections_alerts_once_and_a_fill_resets_it() {
    let (log, out) = logger();
    let alerter = Alerter::<&FakePost>::new(cfg(), log, None);
    let n = cfg().rejected_streak;
    let t0 = at("2026-11-23T10:00:00Z");
    for i in 1..n {
        assert!(
            !alerter.order_result(true, t0).await,
            "rechazo {i}: todavía no"
        );
    }
    assert!(
        alerter.order_result(true, t0).await,
        "el rechazo número {n} alerta"
    );
    assert!(
        !alerter.order_result(true, t0).await,
        "una sola vez por racha"
    );
    // Una ejecutada reinicia la cuenta.
    alerter.order_result(false, t0).await;
    for _ in 1..n {
        assert!(!alerter.order_result(true, t0 + cfg().min_interval).await);
    }
    assert!(alerter.order_result(true, t0 + cfg().min_interval).await);
    assert!(out.text().contains("rejected_streak"));
}

#[test]
fn bad_configs_are_rejected() {
    for (old, new) in [
        ("rejected_streak = 5", "rejected_streak = 0"),
        ("https://api.telegram.org", "http://api.telegram.org"),
        ("https://api.telegram.org", "https://otro.example"),
        (
            "token_env = \"TELEGRAM_BOT_TOKEN\"",
            "token_env = \"telegram token\"",
        ),
    ] {
        let bad = ALERTS.replacen(old, new, 1);
        assert_ne!(bad, ALERTS);
        assert!(AlertConfig::from_toml(&bad).is_err(), "{new}");
    }
}

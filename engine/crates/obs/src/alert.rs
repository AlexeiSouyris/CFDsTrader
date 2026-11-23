//! Alertas (fase 9, #297): una interfaz, el destino de log y el de Telegram, y las reglas.
//!
//! ```text
//! disparadores: kill switch · error de datos · desconexión (gateway o base) · racha de rechazos
//! cada tipo se manda como mucho una vez cada [alerts] min_interval_secs
//! la racha: [alerts] rejected_streak órdenes rechazadas seguidas (una ejecutada la reinicia)
//! ```
//! Telegram: **solo** `POST {api_base}/bot<token>/sendMessage` de la Bot API oficial, y queda
//! apagado salvo que estén las dos variables de `[telegram]` (`TELEGRAM_BOT_TOKEN` y
//! `TELEGRAM_CHAT_ID`). El token va solo en la URL del pedido: nunca se loguea ni se muestra.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use serde::Deserialize;
use serde_json::json;

use crate::log::{Level, Logger};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertKind {
    KillSwitch,
    DataError,
    Disconnect,
    RejectedStreak,
}

impl AlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertKind::KillSwitch => "kill_switch",
            AlertKind::DataError => "data_error",
            AlertKind::Disconnect => "disconnect",
            AlertKind::RejectedStreak => "rejected_streak",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub kind: AlertKind,
    pub message: String,
    pub at: DateTime<Utc>,
}

/// Un destino de alertas.
pub trait AlertSink: Send + Sync {
    fn send(&self, alert: &Alert) -> impl Future<Output = Result<(), String>> + Send;
}

/// Destino de log: cada alerta es una línea `warn` con `event = "alert"`.
pub struct LogSink(pub Arc<Logger>);

impl AlertSink for LogSink {
    async fn send(&self, a: &Alert) -> Result<(), String> {
        self.0.log(
            Level::Warn,
            "alert",
            "alert",
            json!({ "kind": a.kind.as_str(), "message": a.message, "at": a.at.to_rfc3339() }),
        );
        Ok(())
    }
}

/// Un `POST` con cuerpo JSON. En producción, [`ReqwestPost`]; en los tests, uno falso.
pub trait HttpPost: Send + Sync {
    fn post_json(
        &self,
        url: &str,
        body: String,
    ) -> impl Future<Output = Result<u16, String>> + Send;
}

/// `POST` real con `reqwest` (TLS con los certificados del sistema) y tiempo máximo.
pub struct ReqwestPost {
    client: reqwest::Client,
}

impl ReqwestPost {
    pub fn new(timeout: Duration) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| e.without_url().to_string())?;
        Ok(Self { client })
    }
}

impl HttpPost for ReqwestPost {
    async fn post_json(&self, url: &str, body: String) -> Result<u16, String> {
        self.client
            .post(url)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map(|r| r.status().as_u16())
            // `without_url`: el error de reqwest trae la URL, que tiene el token.
            .map_err(|e| e.without_url().to_string())
    }
}

/// Destino de Telegram: `sendMessage` de la Bot API oficial.
pub struct TelegramSink<H: HttpPost> {
    http: H,
    /// `{api_base}/bot<token>/sendMessage`. Tiene el token: nunca se loguea.
    url: String,
    chat_id: String,
}

impl<H: HttpPost> std::fmt::Debug for TelegramSink<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Sin la URL ni el chat: el Debug nunca muestra el token.
        f.write_str("TelegramSink { .. }")
    }
}

impl<H: HttpPost> TelegramSink<H> {
    /// `None` (apagado) salvo que estén las dos variables. `env` lee una variable (en
    /// producción, `std::env::var`).
    pub fn from_env(
        cfg: &AlertConfig,
        http: H,
        env: impl Fn(&str) -> Option<String>,
    ) -> Option<Self> {
        let token = env(&cfg.telegram_token_env).filter(|t| !t.trim().is_empty())?;
        let chat_id = env(&cfg.telegram_chat_env).filter(|c| !c.trim().is_empty())?;
        Some(Self {
            http,
            url: format!(
                "{}/bot{}/sendMessage",
                cfg.telegram_api_base.trim_end_matches('/'),
                token.trim()
            ),
            chat_id: chat_id.trim().to_owned(),
        })
    }
}

impl<H: HttpPost> AlertSink for TelegramSink<H> {
    async fn send(&self, a: &Alert) -> Result<(), String> {
        let text = format!(
            "CFDsTrader (paper) — {}: {} ({})",
            a.kind.as_str(),
            a.message,
            a.at.to_rfc3339()
        );
        let body = json!({ "chat_id": self.chat_id, "text": text }).to_string();
        match self.http.post_json(&self.url, body).await {
            Ok(status) if (200..300).contains(&status) => Ok(()),
            Ok(status) => Err(format!("Telegram respondió {status}")),
            Err(e) => Err(format!("Telegram no respondió: {e}")),
        }
    }
}

/// `config/alerts.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertConfig {
    /// Órdenes rechazadas seguidas que disparan la alerta.
    pub rejected_streak: u32,
    /// Cada tipo de alerta se manda como mucho una vez en este lapso.
    pub min_interval: TimeDelta,
    pub telegram_api_base: String,
    pub telegram_token_env: String,
    pub telegram_chat_env: String,
    pub telegram_timeout: Duration,
}

impl AlertConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            alerts: RawAlerts,
            telegram: RawTelegram,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawAlerts {
            rejected_streak: u32,
            min_interval_secs: u32,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawTelegram {
            api_base: String,
            token_env: String,
            chat_env: String,
            timeout_secs: u32,
        }
        let r: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        if r.alerts.rejected_streak == 0 || r.telegram.timeout_secs == 0 {
            return Err("alerts.rejected_streak y telegram.timeout_secs tienen que ser > 0".into());
        }
        // Solo la Bot API oficial, por https.
        if r.telegram.api_base.trim_end_matches('/') != "https://api.telegram.org" {
            return Err("telegram.api_base tiene que ser https://api.telegram.org".into());
        }
        let env_name = |n: &str| {
            !n.is_empty()
                && n.bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        };
        if !env_name(&r.telegram.token_env) || !env_name(&r.telegram.chat_env) {
            return Err("telegram: nombres de variables de entorno inválidos".into());
        }
        Ok(Self {
            rejected_streak: r.alerts.rejected_streak,
            min_interval: TimeDelta::seconds(i64::from(r.alerts.min_interval_secs)),
            telegram_api_base: r.telegram.api_base,
            telegram_token_env: r.telegram.token_env,
            telegram_chat_env: r.telegram.chat_env,
            telegram_timeout: Duration::from_secs(u64::from(r.telegram.timeout_secs)),
        })
    }
}

/// Decide cuándo alertar y manda a los destinos. Un destino que falla no frena a los otros: su
/// error se escribe en el log.
pub struct Alerter<T: HttpPost> {
    cfg: AlertConfig,
    logger: Arc<Logger>,
    log_sink: LogSink,
    telegram: Option<TelegramSink<T>>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    last: HashMap<AlertKind, DateTime<Utc>>,
    rejected: u32,
}

impl<T: HttpPost> Alerter<T> {
    pub fn new(cfg: AlertConfig, logger: Arc<Logger>, telegram: Option<TelegramSink<T>>) -> Self {
        Self {
            log_sink: LogSink(logger.clone()),
            logger,
            cfg,
            telegram,
            state: Mutex::new(State::default()),
        }
    }

    pub fn telegram_enabled(&self) -> bool {
        self.telegram.is_some()
    }

    /// Manda la alerta si no se mandó una del mismo tipo hace menos de `min_interval`.
    /// Devuelve `true` si se mandó.
    pub async fn raise(&self, kind: AlertKind, message: &str, at: DateTime<Utc>) -> bool {
        {
            let Ok(mut st) = self.state.lock() else {
                return false;
            };
            if st
                .last
                .get(&kind)
                .is_some_and(|prev| at - *prev < self.cfg.min_interval)
            {
                return false;
            }
            st.last.insert(kind, at);
        }
        let alert = Alert {
            kind,
            // El mensaje sale del proceso (Telegram): sin secretos.
            message: self.logger.scrub(message),
            at,
        };
        let _ = self.log_sink.send(&alert).await;
        if let Some(t) = &self.telegram
            && let Err(e) = t.send(&alert).await
        {
            self.logger.log(
                Level::Error,
                "alert",
                "alert_sink_failed",
                json!({ "sink": "telegram", "error": e }),
            );
        }
        true
    }

    /// Cuenta el resultado de una orden: una racha de `rejected_streak` rechazos seguidos dispara
    /// la alerta (una vez por racha); una orden ejecutada la reinicia.
    pub async fn order_result(&self, rejected: bool, at: DateTime<Utc>) -> bool {
        let fire = {
            let Ok(mut st) = self.state.lock() else {
                return false;
            };
            if rejected {
                st.rejected += 1;
                st.rejected == self.cfg.rejected_streak
            } else {
                st.rejected = 0;
                false
            }
        };
        if fire {
            let msg = format!(
                "{} órdenes rechazadas seguidas en el broker simulado",
                self.cfg.rejected_streak
            );
            return self.raise(AlertKind::RejectedStreak, &msg, at).await;
        }
        false
    }
}

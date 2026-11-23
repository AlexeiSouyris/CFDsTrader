//! Logs estructurados: una línea JSON por evento.
//!
//! ```text
//! {"ts":"2026-11-23T18:00:00Z","level":"info","target":"broker","event":"order_report", ...campos}
//! ```
//! Redacción, en este orden (CLAUDE.md §5: nunca credenciales en logs):
//! 1. un campo cuyo nombre parece de secreto (`token`, `password`, `secret`, `api_key`,
//!    `authorization`, `database_url`) se reemplaza por `"***"`;
//! 2. en cualquier texto, la contraseña de una URL `scheme://usuario:clave@host` se tapa;
//! 3. cualquier valor registrado con [`Logger::register_secret`] se reemplaza en la línea final.

use std::io::Write;
use std::sync::Mutex;

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

const MASK: &str = "***";
/// Secretos más cortos que esto no se registran (taparían texto normal por accidente).
const MIN_SECRET_LEN: usize = 8;

fn secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    [
        "token",
        "password",
        "passwd",
        "secret",
        "api_key",
        "apikey",
        "authorization",
        "database_url",
    ]
    .iter()
    .any(|s| k.contains(s))
}

/// Tapa la contraseña de una URL con credenciales (`postgres://u:clave@h/db` →
/// `postgres://u:***@h/db`), en cualquier parte del texto.
pub fn redact_url(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        // La autoridad termina en el primer '/', espacio o comilla.
        let end = tail
            .find(|c: char| c == '/' || c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(tail.len());
        let authority = &tail[..end];
        match (authority.rfind('@'), authority.find(':')) {
            (Some(at), Some(colon)) if colon < at => {
                out.push_str(&authority[..=colon]);
                out.push_str(MASK);
                out.push_str(&authority[at..]);
            }
            _ => out.push_str(authority),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn redact(v: Value) -> Value {
    match v {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let v = if secret_key(&k) && !v.is_null() {
                        Value::String(MASK.into())
                    } else {
                        redact(v)
                    };
                    (k, v)
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.into_iter().map(redact).collect()),
        Value::String(s) => Value::String(redact_url(&s)),
        other => other,
    }
}

/// Escribe eventos como líneas JSON en `out` (por defecto, la salida de errores).
pub struct Logger {
    out: Mutex<Box<dyn Write + Send>>,
    secrets: Mutex<Vec<String>>,
}

impl Logger {
    pub fn new(out: Box<dyn Write + Send>) -> Self {
        Self {
            out: Mutex::new(out),
            secrets: Mutex::new(Vec::new()),
        }
    }

    pub fn stderr() -> Self {
        Self::new(Box::new(std::io::stderr()))
    }

    /// Un valor que nunca puede aparecer en un log (por ejemplo, el token del gateway o la
    /// contraseña de la base). Se reemplaza por `***` donde esté.
    pub fn register_secret(&self, value: &str) {
        if value.len() >= MIN_SECRET_LEN
            && let Ok(mut s) = self.secrets.lock()
        {
            s.push(value.to_owned());
        }
    }

    /// La línea JSON de un evento, ya redactada (la usa [`Logger::log`]; pública para los tests).
    pub fn line(
        &self,
        at: DateTime<Utc>,
        level: Level,
        target: &str,
        event: &str,
        fields: Value,
    ) -> String {
        let mut doc = Map::new();
        doc.insert(
            "ts".into(),
            json!(at.to_rfc3339_opts(SecondsFormat::Secs, true)),
        );
        doc.insert("level".into(), json!(level.as_str()));
        doc.insert("target".into(), json!(target));
        doc.insert("event".into(), json!(event));
        if let Value::Object(f) = redact(fields) {
            for (k, v) in f {
                if !doc.contains_key(&k) {
                    doc.insert(k, v);
                }
            }
        }
        let mut line = Value::Object(doc).to_string();
        if let Ok(secrets) = self.secrets.lock() {
            for s in secrets.iter() {
                line = line.replace(s.as_str(), MASK);
            }
        }
        line
    }

    /// Un texto sin secretos: contraseñas de URL y valores registrados tapados. Lo usan las
    /// alertas antes de mandar un mensaje fuera del proceso.
    pub fn scrub(&self, text: &str) -> String {
        let mut out = redact_url(text);
        if let Ok(secrets) = self.secrets.lock() {
            for s in secrets.iter() {
                out = out.replace(s.as_str(), MASK);
            }
        }
        out
    }

    /// Escribe el evento. Un error de escritura no frena al motor (se ignora).
    pub fn log(&self, level: Level, target: &str, event: &str, fields: Value) {
        let line = self.line(Utc::now(), level, target, event, fields);
        if let Ok(mut out) = self.out.lock() {
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
        }
    }
}

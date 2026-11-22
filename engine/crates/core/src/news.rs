//! Interpretación de una noticia. Contrato: `contracts/news_interpretation.v1.schema.json`.
//!
//! La escribe Python (`cfds_intel.news.interpret`, #122). Rust solo la **lee** para el ajuste de
//! noticias del puntaje (#130). Acepta exactamente los mismos documentos que el contrato, y
//! además exige `interpreted_at >= published_at`, que el JSON Schema no puede expresar.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Raw", into = "Raw")]
pub struct NewsInterpretation {
    pub interpretation_id: String,
    pub source: String,
    pub item_id: String,
    /// Hora de publicación de la noticia.
    pub published_at: DateTime<Utc>,
    /// Desde cuándo existe la interpretación para el sistema (nunca antes de `published_at`).
    pub interpreted_at: DateTime<Utc>,
    pub interpreter: Interpreter,
    pub summary_es: String,
    pub impacts: Vec<NewsImpact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interpreter {
    pub name: String,
    pub version: String,
    pub model: Option<String>,
    pub prompt_sha256: Option<String>,
}

/// Sentimiento en [−1, 1] y relevancia en [0, 1] para un instrumento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsImpact {
    pub instrument_id: InstrumentId,
    pub sentiment: Decimal,
    pub relevance: Decimal,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    interpretation_id: String,
    source: String,
    item_id: String,
    #[serde(with = "utc_z")]
    published_at: DateTime<Utc>,
    #[serde(with = "utc_z")]
    interpreted_at: DateTime<Utc>,
    interpreter: RawInterpreter,
    summary_es: String,
    impacts: Vec<RawImpact>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInterpreter {
    name: String,
    version: String,
    // Obligatorios aunque sean `null`, como en el contrato.
    #[serde(deserialize_with = "required")]
    model: Option<String>,
    #[serde(deserialize_with = "required")]
    prompt_sha256: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawImpact {
    instrument_id: InstrumentId,
    sentiment: String,
    relevance: String,
}

fn required<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `^[a-z]` y después hasta `max_rest` caracteres de `rest`.
fn label(s: &str, max_rest: usize, rest: impl Fn(u8) -> bool) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= max_rest + 1
        && b[0].is_ascii_lowercase()
        && b[1..].iter().all(|&c| rest(c))
}

fn chars_between(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.chars().count())
}

/// `^(0(\.\d{1,4})?|1(\.0{1,4})?)$`, con un `-` opcional delante si `signed`.
fn unit_decimal(s: &str, signed: bool) -> Option<Decimal> {
    let body = match s.strip_prefix('-') {
        Some(rest) if signed => rest,
        Some(_) => return None,
        None => s,
    };
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let frac_ok =
        frac.is_none_or(|f| (1..=4).contains(&f.len()) && f.bytes().all(|b| b.is_ascii_digit()));
    let ok = match int {
        "0" => frac_ok,
        "1" => frac_ok && frac.is_none_or(|f| f.bytes().all(|b| b == b'0')),
        _ => false,
    };
    if ok { s.parse().ok() } else { None }
}

impl TryFrom<Raw> for NewsInterpretation {
    type Error = String;

    fn try_from(r: Raw) -> Result<Self, String> {
        if !is_hex64(&r.interpretation_id) {
            return Err("interpretation_id tiene que ser un SHA-256 en hex".into());
        }
        if !label(&r.source, 31, |c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_'
        }) {
            return Err(format!("source inválido: {:?}", r.source));
        }
        if !chars_between(&r.item_id, 1, 500) {
            return Err("item_id tiene que tener de 1 a 500 caracteres".into());
        }
        if r.interpreted_at < r.published_at {
            return Err("interpreted_at no puede ser anterior a published_at".into());
        }
        let i = r.interpreter;
        if !label(&i.name, 63, |c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-'
        }) {
            return Err(format!("interpreter.name inválido: {:?}", i.name));
        }
        let vb = i.version.as_bytes();
        let version_ok = !vb.is_empty()
            && vb.len() <= 64
            && vb[0].is_ascii_alphanumeric()
            && vb
                .iter()
                .all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'+' | b'-'));
        if !version_ok {
            return Err(format!("interpreter.version inválida: {:?}", i.version));
        }
        if i.model
            .as_deref()
            .is_some_and(|m| !chars_between(m, 1, 200))
        {
            return Err("interpreter.model tiene que ser null o de 1 a 200 caracteres".into());
        }
        if i.prompt_sha256.as_deref().is_some_and(|p| !is_hex64(p)) {
            return Err("interpreter.prompt_sha256 tiene que ser null o un SHA-256 en hex".into());
        }
        if !chars_between(&r.summary_es, 1, 1000) {
            return Err("summary_es tiene que tener de 1 a 1000 caracteres".into());
        }
        if r.impacts.len() > 64 {
            return Err("más de 64 instrumentos afectados".into());
        }
        let mut impacts = Vec::with_capacity(r.impacts.len());
        for x in r.impacts {
            let sentiment = unit_decimal(&x.sentiment, true)
                .ok_or_else(|| format!("sentiment fuera del contrato: {:?}", x.sentiment))?;
            let relevance = unit_decimal(&x.relevance, false)
                .ok_or_else(|| format!("relevance fuera del contrato: {:?}", x.relevance))?;
            impacts.push(NewsImpact {
                instrument_id: x.instrument_id,
                sentiment,
                relevance,
            });
        }
        Ok(Self {
            interpretation_id: r.interpretation_id,
            source: r.source,
            item_id: r.item_id,
            published_at: r.published_at,
            interpreted_at: r.interpreted_at,
            interpreter: Interpreter {
                name: i.name,
                version: i.version,
                model: i.model,
                prompt_sha256: i.prompt_sha256,
            },
            summary_es: r.summary_es,
            impacts,
        })
    }
}

impl From<NewsInterpretation> for Raw {
    fn from(n: NewsInterpretation) -> Self {
        Raw {
            interpretation_id: n.interpretation_id,
            source: n.source,
            item_id: n.item_id,
            published_at: n.published_at,
            interpreted_at: n.interpreted_at,
            interpreter: RawInterpreter {
                name: n.interpreter.name,
                version: n.interpreter.version,
                model: n.interpreter.model,
                prompt_sha256: n.interpreter.prompt_sha256,
            },
            summary_es: n.summary_es,
            impacts: n
                .impacts
                .into_iter()
                .map(|x| RawImpact {
                    instrument_id: x.instrument_id,
                    sentiment: x.sentiment.to_string(),
                    relevance: x.relevance.to_string(),
                })
                .collect(),
        }
    }
}

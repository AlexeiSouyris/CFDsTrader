//! Respuestas de la API (labs.ig.com/reference) y su conversión a [`Candle`].
//! Funciones puras: sin red ni reloj.
//!
//! Los números se leen como texto JSON (`RawValue`) y se convierten a `Decimal` exacto,
//! sin pasar nunca por `f64` (CLAUDE.md §4).

use std::str::FromStr;

use cfds_core::{Candle, InstrumentId, Timeframe};
use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::value::RawValue;

/// `resolution` de `GET /prices/{epic}` para cada timeframe del sistema.
pub fn resolution(tf: Timeframe) -> &'static str {
    match tf {
        Timeframe::M15 => "MINUTE_15",
        Timeframe::H1 => "HOUR",
    }
}

/// Formato de `from`/`to` en `GET /prices/{epic}` v3: `yyyy-MM-dd'T'HH:mm:ss`.
pub const QUERY_TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";
/// Formato de `snapshotTimeUTC` (sin `Z`, medido en la demo).
const SNAPSHOT_UTC_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

/// Cuerpo de un error de IG: `{ "errorCode": "..." }`.
#[derive(Debug, Deserialize)]
pub struct ApiError {
    #[serde(rename = "errorCode")]
    pub error_code: String,
}

pub fn error_code(body: &str) -> Option<String> {
    serde_json::from_str::<ApiError>(body)
        .ok()
        .map(|e| e.error_code)
}

/// Lo único que se usa del login (`POST /session` v2). El resto del cuerpo (ids y saldo de la
/// cuenta) no se lee ni se guarda.
#[derive(Debug, Deserialize)]
pub struct SessionBody {
    #[serde(rename = "timezoneOffset")]
    timezone_offset: Box<RawValue>,
}

impl SessionBody {
    /// Diferencia de la hora local de la cuenta con UTC ("in hours"; puede tener fracción).
    pub fn timezone_offset(&self) -> Result<TimeDelta, String> {
        let hours = decimal(&self.timezone_offset)?;
        let minutes = hours * Decimal::from(60);
        if minutes.fract() != Decimal::ZERO || minutes.abs() > Decimal::from(24 * 60) {
            return Err(format!("timezoneOffset inválido: {hours}"));
        }
        let m = i64::try_from(minutes).map_err(|e| e.to_string())?;
        Ok(TimeDelta::minutes(m))
    }
}

#[derive(Debug, Deserialize)]
pub struct PricesBody {
    pub prices: Vec<RawPrice>,
    pub metadata: Metadata,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub allowance: AllowanceBody,
    #[serde(rename = "pageData")]
    pub page_data: PageData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct AllowanceBody {
    #[serde(rename = "remainingAllowance")]
    pub remaining: u64,
    #[serde(rename = "totalAllowance")]
    pub total: u64,
    /// Segundos hasta que se renueva el cupo.
    #[serde(rename = "allowanceExpiry")]
    pub expiry_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct PageData {
    #[serde(rename = "pageNumber")]
    pub page_number: u32,
    #[serde(rename = "totalPages")]
    pub total_pages: u32,
}

#[derive(Debug, Deserialize)]
pub struct RawPrice {
    #[serde(rename = "snapshotTimeUTC")]
    snapshot_time_utc: String,
    #[serde(rename = "openPrice")]
    open: BidAsk,
    #[serde(rename = "highPrice")]
    high: BidAsk,
    #[serde(rename = "lowPrice")]
    low: BidAsk,
    #[serde(rename = "closePrice")]
    close: BidAsk,
}

#[derive(Debug, Deserialize)]
struct BidAsk {
    bid: Option<Box<RawValue>>,
    ask: Option<Box<RawValue>>,
}

fn decimal(raw: &RawValue) -> Result<Decimal, String> {
    let s = raw.get();
    // Notación científica (`1e3`) no se espera en precios; si llega, se acepta exacta.
    Decimal::from_str(s)
        .or_else(|_| Decimal::from_scientific(s))
        .map_err(|_| format!("número inválido: {s}"))
}

impl BidAsk {
    /// Precio medio exacto. Si falta bid o ask, la vela no se inventa: es un error.
    fn mid(&self, what: &str, at: &str) -> Result<Decimal, String> {
        match (&self.bid, &self.ask) {
            (Some(b), Some(a)) => Ok((decimal(b)? + decimal(a)?) / Decimal::TWO),
            _ => Err(format!("{what} sin bid o ask en {at}")),
        }
    }
}

/// Convierte las velas de IG en [`Candle`] con **precio medio** `(bid + ask) / 2`, que es lo que
/// asume el backtest al cobrar medio spread (`cfds-backtest`, `costs.rs`).
///
/// - El inicio es `snapshotTimeUTC` y tiene que estar alineado al timeframe.
/// - Se devuelven solo las que empiezan en `[from, to)`: el pedido a IG lleva margen.
/// - El volumen queda vacío: en CFD, `lastTradedVolume` no es volumen del mercado.
/// - No se ordena ni se corrige nada: la validación de la ingesta detecta desorden y huecos.
pub fn to_candles(
    prices: &[RawPrice],
    instrument: &InstrumentId,
    tf: Timeframe,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Candle>, String> {
    let step = tf.duration().num_seconds();
    let mut out = Vec::new();
    for p in prices {
        let at = p.snapshot_time_utc.as_str();
        let start = NaiveDateTime::parse_from_str(at, SNAPSHOT_UTC_FORMAT)
            .map_err(|_| format!("snapshotTimeUTC inválido: {at:?}"))?
            .and_utc();
        if start.timestamp().rem_euclid(step) != 0 {
            return Err(format!("vela {at} no alineada a {tf}"));
        }
        if start < from || start >= to {
            continue;
        }
        out.push(Candle {
            instrument_id: instrument.clone(),
            timeframe: tf,
            start,
            open: p.open.mid("openPrice", at)?,
            high: p.high.mid("highPrice", at)?,
            low: p.low.mid("lowPrice", at)?,
            close: p.close.mid("closePrice", at)?,
            volume: None,
        });
    }
    Ok(out)
}

/// Rango a pedir en hora local de la cuenta, con `margin` de más a cada lado.
/// IG interpreta `from`/`to` en hora local (medido en la demo) y `to` es inclusivo.
pub fn local_query_range(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    offset: TimeDelta,
    margin: TimeDelta,
) -> (String, String) {
    let f = (from.naive_utc() + offset - margin).format(QUERY_TIME_FORMAT);
    let t = (to.naive_utc() + offset + margin).format(QUERY_TIME_FORMAT);
    (f.to_string(), t.to_string())
}

/// Puntos que puede costar el pedido: velas del rango con margen, contando los dos extremos.
pub fn estimated_points(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    tf: Timeframe,
    margin: TimeDelta,
) -> u64 {
    let span = (to - from) + margin * 2;
    let step = tf.duration().num_seconds();
    u64::try_from(span.num_seconds().max(0) / step + 1).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn session(offset: &str) -> SessionBody {
        serde_json::from_str(&format!(r#"{{"timezoneOffset": {offset}}}"#)).unwrap()
    }

    #[test]
    fn decimals_are_read_from_the_json_text_without_floats() {
        // 0.1 + 0.2 en f64 no da 0.3; leyendo el texto, sí.
        let a: Box<RawValue> = serde_json::from_str("0.1").unwrap();
        let b: Box<RawValue> = serde_json::from_str("0.2").unwrap();
        assert_eq!(
            decimal(&a).unwrap() + decimal(&b).unwrap(),
            Decimal::new(3, 1)
        );
        let big: Box<RawValue> = serde_json::from_str("26789.45").unwrap();
        assert_eq!(decimal(&big).unwrap(), Decimal::new(2_678_945, 2));
        let bad: Box<RawValue> = serde_json::from_str("\"x\"").unwrap();
        assert!(decimal(&bad).is_err());
    }

    #[test]
    fn timezone_offset_accepts_whole_and_half_hours() {
        assert_eq!(session("1").timezone_offset(), Ok(TimeDelta::hours(1)));
        assert_eq!(session("-5").timezone_offset(), Ok(TimeDelta::hours(-5)));
        assert_eq!(
            session("5.5").timezone_offset(),
            Ok(TimeDelta::minutes(330))
        );
        assert!(session("0.0001").timezone_offset().is_err());
        assert!(session("30").timezone_offset().is_err());
    }

    #[test]
    fn query_range_moves_to_local_time_and_adds_the_margin() {
        let (f, t) = local_query_range(
            at("2026-11-20T09:00:00Z"),
            at("2026-11-20T10:00:00Z"),
            TimeDelta::hours(1),
            TimeDelta::hours(1),
        );
        assert_eq!(
            (f.as_str(), t.as_str()),
            ("2026-11-20T09:00:00", "2026-11-20T12:00:00")
        );
        let (f, t) = local_query_range(
            at("2026-11-20T00:30:00Z"),
            at("2026-11-20T01:00:00Z"),
            TimeDelta::hours(-5),
            TimeDelta::zero(),
        );
        assert_eq!(
            (f.as_str(), t.as_str()),
            ("2026-11-19T19:30:00", "2026-11-19T20:00:00")
        );
    }

    #[test]
    fn estimated_points_count_both_ends_and_the_margin() {
        let (f, t) = (at("2026-11-20T09:00:00Z"), at("2026-11-20T10:00:00Z"));
        // 1 h de 15m + 1 h de margen por lado = 3 h → 12 intervalos → 13 velas posibles.
        assert_eq!(
            estimated_points(f, t, Timeframe::M15, TimeDelta::hours(1)),
            13
        );
        assert_eq!(
            estimated_points(f, t, Timeframe::H1, TimeDelta::hours(1)),
            4
        );
        assert_eq!(estimated_points(f, t, Timeframe::H1, TimeDelta::zero()), 2);
        assert_eq!(estimated_points(t, f, Timeframe::H1, TimeDelta::zero()), 1);
    }

    #[test]
    fn resolutions_are_the_documented_names() {
        assert_eq!(resolution(Timeframe::M15), "MINUTE_15");
        assert_eq!(resolution(Timeframe::H1), "HOUR");
    }

    #[test]
    fn candle_range_is_half_open() {
        let body = |t: &str| {
            format!(
                r#"{{"snapshotTimeUTC":"{t}","openPrice":{{"bid":1,"ask":3}},"highPrice":{{"bid":4,"ask":6}},"lowPrice":{{"bid":0,"ask":2}},"closePrice":{{"bid":2,"ask":2}}}}"#
            )
        };
        let prices: Vec<RawPrice> = [
            "2026-11-20T08:00:00",
            "2026-11-20T09:00:00",
            "2026-11-20T10:00:00",
        ]
        .iter()
        .map(|t| serde_json::from_str(&body(t)).unwrap())
        .collect();
        let id: InstrumentId = "DE40".parse().unwrap();
        let out = to_candles(
            &prices,
            &id,
            Timeframe::H1,
            at("2026-11-20T09:00:00Z"),
            at("2026-11-20T10:00:00Z"),
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, at("2026-11-20T09:00:00Z"));
        assert_eq!(
            (out[0].open, out[0].high, out[0].low, out[0].close),
            (Decimal::TWO, Decimal::from(5), Decimal::ONE, Decimal::TWO)
        );
    }
}

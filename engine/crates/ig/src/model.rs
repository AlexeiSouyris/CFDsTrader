//! Respuestas de la API (labs.ig.com/reference) y su conversión a [`Candle`].
//! Funciones puras: sin red ni reloj.
//!
//! Los números se leen como texto JSON (`RawValue`) y se convierten a `Decimal` exacto,
//! sin pasar nunca por `f64` (CLAUDE.md §4).

use std::collections::BTreeMap;
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
        let start = snapshot_start(p, step, tf)?;
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

/// Condiciones del contrato según `GET /markets/{epic}` v3 (#67).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketTerms {
    pub epic: String,
    /// `instrument.marketId`: el grupo de mercado (lo usa el sentimiento de clientes, #27).
    pub market_id: String,
    pub name: String,
    /// `instrument.type`, por ejemplo `INDICES`.
    pub instrument_type: String,
    /// Moneda del contrato (`currencies[]` con `isDefault`).
    pub currency: String,
    /// `valueOfOnePip`: lo que vale un punto por contrato, en `currency`.
    pub value_per_point: Decimal,
    /// `dealingRules.minDealSize.value`, en contratos.
    pub min_deal_size: Decimal,
    /// `marginFactor` cuando `marginFactorUnit` es `PERCENTAGE`.
    pub margin_pct: Option<Decimal>,
}

#[derive(Deserialize)]
struct MarketBody {
    instrument: MarketInstrument,
    #[serde(rename = "dealingRules")]
    dealing_rules: DealingRules,
}

#[derive(Deserialize)]
struct MarketInstrument {
    epic: String,
    #[serde(rename = "marketId")]
    market_id: String,
    name: String,
    #[serde(rename = "type")]
    instrument_type: String,
    currencies: Vec<MarketCurrency>,
    #[serde(rename = "valueOfOnePip")]
    value_of_one_pip: String,
    #[serde(rename = "marginFactor")]
    margin_factor: Option<Box<RawValue>>,
    #[serde(rename = "marginFactorUnit")]
    margin_factor_unit: Option<String>,
}

#[derive(Deserialize)]
struct MarketCurrency {
    code: String,
    #[serde(rename = "isDefault")]
    is_default: bool,
}

#[derive(Deserialize)]
struct DealingRules {
    #[serde(rename = "minDealSize")]
    min_deal_size: DealingRule,
}

#[derive(Deserialize)]
struct DealingRule {
    value: Box<RawValue>,
}

impl MarketTerms {
    pub fn from_json(body: &str) -> Result<Self, String> {
        let b: MarketBody = serde_json::from_str(body).map_err(|e| format!("GET /markets: {e}"))?;
        let i = b.instrument;
        let currency = match i.currencies.iter().find(|c| c.is_default) {
            Some(c) => c.code.clone(),
            None => return Err(format!("{}: ninguna moneda marcada isDefault", i.epic)),
        };
        let value_per_point = Decimal::from_str(&i.value_of_one_pip)
            .map_err(|_| format!("valueOfOnePip inválido: {:?}", i.value_of_one_pip))?;
        let margin_pct = match (&i.margin_factor, i.margin_factor_unit.as_deref()) {
            (Some(m), Some("PERCENTAGE")) => Some(decimal(m)?),
            _ => None,
        };
        Ok(Self {
            epic: i.epic,
            market_id: i.market_id,
            name: i.name,
            instrument_type: i.instrument_type,
            currency,
            value_per_point,
            min_deal_size: decimal(&b.dealing_rules.min_deal_size.value)?,
            margin_pct,
        })
    }
}

/// Respuesta de `GET /clientsentiment?marketIds=…` (v1). La referencia oficial documenta la
/// ruta como `/client-sentiment`, pero la API demo responde 404 a esa ruta y 200 a
/// `/clientsentiment` (medido el 2026-11-20, #27).
#[derive(Deserialize)]
struct ClientSentimentBody {
    #[serde(rename = "clientSentiments")]
    client_sentiments: Vec<ClientSentimentItem>,
}

#[derive(Deserialize)]
struct ClientSentimentItem {
    #[serde(rename = "marketId")]
    market_id: String,
    #[serde(rename = "longPositionPercentage")]
    long: Box<RawValue>,
    #[serde(rename = "shortPositionPercentage")]
    short: Box<RawValue>,
}

/// % de clientes largos por `marketId`, validado. `None` = IG no tiene dato para ese mercado:
/// a un `marketId` desconocido responde 0 % largos y 0 % cortos, no un error (medido en la demo).
///
/// Es un error (no se adivina): respuesta vacía, un mercado repetido, uno que no se pidió o uno
/// pedido que falta, porcentajes fuera de 0–100, o largos + cortos ≠ 100.
pub fn client_sentiment(
    body: &str,
    requested: &[String],
) -> Result<BTreeMap<String, Option<Decimal>>, String> {
    let b: ClientSentimentBody =
        serde_json::from_str(body).map_err(|e| format!("GET /clientsentiment: {e}"))?;
    if b.client_sentiments.is_empty() {
        return Err("GET /clientsentiment: respuesta vacía".into());
    }
    let mut out = BTreeMap::new();
    for item in b.client_sentiments {
        let id = item.market_id;
        if !requested.contains(&id) {
            return Err(format!("{id}: llegó un mercado que no se pidió"));
        }
        let (long, short) = (decimal(&item.long)?, decimal(&item.short)?);
        // Si los dos son ≥ 0 y suman 100, ninguno pasa de 100: no hace falta otro tope.
        let pct = Decimal::ONE_HUNDRED;
        let reading = if long.is_zero() && short.is_zero() {
            None
        } else if long.is_sign_negative() || short.is_sign_negative() {
            return Err(format!(
                "{id}: porcentajes fuera de 0–100 ({long} / {short})"
            ));
        } else if long + short != pct {
            return Err(format!("{id}: largos + cortos = {} ≠ 100", long + short));
        } else {
            Some(long)
        };
        if out.insert(id.clone(), reading).is_some() {
            return Err(format!("{id}: mercado repetido en la respuesta"));
        }
    }
    if let Some(missing) = requested.iter().find(|r| !out.contains_key(*r)) {
        return Err(format!("{missing}: falta en la respuesta"));
    }
    Ok(out)
}

/// `ask − bid` del cierre de cada vela con inicio en `[from, to)`. Mismas reglas que
/// [`to_candles`]: UTC, alineada al timeframe y sin inventar si falta un lado.
pub fn close_spreads(
    prices: &[RawPrice],
    tf: Timeframe,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<(DateTime<Utc>, Decimal)>, String> {
    let step = tf.duration().num_seconds();
    let mut out = Vec::new();
    for p in prices {
        let start = snapshot_start(p, step, tf)?;
        if start < from || start >= to {
            continue;
        }
        let at = p.snapshot_time_utc.as_str();
        match (&p.close.bid, &p.close.ask) {
            (Some(b), Some(a)) => out.push((start, decimal(a)? - decimal(b)?)),
            _ => return Err(format!("closePrice sin bid o ask en {at}")),
        }
    }
    Ok(out)
}

/// Mediana exacta (promedio de los dos centrales si la cantidad es par). `None` si no hay datos.
pub fn median(mut values: Vec<Decimal>) -> Option<Decimal> {
    values.sort();
    let n = values.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(values[n / 2]),
        _ => Some((values[n / 2 - 1] + values[n / 2]) / Decimal::TWO),
    }
}

fn snapshot_start(p: &RawPrice, step: i64, tf: Timeframe) -> Result<DateTime<Utc>, String> {
    let at = p.snapshot_time_utc.as_str();
    let start = NaiveDateTime::parse_from_str(at, SNAPSHOT_UTC_FORMAT)
        .map_err(|_| format!("snapshotTimeUTC inválido: {at:?}"))?
        .and_utc();
    if start.timestamp().rem_euclid(step) != 0 {
        return Err(format!("vela {at} no alineada a {tf}"));
    }
    Ok(start)
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

    #[test]
    fn median_is_exact_for_odd_and_even_counts() {
        let d = |v: &[i64]| v.iter().map(|&x| Decimal::new(x, 1)).collect::<Vec<_>>();
        assert_eq!(median(d(&[])), None);
        assert_eq!(median(d(&[14])), Some(Decimal::new(14, 1)));
        assert_eq!(median(d(&[50, 14, 28])), Some(Decimal::new(28, 1)));
        assert_eq!(median(d(&[14, 50, 28, 14])), Some(Decimal::new(21, 1)));
    }

    #[test]
    fn client_sentiment_is_validated_not_guessed() {
        let ids = vec!["USTECH".to_string(), "DE30".to_string()];
        let body = |items: &str| format!(r#"{{"clientSentiments": [{items}]}}"#);
        let item = |id: &str, l: &str, s: &str| {
            format!(
                r#"{{"marketId":"{id}","longPositionPercentage":{l},"shortPositionPercentage":{s}}}"#
            )
        };
        let ok = body(&[item("USTECH", "68.0", "32.0"), item("DE30", "0.0", "0.0")].join(","));
        let got = client_sentiment(&ok, &ids).unwrap();
        assert_eq!(got["USTECH"], Some(Decimal::new(680, 1)));
        assert_eq!(got["DE30"], None, "0 % / 0 % = sin dato");
        // 0 % largos con 100 % cortos sí es un dato.
        let all_short = body(&[item("USTECH", "0", "100"), item("DE30", "100", "0")].join(","));
        let got = client_sentiment(&all_short, &ids).unwrap();
        assert_eq!(
            (got["USTECH"], got["DE30"]),
            (Some(Decimal::ZERO), Some(Decimal::ONE_HUNDRED))
        );

        for (case, b) in [
            ("vacía", body("")),
            (
                "repetido",
                body(
                    &[
                        item("USTECH", "68", "32"),
                        item("USTECH", "68", "32"),
                        item("DE30", "60", "40"),
                    ]
                    .join(","),
                ),
            ),
            (
                "no se pidió",
                body(&[item("USTECH", "68", "32"), item("WALL", "60", "40")].join(",")),
            ),
            ("falta", body(&item("USTECH", "68", "32"))),
            (
                "fuera de 0–100",
                body(&[item("USTECH", "101", "-1"), item("DE30", "60", "40")].join(",")),
            ),
            (
                "fuera de 0–100",
                body(&[item("USTECH", "-1", "101"), item("DE30", "60", "40")].join(",")),
            ),
            (
                "≠ 100",
                body(&[item("USTECH", "68", "30"), item("DE30", "60", "40")].join(",")),
            ),
            ("GET /clientsentiment", r#"{"otra": []}"#.to_string()),
        ] {
            let err = client_sentiment(&b, &ids).unwrap_err();
            assert!(err.contains(case), "{case}: {err}");
        }
    }
}

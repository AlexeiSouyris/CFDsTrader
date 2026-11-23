//! Gateway: API **de solo lectura** del motor para la UI (#231). Dos rutas:
//!
//! - `GET /api/v1/liquidity?instrument=<id>[&timeframe=15m|1h|1d]` → `contracts/liquidity_view.v1`:
//!   lo que ya está guardado en PostgreSQL (velas, niveles, FVG, desequilibrio y agregados) de
//!   ese timeframe, tal como se conocía al cierre de su última vela. Sin `timeframe`, 15m.
//! - `GET /api/v1/instruments` → `contracts/gateway_instruments.v1`: los instrumentos del
//!   catálogo, los timeframes y los valores por defecto de la vista.
//!
//! No calcula nada nuevo, no envía órdenes ni escribe en la base.

use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroU32;

use cfds_core::{
    Candle, DailyCandle, FvgEvent, Imbalance, Instrument, InstrumentId, LiquidityLevel,
    LiquidityStats, StatsTimeframe, Timeframe,
};
use cfds_storage::{PgStore, StorageError};
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

/// Los timeframes de la vista, en el orden en que se ofrecen.
pub const TIMEFRAMES: [StatsTimeframe; 3] =
    [StatsTimeframe::M15, StatsTimeframe::H1, StatsTimeframe::D1];

/// El nombre de un timeframe en los contratos y en la query (`15m`, `1h`, `1d`).
pub fn timeframe_name(tf: StatsTimeframe) -> &'static str {
    match tf {
        StatsTimeframe::M15 => "15m",
        StatsTimeframe::H1 => "1h",
        StatsTimeframe::D1 => "1d",
    }
}

fn parse_timeframe(s: &str) -> Option<StatsTimeframe> {
    TIMEFRAMES.into_iter().find(|&tf| timeframe_name(tf) == s)
}

/// `config/gateway.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayConfig {
    pub bind: SocketAddr,
    pub default_instrument: InstrumentId,
    pub default_timeframe: StatsTimeframe,
    /// Niveles con `exists_from` en esta ventana hacia atrás desde `as_of`.
    pub levels: TimeDelta,
    pub m15: Window,
    pub h1: Window,
    pub d1: Window,
}

/// Ventana de un timeframe, hacia atrás desde `as_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub candles: TimeDelta,
    pub fvgs: TimeDelta,
}

impl GatewayConfig {
    pub fn window(&self, tf: StatsTimeframe) -> Window {
        match tf {
            StatsTimeframe::M15 => self.m15,
            StatsTimeframe::H1 => self.h1,
            StatsTimeframe::D1 => self.d1,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    server: RawServer,
    view: RawView,
    windows: RawWindows,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawServer {
    bind: SocketAddr,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawView {
    default_instrument: String,
    default_timeframe: String,
    levels_days: NonZeroU32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindows {
    #[serde(rename = "15m")]
    m15: RawWindow,
    #[serde(rename = "1h")]
    h1: RawWindow,
    #[serde(rename = "1d")]
    d1: RawWindow,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindow {
    candles_days: NonZeroU32,
    fvg_days: NonZeroU32,
}

fn days(n: NonZeroU32) -> TimeDelta {
    TimeDelta::days(i64::from(n.get()))
}

impl GatewayConfig {
    /// Lee y valida `config/gateway.toml`: todos los campos, positivos y sin campos de más.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
        let w = |r: RawWindow| Window {
            candles: days(r.candles_days),
            fvgs: days(r.fvg_days),
        };
        Ok(Self {
            bind: raw.server.bind,
            default_instrument: raw
                .view
                .default_instrument
                .parse()
                .map_err(|e| format!("view.default_instrument: {e}"))?,
            default_timeframe: parse_timeframe(&raw.view.default_timeframe)
                .ok_or("view.default_timeframe: 15m, 1h o 1d")?,
            levels: days(raw.view.levels_days),
            m15: w(raw.windows.m15),
            h1: w(raw.windows.h1),
            d1: w(raw.windows.d1),
        })
    }
}

/// Las velas de la vista: `candle.v1` en 15m y 1h, `daily_candle.v1` en 1d.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Bars {
    Intraday(Vec<Candle>),
    Daily(Vec<DailyCandle>),
}

impl Bars {
    pub fn len(&self) -> usize {
        match self {
            Bars::Intraday(v) => v.len(),
            Bars::Daily(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Bars::Intraday(v) => v.is_empty(),
            Bars::Daily(v) => v.is_empty(),
        }
    }
}

/// `contracts/liquidity_view.v1`: mensajes de otros contratos, sin cambios.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiquidityView {
    pub instrument_id: InstrumentId,
    #[serde(serialize_with = "ser_timeframe")]
    pub timeframe: StatsTimeframe,
    pub generated_at: String,
    pub as_of: Option<String>,
    pub candles: Bars,
    pub levels: Vec<LiquidityLevel>,
    pub fvgs: Vec<FvgEvent>,
    pub imbalance: Vec<Imbalance>,
    pub stats: Vec<LiquidityStats>,
}

fn ser_timeframe<S: serde::Serializer>(tf: &StatsTimeframe, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(timeframe_name(*tf))
}

fn utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

impl LiquidityView {
    /// La vista de un instrumento sin velas guardadas en ese timeframe: "sin datos todavía".
    pub fn empty(instrument: InstrumentId, timeframe: StatsTimeframe, now: DateTime<Utc>) -> Self {
        Self {
            instrument_id: instrument,
            timeframe,
            generated_at: utc(now),
            as_of: None,
            candles: match timeframe {
                StatsTimeframe::D1 => Bars::Daily(Vec::new()),
                _ => Bars::Intraday(Vec::new()),
            },
            levels: Vec::new(),
            fvgs: Vec::new(),
            imbalance: Vec::new(),
            stats: Vec::new(),
        }
    }
}

/// `contracts/gateway_instruments.v1`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InstrumentsReply {
    pub default_instrument: InstrumentId,
    pub default_timeframe: &'static str,
    pub timeframes: Vec<&'static str>,
    pub instruments: Vec<Instrument>,
}

/// De dónde sale la vista. En producción, PostgreSQL ([`PgSource`]); en los tests, uno falso.
pub trait LiquiditySource: Sync {
    fn view(
        &self,
        instrument: &InstrumentId,
        timeframe: StatsTimeframe,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<LiquidityView, StorageError>> + Send;
}

/// La vista leída de PostgreSQL con las lecturas de `cfds-storage`, todas "tal como se conocían
/// en `as_of`" (sin mirar el futuro).
pub struct PgSource {
    pub store: PgStore,
    pub config: GatewayConfig,
    /// `[fvg_stats] reaction_bars` de `config/liquidity.toml`.
    pub reaction_bars: Vec<NonZeroU32>,
}

impl PgSource {
    /// Las velas cerradas de la ventana y `as_of` (cierre de la última), o `None` si no hay.
    async fn bars(
        &self,
        id: &InstrumentId,
        tf: StatsTimeframe,
        now: DateTime<Utc>,
    ) -> Result<Option<(Bars, DateTime<Utc>, DateTime<Utc>)>, StorageError> {
        let w = self.config.window(tf);
        let intraday = match tf {
            StatsTimeframe::M15 => Some(Timeframe::M15),
            StatsTimeframe::H1 => Some(Timeframe::H1),
            StatsTimeframe::D1 => None,
        };
        if let Some(t) = intraday {
            // Solo velas cerradas: la última que empezó a más tardar `now − duración`.
            let Some(last) = self
                .store
                .last_candle_start(id, t, now - t.duration())
                .await?
            else {
                return Ok(None);
            };
            let as_of = last + t.duration();
            let candles = self.store.candles(id, t, as_of - w.candles, as_of).await?;
            let first = candles.first().map_or(as_of, |c| c.start);
            return Ok(Some((Bars::Intraday(candles), as_of, first)));
        }
        // 1d: `as_of` es el fin de la última diaria **terminada** de `daily_candles`, aunque sea
        // vieja, y la ventana se mide desde ahí (no desde ahora).
        let Some(as_of) = self.store.last_daily_end(id, now).await? else {
            return Ok(None);
        };
        let from = (as_of - w.candles).date_naive();
        let daily: Vec<DailyCandle> = self
            .store
            .daily_candles(id, from, as_of.date_naive() + TimeDelta::days(1))
            .await?
            .into_iter()
            .filter(|d| d.end <= as_of)
            .collect();
        let Some(first) = daily.first().map(|d| d.start) else {
            return Ok(None);
        };
        Ok(Some((Bars::Daily(daily), as_of, first)))
    }
}

impl LiquiditySource for PgSource {
    async fn view(
        &self,
        id: &InstrumentId,
        tf: StatsTimeframe,
        now: DateTime<Utc>,
    ) -> Result<LiquidityView, StorageError> {
        let Some((candles, as_of, first)) = self.bars(id, tf, now).await? else {
            return Ok(LiquidityView::empty(id.clone(), tf, now));
        };
        let w = self.config.window(tf);
        let s = &self.store;
        let levels = s
            .liquidity_levels(id, as_of - self.config.levels, as_of)
            .await?;
        let fvgs = s
            .fvg_events(id, tf, as_of - w.fvgs, as_of, &self.reaction_bars)
            .await?;
        let imbalance = s.imbalances(id, tf, first, as_of).await?;
        // Los agregados del timeframe (en 1d, también los de los gaps de apertura), de la última
        // corrida hecha hasta ahora. Se busca por `computed_at` (la hora de la corrida, que es
        // posterior a los datos), así que se compara con `now` y no con `as_of`; cada documento
        // trae su propio `as_of`, que nunca pasa de la última vela guardada.
        let stats = s
            .latest_liquidity_stats(id, now)
            .await?
            .into_iter()
            .filter(|g| g.timeframe == tf && g.as_of <= as_of)
            .collect();
        Ok(LiquidityView {
            instrument_id: id.clone(),
            timeframe: tf,
            generated_at: utc(now),
            as_of: Some(utc(as_of)),
            candles,
            levels,
            fvgs,
            imbalance,
            stats,
        })
    }
}

/// `DATABASE_URL` con la sesión en **solo lectura** (`default_transaction_read_only`): el
/// gateway nunca escribe, y así PostgreSQL rechaza cualquier escritura aunque hubiera un error.
pub fn read_only_url(url: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}options=-c%20default_transaction_read_only%3Don")
}

/// Una respuesta HTTP ya armada: estado, tipo de contenido y cuerpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

fn json<T: Serialize>(value: &T, what: &str) -> Reply {
    match serde_json::to_string(value) {
        Ok(body) => Reply {
            status: 200,
            content_type: "application/json",
            body,
        },
        Err(e) => {
            eprintln!("gateway: no se pudo serializar {what}: {e}");
            text(500, "error interno")
        }
    }
}

fn param<'q>(query: Option<&'q str>, name: &str) -> Option<&'q str> {
    query?
        .split('&')
        .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
}

/// Atiende un pedido (ver el encabezado del módulo). Todo lo demás es 4xx. Los errores de la
/// base no se muestran al cliente.
pub async fn respond<S: LiquiditySource>(
    source: &S,
    config: &GatewayConfig,
    catalog: &[Instrument],
    method: &str,
    path: &str,
    query: Option<&str>,
    now: DateTime<Utc>,
) -> Reply {
    let known = ["/api/v1/liquidity", "/api/v1/instruments"];
    if !known.contains(&path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: el gateway es de solo lectura");
    }
    if path == "/api/v1/instruments" {
        let reply = InstrumentsReply {
            default_instrument: config.default_instrument.clone(),
            default_timeframe: timeframe_name(config.default_timeframe),
            timeframes: TIMEFRAMES.into_iter().map(timeframe_name).collect(),
            instruments: catalog.to_vec(),
        };
        return json(&reply, "los instrumentos");
    }
    let Some(Ok(id)) = param(query, "instrument").map(str::parse::<InstrumentId>) else {
        return text(400, "falta ?instrument=<id> válido");
    };
    let tf = match param(query, "timeframe") {
        None => StatsTimeframe::M15,
        Some(s) => match parse_timeframe(s) {
            Some(tf) => tf,
            None => return text(400, "timeframe: 15m, 1h o 1d"),
        },
    };
    if !catalog.iter().any(|i| i.id == id) {
        return text(404, "instrumento fuera del catálogo");
    }
    match source.view(&id, tf, now).await {
        Ok(view) => json(&view, &format!("la vista de {id}")),
        Err(e) => {
            eprintln!(
                "gateway: error leyendo la vista de {id} {}: {e}",
                timeframe_name(tf)
            );
            text(500, "error interno")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfds_core::{AssetClass, Currency};

    const CONFIG: &str = include_str!("../../../../config/gateway.toml");

    struct Fake;

    impl LiquiditySource for Fake {
        async fn view(
            &self,
            id: &InstrumentId,
            tf: StatsTimeframe,
            now: DateTime<Utc>,
        ) -> Result<LiquidityView, StorageError> {
            Ok(LiquidityView::empty(id.clone(), tf, now))
        }
    }

    fn now() -> DateTime<Utc> {
        "2026-11-23T10:00:00Z".parse().unwrap()
    }

    fn config() -> GatewayConfig {
        GatewayConfig::from_toml(CONFIG).unwrap()
    }

    fn catalog() -> Vec<Instrument> {
        vec![Instrument {
            id: "US100".parse().unwrap(),
            name: "Test".into(),
            asset_class: AssetClass::Index,
            currency: Currency::EUR,
            exchange_timezone: "America/New_York".into(),
        }]
    }

    async fn get(path: &str, q: Option<&str>) -> Reply {
        respond(&Fake, &config(), &catalog(), "GET", path, q, now()).await
    }

    #[test]
    fn database_session_is_read_only() {
        let ro = "options=-c%20default_transaction_read_only%3Don";
        assert_eq!(
            read_only_url("postgres://h/db"),
            format!("postgres://h/db?{ro}")
        );
        assert_eq!(
            read_only_url("postgres://h/db?sslmode=disable"),
            format!("postgres://h/db?sslmode=disable&{ro}")
        );
    }

    #[test]
    fn reads_the_repo_config() {
        let cfg = config();
        assert!(
            cfg.bind.ip().is_loopback(),
            "por defecto solo la máquina local"
        );
        assert_eq!(cfg.default_instrument.as_str(), "DE40");
        assert_eq!(cfg.default_timeframe, StatsTimeframe::D1);
        assert!(cfg.d1.candles >= TimeDelta::days(250));
    }

    #[test]
    fn rejects_zero_bad_timeframe_and_unknown_fields() {
        let zero = CONFIG.replacen("candles_days = 3", "candles_days = 0", 1);
        assert_ne!(zero, CONFIG);
        assert!(GatewayConfig::from_toml(&zero).is_err());
        let bad = CONFIG.replace("default_timeframe = \"1d\"", "default_timeframe = \"4h\"");
        assert_ne!(bad, CONFIG);
        assert!(GatewayConfig::from_toml(&bad).is_err());
        let extra = format!("{CONFIG}\nnope = 1\n");
        assert!(GatewayConfig::from_toml(&extra).is_err());
    }

    #[tokio::test]
    async fn empty_database_is_sin_datos_for_each_timeframe() {
        for (q, tf) in [
            ("instrument=US100", "15m"),
            ("instrument=US100&timeframe=1h", "1h"),
            ("timeframe=1d&instrument=US100", "1d"),
        ] {
            let r = get("/api/v1/liquidity", Some(q)).await;
            assert_eq!(r.status, 200, "{q}");
            let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
            assert_eq!(v["timeframe"], tf);
            assert_eq!(v["as_of"], serde_json::Value::Null);
            assert_eq!(v["candles"], serde_json::json!([]));
        }
    }

    #[tokio::test]
    async fn instruments_come_from_the_catalog_with_the_defaults() {
        let r = get("/api/v1/instruments", None).await;
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["default_instrument"], "DE40");
        assert_eq!(v["default_timeframe"], "1d");
        assert_eq!(v["timeframes"], serde_json::json!(["15m", "1h", "1d"]));
        assert_eq!(v["instruments"][0]["id"], "US100");
    }

    #[tokio::test]
    async fn only_get_on_known_routes_with_valid_params() {
        let c = config();
        let cat = catalog();
        let q = Some("instrument=US100");
        let path = "/api/v1/liquidity";
        let post = respond(&Fake, &c, &cat, "POST", path, q, now()).await;
        assert_eq!(post.status, 405);
        assert_eq!(get("/otra", q).await.status, 404);
        assert_eq!(get(path, None).await.status, 400);
        assert_eq!(get(path, Some("instrument=us100;drop")).await.status, 400);
        assert_eq!(
            get(path, Some("instrument=US100&timeframe=4h"))
                .await
                .status,
            400
        );
        assert_eq!(get(path, Some("instrument=DE40")).await.status, 404);
        // un prefijo parecido no cuenta como el parámetro
        assert_eq!(get(path, Some("instruments=US100")).await.status, 400);
    }
}

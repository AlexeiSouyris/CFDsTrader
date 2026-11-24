//! Gateway: API **de solo lectura** del motor para la UI (#231). Dos rutas:
//!
//! - `GET /api/v1/liquidity?instrument=<id>[&timeframe=15m|1h|1d][&source=ig|futures][&before=<UTC>]`
//!   → `contracts/liquidity_view.v1`: lo que ya está guardado en PostgreSQL (velas, niveles, FVG,
//!   desequilibrio y agregados) de ese timeframe, tal como se conocía al cierre de su última vela.
//!   Sin `timeframe`, 15m. Sin `source`, las velas del CFD de IG; con `futures`, lo calculado
//!   con las velas de los futuros (#259, guardado bajo el instrumento derivado de
//!   `config/candle_sources.toml`). La respuesta dice de qué fuente es. Con `before` (RFC 3339 en
//!   UTC, terminado en `Z`), la misma vista tal como se conocía en ese momento: solo velas que
//!   cerraron a más tardar en `before`. La UI lo usa para pedir historia al desplazarse al
//!   pasado (#401, paginación por fecha). Un `before` futuro cuenta como ahora.
//! - `GET /api/v1/instruments` → `contracts/gateway_instruments.v1`: los instrumentos del
//!   catálogo, los timeframes y los valores por defecto de la vista.
//!
//! En 15m con las velas de IG, la vista trae además el bloque opcional `analysis` (#501): las
//! herramientas de análisis tal como las arma `cfds_backtest::analysis` (el mismo bloque de
//! `chart_export.v1`), sobre las velas de la vista. Solo se muestran: no entran al puntaje.
//!
//! No calcula nada nuevo, no envía órdenes ni escribe en la base.

pub mod context;
pub mod emotions;
pub mod fund;
pub mod learning;
pub mod ml;
pub mod ops;
pub mod ops_store;
pub mod paper;
pub mod paper_runner;
pub mod status;
pub mod walk_forward;

use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroU32;

use cfds_backtest::{AnalysisConfig, ExportAnalysis, analysis_export};
use cfds_core::{
    Candle, DailyCandle, FvgEvent, Imbalance, Instrument, InstrumentId, LiquidityLevel,
    LiquidityStats, StatsTimeframe, Timeframe,
};
use cfds_market_data::{CandleSource, CandleSources};
use cfds_storage::{CandleFeed, PgStore, StorageError};
use chrono::{DateTime, NaiveDate, SecondsFormat, TimeDelta, Utc};
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
    /// `[paper]`: lo lee [`paper::PaperConfig`].
    #[allow(dead_code)]
    paper: toml::Table,
    /// `[context]`: lo lee [`context::ContextConfig`].
    #[allow(dead_code)]
    context: toml::Table,
    /// `[ops]`: lo lee [`status::StatusConfig`] (#314).
    #[allow(dead_code)]
    ops: toml::Table,
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

/// De qué velas sale la vista (#259): `id` (`ig` o `futures`) y su etiqueta para la UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ViewSource {
    pub id: &'static str,
    pub label: String,
}

impl ViewSource {
    pub fn new(source: CandleSource, sources: &CandleSources) -> Self {
        Self {
            id: source.as_str(),
            label: sources.label(source).to_owned(),
        }
    }
}

/// `contracts/liquidity_view.v1`: mensajes de otros contratos, sin cambios.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiquidityView {
    pub instrument_id: InstrumentId,
    pub source: ViewSource,
    #[serde(serialize_with = "ser_timeframe")]
    pub timeframe: StatsTimeframe,
    pub generated_at: String,
    pub as_of: Option<String>,
    pub candles: Bars,
    pub levels: Vec<LiquidityLevel>,
    pub fvgs: Vec<FvgEvent>,
    pub imbalance: Vec<Imbalance>,
    pub stats: Vec<LiquidityStats>,
    /// Herramientas de análisis (#501), un valor por vela de `candles` donde corresponde. Solo
    /// en 15m con las velas de IG ([`has_analysis`]); ausente en los demás casos.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<ExportAnalysis>,
}

/// Si la vista lleva `analysis` (#501): solo en 15m (el timeframe para el que están
/// parametrizadas en `config/analysis.toml`) y con las velas del CFD de IG (los futuros no
/// tienen diarias).
pub fn has_analysis(source: CandleSource, tf: StatsTimeframe) -> bool {
    source == CandleSource::Ig && tf == StatsTimeframe::M15
}

/// Lo que necesita el bloque `analysis`: `config/analysis.toml` y `[pivots] strength` de
/// `config/patterns.toml` (los mismos pivotes que el export).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewAnalysis {
    pub config: AnalysisConfig,
    pub pivot_strength: usize,
}

impl ViewAnalysis {
    pub fn from_toml(analysis: &str, patterns: &str) -> Result<Self, String> {
        Ok(Self {
            config: AnalysisConfig::from_toml(analysis).map_err(|e| format!("analysis: {e}"))?,
            pivot_strength: cfds_patterns::PatternsConfig::from_toml(patterns)
                .map_err(|e| format!("patterns: {e}"))?
                .pivots
                .strength,
        })
    }

    /// El bloque sobre las velas de la vista. `daily` y `vix` se leen hasta `as_of`; las funciones
    /// de `cfds_backtest::analysis` usan para cada vela solo lo ya cerrado o publicado a su cierre.
    pub fn block(
        &self,
        m15: &[Candle],
        daily: &[DailyCandle],
        vix: &[(DateTime<Utc>, rust_decimal::Decimal)],
    ) -> ExportAnalysis {
        analysis_export(m15, daily, vix, self.pivot_strength, &self.config)
    }
}

fn ser_timeframe<S: serde::Serializer>(tf: &StatsTimeframe, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(timeframe_name(*tf))
}

fn utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

impl LiquidityView {
    /// La vista de un instrumento sin velas guardadas en ese timeframe: "sin datos todavía".
    pub fn empty(
        instrument: InstrumentId,
        source: ViewSource,
        timeframe: StatsTimeframe,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            instrument_id: instrument,
            source,
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
            analysis: None,
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
    /// Fuentes de velas de la vista (#259), la primera es la que se usa sin `source`.
    pub sources: Vec<ViewSource>,
}

/// De dónde sale la vista. En producción, PostgreSQL ([`PgSource`]); en los tests, uno falso.
pub trait LiquiditySource: Sync {
    fn view(
        &self,
        instrument: &InstrumentId,
        source: CandleSource,
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
    /// `config/candle_sources.toml`.
    pub sources: CandleSources,
    /// Herramientas de análisis (#501); `None` = la vista no las trae.
    pub analysis: Option<ViewAnalysis>,
}

impl PgSource {
    /// Las velas cerradas de la ventana y `as_of` (cierre de la última), o `None` si no hay.
    async fn bars(
        &self,
        id: &InstrumentId,
        out: &InstrumentId,
        source: CandleSource,
        tf: StatsTimeframe,
        now: DateTime<Utc>,
    ) -> Result<Option<(Bars, DateTime<Utc>, DateTime<Utc>)>, StorageError> {
        let feed = match source {
            CandleSource::Ig => CandleFeed::Ig,
            CandleSource::Futures => CandleFeed::Futures(&self.sources.futures_source),
        };
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
                .last_candle_start_from(feed, id, t, now - t.duration())
                .await?
            else {
                return Ok(None);
            };
            let as_of = last + t.duration();
            let candles = self
                .store
                .candles_from(feed, id, out, t, as_of - w.candles, as_of)
                .await?;
            let first = candles.first().map_or(as_of, |c| c.start);
            return Ok(Some((Bars::Intraday(candles), as_of, first)));
        }
        // Los futuros no tienen diarias (#259): en 1d esa fuente queda "sin datos".
        if source == CandleSource::Futures {
            return Ok(None);
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

    /// El bloque `analysis` (#501) de las velas de 15m de la vista. La EMA diaria necesita los
    /// días anteriores a la ventana (calentamiento): se leen todas las diarias guardadas hasta
    /// `as_of`, como en el export. El VIX, desde dos semanas antes de la primera vela.
    async fn analysis(
        &self,
        va: &ViewAnalysis,
        id: &InstrumentId,
        m15: &[Candle],
        as_of: DateTime<Utc>,
    ) -> Result<ExportAnalysis, StorageError> {
        let first_day = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap_or(NaiveDate::MIN);
        let daily: Vec<DailyCandle> = self
            .store
            .daily_candles(id, first_day, as_of.date_naive() + TimeDelta::days(1))
            .await?
            .into_iter()
            .filter(|d| d.end <= as_of)
            .collect();
        let from = m15.first().map_or(as_of, |c| c.start) - TimeDelta::days(14);
        // `vix_closes` deja afuera `to`: un segundo más para incluir lo publicado justo en `as_of`.
        let vix = self
            .store
            .vix_closes(from, as_of + TimeDelta::seconds(1))
            .await?;
        Ok(va.block(m15, &daily, &vix))
    }
}

impl LiquiditySource for PgSource {
    async fn view(
        &self,
        base: &InstrumentId,
        source: CandleSource,
        tf: StatsTimeframe,
        now: DateTime<Utc>,
    ) -> Result<LiquidityView, StorageError> {
        let view_source = ViewSource::new(source, &self.sources);
        // Lo calculado con los futuros se guardó bajo el instrumento derivado (#259).
        let out = self
            .sources
            .instrument_for(source, base)
            .map_err(StorageError::CorruptRow)?;
        let id = &out;
        let Some((candles, as_of, first)) = self.bars(base, id, source, tf, now).await? else {
            return Ok(LiquidityView::empty(base.clone(), view_source, tf, now));
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
        let analysis = match (&self.analysis, &candles) {
            (Some(va), Bars::Intraday(m15)) if has_analysis(source, tf) && !m15.is_empty() => {
                Some(self.analysis(va, id, m15, as_of).await?)
            }
            _ => None,
        };
        Ok(LiquidityView {
            instrument_id: base.clone(),
            source: view_source,
            timeframe: tf,
            generated_at: utc(now),
            as_of: Some(utc(as_of)),
            candles,
            levels,
            fvgs,
            imbalance,
            stats,
            analysis,
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

pub(crate) fn param<'q>(query: Option<&'q str>, name: &str) -> Option<&'q str> {
    query?
        .split('&')
        .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
}

/// `before` de la query: RFC 3339 en UTC (`…Z`); los `:` pueden llegar codificados (`%3A`).
fn parse_before(s: &str) -> Option<DateTime<Utc>> {
    let s = s.replace("%3A", ":").replace("%3a", ":");
    if !s.ends_with('Z') {
        return None;
    }
    s.parse::<DateTime<Utc>>().ok()
}

/// Atiende un pedido (ver el encabezado del módulo). Todo lo demás es 4xx. Los errores de la
/// base no se muestran al cliente.
#[allow(clippy::too_many_arguments)]
pub async fn respond<S: LiquiditySource>(
    source: &S,
    config: &GatewayConfig,
    sources: &CandleSources,
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
            sources: CandleSource::ALL
                .into_iter()
                .map(|s| ViewSource::new(s, sources))
                .collect(),
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
    let candle_source = match param(query, "source") {
        None => CandleSource::Ig,
        Some(s) => match CandleSource::parse(s) {
            Some(c) => c,
            None => return text(400, "source: ig o futures"),
        },
    };
    // `before` (#401): la vista "tal como se conocía" en ese momento, nunca después de ahora.
    let at = match param(query, "before") {
        None => now,
        Some(s) => match parse_before(s) {
            Some(t) => t.min(now),
            None => return text(400, "before: fecha RFC 3339 en UTC (…Z)"),
        },
    };
    if !catalog.iter().any(|i| i.id == id) {
        return text(404, "instrumento fuera del catálogo");
    }
    match source.view(&id, candle_source, tf, at).await {
        Ok(mut view) => {
            // `generated_at` es cuándo se armó la respuesta, aunque se lea el pasado.
            view.generated_at = utc(now);
            json(&view, &format!("la vista de {id}"))
        }
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
            source: CandleSource,
            tf: StatsTimeframe,
            now: DateTime<Utc>,
        ) -> Result<LiquidityView, StorageError> {
            let s = ViewSource::new(source, &sources());
            Ok(LiquidityView::empty(id.clone(), s, tf, now))
        }
    }

    fn now() -> DateTime<Utc> {
        "2026-11-23T10:00:00Z".parse().unwrap()
    }

    fn config() -> GatewayConfig {
        GatewayConfig::from_toml(CONFIG).unwrap()
    }

    fn sources() -> CandleSources {
        CandleSources::from_toml(include_str!("../../../../config/candle_sources.toml")).unwrap()
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
        respond(
            &Fake,
            &config(),
            &sources(),
            &catalog(),
            "GET",
            path,
            q,
            now(),
        )
        .await
    }

    #[test]
    fn analysis_only_in_15m_from_ig() {
        assert!(has_analysis(CandleSource::Ig, StatsTimeframe::M15));
        assert!(!has_analysis(CandleSource::Ig, StatsTimeframe::H1));
        assert!(!has_analysis(CandleSource::Ig, StatsTimeframe::D1));
        assert!(!has_analysis(CandleSource::Futures, StatsTimeframe::M15));
        // Sin bloque, la respuesta no trae la clave (la forma de antes, #501).
        let s = ViewSource::new(CandleSource::Ig, &sources());
        let v = LiquidityView::empty("US100".parse().unwrap(), s, StatsTimeframe::M15, now());
        let doc = serde_json::to_value(&v).unwrap();
        assert!(doc.get("analysis").is_none());
        let va = ViewAnalysis::from_toml(
            include_str!("../../../../config/analysis.toml"),
            include_str!("../../../../config/patterns.toml"),
        )
        .unwrap();
        assert_eq!(va.config.ema_daily_period, 365);
        assert!(ViewAnalysis::from_toml("x = 1", "").is_err());
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
        let extra = format!("nope = 1\n{CONFIG}");
        // Una clave de más dentro de [paper] o de [context] la rechaza su propia config.
        let in_paper = CONFIG.replace("runs_limit = 50", "runs_limit = 50\nnope = 1");
        assert_ne!(in_paper, CONFIG);
        assert!(crate::paper::PaperConfig::from_toml(&in_paper).is_err());
        let in_context = format!("{CONFIG}\nnope = 1\n");
        assert!(crate::context::ContextConfig::from_toml(&in_context).is_err());
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
        assert_eq!(
            v["sources"],
            serde_json::json!([
                {"id": "ig", "label": "IG CFD"},
                {"id": "futures", "label": "futuros CME/Eurex (base distinta al CFD)"}
            ])
        );
    }

    #[tokio::test]
    async fn the_view_says_which_candles_it_comes_from() {
        let path = "/api/v1/liquidity";
        for (q, id, label) in [
            ("instrument=US100", "ig", "IG CFD"),
            ("instrument=US100&source=ig", "ig", "IG CFD"),
            (
                "instrument=US100&source=futures",
                "futures",
                "futuros CME/Eurex (base distinta al CFD)",
            ),
        ] {
            let r = get(path, Some(q)).await;
            assert_eq!(r.status, 200, "{q}");
            let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
            assert_eq!(v["source"]["id"], id, "{q}");
            assert_eq!(v["source"]["label"], label, "{q}");
            assert_eq!(v["instrument_id"], "US100", "{q}");
        }
        assert_eq!(
            get(path, Some("instrument=US100&source=cfd")).await.status,
            400
        );
    }

    /// Devuelve en `as_of` el momento con que se leyó la vista, para ver qué llega a la fuente.
    struct Echo;

    impl LiquiditySource for Echo {
        async fn view(
            &self,
            id: &InstrumentId,
            source: CandleSource,
            tf: StatsTimeframe,
            at: DateTime<Utc>,
        ) -> Result<LiquidityView, StorageError> {
            let s = ViewSource::new(source, &sources());
            let mut v = LiquidityView::empty(id.clone(), s, tf, at);
            v.as_of = Some(utc(at));
            Ok(v)
        }
    }

    async fn echo(q: &str) -> Reply {
        let path = "/api/v1/liquidity";
        respond(
            &Echo,
            &config(),
            &sources(),
            &catalog(),
            "GET",
            path,
            Some(q),
            now(),
        )
        .await
    }

    #[tokio::test]
    async fn before_reads_the_view_as_known_at_that_time() {
        // sin before: ahora
        let r = echo("instrument=US100&timeframe=1h").await;
        let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["as_of"], "2026-11-23T10:00:00Z");
        // con before (también con los ':' codificados): ese momento; generated_at sigue siendo ahora
        for q in [
            "instrument=US100&timeframe=1h&before=2026-11-20T08:15:00Z",
            "instrument=US100&before=2026-11-20T08%3A15%3A00Z&timeframe=1h",
        ] {
            let r = echo(q).await;
            assert_eq!(r.status, 200, "{q}");
            let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
            assert_eq!(v["as_of"], "2026-11-20T08:15:00Z", "{q}");
            assert_eq!(v["generated_at"], "2026-11-23T10:00:00Z", "{q}");
            assert_eq!(v["timeframe"], "1h", "{q}");
        }
        // un before futuro no deja mirar más allá de ahora
        let r = echo("instrument=US100&before=2030-01-01T00:00:00Z").await;
        let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["as_of"], "2026-11-23T10:00:00Z");
        // fechas mal formadas o sin zona UTC
        for bad in [
            "ayer",
            "2026-11-20T08:15:00",
            "2026-11-20T08:15:00+01:00",
            "",
        ] {
            let r = echo(&format!("instrument=US100&before={bad}")).await;
            assert_eq!(r.status, 400, "{bad}");
        }
    }

    #[tokio::test]
    async fn only_get_on_known_routes_with_valid_params() {
        let c = config();
        let cat = catalog();
        let q = Some("instrument=US100");
        let path = "/api/v1/liquidity";
        let post = respond(&Fake, &c, &sources(), &cat, "POST", path, q, now()).await;
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

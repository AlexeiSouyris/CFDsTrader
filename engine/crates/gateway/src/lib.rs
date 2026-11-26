//! Gateway: API **de solo lectura** del motor para la UI (#231). Dos rutas:
//!
//! - `GET /api/v1/liquidity?instrument=<id>[&timeframe=15m|1h|1d][&source=ig|futures|ibkr|auto][&before=<UTC>]`
//!   → `contracts/liquidity_view.v1`: lo que ya está guardado en PostgreSQL (velas, niveles, FVG,
//!   desequilibrio y agregados) de ese timeframe, tal como se conocía al cierre de su última vela.
//!   Sin `timeframe`, 15m. Sin `source`, las velas del CFD de IG; con `futures`, lo calculado
//!   con las velas de los futuros (#259, guardado bajo el instrumento derivado de
//!   `config/candle_sources.toml`). La respuesta dice de qué fuente es. Con `before` (RFC 3339 en
//!   UTC, terminado en `Z`), la misma vista tal como se conocía en ese momento: solo velas que
//!   cerraron a más tardar en `before`. La UI lo usa para pedir historia al desplazarse al
//!   pasado (#401, paginación por fecha). Un `before` futuro cuenta como ahora.
//!   Con `source=ibkr` (#630), lo calculado con las barras de IBKR de los CFD con mapeo
//!   (`[ibkr]` de `config/candle_sources.toml`). Con `source=auto` (#628), las velas de IG y,
//!   si en la ventana no hay suficientes, la primera
//!   fuente de reemplazo de `[view.auto]` de `config/gateway.toml` que tenga velas: la respuesta
//!   dice cuál usó (`source`) y por qué (`fallback`).
//! - `GET /api/v1/instruments` → `contracts/gateway_instruments.v1`: los instrumentos del
//!   catálogo, los timeframes y los valores por defecto de la vista.
//!
//! En 15m con las velas de IG, la vista trae además el bloque opcional `analysis` (#501): las
//! herramientas de análisis tal como las arma `cfds_backtest::analysis` (el mismo bloque de
//! `chart_export.v1`). Se calculan con `[warmup]` velas de 15m antes de la primera de la vista
//! (`config/analysis.toml`) y se devuelve solo lo de la vista (#511), también con `before`; las
//! diarias y el VIX salen de un caché en memoria ([`series_cache`]). Solo se muestran: no
//! entran al puntaje.
//!
//! No calcula nada nuevo, no envía órdenes ni escribe en la base.

pub mod academy;
pub mod academy_replay;
pub mod context;
pub mod emotions;
pub mod fund;
pub mod ibkr;
pub mod ibkr_live;
pub mod journal;
pub mod lab;
pub mod lab_risk;
pub mod lab_sims;
pub mod learning;
pub mod ml;
pub mod news;
pub mod ops;
pub mod ops_store;
pub mod paper;
pub mod paper_runner;
pub mod series_cache;
pub mod status;
pub mod walk_forward;

use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroU32;

use cfds_backtest::{AnalysisConfig, ExportAnalysis, analysis_export, analysis_view};
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
    /// `[view.auto]` (#628): la fuente automática.
    pub auto: AutoSource,
}

/// La fuente automática de la vista (#628): IG y, si en la ventana hay menos de `min_candles`
/// velas, la primera de `fallback` que tenga velas, con su motivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoSource {
    pub label: String,
    pub min_candles: usize,
    /// Fuentes de reemplazo en orden, cada una con el motivo que muestra la UI.
    pub fallback: Vec<(CandleSource, String)>,
}

impl AutoSource {
    fn from_raw(raw: RawAuto) -> Result<Self, String> {
        let RawAuto {
            label,
            min_candles,
            fallback,
            mut notes,
        } = raw;
        if !(1..=80).contains(&label.chars().count()) {
            return Err("view.auto.label: de 1 a 80 caracteres".into());
        }
        if fallback.is_empty() {
            return Err("view.auto.fallback: hace falta al menos una fuente".into());
        }
        let mut out: Vec<(CandleSource, String)> = Vec::with_capacity(fallback.len());
        for name in fallback {
            let s = CandleSource::parse(&name)
                .filter(|s| *s != CandleSource::Ig)
                .ok_or_else(|| format!("view.auto.fallback: fuente inválida {name:?}"))?;
            if out.iter().any(|(o, _)| *o == s) {
                return Err(format!("view.auto.fallback: {name:?} repetida"));
            }
            let note = notes
                .remove(s.as_str())
                .ok_or_else(|| format!("view.auto.notes.{}: falta", s.as_str()))?;
            if !(1..=300).contains(&note.chars().count()) {
                return Err(format!(
                    "view.auto.notes.{}: de 1 a 300 caracteres",
                    s.as_str()
                ));
            }
            out.push((s, note));
        }
        if let Some(k) = notes.keys().next() {
            return Err(format!("view.auto.notes.{k}: no está en fallback"));
        }
        Ok(Self {
            label,
            min_candles: min_candles.get() as usize,
            fallback: out,
        })
    }
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
    /// `[ibkr]`: lo lee [`ibkr::IbkrViewConfig`] (#551).
    #[allow(dead_code)]
    ibkr: toml::Table,
    /// `[news]` y `[calendar]`: los lee [`news::NewsConfig`] (#600).
    #[allow(dead_code)]
    news: toml::Table,
    #[allow(dead_code)]
    calendar: toml::Table,
    /// `[academy]`: lo lee [`academy::AcademyRoutes`] (#719).
    #[allow(dead_code)]
    academy: toml::Table,
    /// `[lab]`: lo lee [`lab::LabRoutes`] (#776).
    #[allow(dead_code)]
    lab: toml::Table,
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
    auto: RawAuto,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAuto {
    label: String,
    min_candles: NonZeroU32,
    fallback: Vec<String>,
    notes: std::collections::BTreeMap<String, String>,
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
            auto: AutoSource::from_raw(raw.view.auto)?,
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
    /// La fuente automática (#628), solo en la lista de `GET /api/v1/instruments`: una vista
    /// siempre dice la fuente que de verdad usó.
    pub fn auto(auto: &AutoSource) -> Self {
        Self {
            id: "auto",
            label: auto.label.clone(),
        }
    }

    pub fn new(source: CandleSource, sources: &CandleSources) -> Self {
        Self {
            id: source.as_str(),
            label: sources.label(source).to_owned(),
        }
    }
}

/// Por qué la vista no sale de las velas de IG con `source=auto` (#628).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Fallback {
    /// La fuente que se pidió primero (`ig`).
    pub from: &'static str,
    /// El motivo, de `[view.auto.notes]`.
    pub note: String,
}

/// `contracts/liquidity_view.v1`: mensajes de otros contratos, sin cambios.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiquidityView {
    pub instrument_id: InstrumentId,
    pub source: ViewSource,
    /// Solo con `source=auto` cuando la vista sale de una fuente de reemplazo (#628).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Fallback>,
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

    /// El bloque de la vista `view` calculado con las velas de calentamiento `warmup` (las
    /// anteriores a la primera de la vista, en orden, #511): índices de `view`, nada de antes.
    pub fn view_block(
        &self,
        warmup: &[Candle],
        view: &[Candle],
        daily: &[DailyCandle],
        vix: &[(DateTime<Utc>, rust_decimal::Decimal)],
    ) -> ExportAnalysis {
        let all: Vec<Candle> = warmup.iter().chain(view).cloned().collect();
        analysis_view(
            &all,
            warmup.len(),
            daily,
            vix,
            self.pivot_strength,
            &self.config,
        )
    }

    /// Las últimas `[warmup] m15_bars` de `before` (velas que empezaron antes de la primera de
    /// la vista), en orden.
    pub fn warmup_tail(&self, mut before: Vec<Candle>) -> Vec<Candle> {
        let n = self.config.warmup.m15_bars;
        if before.len() > n {
            before.drain(..before.len() - n);
        }
        before
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
            fallback: None,
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
    /// Diarias y VIX ya leídos para `analysis` (#511).
    pub cache: series_cache::SeriesCache,
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
            // Un CFD sin mapeo no tiene velas de IBKR (#630): "sin datos".
            CandleSource::Ibkr => match self.sources.ibkr_instrument(id) {
                Some(instrument) => CandleFeed::Ibkr(cfds_storage::IbkrFeed {
                    source: &self.sources.ibkr.source,
                    instrument,
                    what_to_show: &self.sources.ibkr.what_to_show,
                }),
                None => return Ok(None),
            },
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
        // Los futuros (#259) e IBKR (#630) no tienen diarias: en 1d esas fuentes quedan "sin datos".
        if source != CandleSource::Ig {
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

    /// El bloque `analysis` (#501) de las velas de 15m de la vista. Pivotes, Fibonacci,
    /// spring/upthrust y fondos se calculan con `[warmup]` velas anteriores (#511). La EMA y las
    /// medias diarias necesitan los días anteriores a la ventana: todas las diarias guardadas
    /// hasta `as_of`, como en el export, del caché; el VIX publicado hasta `as_of`, también.
    async fn analysis(
        &self,
        va: &ViewAnalysis,
        feed: CandleFeed<'_>,
        base: &InstrumentId,
        id: &InstrumentId,
        m15: &[Candle],
        as_of: DateTime<Utc>,
    ) -> Result<ExportAnalysis, StorageError> {
        let first = m15.first().map_or(as_of, |c| c.start);
        let lookback = TimeDelta::days(i64::from(va.config.warmup.max_lookback_days));
        let before = self
            .store
            .candles_from(feed, base, id, Timeframe::M15, first - lookback, first)
            .await?;
        let warmup = va.warmup_tail(before);
        let daily = self.daily_known(id, as_of).await?;
        let vix = self.vix_known(as_of).await?;
        Ok(va.view_block(&warmup, m15, &daily, &vix))
    }

    /// Las diarias terminadas hasta `as_of`, del caché; se vuelven a leer todas solo si en la
    /// base hay un día terminado posterior al último del caché.
    async fn daily_known(
        &self,
        id: &InstrumentId,
        as_of: DateTime<Utc>,
    ) -> Result<Vec<DailyCandle>, StorageError> {
        let last_end = self.store.last_daily_end(id, as_of).await?;
        if let Some(days) = self.cache.daily_known(id, as_of, last_end) {
            return Ok(days);
        }
        let first_day = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap_or(NaiveDate::MIN);
        let days = self
            .store
            .daily_candles(id, first_day, as_of.date_naive() + TimeDelta::days(1))
            .await?;
        Ok(self.cache.store_daily(id, days, as_of))
    }

    /// El VIX publicado hasta `as_of`: se leen solo los cierres desde el último del caché.
    async fn vix_known(
        &self,
        as_of: DateTime<Utc>,
    ) -> Result<Vec<series_cache::VixClose>, StorageError> {
        let from = self.cache.vix_last().unwrap_or_else(|| {
            NaiveDate::from_ymd_opt(2000, 1, 1)
                .unwrap_or(NaiveDate::MIN)
                .and_time(chrono::NaiveTime::MIN)
                .and_utc()
        });
        // `vix_closes` deja afuera `to`: un segundo más para incluir lo publicado justo en `as_of`.
        let read = if from <= as_of {
            self.store
                .vix_closes(from, as_of + TimeDelta::seconds(1))
                .await?
        } else {
            Vec::new()
        };
        Ok(self.cache.add_vix(&read, as_of))
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
            (Some(va), Bars::Intraday(m15)) if has_analysis(source, tf) && !m15.is_empty() => Some(
                self.analysis(va, CandleFeed::Ig, base, id, m15, as_of)
                    .await?,
            ),
            _ => None,
        };
        Ok(LiquidityView {
            instrument_id: base.clone(),
            source: view_source,
            fallback: None,
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
pub(crate) fn parse_before(s: &str) -> Option<DateTime<Utc>> {
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
            // La automática primero (#628): es la que abre la UI.
            sources: std::iter::once(ViewSource::auto(&config.auto))
                .chain(
                    CandleSource::ALL
                        .into_iter()
                        .map(|s| ViewSource::new(s, sources)),
                )
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
    // `None` = automática (#628).
    let candle_source = match param(query, "source") {
        None => Some(CandleSource::Ig),
        Some("auto") => None,
        Some(s) => match CandleSource::parse(s) {
            Some(c) => Some(c),
            None => return text(400, "source: ig, futures, ibkr o auto"),
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
    let view = match candle_source {
        Some(c) => source.view(&id, c, tf, at).await,
        None => auto_view(source, &config.auto, sources, &id, tf, at).await,
    };
    match view {
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

/// La vista con `source=auto` (#628): la de IG si tiene al menos `min_candles` velas; si no, la
/// de la primera fuente de reemplazo con alguna vela, con su motivo; si ninguna tiene, la de IG.
/// Cada fuente se lee con sus propias velas cerradas hasta `at`: no se mezclan. En el motivo,
/// `{symbol}` es el instrumento de IBKR del CFD (#630), si tiene.
pub async fn auto_view<S: LiquiditySource>(
    source: &S,
    auto: &AutoSource,
    sources: &CandleSources,
    id: &InstrumentId,
    tf: StatsTimeframe,
    at: DateTime<Utc>,
) -> Result<LiquidityView, StorageError> {
    let ig = source.view(id, CandleSource::Ig, tf, at).await?;
    if ig.candles.len() >= auto.min_candles {
        return Ok(ig);
    }
    for (other, note) in &auto.fallback {
        let mut v = source.view(id, *other, tf, at).await?;
        if !v.candles.is_empty() {
            v.fallback = Some(Fallback {
                from: CandleSource::Ig.as_str(),
                note: match sources.ibkr_instrument(id) {
                    Some(symbol) => note.replace("{symbol}", symbol),
                    None => note.clone(),
                },
            });
            return Ok(v);
        }
    }
    Ok(ig)
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

    /// `[warmup]` de `config/analysis.toml` (#511): se valida al cargar y se usan solo las
    /// últimas `m15_bars` velas antes de la vista.
    #[test]
    fn the_warmup_is_validated_and_keeps_the_last_bars() {
        let real = include_str!("../../../../config/analysis.toml");
        let patterns = include_str!("../../../../config/patterns.toml");
        let va = ViewAnalysis::from_toml(real, patterns).unwrap();
        assert_eq!(va.config.warmup.m15_bars, 1000);
        for bad in [
            real.replace("m15_bars = 1000", "m15_bars = 50"),
            real.replace("max_lookback_days = 45", "max_lookback_days = 3"),
            real.replace("max_lookback_days = 45", "max_lookback_days = \"45\""),
            real.replace("m15_bars = 1000", "m15_bars = 1000\nextra = 1"),
        ] {
            assert!(ViewAnalysis::from_toml(&bad, patterns).is_err(), "{bad}");
        }
        let t0: DateTime<Utc> = "2026-11-20T00:00:00Z".parse().unwrap();
        let bars: Vec<Candle> = (0..1200i64)
            .map(|k| Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * k),
                open: rust_decimal::Decimal::ONE,
                high: rust_decimal::Decimal::ONE,
                low: rust_decimal::Decimal::ONE,
                close: rust_decimal::Decimal::ONE,
                volume: None,
            })
            .collect();
        let tail = va.warmup_tail(bars.clone());
        assert_eq!(tail.len(), 1000);
        assert_eq!(tail[0].start, bars[200].start);
        assert_eq!(va.warmup_tail(bars[..10].to_vec()).len(), 10);
        // El bloque tiene un valor por vela de la vista, nada del calentamiento.
        let block = va.view_block(&tail[..990], &tail[990..], &[], &[]);
        assert_eq!(block.ema_daily.len(), 10);
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
                {"id": "auto", "label": "automática (IG; sin velas de IG, futuros)"},
                {"id": "ig", "label": "IG CFD"},
                {"id": "futures", "label": "futuros CME/Eurex (base distinta al CFD)"},
                {"id": "ibkr", "label": "futuros micro CME de IBKR (~10 min de atraso; base distinta al CFD)"}
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

    /// Una fuente con `ig` velas de IG y `futures` velas de futuros (de 1h, cerradas antes de
    /// `at`), para ver qué elige `source=auto` (#628) sin base de datos.
    struct Stocked {
        ig: usize,
        futures: usize,
        ibkr: usize,
    }

    impl LiquiditySource for Stocked {
        async fn view(
            &self,
            id: &InstrumentId,
            source: CandleSource,
            tf: StatsTimeframe,
            at: DateTime<Utc>,
        ) -> Result<LiquidityView, StorageError> {
            let n = match source {
                CandleSource::Ig => self.ig,
                CandleSource::Futures => self.futures,
                CandleSource::Ibkr => self.ibkr,
            };
            let mut v =
                LiquidityView::empty(id.clone(), ViewSource::new(source, &sources()), tf, at);
            if n == 0 {
                return Ok(v);
            }
            let one = rust_decimal::Decimal::ONE;
            let candles = (0..n)
                .map(|k| Candle {
                    instrument_id: id.clone(),
                    timeframe: Timeframe::H1,
                    start: at - TimeDelta::hours(n as i64 + 1 - k as i64),
                    open: one,
                    high: one,
                    low: one,
                    close: one,
                    volume: None,
                })
                .collect();
            v.candles = Bars::Intraday(candles);
            v.as_of = Some(utc(at - TimeDelta::hours(1)));
            Ok(v)
        }
    }

    async fn stocked(ig: usize, futures: usize, q: &str) -> serde_json::Value {
        stocked3(ig, futures, 0, q).await
    }

    async fn stocked3(ig: usize, futures: usize, ibkr: usize, q: &str) -> serde_json::Value {
        let r = respond(
            &Stocked { ig, futures, ibkr },
            &config(),
            &sources(),
            &catalog(),
            "GET",
            "/api/v1/liquidity",
            Some(q),
            now(),
        )
        .await;
        assert_eq!(r.status, 200, "{q}");
        serde_json::from_str(&r.body).unwrap()
    }

    #[tokio::test]
    async fn auto_falls_back_to_futures_while_ig_has_no_candles() {
        let min = config().auto.min_candles;
        let q = "instrument=US100&timeframe=1h&source=auto";
        // IG vacío o con menos velas que el mínimo: futuros, y la respuesta lo dice.
        for ig in [0, min - 1] {
            let v = stocked(ig, 50, q).await;
            assert_eq!(v["source"]["id"], "futures", "ig={ig}");
            assert_eq!(
                v["source"]["label"],
                "futuros CME/Eurex (base distinta al CFD)"
            );
            assert_eq!(v["fallback"]["from"], "ig");
            let note = v["fallback"]["note"].as_str().unwrap();
            assert!(note.starts_with("IG todavía no tiene velas"), "{note}");
            assert!(note.contains("Base distinta al CFD"), "{note}");
            assert_eq!(v["candles"].as_array().unwrap().len(), 50);
        }
    }

    #[tokio::test]
    async fn auto_goes_back_to_ig_when_it_has_candles() {
        let min = config().auto.min_candles;
        let q = "instrument=US100&timeframe=1h&source=auto";
        let v = stocked(min, 50, q).await;
        assert_eq!(v["source"]["id"], "ig");
        assert_eq!(v["source"]["label"], "IG CFD");
        assert!(v.get("fallback").is_none(), "sin reemplazo no hay motivo");
        assert_eq!(v["candles"].as_array().unwrap().len(), min);
        // Ninguna fuente tiene velas: la vista de IG, vacía y sin motivo.
        let v = stocked(0, 0, q).await;
        assert_eq!(v["source"]["id"], "ig");
        assert_eq!(v["as_of"], serde_json::Value::Null);
        assert!(v.get("fallback").is_none());
        // Pocas velas de IG y ninguna de futuros: las de IG.
        let v = stocked(3, 0, q).await;
        assert_eq!(v["source"]["id"], "ig");
        assert_eq!(v["candles"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn a_source_picked_by_hand_is_never_replaced() {
        for (q, id) in [
            ("instrument=US100&timeframe=1h&source=ig", "ig"),
            ("instrument=US100&timeframe=1h", "ig"),
            ("instrument=US100&timeframe=1h&source=futures", "futures"),
            ("instrument=US100&timeframe=1h&source=ibkr", "ibkr"),
        ] {
            let v = stocked(0, 0, q).await;
            assert_eq!(v["source"]["id"], id, "{q}");
            assert!(v.get("fallback").is_none(), "{q}");
            let v = stocked(0, 50, q).await;
            assert_eq!(v["source"]["id"], id, "{q}");
        }
    }

    /// IBKR primero (#630): US100 tiene MNQ y el motivo lo nombra; sin velas de IBKR, futuros.
    #[tokio::test]
    async fn auto_tries_ibkr_before_the_futures() {
        let q = "instrument=US100&timeframe=1h&source=auto";
        let v = stocked3(0, 50, 30, q).await;
        assert_eq!(v["source"]["id"], "ibkr");
        assert_eq!(
            v["fallback"]["note"],
            "IG todavía no tiene velas: se muestran futuros de CME (MNQ, IBKR, ~10 min de atraso). \
             Base distinta al CFD."
        );
        assert_eq!(v["candles"].as_array().unwrap().len(), 30);
        let v = stocked3(0, 50, 0, q).await;
        assert_eq!(v["source"]["id"], "futures");
        assert!(!v["fallback"]["note"].as_str().unwrap().contains("{symbol}"));
        // Con IG de vuelta, IG aunque IBKR tenga velas.
        let v = stocked3(config().auto.min_candles, 50, 30, q).await;
        assert_eq!(v["source"]["id"], "ig");
        assert!(v.get("fallback").is_none());
    }

    #[test]
    fn the_auto_source_is_validated() {
        let auto = config().auto;
        assert_eq!(auto.fallback.len(), 2);
        assert_eq!(auto.fallback[0].0, CandleSource::Ibkr);
        assert_eq!(auto.fallback[1].0, CandleSource::Futures);
        for (from, to) in [
            ("min_candles = 20", "min_candles = 0"),
            ("fallback = [\"ibkr\", \"futures\"]", "fallback = []"),
            ("fallback = [\"ibkr\", \"futures\"]", "fallback = [\"ig\"]"),
            ("fallback = [\"ibkr\", \"futures\"]", "fallback = [\"cfd\"]"),
            (
                "fallback = [\"ibkr\", \"futures\"]",
                "fallback = [\"futures\"]",
            ),
            (
                "fallback = [\"ibkr\", \"futures\"]",
                "fallback = [\"ibkr\", \"futures\", \"futuros\"]",
            ),
            ("[view.auto.notes]\nibkr = ", "[view.auto.notes]\nnope = "),
            ("[view.auto.notes]\n", "[view.auto.notes]\nig = \"x\"\n"),
            ("label = \"automática", "nope = 1\nlabel = \"automática"),
        ] {
            let bad = CONFIG.replacen(from, to, 1);
            assert_ne!(bad, CONFIG, "{from}");
            assert!(GatewayConfig::from_toml(&bad).is_err(), "{to}");
        }
        let long = CONFIG.replacen(
            "futures = \"IG todavía",
            &format!("futures = \"{}IG todavía", "x".repeat(300)),
            1,
        );
        assert!(GatewayConfig::from_toml(&long).is_err());
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

//! Serie de futuros preparada en memoria (#390, #399), común a `propuestas_por_mes` y
//! `ml_dataset`: velas de 15m y 1h armadas desde las barras de 1 minuto de Databento (con
//! `cfds_market_data::futures_candles`, como el ejemplo `futures_candles` de `cfds-ingest`, sin
//! guardarlas) y preparadas igual que `backtest_run --fuente futuros`: validación (§7.8 y §12),
//! horario, vencimientos y filtro de noticias de alto impacto (§7.2). Sin filtro de spread (no
//! hay spread medido del CFD) y sin ajuste de noticias del puntaje. Solo lee la base.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_backtest::{BacktestConfig, ProposalRules, mark_rollovers};
use cfds_core::{Candle, InstrumentId};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{
    CandleSources, Catalog, ExchangeCalendar, FuturesFeeds, NewsFilterConfig, Session,
    TradingWindowConfig, ValidationConfig, check_series, decision_flags, futures_candles,
    trading_window,
};
use cfds_patterns::PatternsConfig;
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_storage::PgStore;
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../../config/", $f))
    };
}

/// Los archivos de `config/` que leen la preparación y las propuestas, con su nombre (para el
/// `config_sha256` de una corrida).
#[allow(dead_code, reason = "cada ejemplo usa una parte")]
pub const CONFIG_FILES: [(&str, &str); 13] = [
    ("instruments.toml", cfg!("instruments.toml")),
    ("patterns.toml", cfg!("patterns.toml")),
    ("liquidity.toml", cfg!("liquidity.toml")),
    ("indicators.toml", cfg!("indicators.toml")),
    ("strategy.toml", cfg!("strategy.toml")),
    ("risk.toml", cfg!("risk.toml")),
    ("backtest.toml", cfg!("backtest.toml")),
    ("candle_sources.toml", cfg!("candle_sources.toml")),
    ("databento.toml", cfg!("databento.toml")),
    ("data_validation.toml", cfg!("data_validation.toml")),
    ("trading_window.toml", cfg!("trading_window.toml")),
    ("news_filter.toml", cfg!("news_filter.toml")),
    ("sessions.toml", cfg!("sessions.toml")),
];

const SOURCE: &str = "databento";
/// Capital de la cuenta para el tamaño (solo decide los rechazos por tamaño mínimo o margen).
pub const EQUITY: Decimal = Decimal::from_parts(10000, 0, 0, false, 0);

pub fn month_start(t: DateTime<Utc>) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(t.year(), t.month(), 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap_or_default(),
    )
}

pub fn next_month(t: DateTime<Utc>) -> DateTime<Utc> {
    let (y, m) = if t.month() == 12 {
        (t.year() + 1, 1)
    } else {
        (t.year(), t.month() + 1)
    };
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(y, m, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap_or_default(),
    )
}

/// Velas de 15m y 1h del futuro de `id`, armadas en memoria desde las barras de 1 minuto. Quedan
/// bajo `id`, igual que en `backtest_run`: el calendario y la sesión son los del CFD.
async fn candles(
    store: &PgStore,
    id: &InstrumentId,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
) -> Result<(Vec<Candle>, Vec<Candle>), Box<dyn std::error::Error>> {
    let feeds = FuturesFeeds::from_toml(cfg!("databento.toml"))?;
    let feed = feeds.get(id).ok_or("sin futuro en config/databento.toml")?;
    let (loaded_from, loaded_to) = store
        .futures_loaded_range(SOURCE, &feed.dataset, &feed.symbol, id)
        .await?
        .ok_or("sin barras bajadas")?;
    let (from, until) = (from.max(loaded_from), to.min(loaded_to).min(Utc::now()));
    eprintln!(
        "{} ({}): bajado del {loaded_from} al {loaded_to}; se usa del {from} al {until}",
        feed.symbol, feed.dataset
    );
    let (mut m15, mut h1, mut mixed) = (Vec::new(), Vec::new(), 0);
    let mut month = month_start(from);
    while month < until {
        let end = next_month(month).min(until);
        let bars = store
            .futures_bars(
                SOURCE,
                &feed.dataset,
                &feed.symbol,
                id,
                month.max(from),
                end,
            )
            .await?;
        if !bars.is_empty() {
            let f = futures_candles(&bars, id, end)?;
            mixed += f.mixed.len();
            m15.extend(f.m15.into_iter().map(|c| c.candle));
            h1.extend(f.h1.into_iter().map(|c| c.candle));
        }
        month = next_month(month);
    }
    eprintln!(
        "{} velas de 15m y {} de 1h; {mixed} ventanas no se armaron por un vencimiento",
        m15.len(),
        h1.len()
    );
    Ok((m15, h1))
}

/// La serie lista para contar propuestas.
#[allow(dead_code, reason = "cada ejemplo usa una parte")]
pub struct FuturesSeries {
    pub rules: ProposalRules,
    /// Instrumento derivado bajo el que se guarda (por ejemplo `US500.FUT`).
    pub out: InstrumentId,
    pub session: Session,
    pub m15: Vec<Candle>,
    pub h1: Vec<Candle>,
    pub entries: Vec<bool>,
    pub force_close: Vec<bool>,
    pub data_errors: Vec<DateTime<Utc>>,
    pub suspicious: usize,
    pub rolls: usize,
}

/// Prepara la serie de futuros de `id` en `[from, to)` (ver el módulo).
pub async fn prepare(
    store: &PgStore,
    id: &InstrumentId,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
) -> Result<FuturesSeries, Box<dyn std::error::Error>> {
    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let spec = catalog
        .instruments
        .get(id)
        .ok_or("instrumento fuera del catálogo")?;
    if catalog.account_currency.value() != Some(&spec.instrument.currency) {
        return Err(
            "la moneda del instrumento no es la de la cuenta: falta el tipo de cambio".into(),
        );
    }
    let limits = InstrumentLimits::from_spec(spec).ok_or("el instrumento no tiene margen")?;
    let rules = ProposalRules {
        instrument: id.clone(),
        strategy: Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml"))?,
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml"))?,
            trend: TrendConfig::from_toml(cfg!("indicators.toml"))?,
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml"))?,
        },
        risk: RiskConfig::from_toml(cfg!("risk.toml"))?,
        limits,
        exits: BacktestConfig::from_toml(cfg!("backtest.toml"))?.exits,
        equity: EQUITY,
        account_to_instrument: Decimal::ONE,
    };
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let out = sources.instrument_for(cfds_market_data::CandleSource::Futures, id)?;

    let (raw_m15, raw_h1) = candles(store, id, (from, to)).await?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let validation = ValidationConfig::from_toml(cfg!("data_validation.toml"))?;
    let a = check_series(&raw_m15, &calendar, &validation);
    let b = check_series(&raw_h1, &calendar, &validation);
    let suspicious = a.issues.len() + b.issues.len();
    {
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for i in a.issues.iter().chain(&b.issues) {
            let k = format!("{:?}", i.kind);
            let k = k
                .split([' ', '{', '('])
                .next()
                .unwrap_or_default()
                .to_owned();
            *kinds.entry(k).or_insert(0) += 1;
        }
        eprintln!("datos sospechosos por tipo: {kinds:?}");
    }
    let decisions = decision_flags(&a.candles, &[&a, &b]);
    let mut data_errors = [a.data_errors, b.data_errors].concat();
    data_errors.sort();
    let (m15, h1) = (a.candles, b.candles);
    let session = calendar.session_for(id).ok_or("instrumento sin bolsa")?;
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml"))?;
    let (mut entries, mut force_close) = trading_window::flags(session, &m15, &window)
        .ok_or("fechas fuera del calendario de feriados (config/holidays)")?;
    for (e, d) in entries.iter_mut().zip(&decisions) {
        *e &= *d;
    }
    let rolls = sources.roll_instants(id, session, &m15)?;
    mark_rollovers(
        &m15,
        &rolls,
        &mut entries,
        &mut force_close,
        &mut data_errors,
    );
    eprintln!(
        "velas donde se puede entrar (horario y datos): {} de {}",
        entries.iter().filter(|e| **e).count(),
        entries.len()
    );
    let news_cfg = NewsFilterConfig::from_toml(cfg!("news_filter.toml"))?;
    let entries = super::news_filter::apply(store, &m15, &entries, &news_cfg).await?;
    Ok(FuturesSeries {
        rules,
        out,
        session: session.clone(),
        m15,
        h1,
        entries,
        force_close,
        data_errors,
        suspicious,
        rolls: rolls.len(),
    })
}

/// Conecta a `DATABASE_URL` en una sesión de solo lectura.
pub async fn read_only_store() -> Result<PgStore, Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")?;
    let sep = if url.contains('?') { '&' } else { '?' };
    Ok(PgStore::connect(&format!(
        "{url}{sep}options=-c%20default_transaction_read_only%3Don"
    ))
    .await?)
}

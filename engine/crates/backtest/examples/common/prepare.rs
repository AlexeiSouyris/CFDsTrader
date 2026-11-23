//! Preparación común de una corrida sobre la base (`backtest_run` y `walk_forward`): config,
//! financiación diaria, velas de la fuente elegida, validación (§7.8 y §12), horario, vencimientos
//! de los futuros, filtros de spread (§7.3) y de noticias (§7.2) y las interpretaciones de noticias
//! del intérprete fijado (§9, ADR 0008 §5). Ver la documentación de `backtest_run`.

use std::path::PathBuf;

use cfds_backtest::{
    BacktestConfig, DailyRates, FundingRates, Setup, mark_rollovers, setup_from_catalog,
};
use cfds_core::{Candle, InstrumentId, NewsInterpretation, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{
    CandleSource, CandleSources, Catalog, ExchangeCalendar, NewsFilterConfig, SpreadFilterConfig,
    TradingWindowConfig, ValidationConfig, check_series, decision_flags, spread_flags,
    trading_window,
};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_storage::{CandleFeed, PgStore};
use cfds_strategy::{BacktestInterpreter, Configs, InterpreterCutoff, StrategyConfig};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../../config/", $f))
    };
}

/// Todo lo que necesita una corrida, ya preparado.
#[allow(dead_code, reason = "cada ejemplo usa una parte")]
pub struct Prepared {
    pub sources: CandleSources,
    /// Bajo qué instrumento se guarda (con futuros, el derivado).
    pub out: InstrumentId,
    pub store: PgStore,
    pub catalog: Catalog,
    pub setup: Setup,
    pub calendar: ExchangeCalendar,
    pub m15: Vec<Candle>,
    pub h1: Vec<Candle>,
    pub entries: Vec<bool>,
    pub force_close: Vec<bool>,
    pub data_errors: Vec<DateTime<Utc>>,
    pub news: Vec<NewsInterpretation>,
    pub published_from: DateTime<Utc>,
    pub interpreter: BacktestInterpreter,
}

pub async fn prepare(
    id: &InstrumentId,
    source: CandleSource,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
    equity: Decimal,
    reset_days: Option<u32>,
    without_news: bool,
) -> Result<Prepared, Box<dyn std::error::Error>> {
    // Las velas y lo que lee la estrategia (noticias, calendario, sesión) son del CFD `id`; lo que
    // se guarda queda bajo `out` (con futuros, el instrumento derivado: nunca se mezcla).
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let out = sources.instrument_for(source, id)?;
    let feed = match source {
        CandleSource::Ig => CandleFeed::Ig,
        CandleSource::Futures => CandleFeed::Futures(&sources.futures_source),
    };
    println!("velas: {} ({out})", sources.label(source));
    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let strategy = Configs {
        patterns: PatternsConfig::from_toml(cfg!("patterns.toml"))?,
        liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml"))?,
        trend: TrendConfig::from_toml(cfg!("indicators.toml"))?,
        strategy: StrategyConfig::from_toml(cfg!("strategy.toml"))?,
    };
    let mut setup = setup_from_catalog(
        &catalog,
        id,
        strategy,
        RiskConfig::from_toml(cfg!("risk.toml"))?,
        BacktestConfig::from_toml(cfg!("backtest.toml"))?,
        equity,
    )?;
    if let Some(n) = reset_days {
        setup.backtest.kill_switch_reset_days = (n > 0).then_some(n);
    }
    match setup.backtest.kill_switch_reset_days {
        Some(n) => println!(
            "kill switch: SOLO BACKTEST, se reactiva {n} días hábiles después de activarse (#277)"
        ),
        None => println!("kill switch: no se reactiva (como en vivo)"),
    }

    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    // Tipo de referencia de cada noche (#79): la serie diaria guardada, nunca la constante.
    let series = catalog
        .instruments
        .get(id)
        .and_then(|spec| spec.terms.funding_benchmark.value())
        .ok_or("el instrumento no tiene tipo de referencia de financiación")?;
    let daily = DailyRates::new(store.reference_rates(series, to).await?).ok_or_else(|| {
        format!("no hay valores de {series} guardados: cargarlos con `python -m cfds_intel.rates`")
    })?;
    if let Some((day, pct)) = daily.at_with_day(from) {
        println!("financiación: {series} diario (al inicio, {pct} % del {day})");
    }
    setup.funding_rates = FundingRates::Daily(daily);
    let raw_m15 = store
        .candles_from(feed, id, id, Timeframe::M15, from, to)
        .await?;
    let raw_h1 = store
        .candles_from(feed, id, id, Timeframe::H1, from, to)
        .await?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    // Spec §7.8 y §12: se descartan las velas sospechosas y, después de cada error de datos,
    // no se abre hasta volver a tener el calentamiento completo.
    let validation = ValidationConfig::from_toml(cfg!("data_validation.toml"))?;
    let (m15, h1, mut data_errors, decisions) = {
        let a = check_series(&raw_m15, &calendar, &validation);
        let b = check_series(&raw_h1, &calendar, &validation);
        for i in a.issues.iter().chain(&b.issues) {
            eprintln!("⚠️  dato sospechoso en {}: {}", i.start, i.kind);
        }
        // Saltos que se sostuvieron (#271): movimientos reales, se aceptan sin reiniciar el
        // calentamiento. Hasta confirmarlos, no se decide.
        for t in a.volatility.iter().chain(&b.volatility) {
            println!("evento de volatilidad: salto real en {t} (se acepta)");
        }
        let decisions = decision_flags(&a.candles, &[&a, &b]);
        let mut errors = [a.data_errors, b.data_errors].concat();
        errors.sort();
        (a.candles, b.candles, errors, decisions)
    };
    let session = calendar.session_for(id).ok_or("instrumento sin bolsa")?;
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml"))?;
    let (mut entries, mut force_close) =
        trading_window::flags(session, &m15, &window).ok_or("fechas fuera del calendario")?;
    for (e, d) in entries.iter_mut().zip(&decisions) {
        *e &= *d;
    }
    // Vencimientos de los futuros (#267): se cierra antes y el calentamiento empieza de nuevo.
    // Salen del calendario de vencimientos (#269), no de los ids de contrato de la fuente: en
    // Eurex el `instrument_id` de Databento cambia de un día a otro y se reusa.
    if source == CandleSource::Futures {
        let rolls = sources.roll_instants(id, session, &m15)?;
        let closed = mark_rollovers(
            &m15,
            &rolls,
            &mut entries,
            &mut force_close,
            &mut data_errors,
        );
        println!(
            "vencimientos (calendario): {} cambios de contrato; se cierra lo abierto en {} velas",
            rolls.len(),
            closed.len()
        );
    }
    // Filtro de spread (spec §7.3): necesita el spread de cierre guardado (#95), también de los
    // días anteriores al período. Sin ese dato no se abre. Con futuros no hay spread medido del
    // CFD para esas fechas: no se aplica, y los costos usan el spread típico de config (#267).
    let spread_ok = match source {
        CandleSource::Ig => {
            let spread_cfg = SpreadFilterConfig::from_toml(cfg!("spread_filter.toml"))?;
            let spreads = store
                .spreads(id, Timeframe::M15, from - spread_cfg.lookback, to)
                .await?;
            let ok = spread_flags(&m15, &spreads, session.tz, &spread_cfg);
            eprintln!(
                "filtro de spread: {} de {} velas no dejan abrir",
                ok.iter().filter(|ok| !**ok).count(),
                ok.len()
            );
            ok
        }
        CandleSource::Futures => {
            eprintln!(
                "⚠️  filtro de spread (§7.3): NO se aplica, no hay spread medido del CFD para las \
                 velas de futuros. Los costos usan el spread de config ({} puntos), no medido.",
                setup.costs.spread
            );
            vec![true; m15.len()]
        }
    };
    let entries: Vec<bool> = entries
        .iter()
        .zip(&spread_ok)
        .map(|(e, s)| *e && *s)
        .collect();
    // Filtro de noticias (spec §7.2): eventos del calendario guardado (`economic_events`, #243),
    // cada uno solo desde que se vio. Avisa qué tramos no tienen calendario.
    let news_cfg = NewsFilterConfig::from_toml(cfg!("news_filter.toml"))?;
    let entries = super::news_filter::apply(&store, &m15, &entries, &news_cfg).await?;
    println!(
        "{id}: {} velas de 15m y {} de 1h, del {from} al {to}",
        m15.len(),
        h1.len()
    );

    // Noticias (#194): solo las publicadas después del corte del modelo (ADR 0008 §5). Las que
    // pueden contar en alguna vela del período: publicadas dentro de la ventana antes de `from`
    // (`news.window_hours`) e interpretadas hasta `to`.
    let interpreter = BacktestInterpreter::from_toml(cfg!("news_interpreter.toml"))?;
    let cutoff = InterpreterCutoff::from_toml(cfg!("news_interpreter.toml"))?;
    let published_from = cutoff.backtest_published_from();
    let window = setup.strategy.strategy.news.window;
    let news = if without_news {
        Vec::new()
    } else {
        store
            .news_interpretations(
                &interpreter.name,
                &interpreter.version,
                from - window - chrono::TimeDelta::seconds(1),
                to,
                Some(published_from),
            )
            .await?
    };
    let foreign = interpreter.other_models(&news);
    if let Some(n) = foreign.first() {
        return Err(format!(
            "{} interpretaciones de {} {} no son del modelo de config/news_interpreter.toml \
             ({}), por ejemplo {:?}: la fecha de corte {} no vale para ellas",
            foreign.len(),
            interpreter.name,
            interpreter.version,
            interpreter.model,
            n.interpreter.model,
            cutoff.training_cutoff
        )
        .into());
    }
    let news_summary = format!(
        "noticias: {} interpretaciones de {} {} (modelo {}), solo de noticias publicadas desde \
         {published_from} (corte del modelo {}, ADR 0008 §5)",
        news.len(),
        interpreter.name,
        interpreter.version,
        interpreter.model,
        cutoff.training_cutoff
    );
    if without_news {
        println!("noticias: APAGADAS (--sin-noticias): el puntaje no tiene ajuste de noticias");
    } else {
        println!("{news_summary}");
    }
    if news.is_empty() && !without_news {
        println!(
            "⚠️  sin interpretaciones en el período: el backtest corre SIN ajuste de noticias \
             del puntaje (spec §9)"
        );
    }
    Ok(Prepared {
        sources,
        out,
        store,
        catalog,
        setup,
        calendar,
        m15,
        h1,
        entries,
        force_close,
        data_errors,
        news,
        published_from,
        interpreter,
    })
}

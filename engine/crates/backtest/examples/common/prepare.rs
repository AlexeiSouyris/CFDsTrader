//! Preparación común de una corrida sobre la base (`backtest_run` y `walk_forward`): config,
//! financiación diaria, velas de la fuente elegida, validación (§7.8 y §12), horario, vencimientos
//! de los futuros, filtros de spread (§7.3) y de noticias (§7.2) y las interpretaciones de noticias
//! del intérprete fijado (§9, ADR 0008 §5). Ver la documentación de `backtest_run`.

use std::path::PathBuf;

use cfds_backtest::{
    BacktestConfig, DailyRates, FundingRates, FuturesBasis, Setup, StudyAssumptions,
    apply_assumptions, apply_funding_scenario, mark_rollovers, setup_from_catalog, splice_rates,
};
use cfds_core::{Candle, InstrumentId, NewsInterpretation, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{
    CandleSource, CandleSources, Catalog, ExchangeCalendar, NewsFilterConfig, RateSegment,
    SpreadFilterConfig, TradingWindowConfig, ValidationConfig, check_series, decision_flags,
    spread_flags, trading_window,
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

/// Escenario de financiación de un estudio (#405): el tipo de referencia rotulado `series`
/// reemplaza EN MEMORIA al del catálogo (`config/` no cambia). Con `rates`, la serie diaria sale
/// de ahí (por ejemplo, un CSV de la fuente oficial); sin `rates`, de `reference_rates`.
pub struct FundingScenario {
    pub series: String,
    pub rates: Option<DailyRates>,
}

/// Lo que un estudio cambia en memoria (ver `common/study.rs`). Nunca es oficial.
pub struct Study {
    pub funding: Option<FundingScenario>,
    /// Costos supuestos que completan los `pending` del catálogo (#421).
    pub assumptions: Option<StudyAssumptions>,
}

#[allow(dead_code, reason = "walk_forward usa `prepare_with`")]
pub async fn prepare(
    id: &InstrumentId,
    source: CandleSource,
    period: (DateTime<Utc>, DateTime<Utc>),
    equity: Decimal,
    reset_days: Option<u32>,
    without_news: bool,
) -> Result<Prepared, Box<dyn std::error::Error>> {
    prepare_with(id, source, period, equity, reset_days, without_news, None).await
}

pub async fn prepare_with(
    id: &InstrumentId,
    source: CandleSource,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
    equity: Decimal,
    reset_days: Option<u32>,
    without_news: bool,
    study: Option<Study>,
) -> Result<Prepared, Box<dyn std::error::Error>> {
    // Las velas y lo que lee la estrategia (noticias, calendario, sesión) son del CFD `id`; lo que
    // se guarda queda bajo `out` (con futuros, el instrumento derivado: nunca se mezcla).
    let mut sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let mut catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let (scenario, assumptions) = match study {
        Some(s) => (s.funding, s.assumptions),
        None => (None, None),
    };
    if let Some(a) = &assumptions {
        let filled = apply_assumptions(&mut catalog, &mut sources, a)?;
        println!(
            "⚠️  COSTOS SUPUESTOS (estudio, no oficial; fuente \"{}\"): {}. config/ no cambia.",
            a.source,
            filled.join(", ")
        );
    }
    let out = sources.instrument_for(source, id)?;
    let feed = match source {
        CandleSource::Ig => CandleFeed::Ig,
        CandleSource::Futures => CandleFeed::Futures(&sources.futures_source),
        // Las barras de IBKR (#630) son solo para la vista de liquidez: poca historia y sin
        // diarias ni base de financiación.
        CandleSource::Ibkr => return Err("el backtest no usa las barras de IBKR".into()),
    };
    println!("velas: {} ({out})", sources.label(source));
    // Escenario (#405): el valor constante que se agrega solo completa el catálogo en memoria;
    // la corrida usa la serie diaria (más abajo).
    let mut scenario_rates = None;
    if let Some(s) = scenario {
        let value = s
            .rates
            .as_ref()
            .and_then(|r| r.at(to))
            .unwrap_or(Decimal::ZERO);
        let before = apply_funding_scenario(&mut catalog, id, &s.series, value)?;
        println!(
            "⚠️  ESCENARIO DE FINANCIACIÓN (estudio, no oficial): tipo de referencia {} en memoria \
             (en config/instruments.toml: {}). config/ no cambia.",
            s.series,
            before.as_deref().unwrap_or("pendiente")
        );
        scenario_rates = s.rates;
    }
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
    let daily = match scenario_rates {
        Some(d) => d,
        None => DailyRates::new(store.reference_rates(series, to).await?).ok_or_else(|| {
            format!(
                "no hay valores de {series} guardados: cargarlos con `python -m cfds_intel.rates`"
            )
        })?,
    };
    if let Some((day, pct)) = daily.at_with_day(from) {
        println!("financiación: {series} diario (al inicio, {pct} % del {day})");
    }
    setup.funding_rates = FundingRates::Daily(daily);
    // Contrato en otra moneda que la cuenta (el oro, ADR 0013 §4): el tipo de cambio diario
    // guardado, vigente en cada momento. Sin serie, no se corre (nunca un tipo inventado).
    if let cfds_backtest::AccountFx::Missing { pair } = &setup.account_to_instrument {
        let fx = cfds_backtest::FxRates::new(store.fx_rates(pair, to).await?).ok_or_else(|| {
            format!(
                "no hay tipos de {pair} guardados: cargarlos con `python -m cfds_intel.rates.fx`"
            )
        })?;
        if let Some((day, rate)) = fx.at_with_day(from) {
            println!("tipo de cambio: {pair} diario del BCE (al inicio, {rate} del {day})");
        }
        setup.account_to_instrument = cfds_backtest::AccountFx::Daily(fx);
    }
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
        // Base del futuro (#467): lo que el precio ya descontó se resta de la financiación.
        let spec = sources.basis.instruments.get(id).ok_or_else(|| {
            format!(
                "falta futures.basis.rates.{id} en config/candle_sources.toml: sin la tasa de la \
                 base, la financiación con futuros contaría la tasa dos veces (#467)"
            )
        })?;
        let rate = load_spliced(&store, &spec.rate, to).await?;
        let dividends = match &spec.dividends {
            Some(d) => Some(FundingRates::Daily(load_spliced(&store, d, to).await?)),
            None => None,
        };
        if let Some((day, pct)) = rate.at_with_day(from) {
            println!(
                "base del futuro ({:?}): se descuenta {} (al inicio, {pct} % del {day})",
                sources.basis.mode,
                rate.series()
            );
        }
        setup.futures_basis = Some(FuturesBasis {
            rate: FundingRates::Daily(rate),
            dividends,
            rolls,
        });
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
        CandleSource::Futures | CandleSource::Ibkr => {
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

/// Serie diaria empalmada de los tramos `segments` (#467), rotulada con sus nombres
/// (`EFFR→SOFR`). Sin valores guardados, error con el comando que los carga.
async fn load_spliced(
    store: &PgStore,
    segments: &[RateSegment],
    to: DateTime<Utc>,
) -> Result<DailyRates, Box<dyn std::error::Error>> {
    let mut rows = Vec::with_capacity(segments.len());
    for s in segments {
        rows.push(store.reference_rates(&s.series, to).await?);
    }
    let label = segments
        .iter()
        .map(|s| s.series.as_str())
        .collect::<Vec<_>>()
        .join("→");
    splice_rates(segments, rows, &label).ok_or_else(|| {
        format!("no hay valores de {label} guardados: cargarlos con `python -m cfds_intel.rates`")
            .into()
    })
}

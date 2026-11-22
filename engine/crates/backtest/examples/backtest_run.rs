//! Backtest sobre velas reales guardadas en PostgreSQL (#76), con el `Setup` armado solo desde
//! la configuración. Corre la estrategia y comprar y mantener sobre el mismo período.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-backtest --example backtest_run -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> <capital inicial> <tamaño comprar y mantener> \
//!     [<run_id>]
//! ```
//! Con `run_id`, además guarda cada operación en el diario (`proposals` y `outcomes`, de forma
//! idempotente) y muestra la probabilidad de la spec §13 por gatillo y rango de puntaje, solo
//! con los resultados anteriores al tramo fuera de muestra de `config/data_split.toml` (#181).
//! Si al instrumento le falta algún dato del broker (por ejemplo el tipo de referencia de la
//! financiación), no corre y dice qué falta. La financiación usa el tipo de referencia diario
//! guardado (`reference_rates`, #79): si alguna noche no tiene un valor ya publicado, tampoco.
//!
//! Noticias (spec §9, #194): carga las interpretaciones del intérprete fijado en
//! `config/news_interpreter.toml` (`[engine]`), solo de noticias publicadas después de la fecha
//! de corte del modelo (ADR 0008 §5), y las pasa al ajuste del puntaje: en cada vela cuenta lo
//! interpretado hasta su cierre. Si hay interpretaciones de otro modelo, no corre. Si no hay
//! ninguna, corre sin ajuste de noticias y lo dice.

use std::path::PathBuf;

use cfds_backtest::{
    BacktestConfig, DailyRates, FundingRates, buy_and_hold, run_with_news, setup_from_catalog,
};
use cfds_core::Timeframe;
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{
    Catalog, ExchangeCalendar, SpreadFilterConfig, TradingWindowConfig, ValidationConfig,
    check_series, spread_flags, trading_window,
};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_storage::PgStore;
use cfds_strategy::{BacktestInterpreter, Configs, InterpreterCutoff, StrategyConfig};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (instrument, from, to, equity, size, run_id) = match args.as_slice() {
        [i, f, t, e, s] => (i, f, t, e, s, None),
        [i, f, t, e, s, r] => (i, f, t, e, s, Some(r.as_str())),
        _ => {
            return Err(
                "uso: <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <tamaño B&H> [<run_id>]"
                    .into(),
            );
        }
    };
    let id = instrument.parse()?;
    let (from, to) = (from.parse()?, to.parse()?);
    let (equity, size): (Decimal, Decimal) = (equity.parse()?, size.parse()?);

    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let strategy = Configs {
        patterns: PatternsConfig::from_toml(cfg!("patterns.toml"))?,
        liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml"))?,
        trend: TrendConfig::from_toml(cfg!("indicators.toml"))?,
        strategy: StrategyConfig::from_toml(cfg!("strategy.toml"))?,
    };
    let mut setup = setup_from_catalog(
        &catalog,
        &id,
        strategy,
        RiskConfig::from_toml(cfg!("risk.toml"))?,
        BacktestConfig::from_toml(cfg!("backtest.toml"))?,
        equity,
    )?;

    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    // Tipo de referencia de cada noche (#79): la serie diaria guardada, nunca la constante.
    let series = catalog
        .instruments
        .get(&id)
        .and_then(|spec| spec.terms.funding_benchmark.value())
        .ok_or("el instrumento no tiene tipo de referencia de financiación")?;
    let daily = DailyRates::new(store.reference_rates(series, to).await?).ok_or_else(|| {
        format!("no hay valores de {series} guardados: cargarlos con `python -m cfds_intel.rates`")
    })?;
    if let Some((day, pct)) = daily.at_with_day(from) {
        println!("financiación: {series} diario (al inicio, {pct} % del {day})");
    }
    setup.funding_rates = FundingRates::Daily(daily);
    let raw_m15 = store.candles(&id, Timeframe::M15, from, to).await?;
    let raw_h1 = store.candles(&id, Timeframe::H1, from, to).await?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    // Spec §7.8 y §12: se descartan las velas sospechosas y, después de cada error de datos,
    // no se abre hasta volver a tener el calentamiento completo.
    let validation = ValidationConfig::from_toml(cfg!("data_validation.toml"))?;
    let (m15, h1, data_errors) = {
        let a = check_series(&raw_m15, &calendar, &validation);
        let b = check_series(&raw_h1, &calendar, &validation);
        for i in a.issues.iter().chain(&b.issues) {
            eprintln!("⚠️  error de datos en {}: {}", i.start, i.kind);
        }
        let mut errors = [a.data_errors, b.data_errors].concat();
        errors.sort();
        (a.candles, b.candles, errors)
    };
    let session = calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml"))?;
    let (entries, force_close) =
        trading_window::flags(session, &m15, &window).ok_or("fechas fuera del calendario")?;
    // Filtro de spread (spec §7.3): necesita el spread de cierre guardado (#95), también de los
    // días anteriores al período. Sin ese dato no se abre.
    let spread_cfg = SpreadFilterConfig::from_toml(cfg!("spread_filter.toml"))?;
    let spreads = store
        .spreads(&id, Timeframe::M15, from - spread_cfg.lookback, to)
        .await?;
    let spread_ok = spread_flags(&m15, &spreads, session.tz, &spread_cfg);
    eprintln!(
        "filtro de spread: {} de {} velas no dejan abrir",
        spread_ok.iter().filter(|ok| !**ok).count(),
        spread_ok.len()
    );
    let entries: Vec<bool> = entries
        .iter()
        .zip(&spread_ok)
        .map(|(e, s)| *e && *s)
        .collect();
    // Filtro de noticias (spec §7.2, `cfds_market_data::news_flags`): la fuente del calendario
    // económico es de la fase 4b y necesita su ADR (spec §9). Sin eventos no se aplica: avisarlo.
    eprintln!(
        "⚠️  filtro de noticias de alto impacto (§7.2) SIN DATOS: falta la fuente del calendario \
         (fase 4b). El resultado no lo descuenta."
    );
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
    let news = store
        .news_interpretations(
            &interpreter.name,
            &interpreter.version,
            from - window - chrono::TimeDelta::seconds(1),
            to,
            Some(published_from),
        )
        .await?;
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
    println!("{news_summary}");
    if news.is_empty() {
        println!(
            "⚠️  sin interpretaciones en el período: el backtest corre SIN ajuste de noticias \
             del puntaje (spec §9)"
        );
    }

    let report = run_with_news(
        &m15,
        &h1,
        &entries,
        &force_close,
        &data_errors,
        &news,
        published_from,
        &setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    println!(
        "estrategia: {} operaciones, {} señales rechazadas por riesgo, capital final {}",
        report.trades.len(),
        report.rejected.len(),
        report.final_equity
    );
    for t in &report.trades {
        println!(
            "  {:?} entrada {} @ {} tamaño {} → neto {} (costos {})",
            t.side,
            t.entry_time,
            t.entry_price,
            t.size,
            t.net_pnl,
            t.total_costs()
        );
    }

    if let Some(run_id) = run_id {
        journal_and_probability(&store, run_id, &id, &report.trades, &m15).await?;
    }

    let bh = buy_and_hold(&m15, size, &setup).ok_or("sin velas")?;
    let (first, last) = (&m15[0], &m15[m15.len() - 1]);
    println!(
        "comprar y mantener ({size}): apertura {} = {} → cierre {} = {}",
        first.start, first.open, last.start, last.close
    );
    println!("  {bh:?}");
    Ok(())
}

/// Guarda la corrida en el diario y muestra la probabilidad de la spec §13 (#181).
async fn journal_and_probability(
    store: &PgStore,
    run_id: &str,
    id: &cfds_core::InstrumentId,
    trades: &[cfds_backtest::TradeRecord],
    m15: &[cfds_core::Candle],
) -> Result<(), Box<dyn std::error::Error>> {
    use cfds_journal::pg::{PgJournal, Recorded};
    use cfds_journal::probability::{ProbabilityConfig, ProbabilityTable, Shown};
    use cfds_market_data::{SplitConfig, plan};

    let journal = PgJournal::new(store.pool().clone());
    let pairs: Vec<_> = trades
        .iter()
        .map(|t| cfds_journal::from_backtest(run_id, id, t))
        .collect();
    let mut new = 0;
    for (p, o) in &pairs {
        if journal.record(p, Some(o)).await? == Recorded::Inserted {
            new += 1;
        }
    }
    println!(
        "diario: {} operaciones de la corrida {run_id} ({new} nuevas)",
        pairs.len()
    );

    let (Some(first), Some(last)) = (m15.first(), m15.last()) else {
        return Ok(());
    };
    let split = plan(
        first.start,
        last.end(),
        &SplitConfig::from_toml(cfg!("data_split.toml"))?,
    )?;
    let cutoff = split.out_of_sample.start;
    let cfg = ProbabilityConfig::from_toml(cfg!("probability.toml"))?;
    let table = ProbabilityTable::build(&pairs, cutoff, &cfg)?;
    println!("probabilidad §13 (TP1 antes que el stop), con resultados hasta {cutoff}:");
    for (g, _) in table.groups() {
        let lo = cfg.score_bands[g.band];
        let band = match cfg.score_bands.get(g.band + 1) {
            Some(next) if next - 1 > lo => format!("{lo}–{}", next - 1),
            Some(_) => format!("{lo}"),
            None => format!("≥{lo}"),
        };
        let shown = match table.shown_for(g) {
            Shown::Estimate {
                probability,
                low,
                high,
                hits,
                cases,
            } => format!("{probability} [{low}; {high}] ({hits}/{cases})"),
            Shown::Insufficient { cases, needed } => {
                format!("datos insuficientes ({cases} de {needed})")
            }
        };
        println!("  {:?}, puntaje {band}: {shown}", g.trigger);
    }
    Ok(())
}

//! Backtest sobre velas reales guardadas en PostgreSQL (#76), con el `Setup` armado solo desde
//! la configuración. Corre la estrategia y comprar y mantener sobre el mismo período.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-backtest --example backtest_run -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> <capital inicial> <tamaño comprar y mantener> \
//!     [<run_id>] [--sin-noticias]
//! ```
//! Con `run_id`, además guarda cada operación en el diario (`proposals` y `outcomes`, de forma
//! idempotente) y, al terminar, la corrida en `backtest_runs` (`backtest_run.v1`, #200): período,
//! hash de la configuración, costos, intérprete y corte de las noticias y resumen de métricas.
//! Las propuestas quedan ligadas a la corrida por el mismo `run_id`. También guarda las variables
//! de ML de cada propuesta (`ml_features.v1`, #158), con lo disponible al cierre de su señal; las
//! que no tienen todas sus variables quedan sin fila y se cuentan. Antes:
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
//!
//! Filtro de noticias de alto impacto (spec §7.2, #243): usa los eventos del calendario guardado
//! (`economic_events`), cada uno desde que se vio, y avisa qué tramos del período no tienen
//! calendario (ahí no descuenta nada). `--sin-noticias` no lo apaga: solo apaga el ajuste del
//! puntaje.
//!
//! Con `--fuente futuros` (#267) corre sobre las velas de 15m y 1h armadas desde los futuros de
//! Databento (`futures_candles`, ADR 0011), sin mezclarlas con las de IG:
//! - todo lo que guarda (`backtest_runs`, `proposals`, `outcomes` y `ml_features`) queda bajo el
//!   instrumento derivado de `config/candle_sources.toml` (por ejemplo `US500.FUT`);
//! - no hay spread medido del CFD para esas fechas: los costos usan el spread típico de config
//!   (nunca cero), el filtro de spread §7.3 no se aplica y la corrida lo dice
//!   (`costs.spread_source = "config"`);
//! - en cada vencimiento se cierra lo abierto al cierre de la última vela del contrato viejo y el
//!   calentamiento empieza de nuevo (`cfds_backtest::mark_rollovers`). Los vencimientos salen del
//!   calendario (`CandleSources::roll_instants`, #269), no de los ids de contrato de la fuente;
//! - la financiación, las noticias, el calendario y los tercios de la volatilidad son los mismos
//!   que con IG (del instrumento del CFD).
//!
//! Con `--sin-noticias` no carga ninguna interpretación y la corrida queda con `news: null` en
//! `backtest_runs`: es la variante "capa apagada" del reporte de la fase 4b (#221), para medir
//! el ajuste de noticias contra no usarlo con todo lo demás igual.

#[path = "common/news_filter.rs"]
mod news_filter;

use std::path::PathBuf;

use cfds_backtest::{
    BacktestConfig, DailyRates, FundingRates, buy_and_hold, mark_rollovers, run_with_news,
    setup_from_catalog,
};
use cfds_core::Timeframe;
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
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// Archivos de `config/` que lee la corrida, para el hash de `backtest_run.v1` (#200).
const CONFIG_FILES: [(&str, &str); 16] = [
    ("backtest.toml", cfg!("backtest.toml")),
    ("candle_sources.toml", cfg!("candle_sources.toml")),
    ("data_split.toml", cfg!("data_split.toml")),
    ("data_validation.toml", cfg!("data_validation.toml")),
    ("indicators.toml", cfg!("indicators.toml")),
    ("instruments.toml", cfg!("instruments.toml")),
    ("liquidity.toml", cfg!("liquidity.toml")),
    ("news_filter.toml", cfg!("news_filter.toml")),
    ("news_interpreter.toml", cfg!("news_interpreter.toml")),
    ("patterns.toml", cfg!("patterns.toml")),
    ("probability.toml", cfg!("probability.toml")),
    ("risk.toml", cfg!("risk.toml")),
    ("sessions.toml", cfg!("sessions.toml")),
    ("spread_filter.toml", cfg!("spread_filter.toml")),
    ("strategy.toml", cfg!("strategy.toml")),
    ("trading_window.toml", cfg!("trading_window.toml")),
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started_at = chrono::Utc::now();
    let all: Vec<String> = std::env::args().skip(1).collect();
    let without_news = all.iter().any(|a| a == "--sin-noticias");
    // `--fuente ig|futuros` (#267): por omisión, las velas del CFD de IG.
    let (source, all) = match all.iter().position(|a| a == "--fuente") {
        None => (CandleSource::Ig, all),
        Some(i) => {
            let s = all
                .get(i + 1)
                .and_then(|s| CandleSource::parse(s))
                .ok_or("--fuente ig|futuros")?;
            let rest = [&all[..i], all.get(i + 2..).unwrap_or(&[])].concat();
            (s, rest)
        }
    };
    // `--reiniciar-kill-switch-dias N` (#277, SOLO BACKTEST): pisa
    // `config/backtest.toml [kill_switch] reset_after_business_days` en esta corrida; 0 = nunca.
    let (reset_days, all) = match all.iter().position(|a| a == "--reiniciar-kill-switch-dias") {
        None => (None, all),
        Some(i) => {
            let n: u32 = all
                .get(i + 1)
                .and_then(|s| s.parse().ok())
                .filter(|n| *n <= 260)
                .ok_or("--reiniciar-kill-switch-dias N (días hábiles, de 0 a 260)")?;
            let rest = [&all[..i], all.get(i + 2..).unwrap_or(&[])].concat();
            (Some(n), rest)
        }
    };
    let args: Vec<&str> = all
        .iter()
        .map(String::as_str)
        .filter(|a| *a != "--sin-noticias")
        .collect();
    let (instrument, from, to, equity, size, run_id) = match args.as_slice() {
        [i, f, t, e, s] => (i, f, t, e, s, None),
        [i, f, t, e, s, r] => (i, f, t, e, s, Some(*r)),
        _ => {
            return Err(
                "uso: <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <tamaño B&H> [<run_id>] \
                 [--sin-noticias] [--fuente ig|futuros] [--reiniciar-kill-switch-dias N]"
                    .into(),
            );
        }
    };
    let id: cfds_core::InstrumentId = instrument.parse()?;
    // Las velas y lo que lee la estrategia (noticias, calendario, sesión) son del CFD `id`; lo que
    // se guarda queda bajo `out` (con futuros, el instrumento derivado: nunca se mezcla).
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let out = sources.instrument_for(source, &id)?;
    let feed = match source {
        CandleSource::Ig => CandleFeed::Ig,
        CandleSource::Futures => CandleFeed::Futures(&sources.futures_source),
    };
    println!("velas: {} ({out})", sources.label(source));
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
    let raw_m15 = store
        .candles_from(feed, &id, &id, Timeframe::M15, from, to)
        .await?;
    let raw_h1 = store
        .candles_from(feed, &id, &id, Timeframe::H1, from, to)
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
    let session = calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
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
        let rolls = sources.roll_instants(&id, session, &m15)?;
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
                .spreads(&id, Timeframe::M15, from - spread_cfg.lookback, to)
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
    let entries = news_filter::apply(&store, &m15, &entries, &news_cfg).await?;
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
    // Nada pasa en silencio (#273): por qué se rechazaron señales y si el kill switch cortó.
    let reasons = report.rejections_by_reason();
    if !reasons.is_empty() {
        println!("señales rechazadas por motivo: {reasons:?}");
    }
    for e in &report.kill_switch_events {
        match e.kind {
            cfds_backtest::KillSwitchEventKind::Activated => eprintln!(
                "⛔ KILL SWITCH en {}: el drawdown llegó al límite de config/risk.toml \
                 (max_drawdown_pct), capital {}. {}",
                e.at,
                e.equity.round_dp(2),
                match setup.backtest.kill_switch_reset_days {
                    Some(n) =>
                        format!("SOLO BACKTEST: se reactiva después de {n} días hábiles (#277)."),
                    None => "No se abre NADA MÁS hasta el final del período.".to_owned(),
                }
            ),
            cfds_backtest::KillSwitchEventKind::Reset => eprintln!(
                "↻ kill switch reactivado en {} (solo backtest), capital {}: el drawdown se mide \
                 desde acá",
                e.at,
                e.equity.round_dp(2)
            ),
        }
    }
    if !report.kill_switch_events.is_empty() {
        eprintln!(
            "   (queda en backtest_runs: metrics.kill_switch_at y metrics.kill_switch_events)"
        );
    }
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
        if source == CandleSource::Futures {
            // Las tablas del diario referencian `instruments`: se registra el derivado.
            let spec = catalog
                .instruments
                .get(&id)
                .ok_or("instrumento sin catálogo")?;
            store
                .upsert_instrument(&sources.derived_instrument(source, &spec.instrument)?)
                .await?;
        }
        journal_and_probability(&store, run_id, &out, &report.trades, &m15).await?;
        record_ml_features(
            &store,
            run_id,
            // Tercios de la volatilidad de la misma fuente (#282): con futuros, los de las
            // diarias del instrumento derivado.
            (&out, &out),
            &report.trades,
            &m15,
            session,
            from,
            to,
        )
        .await?;
        let news_used = if without_news {
            None
        } else {
            Some(cfds_journal::RunNews {
                interpreter_name: interpreter.name.clone(),
                interpreter_version: interpreter.version.clone(),
                interpreter_model: Some(interpreter.model.clone()),
                published_from,
                interpretations: u64::try_from(news.len())?,
            })
        };
        let spread_source = match source {
            CandleSource::Ig => cfds_journal::SpreadSource::Measured,
            CandleSource::Futures => cfds_journal::SpreadSource::Config,
        };
        let run = run_record(
            run_id,
            (&out, spread_source),
            (from, to),
            &setup,
            &report,
            &m15,
            news_used,
            started_at,
        )?;
        let journal = cfds_journal::pg::PgJournal::new(store.pool().clone());
        let recorded = journal.record_run(&run).await?;
        println!(
            "corrida {run_id} en backtest_runs ({recorded:?}): config {}, {} operaciones",
            &run.config_sha256[..12],
            run.metrics.trades
        );
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

/// La corrida para `backtest_runs` (#200), con las métricas en la moneda de la cuenta.
#[allow(clippy::too_many_arguments)]
fn run_record(
    run_id: &str,
    (id, spread_source): (&cfds_core::InstrumentId, cfds_journal::SpreadSource),
    (from, to): (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>),
    setup: &cfds_backtest::Setup,
    report: &cfds_backtest::Report,
    m15: &[cfds_core::Candle],
    news: Option<cfds_journal::RunNews>,
    started_at: chrono::DateTime<chrono::Utc>,
) -> Result<cfds_journal::BacktestRun, Box<dyn std::error::Error>> {
    let net: Vec<Decimal> = report
        .trades
        .iter()
        .map(|t| t.net_pnl / setup.account_to_instrument)
        .collect();
    let intervals: Vec<_> = report
        .trades
        .iter()
        .map(|t| {
            (
                t.entry_time,
                t.exits.last().map_or(t.entry_time, |e| e.time),
            )
        })
        .collect();
    let period = match (m15.first(), m15.last()) {
        (Some(a), Some(b)) => (a.start, b.end()),
        _ => (from, to),
    };
    let metrics = cfds_backtest::evaluate(
        setup.initial_equity,
        report.final_equity,
        &report.daily_equity,
        &net,
        &intervals,
        period,
        &setup.backtest.metrics,
    );
    Ok(cfds_journal::BacktestRun {
        run_id: run_id.to_owned(),
        instruments: vec![id.clone()],
        timeframe: cfds_journal::RunTimeframe::M15,
        from,
        to,
        config_sha256: cfds_journal::config_sha256(&CONFIG_FILES),
        initial_equity: setup.initial_equity,
        costs: cfds_journal::RunCosts::new(&setup.backtest.costs, &[(id.clone(), setup.costs)])
            .with_spread_source(spread_source),
        news,
        metrics: cfds_journal::RunMetrics::new(report.final_equity, &metrics)
            .with_kill_switch(report.kill_switch_time(m15))
            .with_kill_switch_events(
                setup.backtest.kill_switch_reset_days,
                report
                    .kill_switch_events
                    .iter()
                    .map(|e| cfds_journal::RunKillSwitchEvent {
                        event: match e.kind {
                            cfds_backtest::KillSwitchEventKind::Activated => {
                                cfds_journal::RunKillSwitchKind::Activated
                            }
                            cfds_backtest::KillSwitchEventKind::Reset => {
                                cfds_journal::RunKillSwitchKind::Reset
                            }
                        },
                        at: e.at,
                        equity: e.equity.normalize(),
                    })
                    .collect(),
            ),
        started_at,
        finished_at: chrono::Utc::now(),
    })
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
    // Con pocos meses no se puede separar el tramo fuera de muestra (#269): la tabla queda
    // "datos insuficientes", pero la corrida sigue y se guarda.
    let split = match plan(
        first.start,
        last.end(),
        &SplitConfig::from_toml(cfg!("data_split.toml"))?,
    ) {
        Ok(split) => split,
        Err(cfds_market_data::SplitError::NotEnoughData { needed }) => {
            println!(
                "probabilidad §13: datos insuficientes (hacen falta {needed} meses de velas para \
                 separar el tramo fuera de muestra)"
            );
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
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

/// Guarda las variables de ML de cada operación (`ml_features.v1`, #158). Los tercios de la
/// volatilidad son los guardados (`daily_vol_terciles`, del ejemplo `daily_indicators`) del
/// instrumento `cfd` (con futuros, el derivado: #282); las propuestas son las del instrumento con
/// el que se guardó la corrida (`id`, el derivado con futuros, #267).
#[allow(clippy::too_many_arguments)]
async fn record_ml_features(
    store: &PgStore,
    run_id: &str,
    (id, cfd): (&cfds_core::InstrumentId, &cfds_core::InstrumentId),
    trades: &[cfds_backtest::TradeRecord],
    m15: &[cfds_core::Candle],
    session: &cfds_market_data::Session,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
) -> Result<(), Box<dyn std::error::Error>> {
    use cfds_journal::ml_features::{atr14_m15, from_backtest};
    use cfds_journal::pg::{PgJournal, Recorded};

    let daily_cfg = cfds_indicators::DailyConfig::from_toml(cfg!("indicators.toml"))?;
    // El tercio vigente en `from` puede ser de unos días antes (fines de semana, feriados).
    let daily = store
        .daily_vol_terciles(
            cfd,
            u32::try_from(daily_cfg.realized_vol_window)?,
            daily_cfg.vol_tercile_years,
            from.date_naive() - chrono::Days::new(30),
            to.date_naive() + chrono::Days::new(1),
        )
        .await?;
    if daily.is_empty() {
        eprintln!(
            "⚠️  sin tercios de volatilidad de {cfd}: ninguna propuesta tiene variables de ML \
             (cargarlos con el ejemplo `daily_indicators` de cfds-ingest)"
        );
    }
    let atr = atr14_m15(m15);
    let journal = PgJournal::new(store.pool().clone());
    let (mut with, mut new) = (0, 0);
    for t in trades {
        let Some(f) = from_backtest(run_id, id, t, &atr, &daily, session) else {
            continue;
        };
        with += 1;
        if journal.record_ml_features(&f).await? == Recorded::Inserted {
            new += 1;
        }
    }
    println!(
        "variables de ML: {with} de {} propuestas ({new} nuevas); el resto sin alguna variable \
         (calentamiento, sin historia diaria o fuera de sesión)",
        trades.len()
    );
    Ok(())
}

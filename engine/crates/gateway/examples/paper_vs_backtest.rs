//! Aprobación de la fase 8 (#315): paper trading contra el backtest del mismo período.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-gateway --example paper_vs_backtest -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> [--guardar]
//! ```
//! - **Paper:** el registro del broker simulado (`paper_orders` y `paper_order_reports`) de ese
//!   instrumento, reconstruido con las mismas velas guardadas y valuado por el broker
//!   (`PaperBroker::trade_outcomes`).
//! - **Backtest:** el recorrido sobre las mismas velas cerradas de 15m y 1h (validadas igual) y la
//!   misma config, con los mismos filtros de entrada que usa el runner del gateway (horario,
//!   spread §7.3 y noticias de alto impacto §7.2), el ajuste de noticias §9 y la financiación
//!   diaria guardada. Las velas empiezan `[runner] lookback_days` antes de `desde`, solo para
//!   el calentamiento: el backtest no abre operaciones antes de `desde` (#440). Arranca con el
//!   capital que tenía la cuenta paper en `desde` y llega a `desde` con ese mismo capital.
//! - Empareja por vela de decisión y reparte la diferencia por causa (`paper_comparison.v1`).
//! - Además (#406) compara **todas las decisiones**, no solo las operaciones: cada orden del
//!   registro (llenada o rechazada por el riesgo) contra la señal del backtest en la misma vela y
//!   lado, y lista las que no coinciden.
//!   Con `--guardar`, la guarda en `paper_comparisons` (solo agregar).
//!
//! Solo lee la base (salvo `--guardar`). Nunca llama a IG. Solo paper.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_backtest::{
    BacktestConfig, DailyRates, FundingRates, market_terms_from_catalog, run_with_news,
    setup_from_catalog,
};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, MarketSetup, rejection_code, restore_and_reconcile,
};
use cfds_core::{Candle, InstrumentId, OrderSide, OrderStatus, Timeframe};
use cfds_gateway::paper_runner::{RunnerDeps, entry_flags, same_model};
use cfds_journal::pg::PgJournal;
use cfds_liquidity::Side;
use cfds_market_data::{
    Catalog, NewsFilter, ValidationConfig, check_series, decision_flags, spread_flags,
    trading_window,
};
use cfds_risk::RiskConfig;
use cfds_runner::compare::{Inputs, compare, entries_from};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let save = args.iter().any(|a| a == "--guardar");
    let args: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| *a != "--guardar")
        .collect();
    let [instrument, from, to] = args.as_slice() else {
        return Err("uso: <INSTRUMENTO> <desde UTC> <hasta UTC> [--guardar]".into());
    };
    let id: InstrumentId = instrument.parse()?;
    let (from, to): (DateTime<Utc>, DateTime<Utc>) = (from.parse()?, to.parse()?);
    if to <= from {
        return Err("<hasta> tiene que ser posterior a <desde>".into());
    }

    // La misma config que el runner y el backtest, validada.
    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let deps = RunnerDeps::from_repo_config(&catalog, &config_dir)?;
    let lookback = deps.runner.lock().await.config().lookback;
    let backtest_cfg = BacktestConfig::from_toml(cfg!("backtest.toml"))?;
    let risk = RiskConfig::from_toml(cfg!("risk.toml"))?;
    let broker_cfg = BrokerConfig::from_toml(
        cfg!("broker.toml"),
        risk,
        backtest_cfg.costs,
        backtest_cfg.exits,
        backtest_cfg.day_tz,
    )?
    // El mismo cierre de fin de semana que el backtest (#334).
    .with_calendar(&deps.calendar, catalog.instruments.keys())?;
    let validation = ValidationConfig::from_toml(cfg!("data_validation.toml"))?;

    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let start = from - lookback;
    let closed =
        |c: Vec<Candle>| -> Vec<Candle> { c.into_iter().filter(|c| c.end() <= to).collect() };
    let raw_m15 = closed(store.candles(&id, Timeframe::M15, start, to).await?);
    let raw_h1 = closed(store.candles(&id, Timeframe::H1, start, to).await?);

    // Financiación: la serie diaria guardada (#79), igual en los dos lados.
    let terms = market_terms_from_catalog(&catalog, &id)?;
    let series = catalog.instruments[&id]
        .terms
        .funding_benchmark
        .value()
        .ok_or("el instrumento no tiene tipo de referencia de financiación")?;
    let rates = match DailyRates::new(store.reference_rates(series, to).await?) {
        Some(d) => FundingRates::Daily(d),
        None => terms.funding_rates.clone(),
    };

    // Paper: el registro de este instrumento, reconstruido sobre las mismas velas.
    let journal: Vec<_> = store
        .paper_journal()
        .await?
        .into_iter()
        .filter(|(o, _)| o.instrument_id == id)
        .collect();
    let market = MarketSetup {
        limits: terms.limits,
        costs: terms.costs,
        account_to_instrument: terms.account_to_instrument,
        candles: raw_m15.clone(),
        funding: FundingSetup {
            cutoff: terms.funding_cutoff,
            tz: terms.funding_tz,
            rates: rates.clone(),
        },
    };
    let markets: BTreeMap<_, _> = [(id.clone(), market)].into_iter().collect();
    let (mut paper, rec) = restore_and_reconcile(broker_cfg, markets, &journal, &[], to);
    if !rec.drifts.is_empty() {
        eprintln!(
            "⚠️  {} informes de paper no salen iguales al reconstruirlos: revisar las velas",
            rec.drifts.len()
        );
    }
    let equity = paper.account(from).equity;

    // Backtest: mismas velas validadas, mismos filtros de entrada, mismas noticias.
    let a = check_series(&raw_m15, &deps.calendar, &validation);
    let b = check_series(&raw_h1, &deps.calendar, &validation);
    let decisions = decision_flags(&a.candles, &[&a, &b]);
    let mut errors = [a.data_errors.clone(), b.data_errors.clone()].concat();
    errors.sort();
    let (m15, h1) = (a.candles.clone(), b.candles.clone());
    let session = deps
        .calendar
        .session_for(&id)
        .ok_or("instrumento sin bolsa")?;
    let spreads = store
        .spreads(&id, Timeframe::M15, start - deps.spread.lookback, to)
        .await?;
    let spread_ok = spread_flags(&m15, &spreads, session.tz, &deps.spread);
    let nf = &deps.news_filter;
    let events = store
        .economic_events(start - nf.window, to + nf.window, to)
        .await?;
    let sightings = store.economic_event_sightings(to).await?;
    let news_ok = NewsFilter::new(&m15, &events, &sightings, nf).allowed;
    let allowed = entry_flags(session, &m15, &spread_ok, &news_ok, &deps.window);
    let entries: Vec<bool> = m15
        .iter()
        .zip(&decisions)
        .map(|(c, d)| *d && allowed.get(&c.start).copied().unwrap_or(false))
        .collect();
    // Antes de `desde` solo calentamiento: sin entradas, así el backtest llega a `desde` con el
    // capital de la cuenta paper y sin posición (#440).
    let entries = entries_from(&m15, &entries, from);
    let (_, force_close) =
        trading_window::flags(session, &m15, &deps.window).ok_or("fechas fuera del calendario")?;
    let news = same_model(
        store
            .news_interpretations(
                &deps.interpreter.name,
                &deps.interpreter.version,
                start - deps.strategy.strategy.news.window,
                to,
                Some(deps.news_published_from),
            )
            .await?,
        &deps.interpreter,
    );
    let mut setup = setup_from_catalog(&catalog, &id, deps.strategy, risk, backtest_cfg, equity)?;
    // Como en vivo: el kill switch no se reactiva.
    setup.backtest.kill_switch_reset_days = None;
    setup.funding_rates = rates;
    let report = run_with_news(
        &m15,
        &h1,
        &entries,
        &force_close,
        &errors,
        &news,
        deps.news_published_from,
        &setup,
    )
    .map_err(|e| format!("backtest: {e:?}"))?;

    let c = compare(&Inputs {
        instrument: &id,
        from,
        to,
        journal: &journal,
        paper: &paper.trade_outcomes(to),
        backtest: &report,
        m15: &m15,
        created_at: Utc::now(),
    });
    println!(
        "{id} del {from} al {to}: paper {} operaciones ({}), backtest {} ({})",
        c.paper.trades, c.paper.net_pnl, c.backtest.trades, c.backtest.net_pnl
    );
    println!(
        "emparejadas {}, faltan en paper {}, sobran en paper {}",
        c.matched.len(),
        c.missing.len(),
        c.extra.len()
    );
    println!("diferencia (paper − backtest): {}", c.difference);
    let k = &c.causes;
    for (name, v) in [
        ("precio de ejecución", k.fill_price),
        ("tamaño", k.size),
        ("comisión", k.commission),
        ("swap", k.swap),
        ("cierre de fin de semana", k.weekend_close),
        ("rechazo del riesgo", k.risk_rejection),
        ("falta en paper", k.missing_in_paper),
        ("sobra en paper", k.extra_in_paper),
    ] {
        println!("  {name}: {v}");
    }
    // Las decisiones, no solo las operaciones (#406): cada propuesta del runner (llenada o
    // rechazada por el riesgo) contra cada señal del backtest en la misma vela y lado.
    let paper_decisions: BTreeMap<(DateTime<Utc>, &str), String> = journal
        .iter()
        .filter(|(o, _)| o.created_at >= from && o.created_at <= to)
        .map(|(o, r)| {
            let what = match (r.status, r.rejection) {
                (_, Some(x)) => format!("rechazada:{x:?}"),
                (OrderStatus::Filled, None) => "llenada".to_owned(),
                (st, None) => format!("{st:?}"),
            };
            let side = match o.side {
                OrderSide::Long => "long",
                OrderSide::Short => "short",
            };
            ((o.created_at, side), what)
        })
        .collect();
    let side = |s: Side| match s {
        Side::Long => "long",
        Side::Short => "short",
    };
    let backtest_decisions: BTreeMap<(DateTime<Utc>, &str), String> = report
        .trades
        .iter()
        .map(|t| ((t.signal_at, side(t.side)), "llenada".to_owned()))
        .chain(report.rejected.iter().map(|r| {
            (
                (m15[r.index].end(), side(r.side)),
                format!("rechazada:{:?}", rejection_code(&r.reason)),
            )
        }))
        .filter(|((at, _), _)| *at >= from && *at <= to)
        .collect();
    let same = paper_decisions
        .iter()
        .filter(|(k, v)| backtest_decisions.get(*k) == Some(*v))
        .count();
    println!(
        "decisiones: paper {}, backtest {}, iguales (vela, lado y resultado) {same}",
        paper_decisions.len(),
        backtest_decisions.len()
    );
    for (k, v) in &paper_decisions {
        let b = backtest_decisions.get(k);
        if b != Some(v) {
            println!("  ≠ {} {}: paper {v}, backtest {b:?}", k.0, k.1);
        }
    }
    for (k, v) in &backtest_decisions {
        if !paper_decisions.contains_key(k) {
            println!("  ≠ {} {}: paper —, backtest {v}", k.0, k.1);
        }
    }
    let doc = serde_json::to_value(&c)?;
    if save {
        let j = PgJournal::new(store.pool().clone());
        println!(
            "guardada {}: {:?}",
            c.comparison_id,
            j.record_paper_comparison(&doc).await?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    }
    Ok(())
}

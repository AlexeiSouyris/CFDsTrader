//! El runner de paper trading como modo del gateway (fase 8, #305).
//!
//! ```text
//! cada [runner] poll_secs (si [runner] enabled):
//!   candado del broker (el mismo de las rutas: un solo PaperBroker y un solo registro)
//!   velas nuevas al broker (refresh) → por instrumento:
//!     velas de 15m y 1h de la base (lookback_days) · filtros de entrada del backtest
//!     (horario, spread §7.3, noticias de alto impacto §7.2) · ajuste de noticias §9
//!     → cfds_runner::Runner::step → orden propuesta → submit_and_record (riesgo + registro)
//!     → avisos y eventos del broker al log y a las alertas
//! ```
//! Nunca llama a IG: solo lee lo que la ingesta ya guardó. Con la compuerta fuera de `Ready`
//! (arrancando o apagándose) no propone; el kill switch lo respeta el runner y el broker.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use cfds_broker::{Broker, submit_and_record};
use cfds_core::{Candle, InstrumentId, NewsInterpretation, OrderStatus, Timeframe};
use cfds_journal::ShownProbability;
use cfds_journal::pg::PgJournal;
use cfds_journal::probability::{ProbabilityConfig, ProbabilityTable};
use cfds_market_data::{
    Catalog, ExchangeCalendar, NewsFilter, NewsFilterConfig, Session, SpreadFilterConfig,
    TradingWindowConfig, ValidationConfig, spread_flags, trading_window,
};
use cfds_obs::{AlertKind, Level};
use cfds_runner::journal::{paper_outcome, paper_proposal};
use cfds_runner::{Feed, Runner, RunnerConfig, publish, strategy_from_toml};
use cfds_strategy::{BacktestInterpreter, Configs, InterpreterCutoff, news_adjustment};
use chrono::{DateTime, Utc};
use serde_json::json;
use tokio::sync::Mutex;

use crate::ops::GateState;
use crate::paper::PgPaper;

/// Lo que el runner necesita además del broker: su estado, el calendario y la config de los
/// filtros de entrada y de las noticias (la misma del backtest).
pub struct RunnerDeps {
    pub runner: Mutex<Runner>,
    pub calendar: ExchangeCalendar,
    pub strategy: Configs,
    pub window: TradingWindowConfig,
    pub spread: SpreadFilterConfig,
    pub news_filter: NewsFilterConfig,
    pub interpreter: BacktestInterpreter,
    /// Solo noticias publicadas desde acá (corte del modelo intérprete, ADR 0008 §5).
    pub news_published_from: DateTime<Utc>,
    /// La tabla de la probabilidad mostrada (§13), si `[runner] probability_run_id` está
    /// configurada ([`RunnerDeps::load_probability`]); sin ella, "datos insuficientes".
    pub probability: Option<ProbabilityTable>,
    pub probability_cfg: ProbabilityConfig,
    /// Propuestas de paper con el resultado ya en el diario (en memoria; tras reiniciar se
    /// mira el diario antes de valuar: lo ya guardado no se vuelve a escribir, #438).
    pub outcomes_done: Mutex<BTreeSet<String>>,
}

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

impl RunnerDeps {
    /// Todo desde `config/` del repo, validado al arrancar (CLAUDE.md §5b). `config_dir` es la
    /// carpeta de donde el calendario lee las sesiones y los feriados.
    pub fn from_repo_config(catalog: &Catalog, config_dir: &Path) -> Result<Self, String> {
        let strategy = strategy_from_toml(
            cfg!("patterns.toml"),
            cfg!("liquidity.toml"),
            cfg!("indicators.toml"),
            cfg!("strategy.toml"),
        )?;
        let e = |e: &dyn std::fmt::Display| e.to_string();
        Ok(Self {
            runner: Mutex::new(Runner::new(
                RunnerConfig::from_toml(cfg!("runner.toml"))?,
                strategy,
                ValidationConfig::from_toml(cfg!("data_validation.toml")).map_err(|x| e(&x))?,
            )),
            calendar: ExchangeCalendar::load(config_dir, catalog.exchange_map())
                .map_err(|x| e(&x))?,
            strategy,
            window: TradingWindowConfig::from_toml(cfg!("trading_window.toml"))?,
            spread: SpreadFilterConfig::from_toml(cfg!("spread_filter.toml"))?,
            news_filter: NewsFilterConfig::from_toml(cfg!("news_filter.toml"))?,
            interpreter: BacktestInterpreter::from_toml(cfg!("news_interpreter.toml"))?,
            news_published_from: InterpreterCutoff::from_toml(cfg!("news_interpreter.toml"))?
                .backtest_published_from(),
            probability: None,
            probability_cfg: ProbabilityConfig::from_toml(cfg!("probability.toml"))
                .map_err(|x| e(&x))?,
            outcomes_done: Mutex::new(BTreeSet::new()),
        })
    }

    /// Arma la tabla de la probabilidad mostrada con la corrida de backtest de la config, solo
    /// con resultados terminados hasta `probability_until` (#333). Sin corrida configurada, no
    /// hace nada.
    pub async fn load_probability(&mut self, journal: &PgJournal) -> Result<(), String> {
        let Some((run, until)) = self.runner.lock().await.config().probability.clone() else {
            return Ok(());
        };
        let pairs: Vec<_> = journal
            .run_trades(&run)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter_map(|t| Some((t.proposal, t.outcome?)))
            .collect();
        if pairs.is_empty() {
            return Err(format!(
                "la corrida {run} no tiene propuestas con resultado"
            ));
        }
        self.probability = Some(
            ProbabilityTable::build(&pairs, until, &self.probability_cfg)
                .map_err(|e| e.to_string())?,
        );
        Ok(())
    }

    /// La probabilidad que se muestra para una propuesta.
    pub fn shown(&self, p: &cfds_journal::Proposal) -> ShownProbability {
        match &self.probability {
            Some(t) => t.shown(p).into(),
            None => ShownProbability::Insufficient {
                cases: 0,
                needed: self.probability_cfg.min_cases,
            },
        }
    }
}

/// Si se puede entrar al decidir en el cierre de cada vela (por su inicio): horario de
/// operación, spread de cierre (§7.3) y noticias de alto impacto (§7.2), como en el backtest.
/// Sin cobertura del calendario de sesiones, ninguna vela deja entrar.
pub fn entry_flags(
    session: &Session,
    m15: &[Candle],
    spread_ok: &[bool],
    news_ok: &[bool],
    window: &TradingWindowConfig,
) -> HashMap<DateTime<Utc>, bool> {
    let entries = trading_window::flags(session, m15, window)
        .map_or_else(|| vec![false; m15.len()], |(e, _)| e);
    m15.iter()
        .enumerate()
        .map(|(i, c)| {
            let ok = entries[i]
                && spread_ok.get(i).copied().unwrap_or(false)
                && news_ok.get(i).copied().unwrap_or(false);
            (c.start, ok)
        })
        .collect()
}

/// Solo las interpretaciones del modelo fijado (las de otro modelo no cuentan).
pub fn same_model(
    items: Vec<NewsInterpretation>,
    interpreter: &BacktestInterpreter,
) -> Vec<NewsInterpretation> {
    items
        .into_iter()
        .filter(|n| n.interpreter.model.as_deref() == Some(interpreter.model.as_str()))
        .collect()
}

impl PgPaper {
    async fn read<T, E: std::fmt::Display>(
        &self,
        what: &str,
        r: Result<T, E>,
        now: DateTime<Utc>,
    ) -> Result<T, String> {
        match r {
            Ok(v) => Ok(v),
            Err(e) => {
                let e = format!("{what}: {e}");
                self.fail(AlertKind::Disconnect, "runner_read_failed", &e, now)
                    .await;
                Err(e)
            }
        }
    }

    /// Un paso del runner para todos los instrumentos que opera el broker.
    pub async fn run_once(&self, deps: &RunnerDeps, now: DateTime<Utc>) -> Result<(), String> {
        let mut guard = self.state.lock().await;
        // La compuerta se mira con el candado tomado: si la parada empezó, no propone.
        let ready = self.gate.state() == GateState::Ready;
        let (broker, sched) = &mut *guard;
        self.refresh_checked(broker, sched, now).await?;
        let mut runner = deps.runner.lock().await;
        let lookback = runner.config().lookback;
        let ids: Vec<InstrumentId> = broker.markets().keys().cloned().collect();
        for id in ids {
            let Some(session) = deps.calendar.session_for(&id) else {
                self.ops.log.log(
                    Level::Warn,
                    "runner",
                    "no_session",
                    json!({ "instrument_id": id }),
                );
                continue;
            };
            let from = now - lookback;
            let m15 = self
                .read(
                    "velas 15m",
                    self.store.candles(&id, Timeframe::M15, from, now).await,
                    now,
                )
                .await?;
            let h1 = self
                .read(
                    "velas 1h",
                    self.store.candles(&id, Timeframe::H1, from, now).await,
                    now,
                )
                .await?;
            let closed: Vec<Candle> = m15.iter().filter(|c| c.end() <= now).cloned().collect();
            let spreads = self
                .read(
                    "spreads",
                    self.store
                        .spreads(&id, Timeframe::M15, from - deps.spread.lookback, now)
                        .await,
                    now,
                )
                .await?;
            let spread_ok = spread_flags(&closed, &spreads, session.tz, &deps.spread);
            let nf = &deps.news_filter;
            let events = self
                .read(
                    "calendario económico",
                    self.store
                        .economic_events(from - nf.window, now + nf.window, now)
                        .await,
                    now,
                )
                .await?;
            let sightings = self
                .read(
                    "calendario económico",
                    self.store.economic_event_sightings(now).await,
                    now,
                )
                .await?;
            let news_ok = NewsFilter::new(&closed, &events, &sightings, nf).allowed;
            let allowed = entry_flags(session, &closed, &spread_ok, &news_ok, &deps.window);
            let s = &deps.strategy.strategy;
            let news = self
                .read(
                    "noticias",
                    self.store
                        .news_interpretations(
                            &deps.interpreter.name,
                            &deps.interpreter.version,
                            from - s.news.window,
                            now,
                            Some(deps.news_published_from),
                        )
                        .await,
                    now,
                )
                .await?;
            let news = same_model(news, &deps.interpreter);
            let step = runner.step(
                broker,
                Feed {
                    instrument: &id,
                    m15: &m15,
                    h1: &h1,
                },
                &deps.calendar,
                ready,
                |m, t| allowed.get(&m[t].start).copied().unwrap_or(false),
                |side, at| news_adjustment(&news, &id, side, at, &s.news, &s.score),
                now,
            );
            publish(&self.ops.log, &self.ops.alerts, &step.notices, now).await;
            if let (Some(order), Some(signal)) = (&step.order, &step.signal) {
                // Con el registro vacío, las velas desde esta orden menos la ventana, como las
                // cargará la reconstrucción (#481). Si no se pueden leer, no se manda.
                if let Err(e) = self.cover_first_order(broker, order, now).await {
                    self.fail(AlertKind::Disconnect, "store_failed", &e, now)
                        .await;
                    continue;
                }
                self.anchor_candles(broker);
                // El apalancamiento que decide el riesgo, antes de enviar (la vista previa no
                // cambia nada).
                let verdict = broker.preview(order, order.created_at);
                match submit_and_record(broker, &self.store, order).await {
                    Ok(report) if report.status == OrderStatus::Filled => {
                        let (Some(size), Some(leverage)) = (report.size, verdict.leverage) else {
                            continue;
                        };
                        let run_id = runner.config().order_prefix.clone();
                        let tp1_r = broker.config().exits.tp1_r;
                        let mut p =
                            paper_proposal(&run_id, order, signal, tp1_r, size, leverage, None);
                        p.shown = Some(deps.shown(&p));
                        if let Err(e) = self.journal.record(&p, None).await {
                            self.fail(
                                AlertKind::Disconnect,
                                "proposal_write_failed",
                                &e.to_string(),
                                now,
                            )
                            .await;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        // El informe quedó en el broker: la parada segura lo guarda si la base
                        // vuelve.
                        self.fail(
                            AlertKind::Disconnect,
                            "journal_write_failed",
                            &e.to_string(),
                            now,
                        )
                        .await;
                    }
                }
            }
            self.drain(broker, now).await;
        }
        self.record_outcomes(broker, deps, now).await;
        Ok(())
    }

    /// Los resultados de las operaciones de paper que ya terminaron, al diario (#333). Solo
    /// cuando la vela de la última salida cerró (`paper_outcome`), y una sola vez.
    async fn record_outcomes(
        &self,
        broker: &mut cfds_broker::PaperBroker,
        deps: &RunnerDeps,
        now: DateTime<Utc>,
    ) {
        let mut done = deps.outcomes_done.lock().await;
        let proposals: HashMap<String, String> = broker
            .journal()
            .iter()
            .filter_map(|(o, _)| {
                Some((o.order_id.to_string(), o.proposal_id.as_ref()?.to_string()))
            })
            .collect();
        for t in broker.trade_outcomes(now) {
            let Some(pid) = proposals.get(t.order_id.as_str()) else {
                continue;
            };
            if done.contains(pid) {
                continue;
            }
            let Some(outcome) = paper_outcome(pid, &t) else {
                continue;
            };
            // Después de un reinicio: si el resultado ya está en el diario, no se reescribe
            // (el diario nunca se reescribe; una valuación distinta sería un error en cada paso).
            match self.journal.outcome(pid, outcome.kind).await {
                Ok(Some(_)) => {
                    done.insert(pid.clone());
                    continue;
                }
                Ok(None) => {}
                Err(e) => {
                    self.fail(
                        AlertKind::Disconnect,
                        "outcome_write_failed",
                        &e.to_string(),
                        now,
                    )
                    .await;
                    continue;
                }
            }
            let written = match self.journal.proposal(pid).await {
                Ok(Some(p)) => self.journal.record(&p, Some(&outcome)).await.map(|_| ()),
                // Sin propuesta en el diario (por ejemplo, una orden del ticket manual): nada.
                Ok(None) => Ok(()),
                Err(e) => Err(e),
            };
            match written {
                Ok(()) => {
                    done.insert(pid.clone());
                }
                Err(e) => {
                    self.fail(
                        AlertKind::Disconnect,
                        "outcome_write_failed",
                        &e.to_string(),
                        now,
                    )
                    .await;
                }
            }
        }
    }
}

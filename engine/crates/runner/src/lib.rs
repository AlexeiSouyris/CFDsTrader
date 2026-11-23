//! Runner de paper trading (fase 8, #305): la estrategia de siempre, en tiempo real, contra el
//! broker simulado. Solo paper; `LiveBroker` sigue sin poder construirse.
//!
//! ```text
//! cada poll_secs, por instrumento (velas cerradas ya guardadas; nunca llama a IG):
//!   validar (check_series) ─ error de datos nuevo o vela que falta en sesión → kill switch + alerta
//!   corte de financiación del día → liquidación del swap (posiciones y cuenta al log)
//!   última vela cerrada L, decisión en t = L − 1 (solo con velas hasta el cierre de t)
//!   compuerta lista, kill switch apagado, entrada permitida, señal → orden con id
//!   <prefijo>:<instrumento>:<cierre de t> y created_at = cierre de t → el broker la llena en la
//!   apertura de t + 1 = L, igual que el backtest
//! ```
//! El runner **no envía** la orden: devuelve la [`Order`] y quien lo usa la manda con
//! `submit_and_record` (así queda en el registro). El mismo id para la misma decisión hace que
//! un reinicio no la duplique: el broker restaurado devuelve el informe guardado.

pub mod compare;

use std::collections::{BTreeMap, BTreeSet};

use cfds_backtest::choose;
use cfds_broker::{Broker, PaperBroker};
use cfds_core::{Candle, InstrumentId, Order, OrderSide, PaperMode, Position, Timeframe};
use cfds_liquidity::Side;
use cfds_market_data::{SessionCalendar, ValidationConfig, check_series, decision_flags};
use cfds_obs::{AlertKind, Alerter, HttpPost, Level, Logger};
use cfds_strategy::{Analysis, Configs};
use chrono::{DateTime, NaiveDate, SecondsFormat, TimeDelta, Utc};
use serde::Deserialize;
use serde_json::json;

/// `config/runner.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerConfig {
    pub enabled: bool,
    pub poll: std::time::Duration,
    pub lookback: TimeDelta,
    pub stale_after: TimeDelta,
    pub order_prefix: String,
}

impl RunnerConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            runner: RawRunner,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawRunner {
            enabled: bool,
            poll_secs: u64,
            lookback_days: u32,
            stale_after_minutes: u32,
            order_prefix: String,
        }
        let r = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .runner;
        if !(1..=3600).contains(&r.poll_secs) {
            return Err("runner.poll_secs: de 1 a 3600".into());
        }
        if !(1..=60).contains(&r.lookback_days) {
            return Err("runner.lookback_days: de 1 a 60".into());
        }
        if !(15..=24 * 60).contains(&r.stale_after_minutes) {
            return Err("runner.stale_after_minutes: de 15 a 1440".into());
        }
        let prefix_ok = (1..=20).contains(&r.order_prefix.len())
            && r.order_prefix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
        if !prefix_ok {
            return Err("runner.order_prefix: de 1 a 20 letras, números, '.', '_' o '-'".into());
        }
        Ok(Self {
            enabled: r.enabled,
            poll: std::time::Duration::from_secs(r.poll_secs),
            lookback: TimeDelta::days(i64::from(r.lookback_days)),
            stale_after: TimeDelta::minutes(i64::from(r.stale_after_minutes)),
            order_prefix: r.order_prefix,
        })
    }
}

/// La config de la estrategia (la misma del backtest), validada: `config/patterns.toml`,
/// `liquidity.toml`, `indicators.toml` y `strategy.toml`.
pub fn strategy_from_toml(
    patterns: &str,
    liquidity: &str,
    indicators: &str,
    strategy: &str,
) -> Result<Configs, String> {
    Ok(Configs {
        patterns: cfds_patterns::PatternsConfig::from_toml(patterns).map_err(|e| e.to_string())?,
        liquidity: cfds_liquidity::LiquidityConfig::from_toml(liquidity)
            .map_err(|e| e.to_string())?,
        trend: cfds_indicators::TrendConfig::from_toml(indicators).map_err(|e| e.to_string())?,
        strategy: cfds_strategy::StrategyConfig::from_toml(strategy).map_err(|e| e.to_string())?,
    })
}

/// Lo que pasó en un paso, para el log y las alertas (ver [`publish`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// Problemas de datos encontrados por la validación (spec §12).
    DataIssues {
        instrument: InstrumentId,
        issues: Vec<String>,
    },
    /// Un hueco o error de datos nuevo: se activó el kill switch.
    DataGap {
        instrument: InstrumentId,
        detail: String,
    },
    /// Liquidación diaria del swap: posiciones del instrumento valuadas al corte de financiación.
    SwapSettled {
        instrument: InstrumentId,
        day: NaiveDate,
        positions: Vec<Position>,
    },
    /// No se decidió en esta vela, con el motivo.
    Skipped {
        instrument: InstrumentId,
        decision_at: DateTime<Utc>,
        reason: &'static str,
    },
    /// La estrategia no dio señal (o la entrada no estaba permitida).
    NoSignal {
        instrument: InstrumentId,
        decision_at: DateTime<Utc>,
    },
    /// La estrategia propuso esta orden.
    Proposed { order: Order, score: i32 },
}

/// Resultado de un paso de un instrumento.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Step {
    /// La orden a enviar (con `submit_and_record`), si hubo propuesta.
    pub order: Option<Order>,
    pub notices: Vec<Notice>,
}

#[derive(Debug, Default)]
struct State {
    /// Errores de datos ya vistos (los que había al arrancar no disparan nada: el calentamiento
    /// ya los respeta).
    seen_errors: Option<BTreeSet<DateTime<Utc>>>,
    /// Cierre de la última vela en la que se decidió.
    decided: Option<DateTime<Utc>>,
    /// Último día con el swap liquidado.
    settled: Option<NaiveDate>,
    /// Ya se avisó que faltan velas en vivo (una vez por hueco).
    stale: bool,
}

/// El runner: la estrategia (funciones puras) con su estado mínimo por instrumento.
pub struct Runner {
    cfg: RunnerConfig,
    strategy: Configs,
    validation: ValidationConfig,
    state: BTreeMap<InstrumentId, State>,
}

/// Las velas de un instrumento leídas de la base (pueden incluir la vela en curso: se ignora).
pub struct Feed<'a> {
    pub instrument: &'a InstrumentId,
    pub m15: &'a [Candle],
    pub h1: &'a [Candle],
}

fn order_side(s: Side) -> OrderSide {
    match s {
        Side::Long => OrderSide::Long,
        Side::Short => OrderSide::Short,
    }
}

impl Runner {
    pub fn new(cfg: RunnerConfig, strategy: Configs, validation: ValidationConfig) -> Self {
        Self {
            cfg,
            strategy,
            validation,
            state: BTreeMap::new(),
        }
    }

    pub fn config(&self) -> &RunnerConfig {
        &self.cfg
    }

    /// Id idempotente de la decisión al cierre `decision_at`.
    pub fn order_id(&self, instrument: &InstrumentId, decision_at: DateTime<Utc>) -> String {
        format!(
            "{}:{instrument}:{}",
            self.cfg.order_prefix,
            decision_at.format("%Y%m%dT%H%MZ")
        )
    }

    /// Un paso para un instrumento en `now`.
    ///
    /// - `ready`: la compuerta del broker (solo en `Ready` se proponen órdenes).
    /// - `entry_allowed(m15, t)`: los filtros de entrada del backtest (horario, spread, noticias)
    ///   para decidir al cierre de `m15[t]`.
    /// - `news(side, at)`: el ajuste de noticias del puntaje con lo interpretado hasta `at`, el
    ///   cierre de la vela de la decisión (spec §9; `cfds_strategy::news_adjustment`).
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        broker: &mut PaperBroker,
        feed: Feed<'_>,
        calendar: &impl SessionCalendar,
        ready: bool,
        entry_allowed: impl Fn(&[Candle], usize) -> bool,
        news: impl Fn(Side, DateTime<Utc>) -> i32,
        now: DateTime<Utc>,
    ) -> Step {
        let id = feed.instrument.clone();
        let mut out = Step::default();
        // Solo velas cerradas (CLAUDE.md §5b).
        let closed = |c: &[Candle]| -> Vec<Candle> {
            c.iter().filter(|c| c.end() <= now).cloned().collect()
        };
        let (raw_m15, raw_h1) = (closed(feed.m15), closed(feed.h1));
        let a = check_series(&raw_m15, calendar, &self.validation);
        let b = check_series(&raw_h1, calendar, &self.validation);
        let issues: Vec<String> = a
            .issues
            .iter()
            .chain(&b.issues)
            .map(|i| format!("{}: {}", i.start, i.kind))
            .collect();
        if !issues.is_empty() {
            out.notices.push(Notice::DataIssues {
                instrument: id.clone(),
                issues,
            });
        }
        let mut errors = [a.data_errors.clone(), b.data_errors.clone()].concat();
        errors.sort();
        errors.dedup();
        let decisions = decision_flags(&a.candles, &[&a, &b]);
        let m15 = a.candles.clone();
        let h1 = b.candles.clone();
        let st = self.state.entry(id.clone()).or_default();

        // Huecos y errores de datos nuevos: kill switch y alerta (spec §12).
        let seen = st
            .seen_errors
            .get_or_insert_with(|| errors.iter().copied().collect());
        let fresh: Vec<DateTime<Utc>> = errors
            .iter()
            .copied()
            .filter(|e| !seen.contains(e))
            .collect();
        seen.extend(fresh.iter().copied());
        if let Some(first) = fresh.first() {
            let detail = format!("error de datos en {id} desde {first}");
            broker.activate_kill_switch(format!("Datos: {detail} (spec §12)."), now);
            out.notices.push(Notice::DataGap {
                instrument: id.clone(),
                detail,
            });
        }
        let Some(last) = m15.last() else {
            return out;
        };
        // Velas que tendrían que haber cerrado hace más de `stale_after` y no están.
        let due = now - self.cfg.stale_after - Timeframe::M15.duration();
        let missing = if due > last.end() {
            calendar
                .expected_starts(&id, Timeframe::M15, last.end(), due)
                .map(|v| v.len())
        } else {
            Some(0)
        };
        match missing {
            Some(n) if n > 0 => {
                if !st.stale {
                    st.stale = true;
                    let detail = format!(
                        "faltan {n} velas de 15m de {id} desde {} (datos en vivo)",
                        last.end()
                    );
                    broker.activate_kill_switch(format!("Datos: {detail} (spec §12)."), now);
                    out.notices.push(Notice::DataGap {
                        instrument: id.clone(),
                        detail,
                    });
                }
            }
            Some(_) => st.stale = false,
            None => {}
        }

        // Liquidación diaria del swap, al pasar el corte de financiación del instrumento.
        if let Some(f) = broker.markets().get(&id).map(|m| m.funding.clone()) {
            let local = now.with_timezone(&f.tz);
            let day = local.date_naive();
            if local.time() >= f.cutoff && st.settled != Some(day) {
                st.settled = Some(day);
                let positions = broker
                    .positions(now)
                    .into_iter()
                    .filter(|p| p.instrument_id == id)
                    .collect();
                out.notices.push(Notice::SwapSettled {
                    instrument: id.clone(),
                    day,
                    positions,
                });
            }
        }

        // La decisión: en t = L − 1, ejecutada por el broker en la apertura de L.
        let Some(t) = m15.len().checked_sub(2) else {
            return out;
        };
        let decision_at = m15[t].end();
        if st.decided == Some(decision_at) {
            return out;
        }
        let skip = |reason| Notice::Skipped {
            instrument: id.clone(),
            decision_at,
            reason,
        };
        if !ready {
            // No se marca: cuando la compuerta abra, se decide sobre la última vela.
            out.notices.push(skip("compuerta cerrada"));
            return out;
        }
        st.decided = Some(decision_at);
        if now - last.end() > self.cfg.stale_after {
            out.notices.push(skip("datos viejos"));
            return out;
        }
        if broker.kill_switch_active() {
            out.notices.push(skip("kill switch activo"));
            return out;
        }
        if !decisions[t] || !entry_allowed(&m15, t) {
            out.notices.push(Notice::NoSignal {
                instrument: id,
                decision_at,
            });
            return out;
        }
        // Sin look-ahead: el análisis ve solo las velas hasta el cierre de t.
        let m15_t = &m15[..=t];
        let h1_t: Vec<Candle> = h1
            .iter()
            .filter(|c| c.end() <= decision_at)
            .cloned()
            .collect();
        let analysis = Analysis::new(m15_t, &h1_t, self.strategy).with_data_errors(errors);
        let signal = match choose(&analysis, t, |side| news(side, decision_at)) {
            Ok(Some(s)) => s,
            Ok(None) | Err(_) => {
                out.notices.push(Notice::NoSignal {
                    instrument: id,
                    decision_at,
                });
                return out;
            }
        };
        let Ok(order_id) = self.order_id(&id, decision_at).parse() else {
            out.notices.push(skip("id de orden inválido"));
            return out;
        };
        let order = Order {
            order_id,
            mode: PaperMode::Paper,
            instrument_id: id,
            side: order_side(signal.side),
            stop: signal.stop,
            tp1: None,
            proposal_id: None,
            created_at: decision_at,
        };
        out.notices.push(Notice::Proposed {
            order: order.clone(),
            score: signal.score.total,
        });
        out.order = Some(order);
        out
    }
}

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Escribe los avisos del paso en el log (`target = "runner"`) y alerta los huecos de datos.
/// Los eventos del broker (riesgo, informe, kill switch) los escribe quien lo usa.
pub async fn publish<H: HttpPost>(
    log: &Logger,
    alerts: &Alerter<H>,
    notices: &[Notice],
    now: DateTime<Utc>,
) {
    for n in notices {
        match n {
            Notice::DataIssues { instrument, issues } => log.log(
                Level::Warn,
                "runner",
                "data_issues",
                json!({ "instrument_id": instrument, "issues": issues }),
            ),
            Notice::DataGap { instrument, detail } => {
                log.log(
                    Level::Error,
                    "runner",
                    "data_gap",
                    json!({ "instrument_id": instrument, "detail": detail }),
                );
                alerts.raise(AlertKind::DataError, detail, now).await;
            }
            Notice::SwapSettled {
                instrument,
                day,
                positions,
            } => log.log(
                Level::Info,
                "runner",
                "swap_settled",
                json!({ "instrument_id": instrument, "day": day.to_string(), "positions": positions }),
            ),
            Notice::Skipped {
                instrument,
                decision_at,
                reason,
            } => log.log(
                Level::Info,
                "runner",
                "decision_skipped",
                json!({ "instrument_id": instrument, "decision_at": ts(*decision_at), "reason": reason }),
            ),
            Notice::NoSignal {
                instrument,
                decision_at,
            } => log.log(
                Level::Info,
                "runner",
                "no_signal",
                json!({ "instrument_id": instrument, "decision_at": ts(*decision_at) }),
            ),
            Notice::Proposed { order, score } => log.log(
                Level::Info,
                "runner",
                "proposal",
                json!({ "order": order, "score": score }),
            ),
        }
    }
}

/// Envía la propuesta al broker (para los tests y para quien no guarda en la base; el gateway
/// usa `submit_and_record`).
pub fn submit(broker: &mut PaperBroker, step: &Step) -> Option<cfds_core::OrderReport> {
    step.order.as_ref().map(|o| broker.submit(o))
}

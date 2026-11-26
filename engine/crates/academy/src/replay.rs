//! Replay de exámenes (docs/design/2026-11-academia.md §5, #721): una sesión con velas reales
//! guardadas, instrumento, timeframe y fecha **al azar con una semilla**, sorteados **solo dentro
//! del período de investigación** (nunca los meses fuera de muestra de `config/data_split.toml`,
//! con la misma guardia que la investigación, `research_period`). El cursor vive acá: nada
//! entrega una vela posterior al cursor ([`ReplaySession::bars_until`] la rechaza), y la fecha
//! real no sale hasta el final (las velas van con su número, el día de la sesión y la hora UTC).
//!
//! Las marcas y las entradas llevan el cursor del momento; la calificación es otro módulo.

use std::future::Future;

use cfds_backtest::research::program4::SplitMix64;
use cfds_backtest::research_period;
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{Period, SplitConfig, plan};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplayKind {
    /// Sin nota.
    Practice,
    Levels,
    Trades,
    /// Marcar pivotes confirmados (#800).
    Pivots,
}

/// `[replay]` de `config/academy.toml`. Umbrales educativos y PROVISORIOS.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayConfig {
    pub instruments: Vec<InstrumentId>,
    pub timeframes: Vec<Timeframe>,
    /// Fuente de las velas: hoy `futures` (la tabla `futures_candles` de esta fuente).
    pub source: String,
    pub futures_source: String,
    pub warmup_bars: usize,
    pub exam_bars: usize,
    pub practice_bars: usize,
    pub trades_per_exam: usize,
    /// Pivotes marcados como mínimo en un examen de pivotes (#800).
    pub pivots_per_exam: usize,
    pub level_tolerance_atr: Decimal,
    pub shift_prices: bool,
    pub max_step_bars: usize,
    pub max_sessions: usize,
    pub max_draws: u32,
}

impl ReplayConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let positive = [
            self.warmup_bars,
            self.exam_bars,
            self.practice_bars,
            self.trades_per_exam,
            self.pivots_per_exam,
        ]
        .iter()
        .chain(&[self.max_step_bars, self.max_sessions])
        .all(|n| *n > 0);
        if self.instruments.is_empty()
            || self.timeframes.is_empty()
            || !positive
            || self.max_draws == 0
        {
            return Err("replay: instrumentos, timeframes y cantidades > 0".into());
        }
        if self.source != "futures" || self.futures_source.is_empty() {
            return Err("replay.source: por ahora solo futures, con futures_source".into());
        }
        if self.shift_prices {
            return Err("replay.shift_prices = true todavía no existe (propuesta del diseño: no mover el precio)".into());
        }
        if self.level_tolerance_atr <= Decimal::ZERO {
            return Err("replay.level_tolerance_atr > 0".into());
        }
        Ok(())
    }

    /// Velas del examen según el tipo: la práctica es más corta.
    pub fn bars_for(&self, kind: ReplayKind) -> usize {
        match kind {
            ReplayKind::Practice => self.practice_bars,
            ReplayKind::Levels | ReplayKind::Trades | ReplayKind::Pivots => self.exam_bars,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReplayError {
    #[error("no hay datos suficientes en el período de investigación de {0}")]
    NoData(String),
    #[error("después de {0} sorteos no apareció una ventana con velas completas")]
    NoWindow(u32),
    #[error("la vela {asked} todavía no se ve: el cursor está en {cursor}")]
    AfterCursor { asked: usize, cursor: usize },
    #[error("la sesión ya terminó")]
    Finished,
    #[error("pedido inválido: {0}")]
    Bad(String),
    #[error("datos: {0}")]
    Store(String),
}

/// Inicio de la primera y de la última vela guardada.
pub type BarRange = (DateTime<Utc>, DateTime<Utc>);

/// De dónde salen las velas cerradas guardadas. En producción, PostgreSQL desde el gateway.
pub trait BarStore: Sync {
    /// Inicio de la primera y de la última vela guardada, o `None`.
    fn range(
        &self,
        instrument: &InstrumentId,
        tf: Timeframe,
    ) -> impl Future<Output = Result<Option<BarRange>, String>> + Send;
    /// Velas con inicio en `[from, to)`, en orden.
    fn bars(
        &self,
        instrument: &InstrumentId,
        tf: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Candle>, String>> + Send;
}

/// Lo que se sorteó con la semilla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw {
    pub instrument_id: InstrumentId,
    pub timeframe: Timeframe,
    /// El tramo que se puede usar: desde la primera vela hasta el inicio del fuera de muestra.
    pub research: Period,
}

/// Abre una sesión: sortea instrumento y timeframe con `seed`, y después fechas de inicio dentro
/// del período de investigación hasta encontrar `warmup + examen` velas seguidas que cierren antes
/// del fuera de muestra (como mucho `max_draws` sorteos). Misma semilla y mismos datos, misma
/// sesión.
pub async fn open_session<S: BarStore>(
    store: &S,
    cfg: &ReplayConfig,
    split: &SplitConfig,
    kind: ReplayKind,
    lesson_id: Option<String>,
    seed: u64,
    id_salt: u64,
) -> Result<ReplaySession, ReplayError> {
    let mut rng = SplitMix64::new(seed);
    let instrument = cfg.instruments[rng.below(cfg.instruments.len())].clone();
    let tf = cfg.timeframes[rng.below(cfg.timeframes.len())];
    let no_data = || ReplayError::NoData(instrument.to_string());
    let (first, last) = store
        .range(&instrument, tf)
        .await
        .map_err(ReplayError::Store)?
        .ok_or_else(no_data)?;
    let split_plan = plan(first, last, split).map_err(|_| no_data())?;
    let research = Period {
        start: first,
        end: split_plan.out_of_sample.start,
    };
    let need = cfg.warmup_bars + cfg.bars_for(kind);
    // Ventana de calendario holgada para `need` velas (fines de semana y feriados incluidos).
    let span = tf.duration() * i32::try_from(need).unwrap_or(i32::MAX) * 2 + TimeDelta::days(7);
    let latest_start = research.end - span;
    let choices = (latest_start - research.start).num_days();
    if choices <= 0 {
        return Err(no_data());
    }
    for _ in 0..cfg.max_draws {
        let day =
            i64::try_from(rng.below(usize::try_from(choices).unwrap_or(usize::MAX))).unwrap_or(0);
        let from = research.start + TimeDelta::days(day);
        let mut bars = store
            .bars(&instrument, tf, from, from + span)
            .await
            .map_err(ReplayError::Store)?;
        if bars.len() < need {
            continue;
        }
        bars.truncate(need);
        let window = Period {
            start: bars[0].start,
            end: bars[need - 1].end(),
        };
        // La misma guardia que la investigación: nada que toque el fuera de muestra.
        if research_period(window, &split_plan).is_err() {
            continue;
        }
        let draw = Draw {
            instrument_id: instrument,
            timeframe: tf,
            research,
        };
        let id = format!("rp-{:016x}", SplitMix64::new(seed ^ id_salt).next_u64());
        return ReplaySession::new(id, kind, lesson_id, seed, draw, bars, cfg.warmup_bars);
    }
    Err(ReplayError::NoWindow(cfg.max_draws))
}

/// Una vela sin su fecha real (`replay_session.v1#/$defs/bar`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayBar {
    pub index: usize,
    pub day: u32,
    pub time: String,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkKind {
    Level,
    PivotHigh,
    PivotLow,
}

/// `contracts/replay_mark.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    pub mark_id: u32,
    pub kind: MarkKind,
    pub price: Decimal,
    #[serde(deserialize_with = "Option::deserialize")]
    pub bar_index: Option<usize>,
    pub cursor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkRequest {
    pub kind: MarkKind,
    pub price: Decimal,
    pub bar_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Long,
    Short,
}

/// `contracts/replay_order.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayOrder {
    pub order_id: u32,
    pub side: OrderSide,
    #[serde(deserialize_with = "Option::deserialize")]
    pub stop: Option<Decimal>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub target: Option<Decimal>,
    pub risk_pct: Decimal,
    pub plan: String,
    pub cursor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderRequest {
    pub side: OrderSide,
    pub stop: Option<Decimal>,
    pub target: Option<Decimal>,
    pub risk_pct: Decimal,
    pub plan: String,
}

/// `contracts/replay_step.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub session_id: String,
    pub cursor: usize,
    pub bars: Vec<ReplayBar>,
    pub done: bool,
}

/// Una sesión abierta. Las velas y la fecha real son privadas: solo salen hasta el cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaySession {
    pub id: String,
    pub kind: ReplayKind,
    pub lesson_id: Option<String>,
    pub seed: u64,
    pub draw: Draw,
    pub warmup_bars: usize,
    bars: Vec<Candle>,
    cursor: usize,
    pub marks: Vec<Mark>,
    pub orders: Vec<ReplayOrder>,
    finished: bool,
}

impl ReplaySession {
    /// `bars` son el calentamiento más el examen, en orden; el cursor arranca en la última vela
    /// del calentamiento.
    pub fn new(
        id: String,
        kind: ReplayKind,
        lesson_id: Option<String>,
        seed: u64,
        draw: Draw,
        bars: Vec<Candle>,
        warmup_bars: usize,
    ) -> Result<Self, ReplayError> {
        let ordered = bars.windows(2).all(|w| w[0].end() <= w[1].start);
        if warmup_bars == 0 || bars.len() <= warmup_bars || !ordered {
            return Err(ReplayError::Bad(
                "velas desordenadas o sin examen después del calentamiento".into(),
            ));
        }
        Ok(Self {
            id,
            kind,
            lesson_id,
            seed,
            draw,
            warmup_bars,
            bars,
            cursor: warmup_bars - 1,
            marks: Vec::new(),
            orders: Vec::new(),
            finished: false,
        })
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn last_index(&self) -> usize {
        self.bars.len() - 1
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Las velas `0..=index`. **Rechaza** cualquier vela posterior al cursor.
    pub fn bars_until(&self, index: usize) -> Result<&[Candle], ReplayError> {
        if index > self.cursor {
            return Err(ReplayError::AfterCursor {
                asked: index,
                cursor: self.cursor,
            });
        }
        Ok(&self.bars[..=index])
    }

    /// Todas las velas visibles (hasta el cursor).
    pub fn visible(&self) -> &[Candle] {
        &self.bars[..=self.cursor]
    }

    /// La vela `i` sin su fecha: día de la sesión (días UTC desde la primera) y hora UTC.
    fn hidden(&self, i: usize) -> ReplayBar {
        let c = &self.bars[i];
        let day0 = self.bars[0].start.date_naive();
        let day = (c.start.date_naive() - day0).num_days() + 1;
        ReplayBar {
            index: i,
            day: u32::try_from(day).unwrap_or(u32::MAX),
            time: c.start.format("%H:%M").to_string(),
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
        }
    }

    /// Las velas visibles sin fecha (`replay_session.v1` las trae al abrir).
    pub fn visible_hidden(&self) -> Vec<ReplayBar> {
        (0..=self.cursor).map(|i| self.hidden(i)).collect()
    }

    /// Avanza `n` velas (como mucho `max`, y nunca más allá de la última) y devuelve solo las nuevas.
    pub fn step(&mut self, n: usize, max: usize) -> Result<Step, ReplayError> {
        if self.finished {
            return Err(ReplayError::Finished);
        }
        if n == 0 {
            return Err(ReplayError::Bad("hay que avanzar al menos una vela".into()));
        }
        let to = (self.cursor + n.min(max)).min(self.last_index());
        let bars = (self.cursor + 1..=to).map(|i| self.hidden(i)).collect();
        self.cursor = to;
        Ok(Step {
            session_id: self.id.clone(),
            cursor: to,
            bars,
            done: to == self.last_index(),
        })
    }

    /// Marca un nivel o un pivote con el cursor de ahora. Un pivote va en una vela ya visible.
    pub fn mark(&mut self, req: MarkRequest) -> Result<Mark, ReplayError> {
        if self.finished {
            return Err(ReplayError::Finished);
        }
        let bar_ok = match (req.kind, req.bar_index) {
            (MarkKind::Level, None) => true,
            (MarkKind::PivotHigh | MarkKind::PivotLow, Some(i)) => self.bars_until(i).is_ok(),
            _ => false,
        };
        if !bar_ok || req.price <= Decimal::ZERO {
            return Err(ReplayError::Bad(
                "un nivel sin vela; un pivote en una vela visible; precio > 0".into(),
            ));
        }
        let mark = Mark {
            mark_id: u32::try_from(self.marks.len() + 1).unwrap_or(u32::MAX),
            kind: req.kind,
            price: req.price,
            bar_index: req.bar_index,
            cursor: self.cursor,
        };
        self.marks.push(mark.clone());
        Ok(mark)
    }

    /// Una entrada decidida al cierre de la vela del cursor; se ejecuta en la apertura siguiente,
    /// así que no se puede decidir en la última vela.
    pub fn order(&mut self, req: OrderRequest) -> Result<ReplayOrder, ReplayError> {
        if self.finished {
            return Err(ReplayError::Finished);
        }
        let prices_ok = req.stop.is_none_or(|p| p > Decimal::ZERO)
            && req.target.is_none_or(|p| p > Decimal::ZERO);
        if self.cursor >= self.last_index()
            || !prices_ok
            || req.risk_pct <= Decimal::ZERO
            || req.plan.chars().count() > 2000
        {
            return Err(ReplayError::Bad(
                "sin vela siguiente para entrar, precios > 0, riesgo > 0 o plan de hasta 2000"
                    .into(),
            ));
        }
        let order = ReplayOrder {
            order_id: u32::try_from(self.orders.len() + 1).unwrap_or(u32::MAX),
            side: req.side,
            stop: req.stop,
            target: req.target,
            risk_pct: req.risk_pct,
            plan: req.plan,
            cursor: self.cursor,
        };
        self.orders.push(order.clone());
        Ok(order)
    }

    /// Termina la sesión: desde acá no entran marcas ni entradas, y se puede calificar con todas
    /// las velas y revelar la fecha.
    pub fn finish(&mut self) -> Result<(), ReplayError> {
        if self.finished {
            return Err(ReplayError::Finished);
        }
        self.finished = true;
        Ok(())
    }

    /// Todas las velas, con su fecha: **solo** con la sesión terminada.
    pub fn all_bars(&self) -> Result<&[Candle], ReplayError> {
        if !self.finished {
            return Err(ReplayError::AfterCursor {
                asked: self.last_index(),
                cursor: self.cursor,
            });
        }
        Ok(&self.bars)
    }

    /// `replay_session.v1`: la sesión con las velas visibles, sin la fecha real.
    pub fn view(&self, cfg: &ReplayConfig, rules: ProcessRules) -> SessionView {
        SessionView {
            session_id: self.id.clone(),
            kind: self.kind,
            lesson_id: self.lesson_id.clone(),
            seed: self.seed.to_string(),
            instrument_id: self.draw.instrument_id.clone(),
            timeframe: self.draw.timeframe,
            source: cfg.source.clone(),
            warmup_bars: self.warmup_bars,
            exam_bars: self.bars.len() - self.warmup_bars,
            cursor: self.cursor,
            bars: self.visible_hidden(),
            rules: SessionRules {
                trades_per_exam: cfg.trades_per_exam,
                pivots_per_exam: cfg.pivots_per_exam,
                level_tolerance_atr: cfg.level_tolerance_atr,
                stop_min_atr: rules.stop_min_atr,
                stop_max_atr: rules.stop_max_atr,
                max_risk_pct: rules.max_risk_pct,
                provisional: rules.provisional,
            },
        }
    }
}

/// Los límites de proceso que vienen de `config/risk.toml` (los mismos que el ticket), más si los
/// umbrales de la Academia siguen provisorios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessRules {
    pub stop_min_atr: Decimal,
    pub stop_max_atr: Decimal,
    pub max_risk_pct: Decimal,
    pub provisional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRules {
    pub trades_per_exam: usize,
    /// Opcional en el contrato (llegó con #800).
    #[serde(default)]
    pub pivots_per_exam: usize,
    pub level_tolerance_atr: Decimal,
    pub stop_min_atr: Decimal,
    pub stop_max_atr: Decimal,
    pub max_risk_pct: Decimal,
    pub provisional: bool,
}

/// `contracts/replay_session.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionView {
    pub session_id: String,
    pub kind: ReplayKind,
    #[serde(deserialize_with = "Option::deserialize")]
    pub lesson_id: Option<String>,
    pub seed: String,
    pub instrument_id: InstrumentId,
    pub timeframe: Timeframe,
    pub source: String,
    pub warmup_bars: usize,
    pub exam_bars: usize,
    pub cursor: usize,
    pub bars: Vec<ReplayBar>,
    pub rules: SessionRules,
}

/// `replay_session.v1#/$defs/request`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRequest {
    pub kind: ReplayKind,
    pub lesson_id: Option<String>,
    pub seed: Option<String>,
}

impl SessionRequest {
    /// La semilla pedida (u64 en decimal), o `None` si no vino.
    pub fn seed(&self) -> Result<Option<u64>, ReplayError> {
        let ok = |s: &str| s == "0" || (!s.starts_with('0') && s.len() <= 20);
        match &self.seed {
            None => Ok(None),
            Some(s) if ok(s) => s
                .parse()
                .map(Some)
                .map_err(|_| ReplayError::Bad("seed: u64 en decimal".into())),
            Some(_) => Err(ReplayError::Bad("seed: u64 en decimal".into())),
        }
    }
}

/// `replay_step.v1#/$defs/request`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepRequest {
    pub bars: usize,
}

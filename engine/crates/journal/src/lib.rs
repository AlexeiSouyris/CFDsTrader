//! Diario de propuestas y resultados (spec §15b, diseño en #13).
//!
//! Los tipos de este crate serializan exactamente `contracts/proposal.v1` y
//! `contracts/outcome.v1`, y cada corrida de backtest, `contracts/backtest_run.v1` (#200): el
//! diario lo escribe el motor y lo lee el aprendizaje en Python.
//! Montos en la moneda del instrumento. `pg` los guarda en PostgreSQL.

pub mod curve;
pub mod learning;
pub mod ml_features;
pub mod pg;
pub mod probability;
pub mod run;
pub mod votes;
pub mod walk_forward;

pub use curve::{CurveInvalid, CurvePoint, RunBuyAndHold, RunCurve, RunDetail, RunTrade};
pub use run::{
    BacktestRun, RunCommission, RunCosts, RunInstrumentCosts, RunInvalid, RunKillSwitchEvent,
    RunKillSwitchKind, RunMetrics, RunNews, RunTimeframe, SpreadSource, config_sha256,
};
pub use walk_forward::{
    CandleSourceName, ReportHeader, WalkForwardReport, WfAggregate, WfBuyAndHold, WfFold,
    WfMetrics, WfPeriod, WfSensitivity, WfSplit, WfStability, WfValue,
};

use cfds_backtest::{CloseReason, ExitReason, TradeRecord};
use cfds_core::InstrumentId;
use cfds_core::candle::utc_z;
use cfds_core::decimal_str;
use cfds_strategy::{Component, Side, TriggerKind};
use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Backtest,
    Paper,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Long,
    Short,
}

impl From<Side> for Direction {
    fn from(s: Side) -> Self {
        match s {
            Side::Long => Direction::Long,
            Side::Short => Direction::Short,
        }
    }
}

/// Nombre de una señal en el contrato (spec §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    Trend,
    SmaSide,
    CandlePattern,
    DoubleBreak,
    LevelRejection,
    Sweep,
    FvgOrOrderBlock,
    RecentChoch,
    News,
}

fn trigger_signal(k: TriggerKind) -> Signal {
    match k {
        TriggerKind::CandlePattern => Signal::CandlePattern,
        TriggerKind::DoubleBreak => Signal::DoubleBreak,
        TriggerKind::LevelRejection => Signal::LevelRejection,
        TriggerKind::Sweep => Signal::Sweep,
    }
}

impl From<Component> for Signal {
    fn from(c: Component) -> Self {
        match c {
            Component::Trend => Signal::Trend,
            Component::SmaSide => Signal::SmaSide,
            Component::Trigger(k) => trigger_signal(k),
            Component::FvgOrOrderBlock => Signal::FvgOrOrderBlock,
            Component::RecentChoch => Signal::RecentChoch,
            Component::News => Signal::News,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BreakdownItem {
    pub component: Signal,
    pub points: i32,
}

/// `contracts/proposal.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub id: String,
    pub source: Source,
    pub run_id: String,
    pub instrument_id: InstrumentId,
    pub side: Direction,
    #[serde(with = "utc_z")]
    pub signal_at: DateTime<Utc>,
    #[serde(with = "decimal_str")]
    pub reference: Decimal,
    #[serde(with = "decimal_str")]
    pub stop: Decimal,
    #[serde(with = "decimal_str")]
    pub tp1: Decimal,
    #[serde(with = "decimal_str")]
    pub size: Decimal,
    #[serde(with = "decimal_str")]
    pub leverage: Decimal,
    pub score: i32,
    pub breakdown: Vec<BreakdownItem>,
    pub triggers: Vec<Signal>,
    /// La probabilidad mostrada (§13) tal como se mostró al proponer (opcional, #333). Las
    /// propuestas del backtest no la traen: se recalcula con la tabla de entrenamiento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shown: Option<ShownProbability>,
}

/// La probabilidad mostrada (spec §13) de una propuesta, como se mostró.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShownProbability {
    /// Frecuencia de TP1 antes que el stop en su grupo, con su intervalo, de 0 a 1.
    Estimate {
        #[serde(with = "decimal_str")]
        probability: Decimal,
        #[serde(with = "decimal_str")]
        low: Decimal,
        #[serde(with = "decimal_str")]
        high: Decimal,
        hits: u32,
        cases: u32,
    },
    /// "Datos insuficientes": menos de `needed` casos en el grupo.
    Insufficient { cases: u32, needed: u32 },
}

impl From<probability::Shown> for ShownProbability {
    fn from(s: probability::Shown) -> Self {
        match s {
            probability::Shown::Estimate {
                probability,
                low,
                high,
                hits,
                cases,
            } => Self::Estimate {
                probability,
                low,
                high,
                hits,
                cases,
            },
            probability::Shown::Insufficient { cases, needed } => {
                Self::Insufficient { cases, needed }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    Simulated,
    Real,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Stop,
    Tp1,
    BreakEven,
    Trailing,
    Time,
    Forced,
    EndOfData,
}

impl From<CloseReason> for Reason {
    fn from(r: CloseReason) -> Self {
        match r {
            CloseReason::Exit(ExitReason::Stop) => Reason::Stop,
            CloseReason::Exit(ExitReason::Tp1) => Reason::Tp1,
            CloseReason::Exit(ExitReason::BreakEven) => Reason::BreakEven,
            CloseReason::Exit(ExitReason::Trailing) => Reason::Trailing,
            CloseReason::Exit(ExitReason::Time) => Reason::Time,
            CloseReason::Exit(ExitReason::Forced) => Reason::Forced,
            CloseReason::EndOfData => Reason::EndOfData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    #[serde(with = "utc_z")]
    pub at: DateTime<Utc>,
    #[serde(with = "decimal_str")]
    pub price: Decimal,
    #[serde(with = "decimal_str")]
    pub fraction: Decimal,
    pub reason: Reason,
}

/// `contracts/outcome.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub proposal_id: String,
    pub kind: OutcomeKind,
    #[serde(with = "utc_z")]
    pub entry_at: DateTime<Utc>,
    #[serde(with = "decimal_str")]
    pub entry_price: Decimal,
    pub exits: Vec<Exit>,
    #[serde(with = "decimal_str")]
    pub commission: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub funding: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub gross_pnl: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub net_pnl: Decimal,
    pub hit_tp1: bool,
    pub hit_stop: bool,
    pub exit_reason: Reason,
}

/// Id idempotente de una propuesta: la misma corrida, instrumento, vela y dirección dan
/// siempre el mismo id (spec §5b: órdenes idempotentes con id propio).
pub fn proposal_id(
    run_id: &str,
    instrument: &InstrumentId,
    signal_at: DateTime<Utc>,
    side: Side,
) -> String {
    let side = match side {
        Side::Long => "long",
        Side::Short => "short",
    };
    format!(
        "{run_id}:{}:{}:{side}",
        instrument.as_str(),
        signal_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    )
}

/// Propuesta y resultado simulado de una operación del backtest.
pub fn from_backtest(
    run_id: &str,
    instrument: &InstrumentId,
    r: &TradeRecord,
) -> (Proposal, Outcome) {
    let id = proposal_id(run_id, instrument, r.signal_at, r.side);
    let proposal = Proposal {
        id: id.clone(),
        source: Source::Backtest,
        run_id: run_id.to_string(),
        instrument_id: instrument.clone(),
        side: r.side.into(),
        signal_at: r.signal_at,
        reference: r.reference,
        stop: r.stop,
        tp1: r.tp1,
        size: r.size,
        leverage: r.leverage,
        score: r.score,
        breakdown: r
            .breakdown
            .iter()
            .map(|&(c, points)| BreakdownItem {
                component: c.into(),
                points,
            })
            .collect(),
        triggers: r.triggers.iter().map(|&k| trigger_signal(k)).collect(),
        shown: None,
    };
    let exits: Vec<Exit> = r
        .exits
        .iter()
        .map(|e| Exit {
            at: e.time,
            price: e.price,
            fraction: e.fraction,
            reason: e.reason.into(),
        })
        .collect();
    let outcome = Outcome {
        proposal_id: id,
        kind: OutcomeKind::Simulated,
        entry_at: r.entry_time,
        entry_price: r.entry_price,
        hit_tp1: exits.iter().any(|e| e.reason == Reason::Tp1),
        hit_stop: exits.iter().any(|e| e.reason == Reason::Stop),
        exit_reason: exits.last().map_or(Reason::EndOfData, |e| e.reason),
        exits,
        commission: r.commission,
        funding: r.funding,
        gross_pnl: r.gross_pnl,
        net_pnl: r.net_pnl,
    };
    (proposal, outcome)
}

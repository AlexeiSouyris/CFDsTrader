//! Etiqueta de una propuesta para el modelo de ML v1, **sin P/L** (`contracts/ml_label.v1`,
//! docs/spec/ml-v1.md §1, #399, opción C del dueño). La arma el motor con
//! `cfds_backtest::proposals` (mismas decisiones, riesgo y salidas que el backtest, a precio
//! medio y sin costos) y la lee Python (`cfds_intel.models`), que no recalcula nada.
//!
//! También arma la propuesta (`proposal.v1`) y sus variables (`ml_features.v1`) desde una
//! `cfds_backtest::Proposal`, para guardar las tres juntas bajo un `run_id`.

use cfds_backtest::{ExitReason, Label, Proposal as Counted};
use cfds_core::candle::utc_z;
use cfds_core::{DailyVolTercile, InstrumentId};
use cfds_market_data::Session;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::ml_features::{self, MlFeatures};
use crate::{BreakdownItem, Proposal, Source, proposal_id, trigger_signal};

/// Qué decidió la etiqueta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelReason {
    Tp1,
    Stop,
    Time,
    Forced,
}

/// Cómo se midió (`method` del contrato).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelMethod {
    /// Entrada en la apertura de `t+1` a precio medio; sin spread, slippage, comisión ni
    /// financiación.
    MidNoCosts,
}

/// `contracts/ml_label.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlLabel {
    pub proposal_id: String,
    #[serde(with = "utc_z")]
    pub signal_at: DateTime<Utc>,
    /// 1 si tocó TP1 antes que el stop, si no 0.
    pub y: u8,
    pub exit_reason: LabelReason,
    #[serde(with = "utc_z")]
    pub resolved_at: DateTime<Utc>,
    pub method: LabelMethod,
}

/// De dónde salen las velas (`candle_source` del contrato).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelCandles {
    Futures,
    Ig,
}

/// `contracts/ml_label_run.v1`: de qué datos sale una corrida de etiquetas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MlLabelRun {
    pub run_id: String,
    pub instrument_id: InstrumentId,
    pub candle_source: LabelCandles,
    /// Inicio de la primera vela de 15m.
    #[serde(with = "utc_z")]
    pub from: DateTime<Utc>,
    /// Fin de la última vela de 15m.
    #[serde(with = "utc_z")]
    pub to: DateTime<Utc>,
    pub method: LabelMethod,
    pub config_sha256: String,
}

/// La etiqueta de una propuesta contada, con el id de `run_id`. `None` si no se resolvió
/// (los datos terminan antes) o si la salida no es una de las que puede dar la etiqueta.
pub fn label(run_id: &str, instrument: &InstrumentId, p: &Counted) -> Option<MlLabel> {
    let (y, exit_reason) = match p.label {
        Label::Tp1 => (1, LabelReason::Tp1),
        Label::Miss(ExitReason::Stop) => (0, LabelReason::Stop),
        Label::Miss(ExitReason::Time) => (0, LabelReason::Time),
        Label::Miss(ExitReason::Forced) => (0, LabelReason::Forced),
        // Sin TP1 no hay break-even ni trailing: no puede pasar.
        Label::Miss(ExitReason::Tp1 | ExitReason::BreakEven | ExitReason::Trailing)
        | Label::Unresolved => return None,
    };
    Some(MlLabel {
        proposal_id: proposal_id(run_id, instrument, p.signal_at, p.side),
        signal_at: p.signal_at,
        y,
        exit_reason,
        resolved_at: p.resolved_at,
        method: LabelMethod::MidNoCosts,
    })
}

/// La propuesta (`proposal.v1`, `source = backtest`) de una propuesta contada.
pub fn proposal(run_id: &str, instrument: &InstrumentId, p: &Counted) -> Proposal {
    Proposal {
        id: proposal_id(run_id, instrument, p.signal_at, p.side),
        source: Source::Backtest,
        run_id: run_id.to_string(),
        instrument_id: instrument.clone(),
        side: p.side.into(),
        signal_at: p.signal_at,
        reference: p.reference,
        stop: p.stop,
        tp1: p.tp1,
        size: p.size,
        leverage: p.leverage,
        score: p.score,
        breakdown: p
            .breakdown
            .iter()
            .map(|&(c, points)| BreakdownItem {
                component: c.into(),
                points,
            })
            .collect(),
        triggers: p.triggers.iter().map(|&k| trigger_signal(k)).collect(),
        shown: None,
    }
}

/// Las variables de ML de una propuesta contada (las mismas que `ml_features::from_backtest`).
/// `atr`: `ml_features::atr14_m15` de las mismas velas de 15m. `None` si falta alguna.
pub fn features(
    run_id: &str,
    instrument: &InstrumentId,
    p: &Counted,
    atr: &[Option<Decimal>],
    daily: &[DailyVolTercile],
    session: &Session,
) -> Option<MlFeatures> {
    ml_features::of_signal(
        proposal_id(run_id, instrument, p.signal_at, p.side),
        p.index,
        p.signal_at,
        atr,
        daily,
        session,
    )
}

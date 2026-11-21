//! Exportación para la verificación visual y la comparación con una referencia (ROADMAP 3b, #81).
//! Contrato: `contracts/chart_export.v1.schema.json`.
//!
//! Todo valor sale del motor tal como lo usa la estrategia (`cfds_strategy::Analysis`) y del
//! registro del backtest. Las herramientas que leen esto **no recalculan nada**.

use cfds_core::{Candle, InstrumentId, decimal_str};
use cfds_indicators::Trend;
use cfds_patterns::PivotKind;
use cfds_strategy::{Analysis, Component, ScoreError, Side, TriggerKind, score};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{CloseReason, ExitReason, Report, Setup};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartExport {
    pub instrument_id: InstrumentId,
    /// Siempre `"15m"`: el timeframe de la decisión (spec §2).
    pub timeframe: String,
    pub candles: Vec<ExportCandle>,
    pub pivots: Vec<ExportPivot>,
    pub trades: Vec<ExportTrade>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportCandle {
    #[serde(with = "cfds_core::candle::utc_z")]
    pub start: DateTime<Utc>,
    #[serde(with = "decimal_str")]
    pub open: Decimal,
    #[serde(with = "decimal_str")]
    pub high: Decimal,
    #[serde(with = "decimal_str")]
    pub low: Decimal,
    #[serde(with = "decimal_str")]
    pub close: Decimal,
    #[serde(with = "decimal_str::option")]
    pub sma: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub atr: Option<Decimal>,
    pub trend_1h: Option<String>,
    pub long_score: Option<i32>,
    pub short_score: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportPivot {
    pub kind: String,
    pub index: usize,
    pub confirmed_index: usize,
    #[serde(with = "decimal_str")]
    pub price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportTrade {
    pub side: String,
    pub signal_index: usize,
    pub entry_index: usize,
    #[serde(with = "decimal_str")]
    pub entry_price: Decimal,
    #[serde(with = "decimal_str")]
    pub stop: Decimal,
    #[serde(with = "decimal_str")]
    pub tp1: Decimal,
    #[serde(with = "decimal_str")]
    pub size: Decimal,
    pub score: i32,
    pub breakdown: Vec<ExportPoints>,
    pub exits: Vec<ExportExit>,
    #[serde(with = "decimal_str::signed")]
    pub net_pnl: Decimal,
    pub at_signal: AtSignal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportPoints {
    pub component: String,
    pub points: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportExit {
    pub index: usize,
    #[serde(with = "decimal_str")]
    pub price: Decimal,
    #[serde(with = "decimal_str")]
    pub fraction: Decimal,
    pub reason: String,
}

/// Valores de los indicadores en la vela de la señal (tabla por operación).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtSignal {
    #[serde(with = "decimal_str")]
    pub close: Decimal,
    #[serde(with = "decimal_str::option")]
    pub sma: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub atr: Option<Decimal>,
    pub trend_1h: Option<String>,
}

fn trend_name(t: Trend) -> String {
    match t {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Neutral => "neutral",
    }
    .into()
}

fn side_name(s: Side) -> String {
    match s {
        Side::Long => "long",
        Side::Short => "short",
    }
    .into()
}

/// Mismos nombres que `proposal.v1`.
fn component_name(c: Component) -> String {
    match c {
        Component::Trend => "trend",
        Component::SmaSide => "sma_side",
        Component::Trigger(TriggerKind::CandlePattern) => "candle_pattern",
        Component::Trigger(TriggerKind::DoubleBreak) => "double_break",
        Component::Trigger(TriggerKind::LevelRejection) => "level_rejection",
        Component::Trigger(TriggerKind::Sweep) => "sweep",
        Component::FvgOrOrderBlock => "fvg_or_order_block",
        Component::RecentChoch => "recent_choch",
        Component::News => "news",
    }
    .into()
}

/// Mismos nombres que `outcome.v1`.
fn reason_name(r: CloseReason) -> String {
    match r {
        CloseReason::Exit(ExitReason::Stop) => "stop",
        CloseReason::Exit(ExitReason::Tp1) => "tp1",
        CloseReason::Exit(ExitReason::BreakEven) => "break_even",
        CloseReason::Exit(ExitReason::Trailing) => "trailing",
        CloseReason::Exit(ExitReason::Time) => "time",
        CloseReason::Exit(ExitReason::Forced) => "forced",
        CloseReason::EndOfData => "end_of_data",
    }
    .into()
}

/// Exporta las velas de 15m con sus indicadores y puntajes (sin noticias), los pivotes
/// confirmados y las operaciones de `report`, que tiene que venir de `run_checked` con las
/// mismas velas y los mismos errores de datos.
pub fn chart_export(
    m15: &[Candle],
    h1: &[Candle],
    data_errors: &[DateTime<Utc>],
    setup: &Setup,
    report: &Report,
) -> Result<ChartExport, ScoreError> {
    let a = Analysis::new(m15, h1, setup.strategy).with_data_errors(data_errors.to_vec());
    let score_at = |t: usize, side: Side| -> Result<Option<i32>, ScoreError> {
        Ok(match a.evidence_at(t, side, 0) {
            Some(e) => score(&e, &setup.strategy.strategy.score)?.map(|s| s.total),
            None => None,
        })
    };
    let mut candles = Vec::with_capacity(m15.len());
    for (t, c) in m15.iter().enumerate() {
        candles.push(ExportCandle {
            start: c.start,
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
            sma: a.sma()[t],
            atr: a.atr()[t],
            trend_1h: a.trend_1h()[t].map(trend_name),
            long_score: score_at(t, Side::Long)?,
            short_score: score_at(t, Side::Short)?,
        });
    }
    let pivots = a
        .pivots()
        .iter()
        .map(|p| ExportPivot {
            kind: match p.kind {
                PivotKind::High => "high",
                PivotKind::Low => "low",
            }
            .into(),
            index: p.index,
            confirmed_index: p.confirmed_at,
            price: p.price,
        })
        .collect();
    let trades = report
        .trades
        .iter()
        .map(|t| ExportTrade {
            side: side_name(t.side),
            signal_index: t.signal_index,
            entry_index: t.entry_index,
            entry_price: t.entry_price,
            stop: t.stop,
            tp1: t.tp1,
            size: t.size,
            score: t.score,
            breakdown: t
                .breakdown
                .iter()
                .map(|(c, p)| ExportPoints {
                    component: component_name(*c),
                    points: *p,
                })
                .collect(),
            exits: t
                .exits
                .iter()
                .map(|e| ExportExit {
                    index: e.index,
                    price: e.price,
                    fraction: e.fraction,
                    reason: reason_name(e.reason),
                })
                .collect(),
            net_pnl: t.net_pnl,
            at_signal: AtSignal {
                close: m15[t.signal_index].close,
                sma: a.sma()[t.signal_index],
                atr: a.atr()[t.signal_index],
                trend_1h: a.trend_1h()[t.signal_index].map(trend_name),
            },
        })
        .collect();
    Ok(ChartExport {
        instrument_id: setup.instrument.clone(),
        timeframe: "15m".into(),
        candles,
        pivots,
        trades,
    })
}

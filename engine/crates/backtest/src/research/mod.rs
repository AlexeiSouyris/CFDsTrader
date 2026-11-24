//! Estrategias de **estudio** de la ronda 1 de investigación (#472): ruptura del rango de
//! apertura (familia C) y cambio de mes (familia D1), con las definiciones de
//! `docs/analysis/2026-11-propuesta-investigacion.md` (#466) y su pre-registro
//! (`docs/analysis/2026-11-ronda1-preregistro.md`).
//!
//! Están aparte a propósito: ni el runner, ni el paper, ni la estrategia en uso (la confluencia
//! de `cfds_strategy`) las usan. Solo las corre el ejemplo `ronda1`.
//!
//! ```text
//! señales (funciones puras, solo velas cerradas):   orb_plans / turn_of_month_plans → Plan
//! ejecución (una cuenta por instrumento):            run_plans → riesgo → entrada en t+1
//!                                                     → stop / salida planificada / cierre forzado
//!                                                     → spread, slippage, comisión, financiación
//!                                                       menos la base del futuro (#470)
//! retornos mensuales y regla de decisión:            monthly_returns, stats
//! ```

pub mod config;
pub mod orb;
pub mod run;
pub mod stats;
pub mod turn_of_month;

use cfds_core::Candle;
use cfds_strategy::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

pub use config::{OrbConfig, ResearchConfig, RuleConfig, TurnOfMonthConfig};
pub use orb::orb_plans;
pub use run::{
    StudyExit, StudyRejected, StudyRun, StudyTrade, execute_plan, monthly_returns, run_plans,
    simulate_leg, trade_return,
};
pub use turn_of_month::{SessionBar, session_bars, turn_of_month_plans};

/// Una operación que una familia quiere abrir, decidida al **cierre** de `signal_index`. Se
/// ejecuta en la apertura de `signal_index + 1` y, si no salta antes el stop o un cierre forzado,
/// sale al cierre de `exit_index` (una hora conocida de antemano por el calendario).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Evento al que pertenece (un día en C, un mes en D1). Si salta el stop, las patas que
    /// quedan del mismo evento no se abren.
    pub event: usize,
    pub signal_index: usize,
    pub side: Side,
    /// Cierre de la vela de la señal: la entrada de referencia para el riesgo.
    pub reference: Decimal,
    pub stop: Decimal,
    /// ATR con el que el riesgo mide la distancia al stop.
    pub risk_atr: Decimal,
    pub exit_index: usize,
}

/// Índice de la vela que empieza exactamente en `start`, si existe.
pub(crate) fn index_at(candles: &[Candle], start: DateTime<Utc>) -> Option<usize> {
    let i = candles.partition_point(|c| c.start < start);
    (candles.get(i)?.start == start).then_some(i)
}

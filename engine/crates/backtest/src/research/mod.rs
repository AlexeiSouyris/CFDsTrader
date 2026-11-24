//! Estrategias de **estudio** de las rondas de investigación, con las definiciones de
//! `docs/analysis/2026-11-propuesta-investigacion.md` (#466) y sus pre-registros:
//! - ronda 1 (#472, `docs/analysis/2026-11-ronda1-preregistro.md`): ruptura del rango de
//!   apertura (familia C) y cambio de mes (familia D1);
//! - ronda 2 (#486, `docs/analysis/2026-11-ronda2-preregistro.md`): tendencia mensual (familia A)
//!   y momentum intradía (familia B), con comprar y mantener como vara del criterio 5 de A.
//!
//! Están aparte a propósito: ni el runner, ni el paper, ni la estrategia en uso (la confluencia
//! de `cfds_strategy`) las usan. Solo las corren los ejemplos `ronda1` y `ronda2`.
//!
//! ```text
//! señales (funciones puras, solo velas cerradas):   orb_plans / turn_of_month_plans /
//!                                                     trend_plans / intraday_momentum_plans → Plan
//! ejecución (una cuenta por instrumento):            run_plans → riesgo → entrada en t+1
//!                                                     → stop / salida planificada / cierre forzado
//!                                                     → spread, slippage, comisión, financiación
//!                                                       menos la base del futuro (#470)
//! retornos mensuales y regla de decisión:            monthly_returns, stats
//! ```

pub mod buy_and_hold;
pub mod config;
pub mod intraday_momentum;
pub mod orb;
pub mod run;
pub mod stats;
pub mod trend;
pub mod turn_of_month;

use cfds_core::Candle;
use cfds_strategy::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

pub use buy_and_hold::buy_and_hold_returns;
pub use config::{
    IntradayMomentumConfig, OrbConfig, ResearchConfig, RuleConfig, TrendConfig, TurnOfMonthConfig,
};
pub use intraday_momentum::intraday_momentum_plans;
pub use orb::orb_plans;
pub use run::{
    StudyExit, StudyRejected, StudyRun, StudyTrade, execute_plan, monthly_returns, run_plans,
    simulate_leg, trade_return,
};
pub use trend::{monthly_reviews, trend_plans};
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

/// ¿Se puede decidir al cierre de `i`? La marca de entrada de la preparación está en verdadero y
/// la vela siguiente empieza justo al cierre de `i` (sin hueco de datos).
pub(crate) fn decision_allowed(candles: &[Candle], entries: &[bool], i: usize) -> bool {
    entries.get(i).copied().unwrap_or(false)
        && candles
            .get(i + 1)
            .is_some_and(|n| n.start == candles[i].end())
}

/// Patas de una posición que se mantiene desde `from` hasta el cierre de `exit_index`: cada pata
/// se decide en la primera decisión permitida y termina en el primer cierre forzado (fin de
/// semana o vencimiento) o en `exit_index`; después se reabre en la primera decisión permitida
/// siguiente. Devuelve `(decisión, salida)` de cada pata.
pub(crate) fn legs(
    candles: &[Candle],
    entries: &[bool],
    force_close: &[bool],
    mut from: usize,
    exit_index: usize,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    while let Some(t) = (from..exit_index).find(|&i| decision_allowed(candles, entries, i)) {
        let end = (t + 1..exit_index)
            .find(|&f| force_close.get(f).copied().unwrap_or(false))
            .unwrap_or(exit_index);
        out.push((t, end));
        if end == exit_index {
            break;
        }
        from = end + 1;
    }
    out
}

/// Retorno del cierre de `i0` al cierre de `i1` **dentro de cada contrato**, encadenado en los
/// vencimientos `rolls` (inicio de la primera vela de cada contrato nuevo): en cada cambio se
/// corta en el cierre de la última vela del contrato viejo y sigue desde la apertura de la
/// primera del nuevo, así que el salto de la base no entra. `None` si `i1 < i0`, si falta una
/// vela o si algún precio es cero.
pub fn chained_return(
    candles: &[Candle],
    rolls: &[DateTime<Utc>],
    i0: usize,
    i1: usize,
) -> Option<Decimal> {
    if i1 < i0 {
        return None;
    }
    let (a, b) = (candles.get(i0)?, candles.get(i1)?);
    let mut growth = Decimal::ONE;
    let mut base = a.close;
    for &r in rolls.iter().filter(|&&r| r > a.start && r <= b.start) {
        let next = candles.partition_point(|c| c.start < r);
        let (old, new) = (candles.get(next.checked_sub(1)?)?, candles.get(next)?);
        if base.is_zero() {
            return None;
        }
        growth *= old.close / base;
        base = new.open;
    }
    if base.is_zero() {
        return None;
    }
    Some(growth * b.close / base - Decimal::ONE)
}

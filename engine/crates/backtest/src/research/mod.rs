//! Estrategias de **estudio** de las rondas de investigación, con las definiciones de
//! `docs/analysis/2026-11-propuesta-investigacion.md` (#466) y sus pre-registros:
//! - ronda 1 (#472, `docs/analysis/2026-11-ronda1-preregistro.md`): ruptura del rango de
//!   apertura (familia C) y cambio de mes (familia D1);
//! - ronda 2 (#486, `docs/analysis/2026-11-ronda2-preregistro.md`): tendencia mensual (familia A)
//!   y momentum intradía (familia B), con comprar y mantener como vara del criterio 5 de A;
//! - ronda 3 (#493, `docs/analysis/2026-11-ronda3-preregistro.md`): exposición manejada por
//!   volatilidad (familia V), cascada al cruzar un número redondo (familia M1) y el descriptivo
//!   de la deriva nocturna;
//! - chequeos descriptivos de la revisión externa (#512, sin voto): `review_checks`;
//! - programa Nasdaq (#526, `docs/analysis/2026-11-programa-nasdaq-preregistro.md`): H1, deriva
//!   antes del comunicado del FOMC (`pre_fomc`), y H2, días de CPI y empleo (`macro_day`);
//! - programa 4 (#561, `docs/analysis/2026-11-programa-4-preregistro.md`): estrategia A de
//!   Bruni, literal (H4.1) y normalizada (H4.2), con objetivo en R (`program4`), y H4.3,
//!   estrategia C con perfil de volumen de la sesión del CME (`program4_c`);
//! - programa 5 (#649, `docs/analysis/2026-11-programa-5-fvg-preregistro.md`): reacción al
//!   primer toque de un FVG (`program5`, con los eventos que calcula `cfds_liquidity`).
//!
//! Están aparte a propósito: ni el runner, ni el paper, ni la estrategia en uso (la confluencia
//! de `cfds_strategy`) las usan. Solo las corren los ejemplos `ronda1`, `ronda2` y `ronda3`.
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
pub mod macro_day;
pub mod montecarlo;
pub mod orb;
pub mod overnight;
pub mod pbo;
pub mod pre_fomc;
pub mod program4;
pub mod program4_c;
pub mod program5;
pub mod review_checks;
pub mod round_number;
pub mod run;
pub mod stats;
pub mod trend;
pub mod turn_of_month;
pub mod vol_managed;

use cfds_core::Candle;
use cfds_strategy::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

pub use buy_and_hold::buy_and_hold_returns;
pub use config::{
    ChampionshipConfig, IntradayMomentumConfig, MacroDayConfig, OrbConfig, OvernightDriftConfig,
    Program4Config, Program5Config, ResearchConfig, ReviewChecksConfig, RoundNumberConfig,
    RuleConfig, TrendConfig, TurnOfMonthConfig, VolManagedConfig,
};
pub use intraday_momentum::intraday_momentum_plans;
pub use macro_day::{MacroDayRule, macro_day_windows};
pub use montecarlo::{MonteCarloConfig, MonteCarloReport, PathStats, Summary, monte_carlo};
pub use orb::orb_plans;
pub use overnight::window_returns;
pub use pbo::{Pbo, pbo_cscv};
pub use pre_fomc::{
    CalendarRow, EventOutcome, EventWindow, PreFomcConfig, ReleaseStatus, Skip, event_schedule,
    pre_fomc_windows,
};
pub use review_checks::{
    blend_return, drift_after_close, session_close_returns, strong_drop_flags, vol_match_scale,
};
pub use round_number::{Levels, round_number_plans};
pub use run::{
    StudyExit, StudyRejected, StudyRun, StudyTrade, execute_plan, monthly_returns, run_plans,
    simulate_leg, trade_return,
};
pub use trend::{monthly_reviews, trend_plans};
pub use turn_of_month::{SessionBar, session_bars, turn_of_month_plans};
pub use vol_managed::{
    managed_returns, managed_returns_with_spreads, review_variances, session_close_indices,
    vol_constant, vol_weights,
};

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
    /// Objetivo en múltiplos de R desde el precio medio de la entrada (programa 4): sale al
    /// tocarlo, y si la apertura de la entrada ya quedó del lado del stop, el plan no se opera.
    /// `None` = sin objetivo (las rondas 1 a 3).
    pub target_r: Option<Decimal>,
    /// Tope del objetivo (H4.3 del programa 4: el máximo de la estructura en un largo, el mínimo
    /// en un corto): el objetivo es el más cercano entre `target_r` y este precio. Si la apertura
    /// de la entrada ya llegó al tope, el plan no se opera. `None` = sin tope.
    pub target_cap: Option<Decimal>,
}

impl Plan {
    /// Objetivo para una entrada a `entry` (precio medio): `entry ± target_r × |entry − stop|`.
    pub fn target_from(&self, entry: Decimal) -> Option<Decimal> {
        let r = self.target_r? * (entry - self.stop).abs();
        Some(match (self.side, self.target_cap) {
            (Side::Long, None) => entry + r,
            (Side::Long, Some(cap)) => (entry + r).min(cap),
            (Side::Short, None) => entry - r,
            (Side::Short, Some(cap)) => (entry - r).max(cap),
        })
    }

    /// `true` si una entrada a `open` ya llegó al tope del objetivo (o lo pasó): no queda recorrido.
    pub fn open_beyond_cap(&self, open: Decimal) -> bool {
        match (self.side, self.target_cap) {
            (_, None) => false,
            (Side::Long, Some(cap)) => open >= cap,
            (Side::Short, Some(cap)) => open <= cap,
        }
    }

    /// `true` si una entrada a `open` ya está en el stop o más allá.
    pub fn open_beyond_stop(&self, open: Decimal) -> bool {
        match self.side {
            Side::Long => open <= self.stop,
            Side::Short => open >= self.stop,
        }
    }
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

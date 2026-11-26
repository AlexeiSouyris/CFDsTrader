//! Monte Carlo por bloques de una serie de retornos (#662, punto 5 de
//! `docs/analysis/2026-11-autoanalisis.md`, C4/D3): qué caída, qué racha perdedora y cuánto
//! tiempo bajo el agua son "normales" para esa serie, los cuantiles del retorno final y el
//! riesgo de ruina con tamaño de fracción fija.
//!
//! Cada camino se arma con un **bootstrap circular por bloques** (Politis y Romano, 1992):
//! bloques de `block_len` retornos consecutivos que empiezan en una posición sorteada con
//! SplitMix64 y dan la vuelta al final de la serie, hasta tener tantos retornos como la serie
//! original. Los bloques conservan la autocorrelación de corto plazo; `block_len = 1` es el
//! bootstrap simple. Misma serie, misma config y misma semilla: mismo resultado.
//!
//! El capital arranca en 1 y crece con `capital × (1 + fracción × r)`: con retornos mensuales
//! sobre el capital la fracción es 1; con una serie en R por operación es el riesgo por
//! operación (0,01 = 1 %). El capital nunca baja de 0.
//!
//! Usa `f64`: son estadísticos de una muestra de retornos, **no dinero** (como `stats`).
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de caminos simulados de retornos: no son dinero (CLAUDE.md §4)"
)]

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::program4::SplitMix64;

/// `[montecarlo]` de `config/research.toml`: valores de ingeniería de la simulación, no
/// parámetros de ninguna estrategia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonteCarloConfig {
    /// Caminos simulados.
    pub paths: usize,
    /// Largo de cada bloque, en períodos de la serie.
    pub block_len: usize,
    pub seed: u64,
    /// Pérdida desde el capital **inicial** que cuenta como ruina (0,30 = el capital tocó 0,70).
    pub ruin_threshold: Decimal,
}

impl MonteCarloConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.paths == 0 || self.block_len == 0 {
            return Err("montecarlo: paths y block_len tienen que ser mayores que 0".into());
        }
        if self.ruin_threshold <= Decimal::ZERO || self.ruin_threshold > Decimal::ONE {
            return Err("montecarlo.ruin_threshold tiene que estar en (0, 1]".into());
        }
        Ok(())
    }
}

/// Lo que mide un camino.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathStats {
    /// Caída máxima desde el pico, como fracción (0,25 = −25 %).
    pub max_drawdown: f64,
    /// Períodos seguidos con retorno < 0, la racha más larga.
    pub longest_losing_streak: usize,
    /// Períodos seguidos por debajo del pico anterior, el tramo más largo.
    pub longest_under_water: usize,
    /// Capital final − 1.
    pub final_return: f64,
    /// El capital tocó `1 − ruin_threshold` o menos en algún momento.
    pub ruined: bool,
}

/// Un paso del capital: `capital × (1 + fracción × r)`, nunca menos que 0. Lo usan también el
/// simulador de varianza y el de cuentas fondeadas: un solo cálculo.
pub fn grow(equity: f64, fraction: f64, r: f64) -> f64 {
    (equity * (1.0 + fraction * r)).max(0.0)
}

/// Recorre una serie con el capital en 1 y la `fraction` dada. `ruin_level` es el capital que
/// cuenta como ruina (`1 − ruin_threshold`).
pub fn path_stats(returns: &[f64], fraction: f64, ruin_level: f64) -> PathStats {
    let (mut equity, mut peak) = (1.0_f64, 1.0_f64);
    let mut out = PathStats {
        max_drawdown: 0.0,
        longest_losing_streak: 0,
        longest_under_water: 0,
        final_return: 0.0,
        ruined: false,
    };
    let (mut streak, mut under) = (0_usize, 0_usize);
    for &r in returns {
        equity = grow(equity, fraction, r);
        streak = if r < 0.0 { streak + 1 } else { 0 };
        out.longest_losing_streak = out.longest_losing_streak.max(streak);
        if equity >= peak {
            peak = equity;
            under = 0;
        } else {
            under += 1;
        }
        out.longest_under_water = out.longest_under_water.max(under);
        out.max_drawdown = out.max_drawdown.max(1.0 - equity / peak);
        out.ruined |= equity <= ruin_level;
    }
    out.final_return = equity - 1.0;
    out
}

/// Un camino del bootstrap circular por bloques, del mismo largo que `xs` (no vacío). Genérico:
/// el simulador de cuentas fondeadas remuestrea días enteros (sus índices).
pub fn block_bootstrap<T: Copy>(rng: &mut SplitMix64, xs: &[T], block_len: usize) -> Vec<T> {
    let n = xs.len();
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let start = rng.below(n);
        for k in 0..block_len.min(n - out.len()) {
            out.push(xs[(start + k) % n]);
        }
    }
    out
}

/// `n` valores del bootstrap circular por bloques de `xs` (no vacía), aunque `n` sea más que la
/// serie: se encadenan caminos de [`block_bootstrap`] y se corta en `n`.
pub fn bootstrap_n<T: Copy>(rng: &mut SplitMix64, xs: &[T], block_len: usize, n: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(n + xs.len());
    while out.len() < n {
        out.extend(block_bootstrap(rng, xs, block_len.max(1)));
    }
    out.truncate(n);
    out
}

/// Cuantil `q` (0 a 1) de una muestra **ordenada**, con interpolación lineal (tipo 7 de
/// Hyndman y Fan, el de R y NumPy por defecto).
pub fn quantile_sorted(sorted: &[f64], q: f64) -> Option<f64> {
    let last = sorted.len().checked_sub(1)?;
    let h = last as f64 * q.clamp(0.0, 1.0);
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(last);
    Some(sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo]))
}

/// Media y cuantiles 5, 50 y 95 % de una métrica sobre los caminos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    pub mean: f64,
    pub p5: f64,
    pub p50: f64,
    pub p95: f64,
}

pub fn summarize(mut xs: Vec<f64>) -> Option<Summary> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(f64::total_cmp);
    Some(Summary {
        mean: xs.iter().sum::<f64>() / xs.len() as f64,
        p5: quantile_sorted(&xs, 0.05)?,
        p50: quantile_sorted(&xs, 0.50)?,
        p95: quantile_sorted(&xs, 0.95)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonteCarloReport {
    /// La serie tal como ocurrió.
    pub observed: PathStats,
    pub paths: usize,
    pub max_drawdown: Summary,
    pub longest_losing_streak: Summary,
    pub longest_under_water: Summary,
    pub final_return: Summary,
    /// Fracción de caminos que tocaron la ruina.
    pub risk_of_ruin: f64,
}

/// Monte Carlo de `xs` con la config dada. `None` si la serie está vacía, tiene un valor no
/// finito, la `fraction` no es positiva o la config no valida.
pub fn monte_carlo(xs: &[f64], fraction: f64, cfg: &MonteCarloConfig) -> Option<MonteCarloReport> {
    if xs.is_empty()
        || xs.iter().any(|x| !x.is_finite())
        || !(fraction.is_finite() && fraction > 0.0)
        || cfg.validate().is_err()
    {
        return None;
    }
    let ruin_level = 1.0 - cfg.ruin_threshold.to_f64()?;
    let mut rng = SplitMix64::new(cfg.seed);
    let stats: Vec<PathStats> = (0..cfg.paths)
        .map(|_| {
            path_stats(
                &block_bootstrap(&mut rng, xs, cfg.block_len),
                fraction,
                ruin_level,
            )
        })
        .collect();
    let metric = |f: fn(&PathStats) -> f64| summarize(stats.iter().map(f).collect());
    Some(MonteCarloReport {
        observed: path_stats(xs, fraction, ruin_level),
        paths: cfg.paths,
        max_drawdown: metric(|s| s.max_drawdown)?,
        longest_losing_streak: metric(|s| s.longest_losing_streak as f64)?,
        longest_under_water: metric(|s| s.longest_under_water as f64)?,
        final_return: metric(|s| s.final_return)?,
        risk_of_ruin: stats.iter().filter(|s| s.ruined).count() as f64 / cfg.paths as f64,
    })
}

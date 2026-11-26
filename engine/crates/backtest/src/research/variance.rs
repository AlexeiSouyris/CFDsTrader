//! Simulador de varianza (#801, `docs/design/2026-11-laboratorio.md` §4.2): miles de caminos de
//! capital con la misma esperanza, para ver cuánto puede variar el resultado por pura suerte.
//!
//! Cada camino tiene `trades` operaciones en R, de una de dos fuentes:
//! - **Modelo**: cada operación gana `avg_win_r` con probabilidad `win_rate` o pierde
//!   `avg_loss_r`. Con `observed_trades = N` (incertidumbre del win rate) el win rate **de cada
//!   camino** se sortea de una Beta(p·N + 1, (1 − p)·N + 1) (prior uniforme): un 55 % medido en
//!   40 operaciones puede ser un 48 % de verdad.
//! - **R reales**: una lista de R (por ejemplo, la de un informe del laboratorio) remuestreada con
//!   el bootstrap circular por bloques de [`super::montecarlo`].
//!
//! El capital arranca en 1 y crece como en `montecarlo` (`capital × (1 + riesgo × R)`, riesgo
//! fijo sobre el capital del momento); la caída, la racha y la ruina salen de
//! [`super::montecarlo::path_stats`]. Mismas entradas y misma semilla (`SplitMix64`): mismo
//! resultado. Un solo cálculo de caminos: el simulador de cuentas fondeadas usa el mismo
//! generador de operaciones ([`draw_r`]).
//!
//! Usa `f64`: son estadísticos de caminos simulados, **no dinero** (como `montecarlo`).
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de caminos simulados de retornos: no son dinero (CLAUDE.md §4)"
)]

use super::montecarlo::{
    PathStats, Summary, block_bootstrap, grow, path_stats, quantile_sorted, summarize,
};
use super::program4::SplitMix64;

/// Pago de cada operación del modelo, en R.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Payoff {
    /// Probabilidad de ganar, en [0, 1].
    pub win_rate: f64,
    /// R de una ganadora (> 0).
    pub avg_win_r: f64,
    /// Pérdida de una perdedora, en R y en positivo (> 0; 1 = el stop).
    pub avg_loss_r: f64,
    /// Operaciones con las que se midió el win rate: si está, cada camino sortea su win rate de
    /// la Beta.
    pub observed_trades: Option<u32>,
}

impl Payoff {
    /// Esperanza por operación con el win rate dado: `p·W − (1 − p)·L`.
    pub fn expectancy(&self) -> f64 {
        self.win_rate * self.avg_win_r - (1.0 - self.win_rate) * self.avg_loss_r
    }

    /// El mismo pago sin incertidumbre del win rate.
    pub fn fixed(&self) -> Self {
        Self {
            observed_trades: None,
            ..*self
        }
    }
}

/// De dónde salen los R de cada camino.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RSource<'a> {
    Model(Payoff),
    /// R reales (no vacía) remuestreados en bloques de `block_len`.
    Resample {
        r: &'a [f64],
        block_len: usize,
    },
}

/// Parámetros de la simulación (validados por quien la pide).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarianceParams {
    /// Riesgo por operación sobre el capital (0,01 = 1 %).
    pub risk_fraction: f64,
    /// Operaciones por camino.
    pub trades: usize,
    pub paths: usize,
    pub seed: u64,
    /// Caída desde el capital **inicial** que cuenta como ruina (0,30 = tocó 0,70).
    pub ruin_threshold: f64,
    /// Rachas: P(racha perdedora más larga ≥ k) para k = 1..=streak_max_k.
    pub streak_max_k: usize,
    /// Ancho de los tramos del histograma de la caída máxima (fracción, 0,05 = 5 %).
    pub drawdown_bin: f64,
}

/// Cuantiles del capital (1 = el inicial) después de `trade` operaciones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub trade: usize,
    pub p5: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p95: f64,
}

/// Un tramo del histograma: `[from, to)` (el último incluye `to`) y la fracción de caminos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bin {
    pub from: f64,
    pub to: f64,
    pub share: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarianceReport {
    /// Esperanza por operación en R: la del modelo (con el win rate pedido) o la media de los R
    /// reales.
    pub expectancy_r: f64,
    /// Bandas del capital en hasta ~100 puntos, desde la operación 0 hasta la última.
    pub bands: Vec<Band>,
    /// Capital final: media y la última banda.
    pub final_mean: f64,
    pub max_drawdown: Summary,
    pub drawdown_histogram: Vec<Bin>,
    pub longest_losing_streak: Summary,
    /// (k, P(racha perdedora más larga ≥ k)).
    pub streak_at_least: Vec<(usize, f64)>,
    /// Caminos que tocaron la ruina.
    pub ruined: usize,
    pub paths: usize,
    /// Win rate sorteado de cada camino (solo con incertidumbre del win rate).
    pub win_rate_draws: Option<Summary>,
}

impl VarianceReport {
    pub fn risk_of_ruin(&self) -> f64 {
        self.ruined as f64 / self.paths as f64
    }
}

/// Uniforme en [0, 1) con 53 bits.
fn uniform(rng: &mut SplitMix64) -> f64 {
    (rng.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
}

/// Normal estándar (Box y Muller, 1958).
fn normal(rng: &mut SplitMix64) -> f64 {
    let u1 = 1.0 - uniform(rng); // (0, 1]: ln nunca de 0
    let u2 = uniform(rng);
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Gamma(shape, 1) con shape ≥ 1 (Marsaglia y Tsang, 2000).
fn gamma(rng: &mut SplitMix64, shape: f64) -> f64 {
    let d = shape - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    loop {
        let x = normal(rng);
        let v = 1.0 + c * x;
        if v <= 0.0 {
            continue;
        }
        let v = v * v * v;
        let u = 1.0 - uniform(rng);
        if u < 1.0 - 0.0331 * x.powi(4) || u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) {
            return d * v;
        }
    }
}

/// Beta(a, b) con a, b ≥ 1, como X / (X + Y) con X ~ Gamma(a) e Y ~ Gamma(b).
pub fn beta(rng: &mut SplitMix64, a: f64, b: f64) -> f64 {
    let x = gamma(rng, a);
    let y = gamma(rng, b);
    x / (x + y)
}

/// El win rate de un camino: el pedido, o uno sorteado de la Beta si hay incertidumbre.
pub fn path_win_rate(rng: &mut SplitMix64, payoff: &Payoff) -> f64 {
    match payoff.observed_trades {
        Some(n) if n > 0 => {
            let n = f64::from(n);
            beta(
                rng,
                payoff.win_rate * n + 1.0,
                (1.0 - payoff.win_rate) * n + 1.0,
            )
        }
        _ => payoff.win_rate,
    }
}

/// R de una operación del modelo con el win rate `p` del camino.
pub fn draw_r(rng: &mut SplitMix64, payoff: &Payoff, p: f64) -> f64 {
    if uniform(rng) < p {
        payoff.avg_win_r
    } else {
        -payoff.avg_loss_r
    }
}

/// `n` R remuestreados de `r` (no vacía) con bloques circulares, aunque `n` sea más que la lista.
pub fn resample_r(rng: &mut SplitMix64, r: &[f64], block_len: usize, n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(n + r.len());
    while out.len() < n {
        out.extend(block_bootstrap(rng, r, block_len.max(1)));
    }
    out.truncate(n);
    out
}

/// Índices (operaciones hechas) donde se miden las bandas: ~100 puntos, siempre 0 y la última.
fn checkpoints(trades: usize) -> Vec<usize> {
    let step = trades.div_ceil(100).max(1);
    let mut out: Vec<usize> = (0..=trades).step_by(step).collect();
    if out.last() != Some(&trades) {
        out.push(trades);
    }
    out
}

/// Corre la simulación. `None` si algo no valida (fuente vacía o con valores no finitos, win rate
/// fuera de [0, 1], pagos no positivos, riesgo no positivo, 0 caminos u operaciones, ruina
/// fuera de (0, 1], tramo no positivo).
pub fn simulate(source: &RSource<'_>, p: &VarianceParams) -> Option<VarianceReport> {
    let valid_source = match source {
        RSource::Model(m) => {
            (0.0..=1.0).contains(&m.win_rate)
                && m.avg_win_r.is_finite()
                && m.avg_win_r > 0.0
                && m.avg_loss_r.is_finite()
                && m.avg_loss_r > 0.0
        }
        RSource::Resample { r, block_len } => {
            !r.is_empty() && *block_len > 0 && r.iter().all(|x| x.is_finite())
        }
    };
    if !valid_source
        || !(p.risk_fraction.is_finite() && p.risk_fraction > 0.0)
        || p.trades == 0
        || p.paths == 0
        || !(p.ruin_threshold > 0.0 && p.ruin_threshold <= 1.0)
        || !(p.drawdown_bin > 0.0 && p.drawdown_bin <= 1.0)
    {
        return None;
    }
    let ruin_level = 1.0 - p.ruin_threshold;
    let marks = checkpoints(p.trades);
    let mut at: Vec<Vec<f64>> = vec![Vec::with_capacity(p.paths); marks.len()];
    let mut stats: Vec<PathStats> = Vec::with_capacity(p.paths);
    let mut win_rates: Vec<f64> = Vec::new();
    let mut rng = SplitMix64::new(p.seed);
    for _ in 0..p.paths {
        let rs: Vec<f64> = match source {
            RSource::Model(m) => {
                let wr = path_win_rate(&mut rng, m);
                if m.observed_trades.is_some() {
                    win_rates.push(wr);
                }
                (0..p.trades).map(|_| draw_r(&mut rng, m, wr)).collect()
            }
            RSource::Resample { r, block_len } => resample_r(&mut rng, r, *block_len, p.trades),
        };
        let mut equity = 1.0_f64;
        let mut next = 0;
        for (i, r) in std::iter::once(0.0).chain(rs.iter().copied()).enumerate() {
            if i > 0 {
                equity = grow(equity, p.risk_fraction, r);
            }
            if marks.get(next) == Some(&i) {
                at[next].push(equity);
                next += 1;
            }
        }
        stats.push(path_stats(&rs, p.risk_fraction, ruin_level));
    }
    let bands = marks
        .iter()
        .zip(at.iter_mut())
        .map(|(&trade, xs)| {
            xs.sort_by(f64::total_cmp);
            let q = |x| quantile_sorted(xs, x).unwrap_or(f64::NAN);
            Band {
                trade,
                p5: q(0.05),
                p25: q(0.25),
                p50: q(0.50),
                p75: q(0.75),
                p95: q(0.95),
            }
        })
        .collect();
    let finals: Vec<f64> = stats.iter().map(|s| s.final_return + 1.0).collect();
    let dds: Vec<f64> = stats.iter().map(|s| s.max_drawdown).collect();
    let n = p.paths as f64;
    let bins = (1.0 / p.drawdown_bin - 1e-9).ceil().max(1.0) as usize;
    let drawdown_histogram = (0..bins)
        .map(|b| {
            let from = b as f64 * p.drawdown_bin;
            let to = ((b + 1) as f64 * p.drawdown_bin).min(1.0);
            let last = b + 1 == bins;
            let hits = dds
                .iter()
                .filter(|&&d| d >= from && (d < to || (last && d <= to)))
                .count();
            Bin {
                from,
                to,
                share: hits as f64 / n,
            }
        })
        .collect();
    let streak_at_least = (1..=p.streak_max_k)
        .map(|k| {
            let hits = stats
                .iter()
                .filter(|s| s.longest_losing_streak >= k)
                .count();
            (k, hits as f64 / n)
        })
        .collect();
    Some(VarianceReport {
        expectancy_r: match source {
            RSource::Model(m) => m.expectancy(),
            RSource::Resample { r, .. } => r.iter().sum::<f64>() / r.len() as f64,
        },
        bands,
        final_mean: finals.iter().sum::<f64>() / n,
        max_drawdown: summarize(dds)?,
        drawdown_histogram,
        longest_losing_streak: summarize(
            stats
                .iter()
                .map(|s| s.longest_losing_streak as f64)
                .collect(),
        )?,
        streak_at_least,
        ruined: stats.iter().filter(|s| s.ruined).count(),
        paths: p.paths,
        win_rate_draws: summarize(win_rates),
    })
}

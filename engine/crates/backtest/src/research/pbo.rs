//! Probabilidad de sobreajuste del backtest (PBO) por validación cruzada simétrica combinatoria
//! (CSCV; Bailey, Borwein, López de Prado y Zhu, 2015, "The probability of backtest
//! overfitting"), #664, punto 5 de `docs/analysis/2026-11-autoanalisis.md` (C3).
//!
//! Entrada: una matriz de retornos `T × N` (filas = períodos, columnas = variantes probadas).
//! 1. Las filas se parten en `S` bloques contiguos de `T / S` filas (`S` par); las que sobran al
//!    final no se usan.
//! 2. Para cada una de las `C(S, S/2)` combinaciones de `S/2` bloques: esos bloques son la muestra
//!    de entrenamiento (IS) y el resto, la de prueba (OOS).
//! 3. Se elige la variante con el mejor Sharpe IS (`n*`; ante empate, la de menor índice) y se mira
//!    su rango relativo OOS `ω = rango / (N + 1)` (rango 1 = peor, N = mejor; los empates toman el
//!    rango promedio), con el logit `λ = ln(ω / (1 − ω))`.
//! 4. `PBO = P(λ ≤ 0)`: la parte de las combinaciones en que la mejor del entrenamiento quedó en la
//!    mitad de abajo (o justo en la mediana) en la prueba.
//!
//! El Sharpe es `media / desvío muestral` por período, sin anualizar (el rango no cambia al
//! anualizar). Con desvío 0, vale `+∞`, `−∞` o 0 según el signo de la media.
//!
//! Usa `f64`: son estadísticos de una muestra de retornos, **no dinero** (como `stats`).
#![allow(
    clippy::float_arithmetic,
    reason = "estadísticos de una matriz de retornos: no son dinero (CLAUDE.md §4)"
)]

use super::montecarlo::{Summary, summarize};
use super::stats::{mean, std_dev};

/// Máximo de particiones: `C(20, 10) = 184.756` combinaciones.
pub const MAX_PARTITIONS: usize = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct Pbo {
    /// Probabilidad de sobreajuste: parte de las combinaciones con `λ ≤ 0`.
    pub pbo: f64,
    /// Combinaciones evaluadas, `C(S, S/2)`.
    pub combinations: usize,
    /// Filas usadas (`S × ⌊T / S⌋`).
    pub rows_used: usize,
    /// Media y cuantiles 5, 50 y 95 % de los logits `λ`.
    pub logits: Summary,
    /// Parte de las combinaciones en que la mejor del entrenamiento tuvo Sharpe OOS < 0.
    pub oos_loss: f64,
}

/// Sharpe por período de las filas `rows` de la columna `col`.
fn sharpe(matrix: &[Vec<f64>], rows: &[usize], col: usize) -> f64 {
    let xs: Vec<f64> = rows.iter().map(|&r| matrix[r][col]).collect();
    let m = mean(&xs).unwrap_or(0.0);
    match std_dev(&xs) {
        Some(s) if s > 0.0 => m / s,
        _ if m > 0.0 => f64::INFINITY,
        _ if m < 0.0 => f64::NEG_INFINITY,
        _ => 0.0,
    }
}

/// PBO por CSCV de `matrix` (filas = períodos, todas con `N ≥ 2` columnas) con `partitions`
/// bloques. `None` si `partitions` es impar, menor que 2 o mayor que [`MAX_PARTITIONS`], si las
/// filas no tienen todas el mismo largo, si hay un valor no finito o si cada bloque no llega a
/// 2 filas.
pub fn pbo_cscv(matrix: &[Vec<f64>], partitions: usize) -> Option<Pbo> {
    let n = matrix.first()?.len();
    let s = partitions;
    if !s.is_multiple_of(2) || !(2..=MAX_PARTITIONS).contains(&s) || n < 2 {
        return None;
    }
    if matrix
        .iter()
        .any(|r| r.len() != n || r.iter().any(|x| !x.is_finite()))
    {
        return None;
    }
    let block = matrix.len() / s;
    if block < 2 {
        return None;
    }
    let mut logits = Vec::new();
    let mut losses = 0_usize;
    for mask in 0_u32..(1 << s) {
        if mask.count_ones() as usize != s / 2 {
            continue;
        }
        let (mut is_rows, mut oos_rows) = (Vec::new(), Vec::new());
        for b in 0..s {
            let rows = (b * block)..((b + 1) * block);
            if mask & (1 << b) != 0 {
                is_rows.extend(rows);
            } else {
                oos_rows.extend(rows);
            }
        }
        let is: Vec<f64> = (0..n).map(|c| sharpe(matrix, &is_rows, c)).collect();
        let best = (0..n).fold(0, |b, c| if is[c] > is[b] { c } else { b });
        let oos: Vec<f64> = (0..n).map(|c| sharpe(matrix, &oos_rows, c)).collect();
        let x = oos[best];
        let below = oos.iter().filter(|&&v| v < x).count() as f64;
        let equal = oos.iter().filter(|&&v| v == x).count() as f64;
        let rank = below + (equal + 1.0) / 2.0;
        let w = rank / (n as f64 + 1.0);
        logits.push((w / (1.0 - w)).ln());
        losses += usize::from(x < 0.0);
    }
    let total = logits.len();
    Some(Pbo {
        pbo: logits.iter().filter(|&&l| l <= 0.0).count() as f64 / total as f64,
        combinations: total,
        rows_used: block * s,
        oos_loss: losses as f64 / total as f64,
        logits: summarize(logits)?,
    })
}

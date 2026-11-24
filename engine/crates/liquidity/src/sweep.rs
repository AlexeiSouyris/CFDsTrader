//! Sweep de liquidez externa (spec §5.4, gatillo).
//!
//! ```text
//! P = último pivote bajo de 1h confirmado al cierre de la vela t (de 15m)
//! largo: L[t] < P y C[t] > P      (pinchó por debajo y volvió a cerrar adentro)
//! corto: H[t] > P' y C[t] < P'    (P' = último pivote alto de 1h confirmado)
//! ```
//! Un pivote de 1h está confirmado cuando **cerró** su vela de confirmación
//! (`h1[confirmed_at].end() ≤ c.end()`): antes no se sabe que es pivote.
//!
//! Volumen de la barrida (#497, informe `docs/analysis/2026-11-patrones-de-manipulacion.md`
//! §4.6). Solo contexto: no cambia qué es un sweep ni suma al puntaje.
//! ```text
//! volumen relativo en t = V[t] / mediana(V[t−N], …, V[t−1])      N = [sweep_volume] median_bars
//! ```
//! - Vacío (`None`) si la vela o alguna de las N anteriores no trae volumen (las velas de IG
//!   pueden no tenerlo: no se inventa), si todavía no hay N velas anteriores (calentamiento) o si
//!   la mediana es 0.
//! - Usa solo velas cerradas hasta `t`: no repinta. Velas faltantes: se usan las N anteriores
//!   de la serie, como en el resto del crate.

use std::num::NonZeroUsize;

use cfds_core::Candle;
use cfds_patterns::{Pivot, PivotKind};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sweep {
    pub side: Side,
    /// El pivote de 1h barrido (referencia del stop, spec §8: la mecha del sweep).
    pub level: Decimal,
}

fn last_known(c: &Candle, h1: &[Candle], pivots: &[Pivot], kind: PivotKind) -> Option<Decimal> {
    pivots
        .iter()
        .filter(|p| p.kind == kind)
        .filter(|p| h1.get(p.confirmed_at).is_some_and(|k| k.end() <= c.end()))
        .max_by_key(|p| p.index)
        .map(|p| p.price)
}

/// Sweeps que hace la vela de 15m `c` contra los pivotes de 1h (`h1_pivots`, calculados sobre `h1`).
pub fn sweeps_at(c: &Candle, h1: &[Candle], h1_pivots: &[Pivot]) -> Vec<Sweep> {
    let mut out = Vec::new();
    if let Some(p) = last_known(c, h1, h1_pivots, PivotKind::Low)
        && c.low < p
        && c.close > p
    {
        out.push(Sweep {
            side: Side::Long,
            level: p,
        });
    }
    if let Some(p) = last_known(c, h1, h1_pivots, PivotKind::High)
        && c.high > p
        && c.close < p
    {
        out.push(Sweep {
            side: Side::Short,
            level: p,
        });
    }
    out
}

/// Sección `[sweep_volume]` de `config/liquidity.toml` (#497).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepVolumeConfig {
    pub median_bars: NonZeroUsize,
}

#[derive(Deserialize)]
struct RawFile {
    sweep_volume: RawSweepVolume,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSweepVolume {
    median_bars: NonZeroUsize,
}

impl SweepVolumeConfig {
    /// Lee y valida `[sweep_volume]`; ignora las demás secciones de `liquidity.toml`.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw = toml::from_str::<RawFile>(text)
            .map_err(|e| e.to_string())?
            .sweep_volume;
        Ok(Self {
            median_bars: raw.median_bars,
        })
    }
}

/// Un sweep con el volumen relativo de su vela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepWithVolume {
    pub sweep: Sweep,
    /// `V[t] / mediana` de las `median_bars` velas anteriores; `None` si no hay volumen.
    pub relative_volume: Option<Decimal>,
}

/// Volumen de la vela `t` contra la mediana del volumen de las `n` velas anteriores (sin la
/// propia). `None` si falta algún volumen, si no hay `n` velas antes o si la mediana es 0.
pub fn relative_volume(candles: &[Candle], t: usize, n: NonZeroUsize) -> Option<Decimal> {
    let n = n.get();
    let v = candles.get(t)?.volume?;
    let from = t.checked_sub(n)?;
    let mut window: Vec<Decimal> = candles[from..t]
        .iter()
        .map(|c| c.volume)
        .collect::<Option<_>>()?;
    window.sort();
    let median = if n % 2 == 1 {
        window[n / 2]
    } else {
        (window[n / 2 - 1] + window[n / 2]) / Decimal::TWO
    };
    if median <= Decimal::ZERO {
        return None;
    }
    v.checked_div(median)
}

/// [`sweeps_at`] de la vela `m15[t]`, cada uno con el [`relative_volume`] de esa vela.
pub fn sweeps_with_volume_at(
    m15: &[Candle],
    t: usize,
    h1: &[Candle],
    h1_pivots: &[Pivot],
    cfg: &SweepVolumeConfig,
) -> Vec<SweepWithVolume> {
    let Some(c) = m15.get(t) else {
        return Vec::new();
    };
    let sweeps = sweeps_at(c, h1, h1_pivots);
    if sweeps.is_empty() {
        return Vec::new();
    }
    let relative_volume = relative_volume(m15, t, cfg.median_bars);
    sweeps
        .into_iter()
        .map(|sweep| SweepWithVolume {
            sweep,
            relative_volume,
        })
        .collect()
}

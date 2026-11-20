//! Soportes y resistencias (spec §5.3), evaluados en la vela cerrada `t`.
//!
//! ```text
//! nivel de soporte = grupo de ≥ 3 pivotes bajos confirmados de las últimas 200 velas,
//!                    todos dentro de ±0,15 % del promedio del grupo (nivel = ese promedio)
//! rechazo (largo):   L[t] ≤ nivel · (1 + 0,15 %) y C[t] > nivel
//! resistencia y rechazo (corto): el espejo con pivotes altos
//! ```
//!
//! **Cómo se arman los grupos** (decisión del agente, la spec no lo fija): los precios se
//! ordenan de menor a mayor y se recorren agregando cada precio al grupo actual mientras todos
//! sigan dentro de la tolerancia del nuevo promedio; si no entra, se cierra el grupo y empieza
//! otro. Es determinista y no depende del orden temporal de los pivotes.

use cfds_core::Candle;
use rust_decimal::Decimal;

use crate::config::LevelConfig;
use crate::pivots::{Pivot, PivotKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelKind {
    Support,
    Resistance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Level {
    pub kind: LevelKind,
    pub price: Decimal,
    pub touches: usize,
}

fn within(prices: &[Decimal], tol: Decimal) -> bool {
    let mean = prices.iter().sum::<Decimal>() / Decimal::from(prices.len());
    prices.iter().all(|p| (*p - mean).abs() <= tol * mean)
}

fn group(mut prices: Vec<Decimal>, kind: LevelKind, cfg: &LevelConfig) -> Vec<Level> {
    prices.sort();
    let mut levels = Vec::new();
    let mut current: Vec<Decimal> = Vec::new();
    let close = |g: &Vec<Decimal>, out: &mut Vec<Level>| {
        if g.len() >= cfg.min_touches {
            let price = g.iter().sum::<Decimal>() / Decimal::from(g.len());
            out.push(Level {
                kind,
                price,
                touches: g.len(),
            });
        }
    };
    for p in prices {
        current.push(p);
        if !within(&current, cfg.tolerance) {
            current.pop();
            close(&current, &mut levels);
            current = vec![p];
        }
    }
    close(&current, &mut levels);
    levels
}

/// Niveles vigentes en `t`: con pivotes confirmados en `t` cuya vela está en las últimas
/// `lookback_bars` velas.
pub fn levels_at(t: usize, pivots: &[Pivot], cfg: &LevelConfig) -> Vec<Level> {
    let from = t.saturating_sub(cfg.lookback_bars - 1);
    let recent = |k: PivotKind| -> Vec<Decimal> {
        pivots
            .iter()
            .filter(|p| p.kind == k && p.confirmed_at <= t && p.index >= from)
            .map(|p| p.price)
            .collect()
    };
    let mut out = group(recent(PivotKind::Low), LevelKind::Support, cfg);
    out.extend(group(recent(PivotKind::High), LevelKind::Resistance, cfg));
    out
}

/// Niveles que la vela `t` rechaza (toca y cierra del lado correcto).
pub fn rejections_at(
    candles: &[Candle],
    t: usize,
    levels: &[Level],
    cfg: &LevelConfig,
) -> Vec<Level> {
    let Some(c) = candles.get(t) else {
        return Vec::new();
    };
    levels
        .iter()
        .filter(|l| match l.kind {
            LevelKind::Support => {
                c.low <= l.price * (Decimal::ONE + cfg.tolerance) && c.close > l.price
            }
            LevelKind::Resistance => {
                c.high >= l.price * (Decimal::ONE - cfg.tolerance) && c.close < l.price
            }
        })
        .copied()
        .collect()
}

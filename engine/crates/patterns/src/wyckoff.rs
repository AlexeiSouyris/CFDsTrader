//! Spring y upthrust de Wyckoff (#491). Herramienta de **análisis**: no entra al puntaje.
//!
//! Construye sobre los soportes y resistencias de `levels` (spec §5.3: grupos de ≥ k pivotes
//! confirmados dentro de ±X %), con sus propios parámetros (`config/analysis.toml`, `[wyckoff]`).
//!
//! ```text
//! rango en b:  S = soporte más alto con S · (1 − tol) < C[b−1];  R = resistencia más baja con
//!              R · (1 + tol) > C[b−1]  (C[b−1] dentro del rango, bandas de tolerancia incluidas;
//!              niveles de `levels_at(b)`: solo pivotes confirmados hasta b)
//! spring:      L[b] < S · (1 − tol)                       ruptura falsa por debajo del rango
//!              t = primera vela en b..=b+M con C[t] > S,  y C[b..t] ≤ S (cierra de vuelta adentro)
//!              mín(L[b..=t]) ≥ S · (1 − max_penetración)  (si baja más, es una ruptura de verdad)
//! upthrust:    el espejo contra R
//! ```
//! La señal va en la vela de confirmación `t` (cuando cerró de vuelta adentro). Con `M = 0` y una
//! sola vela, un spring es un **sweep** de un soporte de varios toques; el sweep de `liquidity`
//! (spec §5.4) barre solo el último pivote de 1h y no pide rango, así que no se duplican.
//! Todo depende solo de `0..=t` (sin repintado, hay test).

use cfds_core::Candle;
use rust_decimal::Decimal;

use crate::config::LevelConfig;
use crate::levels::{LevelKind, levels_at};
use crate::pivots::Pivot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WyckoffConfig {
    /// Soportes y resistencias del rango (ventana, toques mínimos y tolerancia).
    pub levels: LevelConfig,
    /// Velas después de la ruptura para volver a cerrar adentro (0 = la misma vela).
    pub max_bars_to_reclaim: usize,
    /// Penetración máxima más allá del nivel, como fracción (0,5 % = 0,005).
    pub max_penetration: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WyckoffKind {
    Spring,
    Upthrust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WyckoffEvent {
    pub kind: WyckoffKind,
    /// Vela que rompe el nivel.
    pub break_index: usize,
    /// Vela que vuelve a cerrar adentro: la señal.
    pub confirmed_at: usize,
    /// Nivel roto (soporte del spring, resistencia del upthrust).
    pub level: Decimal,
    /// Toques del nivel.
    pub touches: usize,
    /// Precio más extremo de la ruptura (mínimo del spring, máximo del upthrust).
    pub extreme: Decimal,
}

/// Springs y upthrusts de la serie, en orden de ruptura.
pub fn wyckoff_events(
    candles: &[Candle],
    pivots: &[Pivot],
    cfg: &WyckoffConfig,
) -> Vec<WyckoffEvent> {
    let mut out = Vec::new();
    let tol = cfg.levels.tolerance;
    for b in 1..candles.len() {
        let prev = candles[b - 1].close;
        let levels = levels_at(b, pivots, &cfg.levels);
        let (lower, upper) = (Decimal::ONE - tol, Decimal::ONE + tol);
        let support = levels
            .iter()
            .filter(|l| l.kind == LevelKind::Support && l.price * lower < prev)
            .max_by_key(|l| l.price);
        let resistance = levels
            .iter()
            .filter(|l| l.kind == LevelKind::Resistance && l.price * upper > prev)
            .min_by_key(|l| l.price);
        let (Some(s), Some(r)) = (support, resistance) else {
            continue;
        };
        let window = || b..=(b + cfg.max_bars_to_reclaim).min(candles.len() - 1);
        if candles[b].low < s.price * lower
            && let Some(t) = window().find(|&t| candles[t].close > s.price)
        {
            let extreme = candles[b..=t]
                .iter()
                .map(|c| c.low)
                .min()
                .unwrap_or(s.price);
            if extreme >= s.price * (Decimal::ONE - cfg.max_penetration) {
                out.push(WyckoffEvent {
                    kind: WyckoffKind::Spring,
                    break_index: b,
                    confirmed_at: t,
                    level: s.price,
                    touches: s.touches,
                    extreme,
                });
            }
        }
        if candles[b].high > r.price * upper
            && let Some(t) = window().find(|&t| candles[t].close < r.price)
        {
            let extreme = candles[b..=t]
                .iter()
                .map(|c| c.high)
                .max()
                .unwrap_or(r.price);
            if extreme <= r.price * (Decimal::ONE + cfg.max_penetration) {
                out.push(WyckoffEvent {
                    kind: WyckoffKind::Upthrust,
                    break_index: b,
                    confirmed_at: t,
                    level: r.price,
                    touches: r.touches,
                    extreme,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candles::tests::bar;
    use crate::pivots::confirmed_pivots;
    use rust_decimal::dec;

    fn cfg(max_bars_to_reclaim: usize) -> WyckoffConfig {
        WyckoffConfig {
            levels: LevelConfig {
                lookback_bars: 200,
                min_touches: 2,
                tolerance: dec!(0.001),
            },
            max_bars_to_reclaim,
            max_penetration: dec!(0.02),
        }
    }

    fn hlc(i: usize, h: i64, l: i64, c: i64) -> Candle {
        let d = Decimal::from;
        bar(i, d(c), d(h), d(l), d(c))
    }

    /// Rango 100–110 con dos mínimos en 100 y dos máximos en 110 (pivotes con N = 1).
    fn range(tail: &[(i64, i64, i64)]) -> Vec<Candle> {
        let base = [
            (106, 103, 105),
            (110, 104, 108), // 1: alto
            (107, 102, 104),
            (105, 100, 103), // 3: bajo
            (107, 102, 106),
            (110, 104, 107), // 5: alto
            (107, 103, 104),
            (105, 100, 103), // 7: bajo
            (107, 102, 105), // 8: confirma el bajo de 7
        ];
        base.iter()
            .chain(tail)
            .enumerate()
            .map(|(i, &(h, l, c))| hlc(i, h, l, c))
            .collect()
    }

    fn events(c: &[Candle], m: usize) -> Vec<WyckoffEvent> {
        wyckoff_events(c, &confirmed_pivots(c, 1), &cfg(m))
    }

    #[test]
    fn one_candle_spring_pierces_the_support_and_closes_back_inside() {
        let e = events(&range(&[(104, 99, 102)]), 0);
        assert_eq!(e.len(), 1);
        let s = e[0];
        assert_eq!(
            (s.kind, s.break_index, s.confirmed_at, s.level, s.extreme),
            (WyckoffKind::Spring, 9, 9, dec!(100), dec!(99))
        );
        assert_eq!(s.touches, 2);
    }

    #[test]
    fn a_spring_can_take_several_candles_to_reclaim_the_range() {
        let tail = [(104, 99, 99), (100, 98, 99), (103, 99, 102)];
        assert!(events(&range(&tail), 1).is_empty(), "con M = 1 no llega");
        let e = events(&range(&tail), 2);
        assert_eq!((e[0].break_index, e[0].confirmed_at), (9, 11));
        assert_eq!(e[0].extreme, dec!(98));
    }

    #[test]
    fn a_deep_break_is_not_a_spring() {
        // Baja a 95 (5 % > 2 %): es una ruptura de verdad.
        assert!(events(&range(&[(104, 95, 102)]), 0).is_empty());
    }

    #[test]
    fn piercing_within_the_tolerance_is_not_a_break() {
        // 99,95 está dentro de ±0,1 % de 100.
        let mut c = range(&[]);
        c.push(bar(9, dec!(102), dec!(104), dec!(99.95), dec!(102)));
        assert!(events(&c, 0).is_empty());
    }

    #[test]
    fn upthrust_is_the_mirror() {
        let e = events(&range(&[(111, 106, 108)]), 0);
        assert_eq!(e.len(), 1);
        assert_eq!(
            (e[0].kind, e[0].level, e[0].extreme),
            (WyckoffKind::Upthrust, dec!(110), dec!(111))
        );
    }

    #[test]
    fn no_spring_without_a_range_or_with_few_candles() {
        // Sin resistencia (solo mínimos): no hay rango.
        let c: Vec<Candle> = [(106, 100, 103), (108, 104, 106), (107, 100, 104)]
            .iter()
            .chain(&[(109, 105, 107), (106, 99, 103)])
            .enumerate()
            .map(|(i, &(h, l, c))| hlc(i, h, l, c))
            .collect();
        assert!(events(&c, 0).is_empty());
        assert!(events(&[], 0).is_empty());
        assert!(events(&c[..1], 0).is_empty());
    }

    #[test]
    fn events_never_change_when_new_candles_arrive() {
        let all = range(&[
            (104, 99, 99),
            (100, 98, 99),
            (103, 99, 102),
            (111, 104, 108),
            (109, 103, 105),
            (104, 99, 103),
        ]);
        let full = events(&all, 2);
        assert!(!full.is_empty());
        for len in 1..all.len() {
            let part = events(&all[..len], 2);
            let expected: Vec<_> = full
                .iter()
                .filter(|e| e.confirmed_at < len)
                .copied()
                .collect();
            assert_eq!(part, expected, "con {len} velas");
        }
    }
}

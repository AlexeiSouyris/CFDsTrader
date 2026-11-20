//! Pivotes (spec §4): la vela `i` es **pivote alto** si su máximo es estrictamente mayor que el
//! de las N velas anteriores y las N posteriores; **pivote bajo**, lo mismo con mínimos.
//!
//! Un pivote se **confirma** en la vela `i + N`: antes no existe para el sistema, porque todavía
//! no se sabe si alguna de las velas siguientes lo supera. Cualquier señal que use un pivote
//! se evalúa en la vela de confirmación o después, nunca en la del pivote.

use cfds_core::Candle;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PivotKind {
    High,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pivot {
    pub kind: PivotKind,
    /// Vela del máximo o mínimo.
    pub index: usize,
    /// Vela en la que se confirma (`index + N`): recién ahí se puede usar.
    pub confirmed_at: usize,
    pub price: Decimal,
}

/// Pivotes ya confirmados en la serie (todos los que tienen sus N velas posteriores), en orden.
pub fn confirmed_pivots(candles: &[Candle], strength: usize) -> Vec<Pivot> {
    let n = strength;
    let mut out = Vec::new();
    if n == 0 || candles.len() < 2 * n + 1 {
        return out;
    }
    for i in n..candles.len() - n {
        let neighbors = (i - n..i).chain(i + 1..=i + n);
        let (h, l) = (candles[i].high, candles[i].low);
        if neighbors.clone().all(|j| h > candles[j].high) {
            out.push(Pivot {
                kind: PivotKind::High,
                index: i,
                confirmed_at: i + n,
                price: h,
            });
        }
        if neighbors.into_iter().all(|j| l < candles[j].low) {
            out.push(Pivot {
                kind: PivotKind::Low,
                index: i,
                confirmed_at: i + n,
                price: l,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candles::tests::bar;
    use rust_decimal::Decimal;

    /// Velas con máximo = valor y mínimo = valor − 10.
    fn highs(v: &[i64]) -> Vec<Candle> {
        v.iter()
            .enumerate()
            .map(|(i, &h)| {
                let h = Decimal::from(h);
                bar(
                    i,
                    h - Decimal::from(5),
                    h,
                    h - Decimal::from(10),
                    h - Decimal::from(5),
                )
            })
            .collect()
    }

    #[test]
    fn finds_a_high_pivot_and_confirms_it_n_candles_later() {
        let c = highs(&[10, 11, 12, 20, 13, 12, 11]);
        let p = confirmed_pivots(&c, 3);
        assert_eq!(
            p,
            vec![Pivot {
                kind: PivotKind::High,
                index: 3,
                confirmed_at: 6,
                price: Decimal::from(20)
            }]
        );
    }

    #[test]
    fn a_pivot_does_not_exist_before_its_confirmation_candle() {
        // Con solo 2 velas después del máximo, todavía no está confirmado.
        let c = highs(&[10, 11, 12, 20, 13, 12]);
        assert!(confirmed_pivots(&c, 3).is_empty());
    }

    #[test]
    fn a_later_higher_candle_cancels_the_pivot() {
        let c = highs(&[10, 11, 12, 20, 13, 21, 11]);
        assert!(confirmed_pivots(&c, 3).iter().all(|p| p.index != 3));
    }

    #[test]
    fn equal_highs_are_not_a_pivot() {
        // "Estrictamente mayor": un empate no es pivote.
        let c = highs(&[10, 11, 20, 20, 13, 12, 11]);
        assert!(
            confirmed_pivots(&c, 3)
                .iter()
                .all(|p| p.kind != PivotKind::High)
        );
    }

    #[test]
    fn finds_low_pivots_too() {
        let c = highs(&[20, 19, 18, 10, 17, 18, 19]);
        let p = confirmed_pivots(&c, 3);
        assert_eq!(p.len(), 1);
        assert_eq!(
            (p[0].kind, p[0].index, p[0].price),
            (PivotKind::Low, 3, Decimal::ZERO)
        );
    }

    #[test]
    fn pivots_never_change_when_new_candles_arrive() {
        let all = highs(&[10, 11, 12, 20, 13, 12, 11, 15, 16, 30, 14, 13, 12, 18]);
        let full = confirmed_pivots(&all, 3);
        for len in 1..all.len() {
            let partial = confirmed_pivots(&all[..len], 3);
            let expected: Vec<_> = full
                .iter()
                .filter(|p| p.confirmed_at < len)
                .copied()
                .collect();
            assert_eq!(partial, expected, "con {len} velas");
        }
    }
}

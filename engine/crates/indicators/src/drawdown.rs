//! Caída desde el máximo sobre velas diarias (#213, spec §9.1, componente del índice de
//! sentimiento).
//!
//! ```text
//! máximo_t = máximo de los cierres de las últimas n velas (t incluida)
//! caída_t  = (cierre_t − máximo_t) / máximo_t                         (≤ 0)
//! ```
//! - Calentamiento: hacen falta `n` cierres. Antes, `None`.
//! - Sin repintado: el valor de `t` solo depende de los cierres `0..=t`.
//! - Un máximo ≤ 0 no permite dividir: ese valor queda en `None`.

use rust_decimal::Decimal;

/// Caída desde el máximo de ventana `window` (en fracción: −0,1 = 10 % por debajo), alineada
/// por índice.
///
/// # Panics
/// Si `window` es 0: se rechaza al cargar `config/`.
pub fn drawdown_from_high(closes: &[Decimal], window: usize) -> Vec<Option<Decimal>> {
    assert!(window >= 1, "la ventana de la caída tiene que ser ≥ 1");
    (0..closes.len())
        .map(|i| {
            let first = (i + 1).checked_sub(window)?;
            let high = closes[first..=i].iter().copied().max()?;
            (high > Decimal::ZERO).then(|| (closes[i] - high) / high)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn warms_up_with_window_closes() {
        let v = drawdown_from_high(&[dec!(100), dec!(90), dec!(95)], 3);
        assert_eq!(v, [None, None, Some(dec!(-0.05))]);
    }

    #[test]
    fn includes_the_current_close_and_only_the_window() {
        // Ventana 2: en t=2 el 120 de t=0 ya quedó afuera; el máximo es 100.
        let v = drawdown_from_high(&[dec!(120), dec!(100), dec!(80)], 2);
        assert_eq!(v[1], Some(dec!(-0.1666666666666666666666666667)));
        assert_eq!(v[2], Some(dec!(-0.2)));
        // Un cierre nuevo en máximo vale 0.
        assert_eq!(drawdown_from_high(&[dec!(1), dec!(2)], 2)[1], Some(dec!(0)));
    }

    #[test]
    fn a_non_positive_high_has_no_value() {
        let v = drawdown_from_high(&[dec!(0), dec!(0)], 1);
        assert_eq!(v, [None, None]);
    }

    #[test]
    fn never_repaints() {
        let closes = [
            dec!(100),
            dec!(103),
            dec!(101),
            dec!(99),
            dec!(104),
            dec!(98),
        ];
        let full = drawdown_from_high(&closes, 3);
        for t in 0..closes.len() {
            assert_eq!(drawdown_from_high(&closes[..=t], 3)[t], full[t]);
        }
    }
}

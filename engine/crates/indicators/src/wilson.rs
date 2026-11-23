//! Intervalo de Wilson de una proporción, con decimales exactos. Único cálculo del motor: lo
//! usan la probabilidad mostrada de la spec §13 (`cfds-journal`) y las estadísticas de liquidez
//! de la spec §5.4.1 (`cfds-liquidity`).
//!
//! ```text
//! p = hits / n
//! centro = (p + z² / 2n) / (1 + z² / n)
//! medio  = z · √(p(1 − p) / n + z² / 4n²) / (1 + z² / n)
//! IC     = [max(0, centro − medio), min(1, centro + medio)]
//! ```

use rust_decimal::{Decimal, MathematicalOps};

/// Intervalo de Wilson de `hits / n` con el `z` dado, en [0, 1]. `n` tiene que ser > 0.
pub fn wilson(hits: u32, n: u32, z: Decimal) -> (Decimal, Decimal) {
    debug_assert!(n > 0 && hits <= n);
    let n_d = Decimal::from(n);
    let p = Decimal::from(hits) / n_d;
    let z2 = z * z;
    let denom = Decimal::ONE + z2 / n_d;
    let center = p + z2 / (Decimal::TWO * n_d);
    let radicand = p * (Decimal::ONE - p) / n_d + z2 / (Decimal::from(4) * n_d * n_d);
    let half = z * radicand.sqrt().unwrap_or(Decimal::ZERO);
    let low = ((center - half) / denom).max(Decimal::ZERO);
    let high = ((center + half) / denom).min(Decimal::ONE);
    (low, high)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    fn rounded(hits: u32, n: u32) -> (Decimal, Decimal) {
        let (lo, hi) = wilson(hits, n, dec!(1.96));
        (lo.round_dp(4), hi.round_dp(4))
    }

    #[test]
    fn matches_values_computed_by_hand() {
        // 7/20: p = 0,35; z²/n = 0,19208; centro = 0,44604 / 1,19208 = 0,37417;
        // medio = 1,96 · √(0,011375 + 0,0024010) / 1,19208 = 0,19298.
        assert_eq!(rounded(7, 20), (dec!(0.1812), dec!(0.5671)));
        // 0/30: el piso es 0 y el techo z²/n / (1 + z²/n) = 0,128053 / 1,128053.
        assert_eq!(rounded(0, 30), (dec!(0), dec!(0.1135)));
        // 30/30: el espejo.
        assert_eq!(rounded(30, 30), (dec!(0.8865), dec!(1)));
        assert_eq!(rounded(5, 10), (dec!(0.2366), dec!(0.7634)));
    }
}

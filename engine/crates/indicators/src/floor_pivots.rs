//! Puntos pivote clásicos del día (floor pivots, #491). Herramienta de **análisis**.
//!
//! Con el máximo `H`, el mínimo `L` y el cierre `C` del **último día terminado**:
//! ```text
//! P  = (H + L + C) / 3
//! R1 = 2P − L        S1 = 2P − H
//! R2 = P + (H − L)   S2 = P − (H − L)
//! R3 = H + 2(P − L)  S3 = L − 2(H − P)
//! ```
//! Son las fórmulas "clásicas" de TradingView y de los manuales de floor trader. Valen todo el
//! día siguiente; nunca se calculan con el día en curso.

use rust_decimal::Decimal;

use crate::atr::Hlc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorPivots {
    pub pp: Decimal,
    pub r1: Decimal,
    pub r2: Decimal,
    pub r3: Decimal,
    pub s1: Decimal,
    pub s2: Decimal,
    pub s3: Decimal,
}

/// Niveles del día siguiente a `day`.
pub fn floor_pivots<T: Hlc>(day: &T) -> FloorPivots {
    let (h, l, c) = (day.high(), day.low(), day.close());
    let two = Decimal::TWO;
    let pp = (h + l + c) / Decimal::from(3);
    FloorPivots {
        pp,
        r1: two * pp - l,
        s1: two * pp - h,
        r2: pp + (h - l),
        s2: pp - (h - l),
        r3: h + two * (pp - l),
        s3: l - two * (h - pp),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moving_average::tests::closes;
    use rust_decimal::dec;

    #[test]
    fn classic_formulas_match_hand_calculation() {
        let mut d = closes(&[dec!(105)]).remove(0);
        d.high = dec!(110);
        d.low = dec!(95);
        // P = 310 / 3; con H − L = 15.
        let p = floor_pivots(&d);
        let pp = dec!(310) / dec!(3);
        assert_eq!(p.pp, pp);
        assert_eq!(p.r1, dec!(2) * pp - dec!(95));
        assert_eq!(p.s1, dec!(2) * pp - dec!(110));
        assert_eq!(p.r2, pp + dec!(15));
        assert_eq!(p.s2, pp - dec!(15));
        assert_eq!(p.r3, dec!(110) + dec!(2) * (pp - dec!(95)));
        assert_eq!(p.s3, dec!(95) - dec!(2) * (dec!(110) - pp));
        assert!(p.s3 < p.s2 && p.s2 < p.s1 && p.s1 < p.pp);
        assert!(p.pp < p.r1 && p.r1 < p.r2 && p.r2 < p.r3);
    }

    #[test]
    fn a_flat_day_collapses_every_level() {
        let d = closes(&[dec!(100)]).remove(0);
        let p = floor_pivots(&d);
        assert!(
            [p.r1, p.r2, p.r3, p.s1, p.s2, p.s3]
                .iter()
                .all(|x| *x == p.pp)
        );
    }
}

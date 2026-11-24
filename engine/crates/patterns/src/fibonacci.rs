//! Fibonacci y golden pocket (#491). Herramienta de **análisis**: no entra al puntaje ni a la
//! estrategia en uso.
//!
//! **Número de oro** φ = (1 + √5) / 2 ≈ 1,618…: la base de los niveles. No es un indicador aparte:
//! cada nivel habitual es una potencia (o raíz) de φ, redondeada a tres decimales.
//!
//! ```text
//! 0,236 ≈ 1/φ³   0,382 ≈ 1/φ²   0,618 ≈ 1/φ   0,786 ≈ √(1/φ)   (0,5 no sale de φ: es costumbre)
//! 1,272 ≈ √φ    1,618 ≈ φ      2,618 ≈ φ²
//! ```
//!
//! **Tramo** (leg): entre dos pivotes confirmados consecutivos de tipo opuesto. Para cada pivote
//! confirmado `B`, el inicio `A` es el último pivote del tipo opuesto con vela anterior a la de `B`.
//! Sube si `A` es bajo y `B` alto (y `B > A`); baja en el espejo. El tramo **existe recién en la
//! vela de confirmación de `B`** (`B.confirmed_at`): antes no se sabe que `B` es pivote.
//!
//! ```text
//! retroceso r:  precio = B − r · (B − A)      (vuelve hacia A)
//! extensión e:  precio = A + e · (B − A)      (sigue más allá de B en la dirección del tramo)
//! golden pocket: zona entre los retrocesos 0,618 y 0,65
//! ```
//!
//! Eventos del golden pocket en la vela cerrada `t`, contra el último tramo confirmado **antes**
//! de `t` (`confirmed_at < t`):
//! - `touches`: el rango de la vela toca la zona (`L[t] ≤ techo` y `H[t] ≥ piso`).
//! - `closes_inside`: el cierre queda dentro de la zona.
//!
//! Un tramo deja de valer cuando una vela posterior a `B` **cierra** más allá de `A` (retroceso de
//! más del 100 %): desde ahí no hay eventos hasta el próximo tramo. Todo depende solo de las
//! velas `0..=t` (sin repintado, hay test).

use cfds_core::Candle;
use rust_decimal::Decimal;
use rust_decimal::dec;

use crate::pivots::{Pivot, PivotKind};

/// φ = (1 + √5) / 2 con los 28 decimales que admite `Decimal` (exacto hasta ahí, nunca `f64`).
pub const PHI: Decimal = dec!(1.6180339887498948482045868344);

/// Niveles a dibujar (`config/analysis.toml`, `[fibonacci]`), validados al cargar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FibConfig {
    /// Retrocesos, cada uno en (0, 1), en orden creciente.
    pub retracements: Vec<Decimal>,
    /// Extensiones, cada una > 1, en orden creciente.
    pub extensions: Vec<Decimal>,
    /// Zona del golden pocket (desde, hasta), como retrocesos: 0 < desde < hasta < 1.
    pub golden_pocket: (Decimal, Decimal),
}

fn strictly_increasing(v: &[Decimal]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}

impl FibConfig {
    pub fn new(
        retracements: Vec<Decimal>,
        extensions: Vec<Decimal>,
        golden_pocket: (Decimal, Decimal),
    ) -> Result<Self, String> {
        let (zero, one) = (Decimal::ZERO, Decimal::ONE);
        if retracements.is_empty()
            || !strictly_increasing(&retracements)
            || retracements.iter().any(|r| *r <= zero || *r >= one)
        {
            return Err("fibonacci.retracements: no vacío, cada uno en (0, 1), creciente".into());
        }
        if !strictly_increasing(&extensions) || extensions.iter().any(|e| *e <= one) {
            return Err("fibonacci.extensions: cada una > 1, creciente".into());
        }
        let (lo, hi) = golden_pocket;
        if !(zero < lo && lo < hi && hi < one) {
            return Err("fibonacci.golden_pocket: 0 < desde < hasta < 1".into());
        }
        Ok(Self {
            retracements,
            extensions,
            golden_pocket,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegDirection {
    Up,
    Down,
}

/// Un tramo con sus niveles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FibLeg {
    pub direction: LegDirection,
    pub start: Pivot,
    pub end: Pivot,
    /// `(razón, precio)` de cada retroceso.
    pub retracements: Vec<(Decimal, Decimal)>,
    /// `(razón, precio)` de cada extensión. Puede ser ≤ 0 en un tramo bajista muy grande.
    pub extensions: Vec<(Decimal, Decimal)>,
    /// Golden pocket como (precio más bajo, precio más alto).
    pub golden_pocket: (Decimal, Decimal),
}

impl FibLeg {
    /// Vela desde la que el tramo existe.
    pub fn confirmed_at(&self) -> usize {
        self.end.confirmed_at
    }

    fn retracement(&self, r: Decimal) -> Decimal {
        self.end.price - r * (self.end.price - self.start.price)
    }

    /// `true` si cerrar en `close` deja el tramo sin valor (pasó el inicio).
    fn broken_by(&self, close: Decimal) -> bool {
        match self.direction {
            LegDirection::Up => close < self.start.price,
            LegDirection::Down => close > self.start.price,
        }
    }
}

/// Tramos en el orden en que se confirman (`pivots` = salida de `confirmed_pivots`).
pub fn fib_legs(pivots: &[Pivot], cfg: &FibConfig) -> Vec<FibLeg> {
    let mut out = Vec::new();
    for (k, end) in pivots.iter().enumerate() {
        let Some(start) = pivots[..k]
            .iter()
            .rev()
            .find(|p| p.kind != end.kind && p.index < end.index)
        else {
            continue;
        };
        let direction = match (start.kind, end.kind) {
            (PivotKind::Low, PivotKind::High) if end.price > start.price => LegDirection::Up,
            (PivotKind::High, PivotKind::Low) if end.price < start.price => LegDirection::Down,
            _ => continue,
        };
        let mut leg = FibLeg {
            direction,
            start: *start,
            end: *end,
            retracements: Vec::new(),
            extensions: Vec::new(),
            golden_pocket: (Decimal::ZERO, Decimal::ZERO),
        };
        leg.retracements = cfg
            .retracements
            .iter()
            .map(|r| (*r, leg.retracement(*r)))
            .collect();
        let range = end.price - start.price;
        leg.extensions = cfg
            .extensions
            .iter()
            .map(|e| (*e, start.price + *e * range))
            .collect();
        let (a, b) = (
            leg.retracement(cfg.golden_pocket.0),
            leg.retracement(cfg.golden_pocket.1),
        );
        leg.golden_pocket = (a.min(b), a.max(b));
        out.push(leg);
    }
    // Todos los pivotes se confirman N velas después de su vela: confirmar en orden = vela en orden.
    out.sort_by_key(|l| (l.confirmed_at(), l.end.index));
    out
}

/// La vela `index` toca (o cierra dentro de) el golden pocket del tramo `leg` (índice en `legs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PocketEvent {
    pub index: usize,
    pub leg: usize,
    pub touches: bool,
    pub closes_inside: bool,
}

/// Eventos del golden pocket de cada vela, contra el último tramo confirmado antes de ella.
pub fn golden_pocket_events(candles: &[Candle], legs: &[FibLeg]) -> Vec<PocketEvent> {
    let mut out = Vec::new();
    let mut current: Option<usize> = None;
    let mut broken = false;
    for (t, c) in candles.iter().enumerate() {
        let latest = legs.iter().rposition(|l| l.confirmed_at() < t);
        if latest != current {
            current = latest;
            // Un tramo nuevo: ¿alguna vela ya cerrada después de su fin lo rompió?
            broken = current.is_some_and(|k| {
                let leg = &legs[k];
                candles[leg.end.index + 1..t]
                    .iter()
                    .any(|x| leg.broken_by(x.close))
            });
        }
        let Some(k) = current else { continue };
        let leg = &legs[k];
        if broken || leg.broken_by(c.close) {
            broken = true;
            continue;
        }
        let (lo, hi) = leg.golden_pocket;
        let touches = c.low <= hi && c.high >= lo;
        if touches {
            out.push(PocketEvent {
                index: t,
                leg: k,
                touches,
                closes_inside: lo <= c.close && c.close <= hi,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candles::tests::bar;
    use crate::pivots::confirmed_pivots;

    fn cfg() -> FibConfig {
        FibConfig::new(
            vec![
                dec!(0.236),
                dec!(0.382),
                dec!(0.5),
                dec!(0.618),
                dec!(0.786),
            ],
            vec![dec!(1.272), dec!(1.618), dec!(2.618)],
            (dec!(0.618), dec!(0.65)),
        )
        .unwrap()
    }

    fn piv(kind: PivotKind, index: usize, price: Decimal) -> Pivot {
        Pivot {
            kind,
            index,
            confirmed_at: index + 3,
            price,
        }
    }

    /// Vela de rango [l, h] que cierra en c.
    fn hlc(i: usize, h: i64, l: i64, c: i64) -> Candle {
        let d = Decimal::from;
        bar(i, d(c), d(h), d(l), d(c))
    }

    #[test]
    fn phi_is_exact_to_decimal_precision_and_the_ratios_come_from_it() {
        // φ² = φ + 1.
        assert!((PHI * PHI - PHI - Decimal::ONE).abs() < dec!(1e-26));
        let close = |a: Decimal, b: Decimal| (a - b).abs() < dec!(0.0005);
        let inv = Decimal::ONE / PHI;
        assert!(close(inv, dec!(0.618)));
        assert!(close(inv * inv, dec!(0.382)));
        assert!(close(inv * inv * inv, dec!(0.236)));
        assert!(close(PHI * PHI, dec!(2.618)));
        // √(1/φ) ≈ 0,786 y √φ ≈ 1,272.
        assert!(close(dec!(0.786) * dec!(0.786), inv));
        assert!(close(dec!(1.272) * dec!(1.272), PHI));
    }

    #[test]
    fn levels_of_an_up_leg_match_hand_calculation() {
        // A = 100 (bajo), B = 200 (alto): rango 100.
        let p = [
            piv(PivotKind::Low, 2, dec!(100)),
            piv(PivotKind::High, 10, dec!(200)),
        ];
        let legs = fib_legs(&p, &cfg());
        assert_eq!(legs.len(), 1);
        let l = &legs[0];
        assert_eq!(l.direction, LegDirection::Up);
        assert_eq!(l.confirmed_at(), 13);
        assert_eq!(l.retracements[3], (dec!(0.618), dec!(138.2)));
        assert_eq!(l.retracements[2], (dec!(0.5), dec!(150)));
        assert_eq!(l.extensions[1], (dec!(1.618), dec!(261.8)));
        assert_eq!(l.golden_pocket, (dec!(135), dec!(138.2)));
    }

    #[test]
    fn a_down_leg_is_the_mirror() {
        let p = [
            piv(PivotKind::High, 2, dec!(200)),
            piv(PivotKind::Low, 10, dec!(100)),
        ];
        let l = &fib_legs(&p, &cfg())[0];
        assert_eq!(l.direction, LegDirection::Down);
        assert_eq!(l.retracements[3], (dec!(0.618), dec!(161.8)));
        assert_eq!(l.extensions[0], (dec!(1.272), dec!(72.8)));
        assert_eq!(l.golden_pocket, (dec!(161.8), dec!(165)));
    }

    #[test]
    fn no_leg_without_an_opposite_pivot_or_with_an_inverted_range() {
        let same = [
            piv(PivotKind::Low, 2, dec!(100)),
            piv(PivotKind::Low, 10, dec!(90)),
        ];
        assert!(fib_legs(&same, &cfg()).is_empty());
        // Un "alto" por debajo del bajo anterior no es un tramo alcista.
        let inverted = [
            piv(PivotKind::Low, 2, dec!(100)),
            piv(PivotKind::High, 10, dec!(90)),
        ];
        assert!(fib_legs(&inverted, &cfg()).is_empty());
        assert!(fib_legs(&[], &cfg()).is_empty());
    }

    /// Sube de 100 a 200 (pivote alto en 6, confirmado en 9 con N = 3) y vuelve.
    fn up_then_back(tail: &[(i64, i64, i64)]) -> Vec<Candle> {
        let mut v: Vec<Candle> = [
            (110, 100, 105), // 0: pivote bajo (100)
            (120, 104, 115),
            (130, 108, 125),
            (150, 120, 145),
            (170, 140, 165),
            (190, 160, 185),
            (200, 180, 190), // 6: pivote alto (200)
            (195, 175, 180),
            (190, 170, 175),
            (185, 165, 170), // 9: confirma el alto
        ]
        .iter()
        .enumerate()
        .map(|(i, &(h, l, c))| hlc(i, h, l, c))
        .collect();
        for &(h, l, c) in tail {
            v.push(hlc(v.len(), h, l, c));
        }
        v
    }

    fn events(c: &[Candle]) -> Vec<PocketEvent> {
        // El bajo de la vela 0 no tiene 3 velas antes: se agrega a mano el pivote de inicio.
        let mut p = vec![piv(PivotKind::Low, 0, dec!(100))];
        p.extend(confirmed_pivots(c, 3).into_iter().filter(|x| x.index > 0));
        let legs = fib_legs(&p, &cfg());
        golden_pocket_events(c, &legs)
    }

    #[test]
    fn touch_and_close_inside_the_golden_pocket() {
        // Zona: 135 – 138,2. Vela 10 toca sin cerrar adentro, vela 11 cierra adentro.
        let c = up_then_back(&[(170, 136, 150), (150, 134, 137)]);
        let e = events(&c);
        assert_eq!(
            e.iter()
                .map(|x| (x.index, x.touches, x.closes_inside))
                .collect::<Vec<_>>(),
            vec![(10, true, false), (11, true, true)]
        );
    }

    #[test]
    fn nothing_before_the_leg_is_confirmed() {
        // La vela 9 (la de confirmación) toca la zona, pero el tramo recién existe al cerrarla.
        let mut c = up_then_back(&[]);
        c[9] = hlc(9, 185, 137, 170);
        assert!(events(&c).is_empty());
    }

    #[test]
    fn a_close_beyond_the_start_breaks_the_leg() {
        // Vela 10 cierra debajo de 100: el tramo deja de valer; la 11 toca la zona y no cuenta.
        let c = up_then_back(&[(150, 95, 99), (140, 130, 137)]);
        assert!(events(&c).is_empty());
    }

    #[test]
    fn events_never_change_when_new_candles_arrive() {
        let c = up_then_back(&[
            (170, 136, 150),
            (150, 134, 137),
            (160, 140, 155),
            (150, 120, 125),
            (140, 90, 95),
            (139, 120, 137),
        ]);
        let full = events(&c);
        for len in 1..c.len() {
            let part = events(&c[..len]);
            let expected: Vec<_> = full.iter().filter(|x| x.index < len).copied().collect();
            assert_eq!(part, expected, "con {len} velas");
        }
    }

    #[test]
    fn rejects_invalid_config() {
        let ok = cfg();
        assert!(FibConfig::new(vec![], ok.extensions.clone(), ok.golden_pocket).is_err());
        assert!(
            FibConfig::new(
                vec![dec!(0.618), dec!(0.382)],
                ok.extensions.clone(),
                ok.golden_pocket
            )
            .is_err()
        );
        assert!(
            FibConfig::new(ok.retracements.clone(), vec![dec!(0.9)], ok.golden_pocket).is_err()
        );
        assert!(
            FibConfig::new(
                ok.retracements.clone(),
                ok.extensions.clone(),
                (dec!(0.65), dec!(0.618))
            )
            .is_err()
        );
    }
}

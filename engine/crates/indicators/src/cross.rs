//! Cruce dorado y cruce de la muerte (#491). Herramienta de **análisis**: no entra al puntaje.
//!
//! ```text
//! cruce dorado en t:       rápida[t] > lenta[t]  y la última vez que fueron distintas, rápida < lenta
//! cruce de la muerte en t:  el espejo
//! ```
//! Sobre dos medias ya calculadas (por ejemplo SMA 50 y SMA 200 de los cierres diarios). Un toque
//! (rápida = lenta) que vuelve al mismo lado no es cruce. Sin las dos medias (calentamiento o un
//! hueco) se empieza de nuevo. El cruce de `t` solo depende de `0..=t`: no repinta.

use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossKind {
    Golden,
    Death,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cross {
    pub index: usize,
    pub kind: CrossKind,
}

/// Cruces de `fast` sobre `slow` (alineadas por índice).
pub fn crosses(fast: &[Option<Decimal>], slow: &[Option<Decimal>]) -> Vec<Cross> {
    let mut out = Vec::new();
    // Lado de la última vela en que las medias eran distintas (`true` = rápida arriba).
    let mut side: Option<bool> = None;
    for (t, (f, s)) in fast.iter().zip(slow).enumerate() {
        let (Some(f), Some(s)) = (f, s) else {
            side = None;
            continue;
        };
        if f == s {
            continue;
        }
        let above = f > s;
        if side.is_some_and(|prev| prev != above) {
            let kind = if above {
                CrossKind::Golden
            } else {
                CrossKind::Death
            };
            out.push(Cross { index: t, kind });
        }
        side = Some(above);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[Option<i64>]) -> Vec<Option<Decimal>> {
        v.iter().map(|x| x.map(Decimal::from)).collect()
    }

    #[test]
    fn golden_and_death_crosses() {
        let fast = s(&[Some(1), Some(2), Some(4), Some(5), Some(3)]);
        let slow = s(&[Some(3), Some(3), Some(3), Some(4), Some(4)]);
        assert_eq!(
            crosses(&fast, &slow),
            vec![
                Cross {
                    index: 2,
                    kind: CrossKind::Golden
                },
                Cross {
                    index: 4,
                    kind: CrossKind::Death
                }
            ]
        );
    }

    #[test]
    fn touching_without_crossing_is_not_a_cross_and_warmup_is_skipped() {
        // Toca (3 = 3) y vuelve abajo: no cruza.
        let fast = s(&[Some(2), Some(3), Some(2)]);
        let slow = s(&[Some(3), Some(3), Some(3)]);
        assert!(crosses(&fast, &slow).is_empty());
        // Toca y después cruza: el cruce es en la vela que pasa al otro lado.
        let fast2 = s(&[Some(2), Some(3), Some(4)]);
        assert_eq!(
            crosses(&fast2, &slow),
            vec![Cross {
                index: 2,
                kind: CrossKind::Golden
            }]
        );
        // Sin la lenta en t − 1 (calentamiento) no hay cruce, aunque en t esté arriba.
        let fast = s(&[Some(1), Some(5)]);
        let slow = s(&[None, Some(3)]);
        assert!(crosses(&fast, &slow).is_empty());
        assert!(crosses(&[], &[]).is_empty());
    }

    #[test]
    fn crosses_never_change_when_new_values_arrive() {
        let fast = s(&[Some(1), Some(4), Some(2), Some(5), Some(1), Some(6)]);
        let slow = s(&[None, Some(3), Some(3), Some(3), Some(3), Some(3)]);
        let full = crosses(&fast, &slow);
        for len in 0..fast.len() {
            let part = crosses(&fast[..len], &slow[..len]);
            let expected: Vec<_> = full.iter().filter(|c| c.index < len).copied().collect();
            assert_eq!(part, expected);
        }
    }
}

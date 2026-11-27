//! "Precio de fondo" (#491). No es un término técnico estándar: lo usamos como el **mínimo que
//! cierra una bajada** (el precio "toca fondo"). Herramienta de **análisis**: no entra al puntaje.
//!
//! Construye sobre los pivotes confirmados (`pivots`); el doble suelo (`double`) son dos fondos
//! parecidos, y el spring (`wyckoff`) pasa justamente en el fondo de un rango.
//!
//! ```text
//! P = pivote bajo confirmado en la vela i, ATR = ATR de Wilder en i
//! bajada previa:  D = máx(H[i−lookback..i]) − P,  con  D ≥ X % · ese máximo   o   D ≥ Y · ATR
//! rebote:         k = primera vela en i+1..=i+N con C[k] ≥ P + Z · ATR
//! señal:          en máx(confirmación del pivote, k)
//! ```
//! Calentamiento: sin `lookback` velas antes del pivote o sin ATR no hay fondo. Todo depende solo
//! de las velas hasta la señal (sin repintado, hay test).

use cfds_core::Candle;
use cfds_indicators::wilder_atr;
use rust_decimal::Decimal;

use crate::pivots::{Pivot, PivotKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BottomConfig {
    /// Velas antes del pivote donde se busca el máximo del que bajó.
    pub lookback: usize,
    /// Bajada mínima como fracción del máximo (1 % = 0,01) …
    pub min_decline: Decimal,
    /// … o en múltiplos del ATR (alcanza con una de las dos).
    pub min_decline_atr: Decimal,
    pub atr_period: usize,
    /// Rebote mínimo desde el fondo, en múltiplos del ATR.
    pub min_rebound_atr: Decimal,
    /// Velas después del pivote para lograr el rebote.
    pub max_bars_to_rebound: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bottom {
    pub pivot: Pivot,
    /// Máximo previo del que bajó.
    pub prior_high: Decimal,
    /// Vela del rebote.
    pub rebound_index: usize,
    /// Vela de la señal: la última entre la confirmación del pivote y el rebote.
    pub confirmed_at: usize,
}

/// Fondos de la serie, en orden de pivote.
pub fn bottoms(candles: &[Candle], pivots: &[Pivot], cfg: &BottomConfig) -> Vec<Bottom> {
    let atr = wilder_atr(candles, cfg.atr_period);
    let mut out = Vec::new();
    for p in pivots.iter().filter(|p| p.kind == PivotKind::Low) {
        let i = p.index;
        let (Some(Some(a)), true) = (atr.get(i), i >= cfg.lookback) else {
            continue;
        };
        let Some(prior_high) = candles[i - cfg.lookback..i].iter().map(|c| c.high).max() else {
            continue;
        };
        let decline = prior_high - p.price;
        if decline < cfg.min_decline * prior_high && decline < cfg.min_decline_atr * *a {
            continue;
        }
        let target = p.price + cfg.min_rebound_atr * *a;
        let last = (i + cfg.max_bars_to_rebound).min(candles.len().saturating_sub(1));
        if let Some(k) = (i + 1..=last).find(|&k| candles[k].close >= target) {
            out.push(Bottom {
                pivot: *p,
                prior_high,
                rebound_index: k,
                confirmed_at: k.max(p.confirmed_at),
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
    use rust_decimal::dec;

    fn cfg() -> BottomConfig {
        BottomConfig {
            lookback: 5,
            min_decline: dec!(0.05),
            min_decline_atr: dec!(100),
            atr_period: 2,
            min_rebound_atr: dec!(1),
            max_bars_to_rebound: 4,
        }
    }

    /// Velas de rango 2 alrededor de cada cierre.
    fn closes(v: &[i64]) -> Vec<Candle> {
        v.iter()
            .enumerate()
            .map(|(i, &c)| {
                let c = Decimal::from(c);
                bar(i, c, c + Decimal::ONE, c - Decimal::ONE, c)
            })
            .collect()
    }

    fn found(c: &[Candle], cfg: &BottomConfig) -> Vec<Bottom> {
        bottoms(c, &confirmed_pivots(c, 2), cfg)
    }

    #[test]
    fn a_drop_then_a_rebound_is_a_bottom() {
        // Cae de 110 a 100 (≈ 10 %), fondo en la vela 6, rebota a 104.
        let c = closes(&[110, 109, 108, 106, 104, 102, 100, 101, 104, 105]);
        let b = found(&c, &cfg());
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].pivot.index, 6);
        assert_eq!(b[0].prior_high, dec!(110)); // máximo de las velas 1..=5
        // ATR(2) en 6 = 2,9375: el rebote pide cierre ≥ 99 + 2,9375 → vela 8, la misma en la
        // que se confirma el pivote.
        assert_eq!((b[0].rebound_index, b[0].confirmed_at), (8, 8));
    }

    #[test]
    fn a_small_dip_is_not_a_bottom() {
        let c = closes(&[102, 102, 101, 101, 101, 101, 100, 101, 104, 105]);
        assert!(found(&c, &cfg()).is_empty());
        // Con el umbral en ATR bajo, la misma caída sí alcanza.
        let atr = BottomConfig {
            min_decline_atr: dec!(1),
            ..cfg()
        };
        assert_eq!(found(&c, &atr).len(), 1);
    }

    #[test]
    fn no_rebound_in_time_no_bottom() {
        let c = closes(&[
            110, 109, 108, 106, 104, 102, 99, 100, 102, 100, 100, 100, 100,
        ]);
        let tight = BottomConfig {
            min_rebound_atr: dec!(3),
            ..cfg()
        };
        assert_eq!(
            found(&c, &cfg()).len(),
            1,
            "con el rebote normal sí hay fondo"
        );
        assert!(found(&c, &tight).is_empty());
    }

    #[test]
    fn warmup_and_few_candles() {
        // El pivote está en la vela 2: no hay 5 velas antes.
        let c = closes(&[104, 102, 100, 102, 104]);
        assert!(found(&c, &cfg()).is_empty());
        assert!(found(&[], &cfg()).is_empty());
    }

    #[test]
    fn bottoms_never_change_when_new_candles_arrive() {
        let all = closes(&[
            110, 109, 108, 106, 104, 102, 100, 101, 104, 105, 103, 100, 97, 95, 96, 99, 102,
        ]);
        let full = found(&all, &cfg());
        assert!(!full.is_empty());
        for len in 1..all.len() {
            let part = found(&all[..len], &cfg());
            let expected: Vec<_> = full
                .iter()
                .filter(|b| b.confirmed_at < len)
                .copied()
                .collect();
            assert_eq!(part, expected, "con {len} velas");
        }
    }
}

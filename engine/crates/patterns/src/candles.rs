//! Patrones de velas (spec §5.1), evaluados en la vela **cerrada** `t`.
//!
//! ```text
//! cuerpo = |C − O|;  rango = H − L;  mecha_inf = min(O,C) − L;  mecha_sup = H − max(O,C)
//!
//! MARTILLO (largo):         C[t−1] < C[t−1−k]  (caída previa; con k = 5: C[t−6])
//!                           mecha_inf ≥ 2·cuerpo, mecha_sup ≤ 0,3·rango,
//!                           cuerpo ≥ 0,05·rango, rango ≥ 0,5·ATR[t]
//! ESTRELLA FUGAZ (corto):   el espejo: subida previa y mecha_sup larga
//! ENVOLVENTE ALCISTA:       vela t−1 bajista, vela t alcista, O[t] ≤ C[t−1], C[t] ≥ O[t−1],
//!                           cuerpo[t] > cuerpo[t−1]
//! ENVOLVENTE BAJISTA:       el espejo
//! ```
//! Los umbrales están en `config/patterns.toml`. Una vela sin rango (H = L) nunca es patrón.

use cfds_core::Candle;
use rust_decimal::Decimal;

use crate::config::CandleConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CandlePattern {
    Hammer,
    ShootingStar,
    BullishEngulfing,
    BearishEngulfing,
}

impl CandlePattern {
    /// `true` si es una señal de compra (largo); `false` si es de venta (corto).
    pub fn is_long(self) -> bool {
        matches!(
            self,
            CandlePattern::Hammer | CandlePattern::BullishEngulfing
        )
    }
}

struct Shape {
    body: Decimal,
    range: Decimal,
    lower_wick: Decimal,
    upper_wick: Decimal,
}

fn shape(c: &Candle) -> Shape {
    Shape {
        body: (c.close - c.open).abs(),
        range: c.high - c.low,
        lower_wick: c.open.min(c.close) - c.low,
        upper_wick: c.high - c.open.max(c.close),
    }
}

/// Patrones presentes en la vela `t`. `atr` es el ATR alineado por índice con `candles`
/// (de `cfds_indicators::wilder_atr`); sin ATR (calentamiento) no hay martillo ni estrella.
pub fn candle_patterns_at(
    candles: &[Candle],
    t: usize,
    atr: &[Option<Decimal>],
    cfg: &CandleConfig,
) -> Vec<CandlePattern> {
    let mut out = Vec::new();
    let Some(c) = candles.get(t) else {
        return out;
    };
    let s = shape(c);
    if s.range <= Decimal::ZERO {
        return out;
    }

    // Martillo y estrella fugaz: necesitan la vela t−1−k y el ATR de t.
    if let (Some(prev), Some(older), Some(Some(a))) = (
        t.checked_sub(1).and_then(|i| candles.get(i)),
        t.checked_sub(1 + cfg.prior_move_lookback)
            .and_then(|i| candles.get(i)),
        atr.get(t),
    ) {
        let sized =
            s.body >= cfg.min_body_to_range * s.range && s.range >= cfg.min_range_to_atr * *a;
        let long_lower = s.lower_wick >= cfg.min_wick_to_body * s.body
            && s.upper_wick <= cfg.max_opposite_wick_to_range * s.range;
        let long_upper = s.upper_wick >= cfg.min_wick_to_body * s.body
            && s.lower_wick <= cfg.max_opposite_wick_to_range * s.range;
        if sized && long_lower && prev.close < older.close {
            out.push(CandlePattern::Hammer);
        }
        if sized && long_upper && prev.close > older.close {
            out.push(CandlePattern::ShootingStar);
        }
    }

    // Envolventes: solo necesitan la vela anterior.
    if let Some(p) = t.checked_sub(1).and_then(|i| candles.get(i)) {
        let prev_body = (p.close - p.open).abs();
        if p.close < p.open
            && c.close > c.open
            && c.open <= p.close
            && c.close >= p.open
            && s.body > prev_body
        {
            out.push(CandlePattern::BullishEngulfing);
        }
        if p.close > p.open
            && c.close < c.open
            && c.open >= p.close
            && c.close <= p.open
            && s.body > prev_body
        {
            out.push(CandlePattern::BearishEngulfing);
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::config::PatternsConfig;
    use cfds_core::Timeframe;
    use chrono::{DateTime, TimeDelta, Utc};
    use rust_decimal::dec;

    pub(crate) fn bar(i: usize, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
        let t0: DateTime<Utc> = "2026-11-19T14:30:00Z".parse().unwrap();
        Candle {
            instrument_id: "US100".parse().unwrap(),
            timeframe: Timeframe::M15,
            start: t0 + TimeDelta::minutes(15 * i as i64),
            open: o,
            high: h,
            low: l,
            close: c,
            volume: None,
        }
    }

    fn cfg() -> CandleConfig {
        PatternsConfig::from_toml(include_str!("../../../../config/patterns.toml"))
            .unwrap()
            .candles
    }

    /// 6 velas de contexto con cierres `closes` (velas chicas, sin patrón), más la vela `last`.
    fn with_context(closes: [i64; 6], last: (Decimal, Decimal, Decimal, Decimal)) -> Vec<Candle> {
        let mut v: Vec<Candle> = closes
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                let c = Decimal::from(c);
                bar(i, c, c + dec!(0.5), c - dec!(0.5), c)
            })
            .collect();
        v.push(bar(6, last.0, last.1, last.2, last.3));
        v
    }

    /// ATR fijo de 10 en la vela evaluada (los otros valores no se usan).
    fn atr10(n: usize) -> Vec<Option<Decimal>> {
        vec![Some(dec!(10)); n]
    }

    const FALLING: [i64; 6] = [110, 108, 106, 104, 102, 100];
    const RISING: [i64; 6] = [90, 92, 94, 96, 98, 100];

    #[test]
    fn detects_a_hammer_after_a_fall() {
        // Cuerpo 1 arriba (100 → 101), mecha inferior 9, mecha superior 0,5. Rango 10,5 ≥ 0,5·ATR.
        let c = with_context(FALLING, (dec!(100), dec!(101.5), dec!(91), dec!(101)));
        assert_eq!(
            candle_patterns_at(&c, 6, &atr10(7), &cfg()),
            vec![CandlePattern::Hammer]
        );
    }

    #[test]
    fn the_same_shape_after_a_rise_is_not_a_hammer() {
        let c = with_context(RISING, (dec!(100), dec!(101.5), dec!(91), dec!(101)));
        assert!(!candle_patterns_at(&c, 6, &atr10(7), &cfg()).contains(&CandlePattern::Hammer));
    }

    #[test]
    fn detects_a_shooting_star_after_a_rise() {
        // El espejo: cuerpo abajo, mecha superior larga.
        let c = with_context(RISING, (dec!(101), dec!(110), dec!(99.5), dec!(100)));
        assert_eq!(
            candle_patterns_at(&c, 6, &atr10(7), &cfg()),
            vec![CandlePattern::ShootingStar]
        );
    }

    #[test]
    fn a_tiny_candle_relative_to_atr_is_not_a_hammer() {
        // Misma forma, pero rango 1,05 < 0,5·ATR(10) = 5.
        let c = with_context(FALLING, (dec!(100), dec!(100.15), dec!(99.1), dec!(100.1)));
        assert!(candle_patterns_at(&c, 6, &atr10(7), &cfg()).is_empty());
    }

    #[test]
    fn without_atr_yet_there_is_no_hammer() {
        let c = with_context(FALLING, (dec!(100), dec!(101.5), dec!(91), dec!(101)));
        let warmup = vec![None; 7];
        assert!(candle_patterns_at(&c, 6, &warmup, &cfg()).is_empty());
    }

    #[test]
    fn detects_engulfing_both_ways() {
        let bull = vec![
            bar(0, dec!(105), dec!(106), dec!(99), dec!(100)), // bajista, cuerpo 5
            bar(1, dec!(99), dec!(108), dec!(98), dec!(107)),  // alcista, envuelve, cuerpo 8
        ];
        assert_eq!(
            candle_patterns_at(&bull, 1, &atr10(2), &cfg()),
            vec![CandlePattern::BullishEngulfing]
        );
        let bear = vec![
            bar(0, dec!(100), dec!(106), dec!(99), dec!(105)),
            bar(1, dec!(106), dec!(107), dec!(97), dec!(98)),
        ];
        assert_eq!(
            candle_patterns_at(&bear, 1, &atr10(2), &cfg()),
            vec![CandlePattern::BearishEngulfing]
        );
    }

    #[test]
    fn a_candle_that_does_not_cover_the_previous_body_is_not_engulfing() {
        let c = vec![
            bar(0, dec!(105), dec!(106), dec!(99), dec!(100)),
            bar(1, dec!(101), dec!(108), dec!(100), dec!(104)), // abre arriba del cierre anterior
        ];
        assert!(candle_patterns_at(&c, 1, &atr10(2), &cfg()).is_empty());
    }

    #[test]
    fn a_flat_candle_is_never_a_pattern() {
        let c = with_context(FALLING, (dec!(100), dec!(100), dec!(100), dec!(100)));
        assert!(candle_patterns_at(&c, 6, &atr10(7), &cfg()).is_empty());
    }

    /// Patrones de la vela 6 después de una caída (ATR = 10).
    fn after_fall(o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Vec<CandlePattern> {
        candle_patterns_at(&with_context(FALLING, (o, h, l, c)), 6, &atr10(7), &cfg())
    }

    // Spec §5.1, una condición del martillo por test: la vela queda justo en el borde (pasa) y
    // después se corre 0,01 (no pasa). Todo lo demás de la vela cumple con margen.

    #[test]
    fn hammer_lower_wick_of_exactly_twice_the_body() {
        // Cuerpo 4 (100 → 104), mecha inferior 8 = 2 × 4.
        assert_eq!(
            after_fall(dec!(100), dec!(104), dec!(92), dec!(104)),
            vec![CandlePattern::Hammer]
        );
        assert!(after_fall(dec!(100), dec!(104), dec!(92.01), dec!(104)).is_empty());
    }

    #[test]
    fn hammer_upper_wick_of_at_most_30_percent_of_the_range() {
        // Rango 20 (87 → 107), mecha superior 6 = 0,3 × 20.
        assert_eq!(
            after_fall(dec!(100), dec!(107), dec!(87), dec!(101)),
            vec![CandlePattern::Hammer]
        );
        assert!(after_fall(dec!(100), dec!(107.01), dec!(87), dec!(101)).is_empty());
    }

    #[test]
    fn hammer_body_of_at_least_5_percent_of_the_range() {
        // Rango 20 (81 → 101), cuerpo 1 = 0,05 × 20.
        assert_eq!(
            after_fall(dec!(100), dec!(101), dec!(81), dec!(101)),
            vec![CandlePattern::Hammer]
        );
        assert!(after_fall(dec!(100), dec!(101), dec!(80.99), dec!(101)).is_empty());
    }

    #[test]
    fn hammer_range_of_at_least_half_the_atr() {
        // Rango 5 (96 → 101) = 0,5 × ATR(10).
        assert_eq!(
            after_fall(dec!(100), dec!(101), dec!(96), dec!(101)),
            vec![CandlePattern::Hammer]
        );
        assert!(after_fall(dec!(100), dec!(101), dec!(96.01), dec!(101)).is_empty());
    }

    #[test]
    fn hammer_prior_fall_is_strict() {
        // caída_previa = C[t−1] < C[t−6]: con los dos cierres iguales no hay caída.
        let hammer = (dec!(100), dec!(101.5), dec!(91), dec!(101));
        let flat = with_context([100, 102, 101, 103, 104, 100], hammer);
        assert!(candle_patterns_at(&flat, 6, &atr10(7), &cfg()).is_empty());
        let mut fell = flat.clone();
        fell[0].close = dec!(100.01);
        assert_eq!(
            candle_patterns_at(&fell, 6, &atr10(7), &cfg()),
            vec![CandlePattern::Hammer]
        );
    }

    /// Envolvente sobre una vela anterior bajista de 105 → 100 (cuerpo 5).
    fn over_bearish(o: Decimal, c: Decimal) -> Vec<CandlePattern> {
        let v = vec![
            bar(0, dec!(105), dec!(106), dec!(99), dec!(100)),
            bar(1, o, o.max(c) + dec!(1), o.min(c) - dec!(1), c),
        ];
        candle_patterns_at(&v, 1, &atr10(2), &cfg())
    }

    #[test]
    fn engulfing_open_and_close_edges_are_inclusive() {
        // O[t] = C[t−1] vale; un centésimo arriba, no.
        assert_eq!(
            over_bearish(dec!(100), dec!(106)),
            vec![CandlePattern::BullishEngulfing]
        );
        assert!(over_bearish(dec!(100.01), dec!(106)).is_empty());
        // C[t] = O[t−1] vale; un centésimo abajo, no.
        assert_eq!(
            over_bearish(dec!(99), dec!(105)),
            vec![CandlePattern::BullishEngulfing]
        );
        assert!(over_bearish(dec!(99), dec!(104.99)).is_empty());
    }

    #[test]
    fn engulfing_needs_a_strictly_bigger_body() {
        // Cubre la vela anterior justo en los dos bordes, pero el cuerpo es igual (5 = 5).
        assert!(over_bearish(dec!(100), dec!(105)).is_empty());
    }

    #[test]
    fn engulfing_needs_a_bearish_candle_before() {
        let v = vec![
            bar(0, dec!(100), dec!(106), dec!(99), dec!(100)), // sin cuerpo
            bar(1, dec!(99), dec!(108), dec!(98), dec!(107)),
        ];
        assert!(candle_patterns_at(&v, 1, &atr10(2), &cfg()).is_empty());
    }

    #[test]
    fn long_and_short_patterns_are_labeled() {
        assert!(CandlePattern::Hammer.is_long() && CandlePattern::BullishEngulfing.is_long());
        assert!(
            !CandlePattern::ShootingStar.is_long() && !CandlePattern::BearishEngulfing.is_long()
        );
    }
}

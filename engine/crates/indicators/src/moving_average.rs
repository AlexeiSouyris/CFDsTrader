//! Medias móviles sobre el **cierre** (spec §4).
//!
//! - **SMA(n)**: media aritmética de los últimos `n` cierres. Primer valor en el índice `n − 1`.
//! - **EMA(n)**: `EMA_t = EMA_{t−1} + α · (C_t − EMA_{t−1})`, con `α = 2 / (n + 1)`, sembrada con la
//!   SMA(n) de las primeras `n` velas (igual que `ta.ema` de TradingView). Primer valor en `n − 1`.
//! - Calentamiento: `None` antes de tener `n` velas. Sin repintado: el valor de `t` solo depende
//!   de las velas `0..=t`.

use cfds_core::Candle;
use rust_decimal::Decimal;

/// SMA(`period`) de los cierres, alineada por índice.
///
/// # Panics
/// Si `period` es 0 (error de configuración que se rechaza al cargar `config/`).
pub fn sma(candles: &[Candle], period: usize) -> Vec<Option<Decimal>> {
    sma_values(&closes_of(candles), period)
}

/// EMA(`period`) de los cierres, alineada por índice.
///
/// # Panics
/// Si `period` es 0.
pub fn ema(candles: &[Candle], period: usize) -> Vec<Option<Decimal>> {
    ema_values(&closes_of(candles), period)
}

fn closes_of(candles: &[Candle]) -> Vec<Decimal> {
    candles.iter().map(|c| c.close).collect()
}

/// SMA(`period`) de una serie de valores (por ejemplo, los cierres de las velas diarias).
///
/// # Panics
/// Si `period` es 0.
pub fn sma_values(values: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    assert!(period > 0, "el período de la SMA tiene que ser mayor que 0");
    let n = Decimal::from(period);
    let mut out = Vec::with_capacity(values.len());
    let mut sum = Decimal::ZERO;
    for (i, v) in values.iter().enumerate() {
        sum += *v;
        if i >= period {
            sum -= values[i - period];
        }
        out.push((i + 1 >= period).then(|| sum / n));
    }
    out
}

/// EMA(`period`) de una serie de valores, con la misma semilla y el mismo α que [`ema`].
///
/// # Panics
/// Si `period` es 0.
pub fn ema_values(values: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    assert!(period > 0, "el período de la EMA tiene que ser mayor que 0");
    let alpha = Decimal::TWO / Decimal::from(period + 1);
    let seed = sma_values(values, period);
    let mut out = Vec::with_capacity(values.len());
    let mut prev: Option<Decimal> = None;
    for (i, v) in values.iter().enumerate() {
        prev = match prev {
            Some(p) => Some(p + alpha * (*v - p)),
            None => seed[i],
        };
        out.push(prev);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use cfds_core::Timeframe;
    use chrono::{DateTime, TimeDelta, Utc};
    use rust_decimal::dec;

    /// Velas de 15m consecutivas con los cierres dados (O = H = L = C).
    pub(crate) fn closes(values: &[Decimal]) -> Vec<Candle> {
        let t0: DateTime<Utc> = "2026-11-19T14:30:00Z".parse().unwrap();
        values
            .iter()
            .enumerate()
            .map(|(i, &c)| Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * i as i64),
                open: c,
                high: c,
                low: c,
                close: c,
                volume: None,
            })
            .collect()
    }

    fn ints(v: &[i64]) -> Vec<Decimal> {
        v.iter().map(|&x| Decimal::from(x)).collect()
    }

    #[test]
    fn sma_matches_hand_calculation() {
        let c = closes(&ints(&[1, 2, 3, 4, 5]));
        assert_eq!(
            sma(&c, 3),
            vec![None, None, Some(dec!(2)), Some(dec!(3)), Some(dec!(4))]
        );
    }

    #[test]
    fn ema_is_seeded_with_the_sma_and_uses_alpha_two_over_n_plus_one() {
        // n = 3 → α = 0,5. Semilla en el índice 2: SMA(1,2,3) = 2.
        // t=3: 2 + 0,5·(4 − 2) = 3. t=4: 3 + 0,5·(10 − 3) = 6,5.
        let c = closes(&ints(&[1, 2, 3, 4, 10]));
        assert_eq!(
            ema(&c, 3),
            vec![None, None, Some(dec!(2)), Some(dec!(3)), Some(dec!(6.5))]
        );
    }

    #[test]
    fn warmup_is_none_until_enough_candles() {
        let c = closes(&ints(&[5; 49]));
        assert!(sma(&c, 50).iter().all(Option::is_none));
        assert!(ema(&c, 50).iter().all(Option::is_none));
        let c = closes(&ints(&[5; 50]));
        assert_eq!(sma(&c, 50)[49], Some(dec!(5)));
        assert_eq!(ema(&c, 50)[49], Some(dec!(5)));
    }

    #[test]
    fn values_never_change_when_new_candles_arrive() {
        let all = closes(&ints(&[3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8]));
        let (full_sma, full_ema) = (sma(&all, 4), ema(&all, 4));
        for len in 1..all.len() {
            assert_eq!(sma(&all[..len], 4), full_sma[..len]);
            assert_eq!(ema(&all[..len], 4), full_ema[..len]);
        }
    }
}

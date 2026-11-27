//! ATR (Average True Range) con suavizado de **Wilder**, igual que `ta.atr` de TradingView.
//!
//! - True Range: `TR_t = max(H_t − L_t, |H_t − C_{t−1}|, |L_t − C_{t−1}|)`.
//!   En la primera vela no hay cierre previo, así que `TR_0 = H_0 − L_0`.
//! - ATR (RMA de Wilder): el primer valor, en el índice `n − 1`, es el promedio simple
//!   de los primeros `n` TR. Después: `ATR_t = (ATR_{t−1} · (n − 1) + TR_t) / n`.
//! - Calentamiento: las primeras `n − 1` velas devuelven `None`.
//! - Sin repintado: el valor de la vela `t` solo depende de las velas `0..=t`.
//! - Sirve para cualquier timeframe: velas de 15m y 1h ([`Candle`]) o diarias ([`DailyCandle`],
//!   #148). Es la misma fórmula, sin copias.

use cfds_core::{Candle, DailyCandle};
use rust_decimal::Decimal;

/// Máximo, mínimo y cierre de una vela: lo único que usan el True Range y el ATR.
pub trait Hlc {
    fn high(&self) -> Decimal;
    fn low(&self) -> Decimal;
    fn close(&self) -> Decimal;
}

impl Hlc for Candle {
    fn high(&self) -> Decimal {
        self.high
    }
    fn low(&self) -> Decimal {
        self.low
    }
    fn close(&self) -> Decimal {
        self.close
    }
}

impl Hlc for DailyCandle {
    fn high(&self) -> Decimal {
        self.high
    }
    fn low(&self) -> Decimal {
        self.low
    }
    fn close(&self) -> Decimal {
        self.close
    }
}

/// True Range de cada vela, alineado por índice.
pub fn true_range<T: Hlc>(candles: &[T]) -> Vec<Decimal> {
    candles
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let range = c.high() - c.low();
            match i.checked_sub(1).map(|p| candles[p].close()) {
                None => range,
                Some(prev_close) => range
                    .max((c.high() - prev_close).abs())
                    .max((c.low() - prev_close).abs()),
            }
        })
        .collect()
}

/// ATR de Wilder de período `period`, alineado por índice (`None` durante el calentamiento).
///
/// # Panics
/// Si `period` es 0: es un error de configuración que se rechaza al cargar `config/`.
pub fn wilder_atr<T: Hlc>(candles: &[T], period: usize) -> Vec<Option<Decimal>> {
    assert!(period > 0, "el período del ATR tiene que ser mayor que 0");
    let n = Decimal::from(period);
    let tr = true_range(candles);
    let mut out = Vec::with_capacity(tr.len());
    let mut atr: Option<Decimal> = None;
    for (i, value) in tr.iter().enumerate() {
        atr = match atr {
            Some(prev) => Some((prev * (n - Decimal::ONE) + value) / n),
            None if i + 1 == period => Some(tr[..period].iter().sum::<Decimal>() / n),
            None => None,
        };
        out.push(atr);
    }
    out
}

/// Estado del ATR de Wilder **después** de una vela: su cierre y su ATR ya calculado. Con esto
/// la serie sigue igual sin las velas anteriores (puntos de control del broker, #463).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AtrSeed {
    pub prev_close: Decimal,
    pub prev_atr: Decimal,
}

/// El ATR de Wilder de `candles` como continuación de una serie ya calentada: `seed` es el
/// estado después de la vela anterior a `candles[0]`. Da exactamente los mismos valores que
/// [`wilder_atr`] sobre la serie completa (misma fórmula; sin calentamiento porque ya pasó).
///
/// # Panics
/// Si `period` es 0 (error de configuración, como en [`wilder_atr`]).
pub fn wilder_atr_after<T: Hlc>(
    seed: AtrSeed,
    candles: &[T],
    period: usize,
) -> Vec<Option<Decimal>> {
    assert!(period > 0, "el período del ATR tiene que ser mayor que 0");
    let n = Decimal::from(period);
    let mut prev_close = seed.prev_close;
    let mut atr = seed.prev_atr;
    candles
        .iter()
        .map(|c| {
            let tr = (c.high() - c.low())
                .max((c.high() - prev_close).abs())
                .max((c.low() - prev_close).abs());
            atr = (atr * (n - Decimal::ONE) + tr) / n;
            prev_close = c.close();
            Some(atr)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfds_core::Timeframe;
    use chrono::{DateTime, TimeDelta, Utc};
    use rust_decimal::dec;

    /// Velas de 15m consecutivas a partir de (H, L, C); la apertura no influye en el ATR.
    fn series(hlc: &[(Decimal, Decimal, Decimal)]) -> Vec<Candle> {
        let t0: DateTime<Utc> = "2026-11-19T14:30:00Z".parse().unwrap();
        hlc.iter()
            .enumerate()
            .map(|(i, &(high, low, close))| Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * i as i64),
                open: close,
                high,
                low,
                close,
                volume: None,
            })
            .collect()
    }

    #[test]
    fn true_range_uses_previous_close_on_gaps() {
        let c = series(&[
            (dec!(105), dec!(100), dec!(102)),
            (dec!(104), dec!(101), dec!(103)), // dentro del rango: TR = H − L = 3
            (dec!(112), dec!(110), dec!(111)), // gap alcista: TR = H − C₋₁ = 112 − 103 = 9
            (dec!(108), dec!(106), dec!(107)), // gap bajista: TR = |L − C₋₁| = |106 − 111| = 5
        ]);
        assert_eq!(true_range(&c), vec![dec!(5), dec!(3), dec!(9), dec!(5)]);
    }

    #[test]
    fn wilder_atr_matches_hand_calculation() {
        // TR = 5, 3, 9, 5. Con n = 3:
        //   ATR[2] = (5 + 3 + 9) / 3 = 17/3
        //   ATR[3] = (17/3 · 2 + 5) / 3 = 49/9
        let c = series(&[
            (dec!(105), dec!(100), dec!(102)),
            (dec!(104), dec!(101), dec!(103)),
            (dec!(112), dec!(110), dec!(111)),
            (dec!(108), dec!(106), dec!(107)),
        ]);
        let atr = wilder_atr(&c, 3);
        assert_eq!(atr[0], None);
        assert_eq!(atr[1], None);
        assert_eq!(atr[2], Some(dec!(17) / dec!(3)));
        assert_eq!(
            atr[3],
            Some((dec!(17) / dec!(3) * dec!(2) + dec!(5)) / dec!(3))
        );
    }

    #[test]
    fn warmup_returns_none_until_enough_candles() {
        let c = series(&[(dec!(2), dec!(1), dec!(1)); 13]);
        assert!(wilder_atr(&c, 14).iter().all(Option::is_none));
        let c = series(&[(dec!(2), dec!(1), dec!(1)); 14]);
        assert_eq!(wilder_atr(&c, 14)[13], Some(dec!(1)));
    }

    #[test]
    fn values_never_change_when_new_candles_arrive() {
        // Sin repintado: agregar velas no cambia ningún valor ya calculado.
        let all = series(&[
            (dec!(105), dec!(100), dec!(102)),
            (dec!(104), dec!(101), dec!(103)),
            (dec!(112), dec!(110), dec!(111)),
            (dec!(108), dec!(106), dec!(107)),
            (dec!(120), dec!(100), dec!(101)),
        ]);
        let full = wilder_atr(&all, 2);
        for len in 1..all.len() {
            assert_eq!(wilder_atr(&all[..len], 2), full[..len]);
        }
    }

    #[test]
    fn continuing_from_a_seed_gives_the_same_values() {
        // #463: cortar la serie en cualquier punto ya calentado y seguir desde el estado de la
        // vela anterior da exactamente los mismos valores (sin redondeos distintos).
        let all = series(&[
            (dec!(105), dec!(100), dec!(102)),
            (dec!(104), dec!(101), dec!(103)),
            (dec!(112), dec!(110), dec!(111)),
            (dec!(108), dec!(106), dec!(107)),
            (dec!(120), dec!(100), dec!(101)),
            (dec!(103), dec!(99), dec!(100)),
            (dec!(101), dec!(97), dec!(98)),
        ]);
        let full = wilder_atr(&all, 3);
        for cut in 3..all.len() {
            let seed = AtrSeed {
                prev_close: all[cut - 1].close,
                prev_atr: full[cut - 1].unwrap(),
            };
            assert_eq!(wilder_atr_after(seed, &all[cut..], 3), full[cut..]);
        }
    }

    #[test]
    fn empty_series_gives_empty_output() {
        assert!(wilder_atr::<Candle>(&[], 14).is_empty());
    }
}

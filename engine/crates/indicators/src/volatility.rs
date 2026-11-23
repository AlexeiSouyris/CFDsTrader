//! Volatilidad realizada e indicadores sobre velas diarias (#148, spec §13).
//!
//! ```text
//! retorno_t   = cierre_t / cierre_{t−1} − 1                     (desde la segunda vela)
//! vol_t       = desvío estándar muestral de los últimos n retornos (hasta t incluido)
//!             = sqrt( Σ (r − media)² / (n − 1) )
//! ```
//! - Calentamiento: hacen falta `n` retornos, o sea `n + 1` cierres. Antes, `None`.
//! - Sin repintado: el valor de `t` solo depende de los cierres `0..=t`.
//! - Un cierre previo en 0 no tiene retorno: ese valor y los que lo usen quedan en `None`.

use rust_decimal::{Decimal, MathematicalOps};
use serde::Deserialize;

/// Parámetros de los indicadores diarios: sección `[daily]` de `config/indicators.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyConfig {
    pub atr_period: usize,
    pub realized_vol_window: usize,
    /// Velas diarias del máximo de la caída desde el máximo (spec §9.1).
    pub drawdown_window: usize,
}

impl DailyConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct File {
            daily: DailyConfig,
            /// Lo lee [`crate::TrendConfig`].
            #[allow(dead_code)]
            trend: toml::Table,
        }
        let cfg = toml::from_str::<File>(text)
            .map_err(|e| e.to_string())?
            .daily;
        if cfg.atr_period == 0 || cfg.realized_vol_window < 2 || cfg.drawdown_window == 0 {
            return Err("daily.atr_period y daily.drawdown_window tienen que ser > 0, y daily.realized_vol_window ≥ 2".into());
        }
        Ok(cfg)
    }
}

/// Volatilidad realizada de ventana `window` (en fracción, no en %), alineada por índice.
///
/// # Panics
/// Si `window` < 2 (el desvío muestral necesita dos retornos): se rechaza al cargar `config/`.
pub fn realized_volatility(closes: &[Decimal], window: usize) -> Vec<Option<Decimal>> {
    assert!(
        window >= 2,
        "la ventana de la volatilidad tiene que ser ≥ 2"
    );
    let returns: Vec<Option<Decimal>> = closes
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let prev = closes.get(i.checked_sub(1)?)?;
            (!prev.is_zero()).then(|| c / prev - Decimal::ONE)
        })
        .collect();
    let n = Decimal::from(window);
    (0..closes.len())
        .map(|i| {
            let first = (i + 1).checked_sub(window)?;
            if first == 0 {
                return None; // el primer cierre no tiene retorno
            }
            let r: Vec<Decimal> = returns[first..=i].iter().copied().collect::<Option<_>>()?;
            let mean = r.iter().sum::<Decimal>() / n;
            let var =
                r.iter().map(|x| (x - mean) * (x - mean)).sum::<Decimal>() / (n - Decimal::ONE);
            var.sqrt()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn warms_up_with_window_plus_one_closes() {
        let closes = [dec!(100), dec!(101), dec!(100), dec!(102)];
        let v = realized_volatility(&closes, 3);
        assert_eq!(v[..3], [None, None, None]);
        assert!(v[3].is_some());
    }

    #[test]
    fn matches_the_sample_standard_deviation_by_hand() {
        // Retornos: +10 % y −10 %. Media 0; varianza muestral = (0,01 + 0,01) / 1 = 0,02.
        let v = realized_volatility(&[dec!(100), dec!(110), dec!(99)], 2);
        let expected = dec!(0.02).sqrt().unwrap();
        assert_eq!(v[2], Some(expected));
    }

    #[test]
    fn constant_prices_have_zero_volatility() {
        let v = realized_volatility(&[dec!(50); 6], 5);
        assert_eq!(v[5], Some(Decimal::ZERO));
    }

    #[test]
    fn a_zero_close_has_no_return() {
        let v = realized_volatility(&[dec!(100), dec!(0), dec!(10), dec!(11)], 2);
        assert_eq!(v[2], None); // usa el retorno desde 0
        assert_eq!(v[3], None); // ídem
    }

    #[test]
    fn never_repaints() {
        let closes = [dec!(100), dec!(103), dec!(101), dec!(104), dec!(102)];
        let full = realized_volatility(&closes, 3);
        for t in 0..closes.len() {
            assert_eq!(realized_volatility(&closes[..=t], 3)[t], full[t]);
        }
    }
}

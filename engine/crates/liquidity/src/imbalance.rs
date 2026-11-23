//! Desequilibrio aproximado desde el precio (spec §5.4.1, #231). **No es flujo de órdenes**: IG
//! no da volumen ni libro de órdenes. Solo informativo. Contrato: `imbalance.v1`.
//!
//! ```text
//! desplazamiento en t:  |C − O| ≥ displacement_body_atr × ATR[t]  y  |C − O| ≥ min_body_ratio × (H − L)
//!                       alcista si C > O, bajista si C < O
//! índice en t (ventana de las últimas window_bars velas cerradas, la última es t):
//!   (Σ cuerpos de desplazamiento alcistas − Σ bajistas) / Σ (H − L) de la ventana;  0 si Σ (H − L) = 0
//! ```
//! - Calentamiento: el primer valor sale cuando la vela más vieja de la ventana ya tiene ATR.
//! - Sin repintado: el valor de `t` solo depende de las velas `0..=t`.
//! - `|C − O| / (H − L) ≥ ratio` se compara multiplicando, sin dividir por un rango que puede ser 0.

use std::num::NonZeroU32;
use std::str::FromStr;

use cfds_core::{Candle, DailyCandle, Direction, Imbalance, ImbalanceLabel};
use cfds_indicators::wilder_atr;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::stats::StatsBar;

/// Sección `[imbalance]` de `config/liquidity.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImbalanceConfig {
    pub displacement_body_atr: Decimal,
    pub min_body_ratio: Decimal,
    pub window_bars: NonZeroU32,
}

#[derive(Deserialize)]
struct Raw {
    imbalance: RawImbalance,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawImbalance {
    displacement_body_atr: String,
    min_body_ratio: String,
    window_bars: NonZeroU32,
}

impl ImbalanceConfig {
    /// Lee y valida `[imbalance]`; ignora las demás secciones de `liquidity.toml`.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .imbalance;
        let parse = |name: &str, s: &str| {
            Decimal::from_str(s).map_err(|e| format!("imbalance.{name}: {e}"))
        };
        let body = parse("displacement_body_atr", &raw.displacement_body_atr)?;
        let ratio = parse("min_body_ratio", &raw.min_body_ratio)?;
        if body <= Decimal::ZERO {
            return Err("imbalance.displacement_body_atr tiene que ser mayor que 0".into());
        }
        if ratio <= Decimal::ZERO || ratio > Decimal::ONE {
            return Err("imbalance.min_body_ratio tiene que estar en (0, 1]".into());
        }
        Ok(Self {
            displacement_body_atr: body,
            min_body_ratio: ratio,
            window_bars: raw.window_bars,
        })
    }
}

/// Una vela con apertura: lo que necesita el desplazamiento además de [`StatsBar`].
pub trait OhlcBar: StatsBar {
    fn open(&self) -> Decimal;
}

impl OhlcBar for Candle {
    fn open(&self) -> Decimal {
        self.open
    }
}

impl OhlcBar for DailyCandle {
    fn open(&self) -> Decimal {
        self.open
    }
}

/// Dirección del desplazamiento de la vela `c` con su ATR, o `None` si no lo es.
pub fn displacement<T: OhlcBar>(
    c: &T,
    atr: Option<Decimal>,
    cfg: &ImbalanceConfig,
) -> Option<Direction> {
    let a = atr.filter(|a| *a > Decimal::ZERO)?;
    let body = (c.close() - c.open()).abs();
    let range = c.high() - c.low();
    let big = body >= cfg.displacement_body_atr * a && body >= cfg.min_body_ratio * range;
    big.then_some(if c.close() > c.open() {
        Direction::Bullish
    } else {
        Direction::Bearish
    })
}

/// Índice de desequilibrio al cierre de cada vela de `bars` (cerradas, en orden, de un
/// instrumento y timeframe), desde que termina el calentamiento.
pub fn imbalance_series<T: OhlcBar>(
    bars: &[T],
    atr_period: usize,
    cfg: &ImbalanceConfig,
) -> Vec<Imbalance> {
    let atr = wilder_atr(bars, atr_period);
    let disp: Vec<Option<Direction>> = bars
        .iter()
        .zip(&atr)
        .map(|(c, a)| displacement(c, *a, cfg))
        .collect();
    let w = cfg.window_bars.get() as usize;
    (w.saturating_sub(1)..bars.len())
        .filter(|&t| atr[t + 1 - w].is_some())
        .map(|t| {
            let window = t + 1 - w..=t;
            let body = |d: Direction| -> Decimal {
                window
                    .clone()
                    .filter(|&k| disp[k] == Some(d))
                    .map(|k| (bars[k].close() - bars[k].open()).abs())
                    .sum()
            };
            let (bull, bear) = (body(Direction::Bullish), body(Direction::Bearish));
            let range: Decimal = window.clone().map(|k| bars[k].high() - bars[k].low()).sum();
            let index = if range.is_zero() {
                Decimal::ZERO
            } else {
                (bull - bear) / range
            };
            Imbalance {
                instrument_id: bars[t].instrument().clone(),
                timeframe: bars[t].stats_timeframe(),
                at: bars[t].closed_at(),
                window_bars: cfg.window_bars,
                displacement: disp[t],
                bullish_body_sum: bull,
                bearish_body_sum: bear,
                range_sum: range,
                index,
                label: ImbalanceLabel::PriceApproximation,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/liquidity.toml");

    #[test]
    fn the_repository_config_matches_the_spec() {
        assert_eq!(
            ImbalanceConfig::from_toml(REAL).unwrap(),
            ImbalanceConfig {
                displacement_body_atr: dec!(1.5),
                min_body_ratio: dec!(0.7),
                window_bars: NonZeroU32::new(20).unwrap(),
            }
        );
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace(
                r#"displacement_body_atr = "1.5""#,
                r#"displacement_body_atr = "0""#,
            ),
            REAL.replace(r#"min_body_ratio = "0.7""#, r#"min_body_ratio = "1.2""#),
            REAL.replace(r#"min_body_ratio = "0.7""#, r#"min_body_ratio = "0""#),
            REAL.replace("window_bars = 20", "window_bars = 0"),
            REAL.replace("window_bars = 20", "ventana = 20"),
        ] {
            assert!(ImbalanceConfig::from_toml(&bad).is_err(), "{bad}");
            assert!(crate::LiquidityConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

//! Herramientas de análisis del gráfico (#491): `config/analysis.toml` y el bloque `analysis` de
//! `chart_export.v1`. Solo **junta** lo que calculan `cfds_indicators` y `cfds_patterns`; no
//! entra al puntaje ni a la estrategia en uso.
//!
//! Multi-timeframe: una vela de 15m ve el valor diario del **último día ya terminado**
//! (`día.end ≤ vela.end`), nunca el del día en curso.

use std::str::FromStr;

use cfds_core::{Candle, DailyCandle, decimal_str};
use cfds_indicators::ema_values;
use cfds_patterns::{FibConfig, LegDirection, confirmed_pivots, fib_legs, golden_pocket_events};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// `config/analysis.toml`, validado al cargar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisConfig {
    pub ema_daily_period: usize,
    pub fibonacci: FibConfig,
}

impl AnalysisConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            ema_daily: RawEma,
            fibonacci: RawFib,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawEma {
            period: usize,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawFib {
            retracements: Vec<String>,
            extensions: Vec<String>,
            golden_pocket: [String; 2],
        }
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        if raw.ema_daily.period == 0 {
            return Err("ema_daily.period tiene que ser > 0".into());
        }
        let parse = |v: &[String]| -> Result<Vec<Decimal>, String> {
            v.iter()
                .map(|s| Decimal::from_str(s).map_err(|e| format!("fibonacci: {s:?}: {e}")))
                .collect()
        };
        let gp = parse(&raw.fibonacci.golden_pocket)?;
        Ok(Self {
            ema_daily_period: raw.ema_daily.period,
            fibonacci: FibConfig::new(
                parse(&raw.fibonacci.retracements)?,
                parse(&raw.fibonacci.extensions)?,
                (gp[0], gp[1]),
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportAnalysis {
    pub ema_daily_period: usize,
    /// Un valor por vela de 15m (alineado con `candles`): EMA diaria del último día terminado.
    #[serde(with = "option_vec")]
    pub ema_daily: Vec<Option<Decimal>>,
    pub fibonacci: Vec<ExportFibLeg>,
    pub golden_pocket: Vec<ExportPocketEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportFibLeg {
    pub direction: String,
    pub start_index: usize,
    #[serde(with = "decimal_str")]
    pub start_price: Decimal,
    pub end_index: usize,
    #[serde(with = "decimal_str")]
    pub end_price: Decimal,
    pub confirmed_index: usize,
    pub retracements: Vec<ExportRatio>,
    pub extensions: Vec<ExportRatio>,
    #[serde(with = "decimal_str::signed")]
    pub golden_pocket_low: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub golden_pocket_high: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRatio {
    #[serde(with = "decimal_str")]
    pub ratio: Decimal,
    /// Con signo: una extensión de un tramo bajista muy grande puede quedar ≤ 0.
    #[serde(with = "decimal_str::signed")]
    pub price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportPocketEvent {
    pub index: usize,
    /// Índice en `fibonacci`.
    pub leg: usize,
    pub touches: bool,
    pub closes_inside: bool,
}

mod option_vec {
    use cfds_core::decimal_str;
    use rust_decimal::Decimal;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    #[serde(transparent)]
    struct Item(#[serde(with = "decimal_str::option")] Option<Decimal>);

    pub fn serialize<S: Serializer>(v: &[Option<Decimal>], s: S) -> Result<S::Ok, S::Error> {
        v.iter().map(|x| Item(*x)).collect::<Vec<_>>().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Option<Decimal>>, D::Error> {
        Ok(Vec::<Item>::deserialize(d)?
            .into_iter()
            .map(|i| i.0)
            .collect())
    }
}

/// Índice del último día terminado al cierre de cada vela de 15m (`None` si todavía no hay).
/// `daily` tiene que venir en orden.
pub fn last_closed_day(m15: &[Candle], daily: &[DailyCandle]) -> Vec<Option<usize>> {
    let mut out = Vec::with_capacity(m15.len());
    let mut d = 0;
    for c in m15 {
        while d < daily.len() && daily[d].end <= c.end() {
            d += 1;
        }
        out.push(d.checked_sub(1));
    }
    out
}

/// Bloque `analysis` del export: EMA diaria, tramos de Fibonacci y eventos del golden pocket.
/// `pivot_strength` = el de `config/patterns.toml` (los mismos pivotes que muestra el export).
pub fn analysis_export(
    m15: &[Candle],
    daily: &[DailyCandle],
    pivot_strength: usize,
    cfg: &AnalysisConfig,
) -> ExportAnalysis {
    let closes: Vec<Decimal> = daily.iter().map(|d| d.close).collect();
    let ema = ema_values(&closes, cfg.ema_daily_period);
    let ema_daily = last_closed_day(m15, daily)
        .into_iter()
        .map(|d| d.and_then(|d| ema[d]))
        .collect();
    let legs = fib_legs(&confirmed_pivots(m15, pivot_strength), &cfg.fibonacci);
    let golden_pocket = golden_pocket_events(m15, &legs)
        .into_iter()
        .map(|e| ExportPocketEvent {
            index: e.index,
            leg: e.leg,
            touches: e.touches,
            closes_inside: e.closes_inside,
        })
        .collect();
    let ratios = |v: &[(Decimal, Decimal)]| -> Vec<ExportRatio> {
        v.iter()
            .map(|(ratio, price)| ExportRatio {
                ratio: *ratio,
                price: *price,
            })
            .collect()
    };
    let fibonacci = legs
        .iter()
        .map(|l| ExportFibLeg {
            direction: match l.direction {
                LegDirection::Up => "up",
                LegDirection::Down => "down",
            }
            .into(),
            start_index: l.start.index,
            start_price: l.start.price,
            end_index: l.end.index,
            end_price: l.end.price,
            confirmed_index: l.confirmed_at(),
            retracements: ratios(&l.retracements),
            extensions: ratios(&l.extensions),
            golden_pocket_low: l.golden_pocket.0,
            golden_pocket_high: l.golden_pocket.1,
        })
        .collect();
    ExportAnalysis {
        ema_daily_period: cfg.ema_daily_period,
        ema_daily,
        fibonacci,
        golden_pocket,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../../../config/analysis.toml");

    #[test]
    fn the_repository_config_loads() {
        let c = AnalysisConfig::from_toml(REAL).unwrap();
        assert_eq!(c.ema_daily_period, 365);
        assert_eq!(c.fibonacci.retracements.len(), 5);
        assert_eq!(
            c.fibonacci.golden_pocket,
            (rust_decimal::dec!(0.618), rust_decimal::dec!(0.65))
        );
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace("period = 365", "period = 0"),
            REAL.replace(r#""0.618", "0.65""#, r#""0.65", "0.618""#),
            REAL.replace(r#""1.272""#, r#""0.9""#),
            REAL.replace("retracements", "retrocesos"),
            REAL.replace(r#""0.236""#, "0.236"),
        ] {
            assert!(AnalysisConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

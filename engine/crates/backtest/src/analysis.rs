//! Herramientas de análisis del gráfico (#491): `config/analysis.toml` y el bloque `analysis` de
//! `chart_export.v1`. Solo **junta** lo que calculan `cfds_indicators` y `cfds_patterns`; no
//! entra al puntaje ni a la estrategia en uso.
//!
//! Multi-timeframe: una vela de 15m ve el valor diario del **último día ya terminado**
//! (`día.end ≤ vela.end`), nunca el del día en curso; y el VIX publicado antes de su cierre.

use std::str::FromStr;

use cfds_core::{Candle, DailyCandle, decimal_str};
use cfds_indicators::{CrossKind, crosses, ema_values, floor_pivots, sma_values};
use cfds_patterns::{
    BottomConfig, FibConfig, LegDirection, LevelConfig, WyckoffConfig, WyckoffKind, bottoms,
    confirmed_pivots, fib_legs, golden_pocket_events, wyckoff_events,
};
use cfds_regime::{VixConfig, vix_known_at, vix_regime};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// `config/analysis.toml`, validado al cargar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisConfig {
    pub ema_daily_period: usize,
    pub fibonacci: FibConfig,
    pub wyckoff: WyckoffConfig,
    pub bottom: BottomConfig,
    pub ma_cross: MaCrossConfig,
    pub vix: VixConfig,
}

/// Media de los cruces dorado y de la muerte (sobre los cierres diarios).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaKind {
    Sma,
    Ema,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaCrossConfig {
    pub kind: MaKind,
    pub fast: usize,
    pub slow: usize,
}

fn dec(name: &str, s: &str) -> Result<Decimal, String> {
    let d = Decimal::from_str(s).map_err(|e| format!("{name}: {s:?} no es decimal ({e})"))?;
    if d > Decimal::ZERO {
        Ok(d)
    } else {
        Err(format!("{name} tiene que ser mayor que 0"))
    }
}

impl AnalysisConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            ema_daily: RawEma,
            fibonacci: RawFib,
            wyckoff: RawWyckoff,
            bottom: RawBottom,
            ma_cross: RawCross,
            vix: RawVix,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawWyckoff {
            lookback_bars: usize,
            min_touches: usize,
            tolerance_pct: String,
            max_bars_to_reclaim: usize,
            max_penetration_pct: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawBottom {
            lookback: usize,
            min_decline_pct: String,
            min_decline_atr: String,
            atr_period: usize,
            min_rebound_atr: String,
            max_bars_to_rebound: usize,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawCross {
            kind: String,
            fast: usize,
            slow: usize,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawVix {
            normal_from: String,
            elevated_from: String,
            stress_from: String,
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
        let hundred = Decimal::ONE_HUNDRED;
        let w = raw.wyckoff;
        if w.lookback_bars == 0 || w.min_touches < 2 {
            return Err("wyckoff: lookback_bars > 0 y min_touches ≥ 2".into());
        }
        let b = raw.bottom;
        if b.lookback == 0 || b.atr_period == 0 || b.max_bars_to_rebound == 0 {
            return Err(
                "bottom: lookback, atr_period y max_bars_to_rebound tienen que ser > 0".into(),
            );
        }
        let c = raw.ma_cross;
        let kind = match c.kind.as_str() {
            "sma" => MaKind::Sma,
            "ema" => MaKind::Ema,
            other => return Err(format!("ma_cross.kind: {other:?} (sma o ema)")),
        };
        if c.fast == 0 || c.fast >= c.slow {
            return Err("ma_cross: 0 < fast < slow".into());
        }
        let v = raw.vix;
        Ok(Self {
            ema_daily_period: raw.ema_daily.period,
            fibonacci: FibConfig::new(
                parse(&raw.fibonacci.retracements)?,
                parse(&raw.fibonacci.extensions)?,
                (gp[0], gp[1]),
            )?,
            wyckoff: WyckoffConfig {
                levels: LevelConfig {
                    lookback_bars: w.lookback_bars,
                    min_touches: w.min_touches,
                    tolerance: dec("wyckoff.tolerance_pct", &w.tolerance_pct)? / hundred,
                },
                max_bars_to_reclaim: w.max_bars_to_reclaim,
                max_penetration: dec("wyckoff.max_penetration_pct", &w.max_penetration_pct)?
                    / hundred,
            },
            bottom: BottomConfig {
                lookback: b.lookback,
                min_decline: dec("bottom.min_decline_pct", &b.min_decline_pct)? / hundred,
                min_decline_atr: dec("bottom.min_decline_atr", &b.min_decline_atr)?,
                atr_period: b.atr_period,
                min_rebound_atr: dec("bottom.min_rebound_atr", &b.min_rebound_atr)?,
                max_bars_to_rebound: b.max_bars_to_rebound,
            },
            ma_cross: MaCrossConfig {
                kind,
                fast: c.fast,
                slow: c.slow,
            },
            vix: VixConfig::new(
                dec("vix.normal_from", &v.normal_from)?,
                dec("vix.elevated_from", &v.elevated_from)?,
                dec("vix.stress_from", &v.stress_from)?,
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
    pub wyckoff: Vec<ExportWyckoff>,
    pub bottoms: Vec<ExportBottom>,
    pub ma_cross: ExportMaCross,
    pub floor_pivots: Vec<ExportFloorPivots>,
    /// Un valor por vela de 15m: el VIX publicado antes de su cierre y su régimen.
    pub vix: Vec<Option<ExportVix>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportWyckoff {
    pub kind: String,
    pub break_index: usize,
    pub confirmed_index: usize,
    #[serde(with = "decimal_str")]
    pub level: Decimal,
    pub touches: usize,
    #[serde(with = "decimal_str")]
    pub extreme: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportBottom {
    pub index: usize,
    #[serde(with = "decimal_str")]
    pub price: Decimal,
    #[serde(with = "decimal_str")]
    pub prior_high: Decimal,
    pub rebound_index: usize,
    pub confirmed_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportMaCross {
    pub kind: String,
    pub fast_period: usize,
    pub slow_period: usize,
    /// Un valor por vela de 15m: la media diaria rápida y la lenta del último día terminado.
    #[serde(with = "option_vec")]
    pub fast: Vec<Option<Decimal>>,
    #[serde(with = "option_vec")]
    pub slow: Vec<Option<Decimal>>,
    pub crosses: Vec<ExportCross>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportCross {
    pub kind: String,
    /// Día del cruce (AAAA-MM-DD).
    pub day: String,
    /// Primera vela de 15m que ya lo ve (el día terminó).
    pub known_from_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportFloorPivots {
    /// Día del que salen (AAAA-MM-DD); valen desde `from_index` hasta que termina el siguiente.
    pub day: String,
    pub from_index: usize,
    #[serde(with = "decimal_str::signed")]
    pub pp: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub r1: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub r2: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub r3: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub s1: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub s2: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub s3: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportVix {
    #[serde(with = "decimal_str")]
    pub close: Decimal,
    pub regime: String,
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

/// Bloque `analysis` del export. `pivot_strength` = el de `config/patterns.toml` (los mismos
/// pivotes que muestra el export); `vix` = `(published_at, cierre)` en orden de publicación.
pub fn analysis_export(
    m15: &[Candle],
    daily: &[DailyCandle],
    vix: &[(DateTime<Utc>, Decimal)],
    pivot_strength: usize,
    cfg: &AnalysisConfig,
) -> ExportAnalysis {
    let closes: Vec<Decimal> = daily.iter().map(|d| d.close).collect();
    let days = last_closed_day(m15, daily);
    let per_candle = |series: &[Option<Decimal>]| -> Vec<Option<Decimal>> {
        days.iter().map(|d| d.and_then(|d| series[d])).collect()
    };
    let ema_daily = per_candle(&ema_values(&closes, cfg.ema_daily_period));
    let pivots = confirmed_pivots(m15, pivot_strength);
    let legs = fib_legs(&pivots, &cfg.fibonacci);
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
    let wyckoff = wyckoff_events(m15, &pivots, &cfg.wyckoff)
        .into_iter()
        .map(|e| ExportWyckoff {
            kind: match e.kind {
                WyckoffKind::Spring => "spring",
                WyckoffKind::Upthrust => "upthrust",
            }
            .into(),
            break_index: e.break_index,
            confirmed_index: e.confirmed_at,
            level: e.level,
            touches: e.touches,
            extreme: e.extreme,
        })
        .collect();
    let bottoms = bottoms(m15, &pivots, &cfg.bottom)
        .into_iter()
        .map(|b| ExportBottom {
            index: b.pivot.index,
            price: b.pivot.price,
            prior_high: b.prior_high,
            rebound_index: b.rebound_index,
            confirmed_index: b.confirmed_at,
        })
        .collect();
    let mc = cfg.ma_cross;
    let ma = |p| match mc.kind {
        MaKind::Sma => sma_values(&closes, p),
        MaKind::Ema => ema_values(&closes, p),
    };
    let (fast, slow) = (ma(mc.fast), ma(mc.slow));
    // Primera vela de 15m que ya ve el día `d`; los días terminados antes de la primera vela no
    // son de este período.
    let first_seeing = |d: usize| -> Option<usize> {
        let t = days.iter().position(|x| x.is_some_and(|x| x >= d))?;
        (t > 0 || m15.first().is_some_and(|c| daily[d].end > c.start)).then_some(t)
    };
    let cross_list = crosses(&fast, &slow)
        .into_iter()
        .filter_map(|c| {
            Some(ExportCross {
                kind: match c.kind {
                    CrossKind::Golden => "golden",
                    CrossKind::Death => "death",
                }
                .into(),
                day: daily[c.index].day.to_string(),
                known_from_index: first_seeing(c.index)?,
            })
        })
        .collect();
    let mut floor = Vec::new();
    for (t, d) in days.iter().enumerate() {
        if let Some(d) = *d
            && (t == 0 || days[t - 1] != Some(d))
        {
            let p = floor_pivots(&daily[d]);
            floor.push(ExportFloorPivots {
                day: daily[d].day.to_string(),
                from_index: t,
                pp: p.pp,
                r1: p.r1,
                r2: p.r2,
                r3: p.r3,
                s1: p.s1,
                s2: p.s2,
                s3: p.s3,
            });
        }
    }
    let ends: Vec<DateTime<Utc>> = m15.iter().map(Candle::end).collect();
    let vix = vix_known_at(vix, &ends)
        .into_iter()
        .map(|v| {
            v.map(|close| ExportVix {
                close,
                regime: vix_regime(close, &cfg.vix).as_str().into(),
            })
        })
        .collect();
    ExportAnalysis {
        ema_daily_period: cfg.ema_daily_period,
        ema_daily,
        fibonacci,
        golden_pocket,
        wyckoff,
        bottoms,
        ma_cross: ExportMaCross {
            kind: match mc.kind {
                MaKind::Sma => "sma",
                MaKind::Ema => "ema",
            }
            .into(),
            fast_period: mc.fast,
            slow_period: mc.slow,
            fast: per_candle(&fast),
            slow: per_candle(&slow),
            crosses: cross_list,
        },
        floor_pivots: floor,
        vix,
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
            REAL.replace("min_touches = 3", "min_touches = 1"),
            REAL.replace(r#"kind = "sma""#, r#"kind = "wma""#),
            REAL.replace("fast = 50", "fast = 300"),
            REAL.replace(r#"elevated_from = "20""#, r#"elevated_from = "10""#),
            REAL.replace(r#"min_rebound_atr = "1""#, r#"min_rebound_atr = "-1""#),
        ] {
            assert!(AnalysisConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}

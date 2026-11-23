//! Estadísticas de FVG y de gaps de apertura (spec §5.4.1, #231). Solo informativas: no suman al
//! puntaje de §6. Contratos: `fvg_event.v1`, `opening_gap.v1` y `liquidity_stats.v1`.
//!
//! ```text
//! FVG (definición y llenado de §5.4, `fvg.rs`), creado en la vela i, alcista (bajista: espejo):
//!   toque   = primera t > i con L[t] ≤ top                 (entra a la zona)
//!   mitad   = primera t > i con L[t] ≤ (bottom + top) / 2
//!   llenado = primera t > i con C[t] < bottom              (§5.4)
//!   vence   = sin llenado hasta t = i + max_age_bars;  no se sigue después de llenarse o vencer
//!   tamaño  = (top − bottom) / ATR[i];  reacción_k = (C[toque + k] − C[toque]) / ATR[i]
//! Gap de apertura en la sesión s:  |O[s] − C[s−1]| ≥ min_atr × ATR[s−1]  (ATR diario)
//!   se llena en la primera j ≥ s con L[j] ≤ C[s−1] (gap alcista; bajista: H[j] ≥ C[s−1]);
//!   vence sin llenarse en j = s + max_sessions
//! ```
//! - Solo velas cerradas: el estado se evalúa al cierre de la última vela recibida (`as_of`) y un
//!   evento ya registrado no cambia con velas nuevas (sin repintado). Un FVG abierto puede estar
//!   tocado, pero no está ni lleno ni vencido.
//! - Calentamiento: no hay FVG ni gap sin ATR válido (> 0).
//! - Las velas faltantes no se rellenan: las velas y sesiones se cuentan entre las presentes.
//! - Agregados por instrumento, tipo, timeframe, dirección y tercil de tamaño: toque y mitad sobre
//!   todos los casos; llenado sobre los cerrados (llenos o vencidos). IC 95 % de Wilson
//!   (`cfds_indicators::wilson`). Con menos de `min_cases` casos: "datos insuficientes".

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::str::FromStr;

use cfds_core::{
    Candle, DailyCandle, Direction, DistMetric, DistStat, FvgEvent, FvgEventPoint, FvgReaction,
    GapSession, InstrumentId, LiquidityStats, OpeningGap, RateMetric, RateStat, SizeBucket,
    SizeCuts, StatsSubject, StatsTimeframe, ZoneStatus,
};
use cfds_indicators::{Hlc, wilder_atr, wilson};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::Side;
use crate::fvg::{fvg_created_at, is_filled_by};

/// Decimales de las tasas y sus intervalos (igual que la probabilidad de §13).
const RATE_DP: u32 = 4;

/// Secciones `[fvg_stats]`, `[gaps]` y `[stats]` de `config/liquidity.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsConfig {
    pub reaction_bars: Vec<NonZeroU32>,
    pub max_age_bars: NonZeroU32,
    pub gap_min_atr: Decimal,
    pub gap_max_sessions: NonZeroU32,
    pub min_cases: NonZeroU32,
    pub z: Decimal,
}

#[derive(Deserialize)]
struct Raw {
    fvg_stats: RawFvgStats,
    gaps: RawGaps,
    stats: RawStats,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFvgStats {
    reaction_bars: Vec<NonZeroU32>,
    max_age_bars: NonZeroU32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGaps {
    min_atr: String,
    max_sessions: NonZeroU32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStats {
    min_cases: NonZeroU32,
    z: String,
}

impl StatsConfig {
    /// Lee y valida las secciones de estadísticas; ignora las demás de `liquidity.toml`.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let positive = |name: &str, s: &str| {
            Decimal::from_str(s)
                .ok()
                .filter(|d| *d > Decimal::ZERO)
                .ok_or_else(|| format!("{name} tiene que ser un decimal mayor que 0: {s:?}"))
        };
        let bars = raw.fvg_stats.reaction_bars;
        if bars.is_empty() || bars.windows(2).any(|w| w[0] >= w[1]) {
            return Err(
                "fvg_stats.reaction_bars: al menos uno, de menor a mayor y sin repetir".into(),
            );
        }
        Ok(Self {
            reaction_bars: bars,
            max_age_bars: raw.fvg_stats.max_age_bars,
            gap_min_atr: positive("gaps.min_atr", &raw.gaps.min_atr)?,
            gap_max_sessions: raw.gaps.max_sessions,
            min_cases: raw.stats.min_cases,
            z: positive("stats.z", &raw.stats.z)?,
        })
    }
}

/// Una vela cerrada de cualquier timeframe: lo que necesitan las estadísticas de FVG.
pub trait StatsBar: Hlc {
    fn instrument(&self) -> &InstrumentId;
    fn stats_timeframe(&self) -> StatsTimeframe;
    /// Desde cuándo la vela está cerrada y se puede usar.
    fn closed_at(&self) -> DateTime<Utc>;
}

impl StatsBar for Candle {
    fn instrument(&self) -> &InstrumentId {
        &self.instrument_id
    }
    fn stats_timeframe(&self) -> StatsTimeframe {
        self.timeframe.into()
    }
    fn closed_at(&self) -> DateTime<Utc> {
        self.end()
    }
}

impl StatsBar for DailyCandle {
    fn instrument(&self) -> &InstrumentId {
        &self.instrument_id
    }
    fn stats_timeframe(&self) -> StatsTimeframe {
        StatsTimeframe::D1
    }
    fn closed_at(&self) -> DateTime<Utc> {
        self.end
    }
}

fn direction(side: Side) -> Direction {
    match side {
        Side::Long => Direction::Bullish,
        Side::Short => Direction::Bearish,
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Cantidad de velas (≥ 1) entre la creación y un evento.
fn bars_since(n: usize) -> NonZeroU32 {
    NonZeroU32::new(count(n)).unwrap_or(NonZeroU32::MIN)
}

/// Cada FVG de `bars` (velas cerradas, en orden, de un instrumento y timeframe) con lo que le
/// pasó hasta el cierre de la última. `atr_period` es el de `[order_blocks]`.
pub fn fvg_events<T: StatsBar>(bars: &[T], atr_period: usize, cfg: &StatsConfig) -> Vec<FvgEvent> {
    let Some(last) = bars.last() else {
        return Vec::new();
    };
    let atr = wilder_atr(bars, atr_period);
    let n = bars.len();
    let max_age = cfg.max_age_bars.get() as usize;
    (2..n)
        .filter_map(|i| {
            let f = fvg_created_at(bars, i)?;
            let a = atr[i].filter(|a| *a > Decimal::ZERO)?;
            let middle = (f.bottom + f.top) / Decimal::TWO;
            let (mut touch, mut mid, mut fill) = (None, None, None);
            for (t, c) in bars
                .iter()
                .enumerate()
                .take(n.min(i + max_age + 1))
                .skip(i + 1)
            {
                let (zone, half) = match f.side {
                    Side::Long => (c.low() <= f.top, c.low() <= middle),
                    Side::Short => (c.high() >= f.bottom, c.high() >= middle),
                };
                touch = touch.or(zone.then_some(t));
                mid = mid.or(half.then_some(t));
                if is_filled_by(&f, c) {
                    fill = Some(t);
                    break;
                }
            }
            let expiry = (fill.is_none() && i + max_age < n).then_some(i + max_age);
            let point = |t: usize| FvgEventPoint {
                at: bars[t].closed_at(),
                bars: bars_since(t - i),
            };
            let signed = |d: Decimal| if f.side == Side::Long { d } else { -d };
            let reactions = cfg
                .reaction_bars
                .iter()
                .map(|&k| FvgReaction {
                    bars: k,
                    return_atr: touch.and_then(|t| {
                        let later = bars.get(t + k.get() as usize)?;
                        Some(signed(later.close() - bars[t].close()) / a)
                    }),
                })
                .collect();
            let status = match (fill, expiry) {
                (Some(_), _) => ZoneStatus::Filled,
                (None, Some(_)) => ZoneStatus::Expired,
                (None, None) => ZoneStatus::Open,
            };
            Some(FvgEvent {
                instrument_id: bars[i].instrument().clone(),
                timeframe: bars[i].stats_timeframe(),
                direction: direction(f.side),
                created_at: bars[i].closed_at(),
                bottom: f.bottom,
                top: f.top,
                atr: a,
                size_atr: (f.top - f.bottom) / a,
                touch: touch.map(point),
                mid: mid.map(point),
                fill: fill.map(point),
                expiry: expiry.map(point),
                reactions,
                status,
                as_of: last.closed_at(),
            })
        })
        .collect()
}

/// Gaps de apertura de `daily` (velas diarias cerradas de un instrumento, solo días con sesión,
/// en orden), hasta el fin de la última. `atr_period` es el del ATR diario.
pub fn opening_gaps(
    daily: &[DailyCandle],
    atr_period: usize,
    cfg: &StatsConfig,
) -> Vec<OpeningGap> {
    let Some(last) = daily.last() else {
        return Vec::new();
    };
    let atr = wilder_atr(daily, atr_period);
    let n = daily.len();
    let max = cfg.gap_max_sessions.get() as usize;
    (1..n)
        .filter_map(|s| {
            let prev_close = daily[s - 1].close;
            let a = atr[s - 1].filter(|a| *a > Decimal::ZERO)?;
            let size = (daily[s].open - prev_close).abs();
            if size.is_zero() || size < cfg.gap_min_atr * a {
                return None;
            }
            let dir = if daily[s].open > prev_close {
                Direction::Bullish
            } else {
                Direction::Bearish
            };
            let fill = (s..n.min(s + max + 1)).find(|&j| match dir {
                Direction::Bullish => daily[j].low <= prev_close,
                Direction::Bearish => daily[j].high >= prev_close,
            });
            let expiry = (fill.is_none() && s + max < n).then_some(s + max);
            let session = |j: usize| GapSession {
                day: daily[j].day,
                sessions: count(j - s),
            };
            Some(OpeningGap {
                instrument_id: daily[s].instrument_id.clone(),
                day: daily[s].day,
                direction: dir,
                prev_close,
                open: daily[s].open,
                atr: a,
                size_atr: size / a,
                fill: fill.map(session),
                same_session: fill == Some(s),
                expiry: expiry.map(session),
                status: match (fill, expiry) {
                    (Some(_), _) => ZoneStatus::Filled,
                    (None, Some(_)) => ZoneStatus::Expired,
                    (None, None) => ZoneStatus::Open,
                },
                as_of: last.end,
            })
        })
        .collect()
}

/// Lo que necesita el agregado de un caso (FVG o gap).
struct Case {
    instrument: InstrumentId,
    timeframe: StatsTimeframe,
    direction: Direction,
    size_atr: Decimal,
    status: ZoneStatus,
    as_of: DateTime<Utc>,
    /// Velas o sesiones hasta cada evento (`None` si no pasó).
    touch: Option<u32>,
    mid: Option<u32>,
    fill: Option<u32>,
    reactions: Vec<(NonZeroU32, Option<Decimal>)>,
}

/// Agregados de FVG por instrumento, timeframe, dirección y tercil de tamaño.
pub fn fvg_stats(events: &[FvgEvent], cfg: &StatsConfig) -> Vec<LiquidityStats> {
    let cases = events
        .iter()
        .map(|e| Case {
            instrument: e.instrument_id.clone(),
            timeframe: e.timeframe,
            direction: e.direction,
            size_atr: e.size_atr,
            status: e.status,
            as_of: e.as_of,
            touch: e.touch.map(|p| p.bars.get()),
            mid: e.mid.map(|p| p.bars.get()),
            fill: e.fill.map(|p| p.bars.get()),
            reactions: e.reactions.iter().map(|r| (r.bars, r.return_atr)).collect(),
        })
        .collect();
    aggregate(StatsSubject::Fvg, cases, cfg)
}

/// Agregados de gaps de apertura por instrumento, dirección y tercil de tamaño.
pub fn gap_stats(gaps: &[OpeningGap], cfg: &StatsConfig) -> Vec<LiquidityStats> {
    let cases = gaps
        .iter()
        .map(|g| Case {
            instrument: g.instrument_id.clone(),
            timeframe: StatsTimeframe::D1,
            direction: g.direction,
            size_atr: g.size_atr,
            status: g.status,
            as_of: g.as_of,
            touch: None,
            mid: None,
            fill: g.fill.map(|f| f.sessions),
            reactions: Vec::new(),
        })
        .collect();
    aggregate(StatsSubject::OpeningGap, cases, cfg)
}

/// Cuantil `num/den` de valores ordenados, con interpolación lineal (en enteros, sin redondeo).
fn quantile(sorted: &[Decimal], num: usize, den: usize) -> Option<Decimal> {
    let last = sorted.len().checked_sub(1)?;
    let (lo, rest) = ((last * num) / den, (last * num) % den);
    let a = sorted[lo];
    let b = sorted.get(lo + 1).copied().unwrap_or(a);
    Some(a + (b - a) * Decimal::from(rest) / Decimal::from(den))
}

fn rate(metric: RateMetric, successes: usize, n: usize, cfg: &StatsConfig) -> RateStat {
    let (successes, n) = (count(successes), count(n));
    let insufficient = n < cfg.min_cases.get();
    let (rate, ci) = if insufficient {
        (None, None)
    } else {
        let (lo, hi) = wilson(successes, n, cfg.z);
        let p = Decimal::from(successes) / Decimal::from(n);
        (
            Some(p.round_dp(RATE_DP)),
            Some((lo.round_dp(RATE_DP), hi.round_dp(RATE_DP))),
        )
    };
    RateStat {
        metric,
        successes,
        n,
        insufficient,
        rate,
        ci_low: ci.map(|c| c.0),
        ci_high: ci.map(|c| c.1),
    }
}

fn dist(
    metric: DistMetric,
    horizon_bars: Option<NonZeroU32>,
    mut values: Vec<Decimal>,
    cfg: &StatsConfig,
) -> DistStat {
    values.sort();
    let n = count(values.len());
    let insufficient = n < cfg.min_cases.get();
    let q = |num| (!insufficient).then(|| quantile(&values, num, 4)).flatten();
    let (p25, median, p75) = (q(1), q(2), q(3));
    DistStat {
        metric,
        horizon_bars,
        n,
        insufficient,
        median,
        p25,
        p75,
        iqr: p25.zip(p75).map(|(a, b)| b - a),
    }
}

fn group_stats(
    subject: StatsSubject,
    cases: &[&Case],
    cfg: &StatsConfig,
) -> (Vec<RateStat>, Vec<DistStat>) {
    let all = cases.len();
    let closed = cases
        .iter()
        .filter(|c| c.status != ZoneStatus::Open)
        .count();
    let filled = cases.iter().filter(|c| c.fill.is_some()).count();
    let values = |f: fn(&Case) -> Option<u32>| -> Vec<Decimal> {
        cases
            .iter()
            .filter_map(|c| f(c))
            .map(Decimal::from)
            .collect()
    };
    match subject {
        StatsSubject::Fvg => {
            let rates = vec![
                rate(
                    RateMetric::Touch,
                    cases.iter().filter(|c| c.touch.is_some()).count(),
                    all,
                    cfg,
                ),
                rate(
                    RateMetric::Mid,
                    cases.iter().filter(|c| c.mid.is_some()).count(),
                    all,
                    cfg,
                ),
                rate(RateMetric::Fill, filled, closed, cfg),
            ];
            let mut dists = vec![
                dist(DistMetric::BarsToTouch, None, values(|c| c.touch), cfg),
                dist(DistMetric::BarsToMid, None, values(|c| c.mid), cfg),
                dist(DistMetric::BarsToFill, None, values(|c| c.fill), cfg),
            ];
            for &k in &cfg.reaction_bars {
                let at_k = cases
                    .iter()
                    .flat_map(|c| &c.reactions)
                    .filter(|(bars, _)| *bars == k)
                    .filter_map(|(_, r)| *r)
                    .collect();
                dists.push(dist(DistMetric::Reaction, Some(k), at_k, cfg));
            }
            (rates, dists)
        }
        StatsSubject::OpeningGap => {
            let same = cases.iter().filter(|c| c.fill == Some(0)).count();
            let rates = vec![
                rate(RateMetric::Fill, filled, closed, cfg),
                rate(RateMetric::SameSessionFill, same, all, cfg),
            ];
            let dists = vec![dist(
                DistMetric::SessionsToFill,
                None,
                values(|c| c.fill),
                cfg,
            )];
            (rates, dists)
        }
    }
}

fn aggregate(subject: StatsSubject, cases: Vec<Case>, cfg: &StatsConfig) -> Vec<LiquidityStats> {
    let mut by_series: BTreeMap<(InstrumentId, StatsTimeframe), Vec<Case>> = BTreeMap::new();
    for c in cases {
        by_series
            .entry((c.instrument.clone(), c.timeframe))
            .or_default()
            .push(c);
    }
    let mut out = Vec::new();
    for ((instrument, timeframe), series) in by_series {
        let mut sizes: Vec<Decimal> = series.iter().map(|c| c.size_atr).collect();
        sizes.sort();
        let (Some(low_max), Some(mid_max), Some(as_of)) = (
            quantile(&sizes, 1, 3),
            quantile(&sizes, 2, 3),
            series.iter().map(|c| c.as_of).max(),
        ) else {
            continue;
        };
        let bucket = |s: Decimal| {
            if s <= low_max {
                SizeBucket::Low
            } else if s <= mid_max {
                SizeBucket::Mid
            } else {
                SizeBucket::High
            }
        };
        for dir in [Direction::Bullish, Direction::Bearish] {
            if !series.iter().any(|c| c.direction == dir) {
                continue;
            }
            for size in [
                SizeBucket::Low,
                SizeBucket::Mid,
                SizeBucket::High,
                SizeBucket::All,
            ] {
                let group: Vec<&Case> = series
                    .iter()
                    .filter(|c| c.direction == dir)
                    .filter(|c| size == SizeBucket::All || bucket(c.size_atr) == size)
                    .collect();
                let (rates, distributions) = group_stats(subject, &group, cfg);
                out.push(LiquidityStats {
                    instrument_id: instrument.clone(),
                    subject,
                    timeframe,
                    direction: dir,
                    size_tercile: size,
                    size_cuts_atr: SizeCuts { low_max, mid_max },
                    min_cases: cfg.min_cases,
                    n: count(group.len()),
                    rates,
                    distributions,
                    as_of,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/liquidity.toml");

    #[test]
    fn the_repository_config_matches_the_spec() {
        let c = StatsConfig::from_toml(REAL).unwrap();
        let nz = |n| NonZeroU32::new(n).unwrap();
        assert_eq!(
            c,
            StatsConfig {
                reaction_bars: vec![nz(4), nz(8)],
                max_age_bars: nz(500),
                gap_min_atr: dec!(0.25),
                gap_max_sessions: nz(20),
                min_cases: nz(30),
                z: dec!(1.96),
            }
        );
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace("reaction_bars = [4, 8]", "reaction_bars = []"),
            REAL.replace("reaction_bars = [4, 8]", "reaction_bars = [8, 4]"),
            REAL.replace("reaction_bars = [4, 8]", "reaction_bars = [0, 8]"),
            REAL.replace("max_age_bars = 500", "max_age_bars = 0"),
            REAL.replace(r#"min_atr = "0.25""#, r#"min_atr = "0""#),
            REAL.replace(r#"min_atr = "0.25""#, r#"min_atr = "x""#),
            REAL.replace("max_sessions = 20", "max_sessions = 0"),
            REAL.replace("min_cases = 30", "min_cases = 0"),
            REAL.replace(r#"z = "1.96""#, r#"z = "-1""#),
            REAL.replace("min_cases = 30", "min_casos = 30"),
        ] {
            assert!(StatsConfig::from_toml(&bad).is_err(), "{bad}");
            assert!(crate::LiquidityConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn quantiles_interpolate_linearly() {
        let v = [dec!(1), dec!(2), dec!(3), dec!(4)];
        assert_eq!(quantile(&v, 1, 2), Some(dec!(2.5)));
        assert_eq!(quantile(&v, 1, 4), Some(dec!(1.75)));
        assert_eq!(quantile(&v, 1, 3), Some(dec!(2)));
        assert_eq!(quantile(&[dec!(7)], 2, 3), Some(dec!(7)));
        assert_eq!(quantile(&[], 1, 2), None);
    }
}

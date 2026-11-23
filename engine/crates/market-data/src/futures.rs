//! Barras de 1 minuto de futuros (ADR 0011, #248): agrupación en velas y verificación de las
//! diarias contra las del CFD de IG. Funciones puras.
//!
//! ```text
//! 15m: barras con inicio en [k·15 min, (k+1)·15 min) UTC; la vela existe solo si la ventana
//!   cerró (fin ≤ until) y si todas sus barras son del mismo contrato (un vencimiento en el
//!   medio no se mezcla: esa vela no se arma y se avisa). Sin barras, no hay vela.
//! 1h: las cuatro velas de 15m de la hora (spec §3, #150), del mismo contrato
//! diaria del futuro para un día de IG: barras con inicio en [inicio, fin) de la diaria de IG;
//!   cierre = el de la última barra
//! base del día = cierre IG − cierre futuro
//! retornos: C[t]/C[t−1] − 1, solo entre diarias seguidas de IG con futuro los dos días, del
//!   mismo contrato y sin vencimiento en t (el salto del rollover no es un movimiento)
//! correlación de Pearson de los retornos (al menos 3 pares); base: media y desvío muestral
//! ```

use std::collections::BTreeMap;

use cfds_core::{Candle, DailyCandle, FuturesBar, FuturesCandle, InstrumentId, Timeframe};

use crate::{AggregateError, aggregate_h1};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::{Decimal, MathematicalOps};

/// Velas de 15m y 1h armadas desde barras de 1 minuto, y las ventanas descartadas.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FuturesCandles {
    pub m15: Vec<FuturesCandle>,
    pub h1: Vec<FuturesCandle>,
    /// Ventanas cerradas que no se armaron porque mezclaban dos contratos (vencimiento).
    pub mixed: Vec<(Timeframe, DateTime<Utc>)>,
}

fn bucket(t: DateTime<Utc>, step: TimeDelta) -> DateTime<Utc> {
    let s = step.num_seconds();
    t - TimeDelta::seconds(t.timestamp().rem_euclid(s))
}

/// Arma velas de 15m y 1h desde barras de 1 minuto de un futuro (en orden estricto), solo de
/// ventanas que cerraron hasta `until` (el fin de lo bajado, nunca después de ahora).
///
/// - 15m: ventanas UTC `[k·15 min, (k+1)·15 min)`, alineadas igual que las de IG. Una vela por
///   ventana con al menos una barra: un minuto sin operaciones no trae barra y no se rellena.
/// - 1h: la regla de la spec §3 (#150), con [`aggregate_h1`]: solo horas con sus **cuatro**
///   velas de 15m.
/// - Ninguna vela mezcla contratos: si en la ventana cambia el contrato, no se arma y queda en
///   `mixed` (una hora con una vela de 15m descartada por eso también).
///
/// `instrument` es el CFD al que corresponde el futuro.
pub fn futures_candles(
    bars: &[FuturesBar],
    instrument: &InstrumentId,
    until: DateTime<Utc>,
) -> Result<FuturesCandles, AggregateError> {
    let step = Timeframe::M15.duration();
    let mut out = FuturesCandles::default();
    let mut i = 0;
    while i < bars.len() {
        let start = bucket(bars[i].start, step);
        let j = i + bars[i..].partition_point(|b| bucket(b.start, step) == start);
        let group = &bars[i..j];
        i = j;
        if start + step > until {
            continue; // todavía no cerró, o falta bajar el resto de la ventana
        }
        if group.iter().any(|b| b.contract_id != group[0].contract_id) {
            out.mixed.push((Timeframe::M15, start));
            continue;
        }
        out.m15.push(FuturesCandle {
            candle: Candle {
                instrument_id: instrument.clone(),
                timeframe: Timeframe::M15,
                start,
                open: group[0].open,
                high: group.iter().map(|b| b.high).max().unwrap_or(group[0].high),
                low: group.iter().map(|b| b.low).min().unwrap_or(group[0].low),
                close: group[group.len() - 1].close,
                volume: Some(group.iter().map(|b| Decimal::from(b.volume)).sum()),
            },
            contract_id: group[0].contract_id,
        });
    }
    let hour = TimeDelta::hours(1);
    let mut mixed_hours: Vec<DateTime<Utc>> =
        out.mixed.iter().map(|(_, t)| bucket(*t, hour)).collect();
    let m15: Vec<Candle> = out.m15.iter().map(|c| c.candle.clone()).collect();
    for h in aggregate_h1(&m15)? {
        let from = out.m15.partition_point(|c| c.candle.start < h.start);
        let parts = &out.m15[from..from + 4];
        if parts.iter().any(|c| c.contract_id != parts[0].contract_id) {
            mixed_hours.push(h.start);
            continue;
        }
        out.h1.push(FuturesCandle {
            candle: h,
            contract_id: parts[0].contract_id,
        });
    }
    mixed_hours.sort();
    mixed_hours.dedup();
    out.mixed
        .extend(mixed_hours.into_iter().map(|t| (Timeframe::H1, t)));
    Ok(out)
}

/// Un día de IG con su cierre del futuro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayPair {
    pub day: NaiveDate,
    pub ig_close: Decimal,
    pub futures_close: Decimal,
    /// Contrato de la última barra del día.
    pub contract_id: u64,
    /// `true` si dentro del día hubo barras de dos contratos (vencimiento).
    pub rolled: bool,
    /// Cierre de IG − cierre del futuro.
    pub basis: Decimal,
}

/// Base de un contrato: días, media y desvío.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractBasis {
    pub contract_id: u64,
    pub days: usize,
    pub mean: Decimal,
    pub std_dev: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyComparison {
    pub days: Vec<DayPair>,
    /// Días de IG sin ninguna barra del futuro.
    pub missing: Vec<NaiveDate>,
    pub return_pairs: usize,
    /// Correlación de Pearson de los retornos diarios (`None` con menos de 3 pares o sin
    /// variación).
    pub correlation: Option<Decimal>,
    pub basis_mean: Option<Decimal>,
    pub basis_std_dev: Option<Decimal>,
    pub by_contract: Vec<ContractBasis>,
}

fn mean(v: &[Decimal]) -> Option<Decimal> {
    (!v.is_empty()).then(|| v.iter().sum::<Decimal>() / Decimal::from(v.len()))
}

/// Desvío muestral (n − 1).
fn std_dev(v: &[Decimal]) -> Option<Decimal> {
    let m = mean(v)?;
    if v.len() < 2 {
        return None;
    }
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<Decimal>() / Decimal::from(v.len() - 1);
    var.sqrt()
}

fn correlation(x: &[Decimal], y: &[Decimal]) -> Option<Decimal> {
    if x.len() < 3 || x.len() != y.len() {
        return None;
    }
    let (mx, my) = (mean(x)?, mean(y)?);
    let cov: Decimal = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let vx: Decimal = x.iter().map(|a| (a - mx) * (a - mx)).sum();
    let vy: Decimal = y.iter().map(|b| (b - my) * (b - my)).sum();
    let den = (vx * vy).sqrt()?;
    (den > Decimal::ZERO).then(|| cov / den)
}

/// Compara las diarias de IG (en orden) con las barras de 1 minuto del futuro (en orden).
pub fn compare_daily(ig: &[DailyCandle], bars: &[FuturesBar]) -> DailyComparison {
    let mut days: Vec<Option<DayPair>> = Vec::with_capacity(ig.len());
    let mut missing = Vec::new();
    for d in ig {
        let from = bars.partition_point(|b| b.start < d.start);
        let group: Vec<&FuturesBar> = bars[from..]
            .iter()
            .take_while(|b| b.start < d.end)
            .filter(|b| b.end() <= d.end)
            .collect();
        match group.last() {
            None => {
                missing.push(d.day);
                days.push(None);
            }
            Some(last) => days.push(Some(DayPair {
                day: d.day,
                ig_close: d.close,
                futures_close: last.close,
                contract_id: last.contract_id,
                rolled: group.iter().any(|b| b.contract_id != last.contract_id),
                basis: d.close - last.close,
            })),
        }
    }
    let (mut rx, mut ry) = (Vec::new(), Vec::new());
    for w in days.windows(2) {
        if let [Some(a), Some(b)] = w
            && a.contract_id == b.contract_id
            && !b.rolled
            && a.ig_close > Decimal::ZERO
            && a.futures_close > Decimal::ZERO
        {
            rx.push(b.ig_close / a.ig_close - Decimal::ONE);
            ry.push(b.futures_close / a.futures_close - Decimal::ONE);
        }
    }
    let days: Vec<DayPair> = days.into_iter().flatten().collect();
    let basis: Vec<Decimal> = days.iter().map(|d| d.basis).collect();
    let mut per: BTreeMap<u64, Vec<Decimal>> = BTreeMap::new();
    for d in &days {
        per.entry(d.contract_id).or_default().push(d.basis);
    }
    DailyComparison {
        return_pairs: rx.len(),
        correlation: correlation(&rx, &ry),
        basis_mean: mean(&basis),
        basis_std_dev: std_dev(&basis),
        by_contract: per
            .into_iter()
            .map(|(contract_id, v)| ContractBasis {
                contract_id,
                days: v.len(),
                mean: mean(&v).unwrap_or_default(),
                std_dev: std_dev(&v),
            })
            .collect(),
        days,
        missing,
    }
}

/// Velas diarias de un futuro (#282), con la misma ventana que las diarias de IG: de medianoche a
/// medianoche en `tz` (`config/candle_sources.toml` `daily_timezone`, la misma zona que
/// `config/ig.toml` `prices_timezone`).
///
/// ```text
/// para cada día D con sesión en el calendario del CFD (`session`, con sus feriados):
///   ventana = [medianoche de D, medianoche de D+1) en tz, cerrada hasta `until`
///   velas = las de 15m de `m15` dentro de la ventana
///   si un vencimiento R (calendario de #270) cae dentro de la ventana, solo las velas desde R:
///     la diaria nunca mezcla dos contratos
///   sin velas, no hay diaria (no se rellena)
/// ```
/// `instrument` es el id con el que se guarda (el derivado, por ejemplo `US500.FUT`).
pub fn futures_daily(
    m15: &[Candle],
    instrument: &InstrumentId,
    session: &crate::Session,
    tz: chrono_tz::Tz,
    rolls: &[DateTime<Utc>],
    until: DateTime<Utc>,
) -> Vec<DailyCandle> {
    let (Some(first), Some(last)) = (m15.first(), m15.last()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut day = first.start.with_timezone(&tz).date_naive();
    let end_day = last.start.with_timezone(&tz).date_naive();
    while day <= end_day {
        let next = day.succ_opt();
        let open = matches!(session.hours(day), Some(Some(_)));
        if let (true, Some((from, to))) = (open, crate::local_day_bounds(day, tz))
            && to <= until
        {
            let from = rolls
                .iter()
                .copied()
                .filter(|r| *r > from && *r < to)
                .max()
                .unwrap_or(from);
            let i = m15.partition_point(|c| c.start < from);
            let group: Vec<&Candle> = m15[i..]
                .iter()
                .take_while(|c| c.start < to)
                .filter(|c| c.end() <= to)
                .collect();
            if let (Some(a), Some(b)) = (group.first(), group.last()) {
                let (window_start, _) = crate::local_day_bounds(day, tz).unwrap_or((from, to));
                out.push(DailyCandle {
                    instrument_id: instrument.clone(),
                    day,
                    start: window_start,
                    end: to,
                    open: a.open,
                    high: group.iter().map(|c| c.high).max().unwrap_or(a.high),
                    low: group.iter().map(|c| c.low).min().unwrap_or(a.low),
                    close: b.close,
                    volume: group.iter().map(|c| c.volume).sum::<Option<Decimal>>(),
                });
            }
        }
        match next {
            Some(n) => day = n,
            None => break,
        }
    }
    out
}

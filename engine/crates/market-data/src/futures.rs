//! Barras de 1 minuto de futuros (ADR 0011, #248): agrupación en velas y verificación de las
//! diarias contra las del CFD de IG. Funciones puras.
//!
//! ```text
//! agrupación (15m, 1h): barras con inicio en [k·paso, (k+1)·paso); la vela existe solo si
//!   cerró (fin ≤ now) y si todas sus barras son del mismo contrato (un vencimiento en el medio
//!   no se mezcla: esa vela no se arma y se cuenta)
//! diaria del futuro para un día de IG: barras con inicio en [inicio, fin) de la diaria de IG;
//!   cierre = el de la última barra
//! base del día = cierre IG − cierre futuro
//! retornos: C[t]/C[t−1] − 1, solo entre diarias seguidas de IG con futuro los dos días, del
//!   mismo contrato y sin vencimiento en t (el salto del rollover no es un movimiento)
//! correlación de Pearson de los retornos (al menos 3 pares); base: media y desvío muestral
//! ```

use std::collections::BTreeMap;

use cfds_core::{Candle, DailyCandle, FuturesBar, InstrumentId, Timeframe};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::{Decimal, MathematicalOps};

/// Velas armadas y cuántas no se armaron por mezclar contratos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregated {
    pub candles: Vec<Candle>,
    pub mixed_contracts: usize,
}

fn bucket(t: DateTime<Utc>, step: TimeDelta) -> DateTime<Utc> {
    let s = step.num_seconds();
    t - TimeDelta::seconds(t.timestamp().rem_euclid(s))
}

/// Agrupa barras de 1 minuto (de un instrumento, en orden) en velas de `timeframe` ya cerradas
/// en `now`. `instrument` es el CFD al que corresponde el futuro.
pub fn aggregate_bars(
    bars: &[FuturesBar],
    instrument: &InstrumentId,
    timeframe: Timeframe,
    now: DateTime<Utc>,
) -> Aggregated {
    let step = timeframe.duration();
    let mut out = Aggregated {
        candles: Vec::new(),
        mixed_contracts: 0,
    };
    let mut i = 0;
    while i < bars.len() {
        let start = bucket(bars[i].start, step);
        let j = i + bars[i..].partition_point(|b| bucket(b.start, step) == start);
        let group = &bars[i..j];
        i = j;
        if start + step > now {
            continue; // todavía no cerró
        }
        if group.iter().any(|b| b.contract_id != group[0].contract_id) {
            out.mixed_contracts += 1;
            continue;
        }
        out.candles.push(Candle {
            instrument_id: instrument.clone(),
            timeframe,
            start,
            open: group[0].open,
            high: group.iter().map(|b| b.high).max().unwrap_or(group[0].high),
            low: group.iter().map(|b| b.low).min().unwrap_or(group[0].low),
            close: group[group.len() - 1].close,
            volume: Some(group.iter().map(|b| Decimal::from(b.volume)).sum()),
        });
    }
    out
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

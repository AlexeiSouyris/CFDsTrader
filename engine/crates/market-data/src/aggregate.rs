//! Vela de 1h armada con las de 15m (spec §3, #150).
//!
//! La spec dice que "las velas de 1h se construyen agregando las de 15m". Una vela de 1h existe
//! solo si están las **cuatro** de 15m de esa hora: con un hueco no se arma (no se inventa).
//!
//! [`compare_h1`] mide cuánto se parece la vela armada a la que da el proveedor. Con IG no tienen
//! por qué coincidir siempre: la base guarda el precio **medio**, y el máximo de 1h de IG es el
//! promedio entre el máximo del bid y el del ask de toda la hora, que pueden caer en cuartos de
//! hora distintos.

use std::collections::BTreeMap;

use cfds_core::{Candle, Timeframe};
use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AggregateError {
    #[error("la serie no es de velas de 15m de un solo instrumento ({0})")]
    MixedSeries(String),
    #[error("las velas de 15m no están en orden estricto en {0}")]
    OutOfOrder(DateTime<Utc>),
}

/// Velas de 1h de las horas con sus cuatro velas de 15m, en orden. `m15` tiene que ser de un
/// solo instrumento, solo de 15m y en orden estricto.
pub fn aggregate_h1(m15: &[Candle]) -> Result<Vec<Candle>, AggregateError> {
    let Some(first) = m15.first() else {
        return Ok(Vec::new());
    };
    for (i, c) in m15.iter().enumerate() {
        if c.timeframe != Timeframe::M15 || c.instrument_id != first.instrument_id {
            return Err(AggregateError::MixedSeries(format!(
                "{} {} en {}",
                c.instrument_id, c.timeframe, c.start
            )));
        }
        if i > 0 && c.start <= m15[i - 1].start {
            return Err(AggregateError::OutOfOrder(c.start));
        }
    }
    let mut hours: BTreeMap<DateTime<Utc>, Vec<&Candle>> = BTreeMap::new();
    for c in m15 {
        let hour = c
            .start
            .duration_trunc(TimeDelta::hours(1))
            .map_err(|_| AggregateError::OutOfOrder(c.start))?;
        hours.entry(hour).or_default().push(c);
    }
    let step = Timeframe::M15.duration();
    Ok(hours
        .into_iter()
        .filter(|(hour, cs)| {
            cs.len() == 4
                && cs
                    .iter()
                    .zip(0..4)
                    .all(|(c, k)| c.start == *hour + step * k)
        })
        .map(|(hour, cs)| Candle {
            instrument_id: first.instrument_id.clone(),
            timeframe: Timeframe::H1,
            start: hour,
            open: cs[0].open,
            high: cs.iter().map(|c| c.high).max().unwrap_or(cs[0].high),
            low: cs.iter().map(|c| c.low).min().unwrap_or(cs[0].low),
            close: cs[3].close,
            volume: cs.iter().map(|c| c.volume).sum::<Option<Decimal>>(),
        })
        .collect())
}

/// Resultado de comparar velas de 1h del proveedor contra las armadas.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct H1Comparison {
    /// Horas que están en las dos series.
    pub compared: usize,
    /// De esas, cuántas coinciden exacto en apertura, máximo, mínimo y cierre.
    pub identical: usize,
    /// Diferencia absoluta máxima por campo (apertura, máximo, mínimo, cierre), en puntos.
    pub max_abs_diff: [Decimal; 4],
    /// Horas con alguna diferencia, con la diferencia de cada campo (proveedor − armada).
    pub differing: Vec<(DateTime<Utc>, [Decimal; 4])>,
    /// Horas que tiene el proveedor y no se pudieron armar (falta alguna de 15m).
    pub only_provider: Vec<DateTime<Utc>>,
    /// Horas armadas que el proveedor no tiene.
    pub only_built: Vec<DateTime<Utc>>,
}

/// Compara las velas de 1h del proveedor (`provider`) con las armadas (`built`).
pub fn compare_h1(provider: &[Candle], built: &[Candle]) -> H1Comparison {
    let by_start = |cs: &[Candle]| -> BTreeMap<DateTime<Utc>, [Decimal; 4]> {
        cs.iter()
            .map(|c| (c.start, [c.open, c.high, c.low, c.close]))
            .collect()
    };
    let (p, b) = (by_start(provider), by_start(built));
    let mut out = H1Comparison::default();
    for (start, pv) in &p {
        let Some(bv) = b.get(start) else {
            out.only_provider.push(*start);
            continue;
        };
        out.compared += 1;
        let diff: [Decimal; 4] = std::array::from_fn(|i| pv[i] - bv[i]);
        if diff.iter().all(Decimal::is_zero) {
            out.identical += 1;
        } else {
            out.differing.push((*start, diff));
        }
        for (max, d) in out.max_abs_diff.iter_mut().zip(diff) {
            *max = (*max).max(d.abs());
        }
    }
    out.only_built = b.keys().filter(|s| !p.contains_key(s)).copied().collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfds_core::InstrumentId;
    use rust_decimal::dec;

    fn id(s: &str) -> InstrumentId {
        s.parse().unwrap()
    }

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn c(start: &str, o: Decimal, h: Decimal, l: Decimal, cl: Decimal) -> Candle {
        Candle {
            instrument_id: id("US100"),
            timeframe: Timeframe::M15,
            start: t(start),
            open: o,
            high: h,
            low: l,
            close: cl,
            volume: None,
        }
    }

    fn hour_15_00() -> Vec<Candle> {
        vec![
            c(
                "2026-11-20T15:00:00Z",
                dec!(100),
                dec!(105),
                dec!(99),
                dec!(104),
            ),
            c(
                "2026-11-20T15:15:00Z",
                dec!(104),
                dec!(110),
                dec!(103),
                dec!(108),
            ),
            c(
                "2026-11-20T15:30:00Z",
                dec!(108),
                dec!(109),
                dec!(95),
                dec!(96),
            ),
            c(
                "2026-11-20T15:45:00Z",
                dec!(96),
                dec!(101),
                dec!(96),
                dec!(100.5),
            ),
        ]
    }

    #[test]
    fn four_quarters_make_one_hour() {
        let h = aggregate_h1(&hour_15_00()).unwrap();
        assert_eq!(h.len(), 1);
        let h = &h[0];
        assert_eq!(h.timeframe, Timeframe::H1);
        assert_eq!(h.start, t("2026-11-20T15:00:00Z"));
        assert_eq!(
            (h.open, h.high, h.low, h.close),
            (dec!(100), dec!(110), dec!(95), dec!(100.5))
        );
        assert_eq!(h.volume, None);
    }

    #[test]
    fn an_incomplete_hour_is_not_built() {
        let mut m15 = hour_15_00();
        m15.remove(2); // falta la de las 15:30
        assert!(aggregate_h1(&m15).unwrap().is_empty());
        // Media hora de sesión (14:30 y 14:45) tampoco hace una vela de 1h.
        let half = vec![
            c("2026-11-20T14:30:00Z", dec!(1), dec!(1), dec!(1), dec!(1)),
            c("2026-11-20T14:45:00Z", dec!(1), dec!(1), dec!(1), dec!(1)),
        ];
        assert!(aggregate_h1(&half).unwrap().is_empty());
    }

    #[test]
    fn volume_is_summed_only_when_all_four_have_it() {
        let mut m15 = hour_15_00();
        for (i, x) in m15.iter_mut().enumerate() {
            x.volume = Some(Decimal::from(i + 1));
        }
        assert_eq!(aggregate_h1(&m15).unwrap()[0].volume, Some(dec!(10)));
        m15[1].volume = None;
        assert_eq!(aggregate_h1(&m15).unwrap()[0].volume, None);
    }

    #[test]
    fn mixed_or_unordered_series_are_errors() {
        let mut other = hour_15_00();
        other[1].instrument_id = id("US500");
        assert!(matches!(
            aggregate_h1(&other),
            Err(AggregateError::MixedSeries(_))
        ));
        let mut h1 = hour_15_00();
        h1[0].timeframe = Timeframe::H1;
        assert!(matches!(
            aggregate_h1(&h1),
            Err(AggregateError::MixedSeries(_))
        ));
        let mut swapped = hour_15_00();
        swapped.swap(0, 1);
        assert!(matches!(
            aggregate_h1(&swapped),
            Err(AggregateError::OutOfOrder(_))
        ));
        assert!(aggregate_h1(&[]).unwrap().is_empty());
    }

    #[test]
    fn comparison_counts_identical_and_reports_differences() {
        let built = aggregate_h1(&hour_15_00()).unwrap();
        let mut ig = built.clone();
        assert_eq!(compare_h1(&ig, &built).identical, 1);
        // El máximo de IG sale del bid y el ask por separado: puede quedar un poco arriba.
        ig[0].high = dec!(110.3);
        let extra = Candle {
            start: t("2026-11-20T16:00:00Z"),
            ..ig[0].clone()
        };
        ig.push(extra);
        let r = compare_h1(&ig, &built);
        assert_eq!((r.compared, r.identical), (1, 0));
        assert_eq!(r.max_abs_diff, [dec!(0), dec!(0.3), dec!(0), dec!(0)]);
        assert_eq!(
            r.differing,
            vec![(
                t("2026-11-20T15:00:00Z"),
                [dec!(0), dec!(0.3), dec!(0), dec!(0)]
            )]
        );
        assert_eq!(r.only_provider, vec![t("2026-11-20T16:00:00Z")]);
        assert!(r.only_built.is_empty());
        let r = compare_h1(&[], &built);
        assert_eq!(r.only_built, vec![t("2026-11-20T15:00:00Z")]);
    }
}

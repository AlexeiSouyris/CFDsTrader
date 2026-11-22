//! Comparación en memoria (ADR 0009 §1). Función pura: nuestros cierres y las filas del ETF entran,
//! solo veredictos salen. Ningún número de Tiingo sale de acá.
//!
//! Se comparan **variaciones de un día al otro**, no precios (un ETF no tiene el nivel del índice):
//! - nuestra: cierre del día contra el del día anterior del ETF;
//! - del ETF: `(cierre + dividendo del día) / cierre anterior − 1`, en puntos porcentuales;
//! - un día con `splitFactor` distinto de 1 queda "sin dato".
//!
//! Nuestro cierre preferido es la vela de 15m que termina a la hora de cierre del ETF; si falta en
//! alguno de los dos días, la vela diaria (con su propia tolerancia); si tampoco, "sin dato".

use std::collections::BTreeMap;

use cfds_core::{Candle, DailyCandle, Timeframe};
use chrono::{NaiveDate, TimeDelta};
use rust_decimal::Decimal;

use crate::config::VerifyConfig;
use crate::eod::EodRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    M15Close,
    DailyClose,
    None,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::M15Close => "m15_close",
            Method::DailyClose => "daily_close",
            Method::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    Match,
    Mismatch,
    NoData,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Match => "match",
            Verdict::Mismatch => "mismatch",
            Verdict::NoData => "no_data",
        }
    }
}

/// Veredicto de un día: lo único que se guarda (opción B). Sin números a propósito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayVerdict {
    pub day: NaiveDate,
    pub method: Method,
    pub verdict: Verdict,
}

/// Nuestros cierres de un día (fecha local de la bolsa del ETF).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OurClose {
    /// Cierre de la vela de 15m que termina a la hora de referencia.
    pub m15: Option<Decimal>,
    /// Cierre de la vela diaria.
    pub daily: Option<Decimal>,
}

/// Arma nuestros cierres por día a partir de velas de 15m y diarias ya cerradas y guardadas.
pub fn our_closes(
    m15: &[Candle],
    daily: &[DailyCandle],
    cfg: &VerifyConfig,
) -> BTreeMap<NaiveDate, OurClose> {
    let mut out: BTreeMap<NaiveDate, OurClose> = BTreeMap::new();
    for c in m15.iter().filter(|c| c.timeframe == Timeframe::M15) {
        let end = (c.start + TimeDelta::minutes(15)).with_timezone(&cfg.timezone);
        if end.time() == cfg.reference_close {
            out.entry(end.date_naive()).or_default().m15 = Some(c.close);
        }
    }
    for d in daily {
        out.entry(d.day).or_default().daily = Some(d.close);
    }
    out
}

/// Veredicto de cada día del ETF que tiene un día anterior en `etf`.
pub fn compare(
    etf: &[EodRow],
    ours: &BTreeMap<NaiveDate, OurClose>,
    tolerance_m15_pp: Decimal,
    tolerance_daily_pp: Decimal,
) -> Vec<DayVerdict> {
    let hundred = Decimal::ONE_HUNDRED;
    etf.windows(2)
        .map(|w| {
            let (prev, today) = (&w[0], &w[1]);
            let none = DayVerdict {
                day: today.day,
                method: Method::None,
                verdict: Verdict::NoData,
            };
            if today.split_factor != Decimal::ONE {
                return none;
            }
            let (p, t) = (ours.get(&prev.day), ours.get(&today.day));
            let pick = |f: fn(&OurClose) -> Option<Decimal>| match (p.and_then(f), t.and_then(f)) {
                (Some(a), Some(b)) if a > Decimal::ZERO => Some((a, b)),
                _ => None,
            };
            let (method, (a, b), tolerance) = if let Some(x) = pick(|o| o.m15) {
                (Method::M15Close, x, tolerance_m15_pp)
            } else if let Some(x) = pick(|o| o.daily) {
                (Method::DailyClose, x, tolerance_daily_pp)
            } else {
                return none;
            };
            let ours_pp = (b - a) / a * hundred;
            let etf_pp = (today.close + today.div_cash - prev.close) / prev.close * hundred;
            let verdict = if (ours_pp - etf_pp).abs() <= tolerance {
                Verdict::Match
            } else {
                Verdict::Mismatch
            };
            DayVerdict {
                day: today.day,
                method,
                verdict,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    // Todo SINTÉTICO, armado a mano.
    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, d).unwrap()
    }

    fn row(d: u32, close: Decimal, div: Decimal) -> EodRow {
        EodRow {
            day: day(d),
            close,
            div_cash: div,
            split_factor: Decimal::ONE,
        }
    }

    fn ours(items: &[(u32, Option<Decimal>, Option<Decimal>)]) -> BTreeMap<NaiveDate, OurClose> {
        items
            .iter()
            .map(|&(d, m15, daily)| (day(d), OurClose { m15, daily }))
            .collect()
    }

    #[test]
    fn same_change_matches_and_a_different_one_does_not() {
        let etf = [
            row(5, dec!(100), dec!(0)),
            row(6, dec!(101), dec!(0)),
            row(7, dec!(102.01), dec!(0)),
        ];
        // Día 6: +1 % y +1 %. Día 7: ETF +1 %, nuestro −1 %.
        let o = ours(&[
            (5, Some(dec!(5000)), None),
            (6, Some(dec!(5050)), None),
            (7, Some(dec!(4999.5)), None),
        ]);
        let v = compare(&etf, &o, dec!(0.10), dec!(0.75));
        assert_eq!(v.len(), 2);
        assert_eq!(
            (v[0].method, v[0].verdict),
            (Method::M15Close, Verdict::Match)
        );
        assert_eq!(v[1].verdict, Verdict::Mismatch);
    }

    #[test]
    fn dividend_is_added_back_on_the_ex_date() {
        // El ETF baja 0,5 por el dividendo; el índice no se mueve.
        let etf = [row(5, dec!(100), dec!(0)), row(6, dec!(99.5), dec!(0.5))];
        let o = ours(&[(5, Some(dec!(5000)), None), (6, Some(dec!(5000)), None)]);
        assert_eq!(
            compare(&etf, &o, dec!(0.10), dec!(0.75))[0].verdict,
            Verdict::Match
        );
    }

    #[test]
    fn falls_back_to_daily_with_its_own_tolerance_and_then_to_no_data() {
        let etf = [
            row(5, dec!(100), dec!(0)),
            row(6, dec!(100.5), dec!(0)),
            row(7, dec!(100.5), dec!(0)),
        ];
        // Día 6 solo con diarias (diferencia 0,5 pp: entra en 0,75). Día 7 sin nuestro dato.
        let o = ours(&[
            (5, Some(dec!(5000)), Some(dec!(5000))),
            (6, None, Some(dec!(5000))),
        ]);
        let v = compare(&etf, &o, dec!(0.10), dec!(0.75));
        assert_eq!(
            (v[0].method, v[0].verdict),
            (Method::DailyClose, Verdict::Match)
        );
        assert_eq!((v[1].method, v[1].verdict), (Method::None, Verdict::NoData));
    }

    #[test]
    fn a_split_day_is_no_data() {
        let mut split = row(6, dec!(50), dec!(0));
        split.split_factor = dec!(2);
        let etf = [row(5, dec!(100), dec!(0)), split];
        let o = ours(&[(5, Some(dec!(5000)), None), (6, Some(dec!(5000)), None)]);
        assert_eq!(
            compare(&etf, &o, dec!(0.10), dec!(0.75))[0].verdict,
            Verdict::NoData
        );
    }

    #[test]
    fn our_m15_close_is_the_candle_ending_at_the_reference_time() {
        use cfds_core::InstrumentId;
        let cfg = crate::TiingoConfig::from_toml(include_str!("../../../../config/tiingo.toml"))
            .unwrap()
            .verify;
        let id: InstrumentId = "US500".parse().unwrap();
        let candle = |start: &str, close: Decimal| Candle {
            instrument_id: id.clone(),
            timeframe: Timeframe::M15,
            start: start.parse().unwrap(),
            open: close,
            high: close,
            low: close,
            close,
            volume: None,
        };
        // 15:45–16:00 en Nueva York (invierno, UTC−5) y la vela siguiente, que no cuenta.
        let m15 = [
            candle("2026-01-05T20:45:00Z", dec!(1)),
            candle("2026-01-05T21:00:00Z", dec!(2)),
        ];
        let o = our_closes(&m15, &[], &cfg);
        assert_eq!(o[&day(5)].m15, Some(dec!(1)));
        // En verano (UTC−4) la misma hora local es otra hora UTC.
        let summer = [candle("2026-07-06T19:45:00Z", dec!(3))];
        let o = our_closes(&summer, &[], &cfg);
        assert_eq!(
            o[&NaiveDate::from_ymd_opt(2026, 7, 6).unwrap()].m15,
            Some(dec!(3))
        );
    }
}

//! Tercio de la volatilidad realizada diaria frente a su propia historia (spec §13, spec ML §3).
//!
//! ```text
//! desde_t   = el mismo día de t, `años` antes (el 29 de febrero pasa al 28)
//! historia  = volatilidades de los días anteriores a t con día ≥ desde_t
//! rango_t   = (# valores de la historia menores que vol_t) / # historia
//! tercio_t  = bajo si rango < 1/3, medio si rango < 2/3, alto si no
//! ```
//! - `None` si `t` no tiene volatilidad, si el primer día con volatilidad es posterior a
//!   `desde_t` (no hay `años` completos de historia) o si la historia queda vacía.
//! - Sin repintado: el valor de `t` solo depende de los días `0..=t`.
//! - Los cortes se comparan en enteros (`3 · menores < n`): exacto, sin redondeo.

use cfds_core::VolTercile;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;

/// El mismo día `years` años antes (el 29 de febrero pasa al 28).
fn years_before(day: NaiveDate, years: u32) -> Option<NaiveDate> {
    let year = day.year() - i32::try_from(years).ok()?;
    day.with_year(year)
        .or_else(|| NaiveDate::from_ymd_opt(year, day.month(), 28))
}

/// Tercio de cada día de una serie (días con sesión de un instrumento, en orden, con su
/// volatilidad realizada), alineado por índice.
pub fn vol_terciles(
    series: &[(NaiveDate, Option<Decimal>)],
    years: u32,
) -> Vec<Option<VolTercile>> {
    let first_with_vol = series.iter().position(|(_, v)| v.is_some());
    (0..series.len())
        .map(|t| {
            let (day, current) = series[t];
            let current = current?;
            let since = years_before(day, years)?;
            let first = first_with_vol.filter(|f| *f < t)?;
            if series[first].0 > since {
                return None;
            }
            let history = series[first..t]
                .iter()
                .filter(|(d, _)| *d >= since)
                .filter_map(|(_, v)| *v);
            let (mut below, mut n) = (0usize, 0usize);
            for v in history {
                n += 1;
                below += usize::from(v < current);
            }
            if n == 0 {
                return None;
            }
            Some(if 3 * below < n {
                VolTercile::Low
            } else if 3 * below < 2 * n {
                VolTercile::Mid
            } else {
                VolTercile::High
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn needs_full_years_of_history() {
        let mut s: Vec<_> = (1..=9)
            .map(|i| (day(&format!("2023-12-0{i}")), Some(Decimal::from(i))))
            .collect();
        s.push((day("2024-01-10"), Some(Decimal::from(5))));
        // Solo diciembre: no llega a un año completo.
        assert_eq!(vol_terciles(&s, 1)[9], None);
        // Con un dato de hace más de un año alcanza: 4 de los 9 de diciembre están por debajo
        // de 5 (el dato viejo queda fuera de la ventana). 4/9 es el segundo tercio.
        s.insert(0, (day("2022-12-01"), Some(Decimal::from(100))));
        assert_eq!(vol_terciles(&s, 1)[10], Some(VolTercile::Mid));
        // Historia que existe pero toda fuera de la ventana: sin dato, no una división por cero.
        let far = [
            (day("2022-12-01"), Some(Decimal::ONE)),
            (day("2024-01-10"), Some(Decimal::ONE)),
        ];
        assert_eq!(vol_terciles(&far, 1)[1], None);
        // Sin volatilidad ese día (calentamiento): sin dato.
        assert_eq!(vol_terciles(&[(day("2024-01-10"), None)], 1), vec![None]);
    }

    #[test]
    fn never_repaints() {
        let s: Vec<_> = (0..800)
            .map(|i| {
                let d = day("2021-01-01") + chrono::Days::new(i);
                (d, (i > 20).then(|| Decimal::from((i * 7919) % 1000)))
            })
            .collect();
        let full = vol_terciles(&s, 1);
        for t in [0, 21, 365, 366, 500, 799] {
            assert_eq!(vol_terciles(&s[..=t], 1)[t], full[t]);
        }
        assert!(full.iter().flatten().count() > 300);
    }
}

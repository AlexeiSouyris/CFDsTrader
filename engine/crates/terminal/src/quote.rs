//! Último precio y variación del día de un instrumento, a partir de velas **ya cerradas** que da
//! IBKR. Es presentación de dos precios (último y cierre de referencia): no es un indicador ni una
//! señal.

use cfds_ibkr::RawBar;
use cfds_market_data::session_day;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::Decimal;

/// De dónde sale el último precio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Cierre de la última vela de 1 minuto cerrada.
    Minute,
    /// No hay velas de 1 minuto recientes (mercado cerrado): cierre de la última vela diaria cerrada.
    Daily,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    pub last: Decimal,
    /// Cuándo cerró la vela del último precio (UTC).
    pub at: DateTime<Utc>,
    pub basis: Basis,
    /// Cierre de referencia (la diaria de la sesión anterior a la del último precio), si hay.
    pub reference: Option<Decimal>,
}

impl Quote {
    pub fn change(&self) -> Option<Decimal> {
        self.reference.map(|r| session_day::change(self.last, r))
    }

    /// Variación porcentual, redondeada a 2 decimales. `None` sin referencia o con referencia 0.
    pub fn change_pct(&self) -> Option<Decimal> {
        session_day::change_pct(self.last, self.reference?)
    }
}

/// Día de sesión de una vela intradía: el criterio compartido con el gateway (#551), en
/// `cfds_market_data::session_day`.
pub use cfds_market_data::session_day::session_date;

/// Arma el último precio y su referencia.
///
/// - `minute`: la última vela de 1 minuto cerrada que se vio (o ninguna).
/// - `daily`: velas diarias **cerradas**, en orden (IBKR las fecha por día de sesión, 00:00 UTC).
/// - `offset`: para saber a qué día de sesión pertenece la vela de 1 minuto ([`session_date`]).
///
/// Con vela de 1 minuto, el último precio es su cierre y la referencia, el cierre de la última diaria
/// de una sesión **anterior** a la suya. Sin ella (fin de semana), el último precio es el cierre de la
/// última diaria y la referencia, el de la anterior.
pub fn quote(minute: Option<&RawBar>, daily: &[RawBar], offset: TimeDelta) -> Option<Quote> {
    if let Some(m) = minute {
        let day = session_date(m.start, offset);
        let reference = daily
            .iter()
            .rev()
            .find(|d| d.start.date_naive() < day)
            .map(|d| d.close);
        return Some(Quote {
            last: m.close,
            at: m.start + TimeDelta::minutes(1),
            basis: Basis::Minute,
            reference,
        });
    }
    let d = daily.last()?;
    let prev = daily.len().checked_sub(2).and_then(|i| daily.get(i));
    Some(Quote {
        last: d.close,
        at: d.start + TimeDelta::days(1),
        basis: Basis::Daily,
        reference: prev.map(|p| p.close),
    })
}

/// Rango y volumen de la sesión de la última vela de 1 minuto: máximo, mínimo y suma de volúmenes
/// de las velas de esa misma sesión (presentación de lo que ya pasó, no un indicador).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayStats {
    pub session: NaiveDate,
    pub high: Decimal,
    pub low: Decimal,
    /// `None` si alguna vela no trae volumen.
    pub volume: Option<Decimal>,
}

pub fn day_stats(minutes: &[RawBar], offset: TimeDelta) -> Option<DayStats> {
    let last = minutes.last()?;
    let session = session_date(last.start, offset);
    let same: Vec<&RawBar> = minutes
        .iter()
        .filter(|b| session_date(b.start, offset) == session)
        .collect();
    Some(DayStats {
        session,
        high: same.iter().map(|b| b.high).max()?,
        low: same.iter().map(|b| b.low).min()?,
        volume: same.iter().map(|b| b.volume).sum(),
    })
}

/// Junta velas de 1 minuto nuevas con las que ya había (sin repetir inicio, en orden) y descarta
/// las anteriores a `keep_from`.
pub fn merge_minutes(old: &mut Vec<RawBar>, new: Vec<RawBar>, keep_from: DateTime<Utc>) {
    for b in new {
        match old.binary_search_by_key(&b.start, |x| x.start) {
            Ok(i) => old[i] = b,
            Err(i) => old.insert(i, b),
        }
    }
    old.retain(|b| b.start >= keep_from);
}

/// Desde cuándo pedir velas de 1 minuto para el último precio: desde la última vista si es reciente;
/// si no, `max_back` hacia atrás.
pub fn minute_window_start(
    last_seen: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    max_back: TimeDelta,
) -> DateTime<Utc> {
    let floor = now - max_back;
    match last_seen {
        Some(t) if t > floor => t,
        _ => floor,
    }
}

/// Precio con al menos 2 decimales (los que traiga IBKR si son más).
pub fn fmt_price(p: Decimal) -> String {
    let n = p.normalize();
    if n.scale() < 2 {
        format!("{:.2}", n)
    } else {
        n.to_string()
    }
}

/// Variación con signo siempre visible (`+12.50`, `-3.25`, `0.00`), para que se lea sin colores.
pub fn fmt_signed(d: Decimal) -> String {
    let s = fmt_price(d.abs());
    if d.is_sign_negative() && !d.is_zero() {
        format!("-{s}")
    } else if d.is_zero() {
        s
    } else {
        format!("+{s}")
    }
}

/// Demora corta: `45 s`, `12 min`, `3 h`, `2 d`.
pub fn fmt_age(age: TimeDelta) -> String {
    let s = age.num_seconds().max(0);
    if s < 60 {
        format!("{s} s")
    } else if s < 3600 {
        format!("{} min", s / 60)
    } else if s < 86_400 {
        format!("{} h", s / 3600)
    } else {
        format!("{} d", s / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::dec;

    fn t(d: u32, h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, d, h, m, 0).unwrap()
    }

    fn bar(start: DateTime<Utc>, close: Decimal) -> RawBar {
        RawBar {
            start,
            open: close,
            high: close,
            low: close,
            close,
            volume: None,
        }
    }

    const FUT: TimeDelta = TimeDelta::hours(2);
    const STK: TimeDelta = TimeDelta::hours(-5);

    #[test]
    fn session_dates() {
        // Futuro: la sesión que abre el 24 a las 23:00 UTC es la del 25.
        assert_eq!(session_date(t(24, 23, 0), FUT), t(25, 0, 0).date_naive());
        assert_eq!(session_date(t(25, 21, 59), FUT), t(25, 0, 0).date_naive());
        // Acción: el horario extendido que termina a las 00:59 UTC del 25 es del 24.
        assert_eq!(session_date(t(25, 0, 59), STK), t(24, 0, 0).date_naive());
        assert_eq!(session_date(t(25, 9, 0), STK), t(25, 0, 0).date_naive());
    }

    #[test]
    fn future_intraday_uses_previous_session_close() {
        let daily = [bar(t(23, 0, 0), dec!(100)), bar(t(24, 0, 0), dec!(200))];
        let m = bar(t(25, 14, 30), dec!(210));
        let q = quote(Some(&m), &daily, FUT).unwrap();
        assert_eq!(q.basis, Basis::Minute);
        assert_eq!(q.last, dec!(210));
        assert_eq!(q.at, t(25, 14, 31));
        assert_eq!(q.reference, Some(dec!(200)));
        assert_eq!(q.change(), Some(dec!(10)));
        assert_eq!(q.change_pct(), Some(dec!(5.00)));
        // 23:30 UTC del 25 ya es la sesión del 26: la referencia es la diaria del 25 si ya está.
        let daily = [bar(t(24, 0, 0), dec!(200)), bar(t(25, 0, 0), dec!(205))];
        let m = bar(t(25, 23, 30), dec!(206));
        assert_eq!(
            quote(Some(&m), &daily, FUT).unwrap().reference,
            Some(dec!(205))
        );
    }

    #[test]
    fn stock_after_hours_keeps_its_session() {
        // QQQ a las 00:59 UTC del 25 es la sesión del 24: referencia, la diaria del 23.
        let daily = [bar(t(23, 0, 0), dec!(600)), bar(t(24, 0, 0), dec!(635))];
        let m = bar(t(25, 0, 59), dec!(636));
        let q = quote(Some(&m), &daily, STK).unwrap();
        assert_eq!(q.reference, Some(dec!(600)));
        assert_eq!(q.change(), Some(dec!(36)));
    }

    #[test]
    fn negative_change_and_rounding() {
        let daily = [bar(t(24, 0, 0), dec!(3))];
        let m = bar(t(25, 1, 0), dec!(2));
        let q = quote(Some(&m), &daily, FUT).unwrap();
        assert_eq!(q.change(), Some(dec!(-1)));
        assert_eq!(q.change_pct(), Some(dec!(-33.33)));
    }

    #[test]
    fn without_minute_falls_back_to_daily_closes() {
        let daily = [bar(t(19, 0, 0), dec!(100)), bar(t(20, 0, 0), dec!(90))];
        let q = quote(None, &daily, FUT).unwrap();
        assert_eq!(q.basis, Basis::Daily);
        assert_eq!(q.last, dec!(90));
        assert_eq!(q.at, t(21, 0, 0));
        assert_eq!(q.reference, Some(dec!(100)));
        assert_eq!(q.change_pct(), Some(dec!(-10.00)));
    }

    #[test]
    fn missing_data() {
        assert_eq!(quote(None, &[], FUT), None);
        let m = bar(t(25, 1, 0), dec!(5));
        let q = quote(Some(&m), &[], FUT).unwrap();
        assert_eq!(q.reference, None);
        assert_eq!(q.change(), None);
        assert_eq!(q.change_pct(), None);
        let one = [bar(t(24, 0, 0), dec!(7))];
        let q = quote(None, &one, FUT).unwrap();
        assert_eq!((q.last, q.reference), (dec!(7), None));
        let zero_ref = Quote {
            last: dec!(1),
            at: t(25, 0, 0),
            basis: Basis::Minute,
            reference: Some(Decimal::ZERO),
        };
        assert_eq!(zero_ref.change_pct(), None);
    }

    #[test]
    fn day_range_and_volume_of_last_session() {
        let v = |start, h: Decimal, l: Decimal, vol: Option<Decimal>| RawBar {
            start,
            open: l,
            high: h,
            low: l,
            close: h,
            volume: vol,
        };
        let bars = vec![
            v(t(24, 21, 0), dec!(500), dec!(1), Some(dec!(9))), // sesión del 24
            v(t(24, 23, 0), dec!(110), dec!(100), Some(dec!(2))), // sesión del 25
            v(t(25, 14, 0), dec!(120), dec!(105), Some(dec!(3))),
        ];
        let d = day_stats(&bars, FUT).unwrap();
        assert_eq!(d.session, t(25, 0, 0).date_naive());
        assert_eq!(
            (d.high, d.low, d.volume),
            (dec!(120), dec!(100), Some(dec!(5)))
        );
        let mut nv = bars.clone();
        nv[2].volume = None;
        assert_eq!(day_stats(&nv, FUT).unwrap().volume, None);
        assert_eq!(day_stats(&[], FUT), None);
    }

    #[test]
    fn merge_keeps_order_replaces_and_trims() {
        let mut old = vec![bar(t(25, 1, 0), dec!(1)), bar(t(25, 1, 1), dec!(2))];
        merge_minutes(
            &mut old,
            vec![bar(t(25, 1, 1), dec!(3)), bar(t(25, 1, 2), dec!(4))],
            t(25, 1, 1),
        );
        let got: Vec<Decimal> = old.iter().map(|b| b.close).collect();
        assert_eq!(got, vec![dec!(3), dec!(4)]);
    }

    #[test]
    fn window_starts_at_last_seen_or_floor() {
        let now = t(25, 12, 0);
        let max = TimeDelta::hours(24);
        assert_eq!(minute_window_start(None, now, max), t(24, 12, 0));
        assert_eq!(
            minute_window_start(Some(t(25, 11, 40)), now, max),
            t(25, 11, 40)
        );
        assert_eq!(
            minute_window_start(Some(t(20, 0, 0)), now, max),
            t(24, 12, 0)
        );
    }

    #[test]
    fn formats() {
        assert_eq!(fmt_price(dec!(26222.75)), "26222.75");
        assert_eq!(fmt_price(dec!(512.3)), "512.30");
        assert_eq!(fmt_price(dec!(500)), "500.00");
        assert_eq!(fmt_price(dec!(1.08345)), "1.08345");
        assert_eq!(fmt_price(dec!(26222.7500)), "26222.75");
        assert_eq!(fmt_signed(dec!(12.5)), "+12.50");
        assert_eq!(fmt_signed(dec!(-3.25)), "-3.25");
        assert_eq!(fmt_signed(dec!(0)), "0.00");
        assert_eq!(fmt_age(TimeDelta::seconds(45)), "45 s");
        assert_eq!(fmt_age(TimeDelta::seconds(725)), "12 min");
        assert_eq!(fmt_age(TimeDelta::hours(3)), "3 h");
        assert_eq!(fmt_age(TimeDelta::days(2)), "2 d");
        assert_eq!(fmt_age(TimeDelta::seconds(-5)), "0 s");
    }
}

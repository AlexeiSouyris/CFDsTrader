//! Día de sesión de una vela intradía y variación contra el cierre de una sesión anterior (#541,
//! #551). Es el **único** criterio: lo usan la terminal `bolsa` (`cfds-terminal`) y el gateway
//! (`GET /api/v1/ibkr/watchlist`). La variación es presentación de dos precios (último y cierre de
//! referencia): no es un indicador ni una señal.
//!
//! El corrimiento sale de `[session]` de `config/terminal.toml`: -5 h para acciones de EE.UU. (el
//! horario extendido 04:00–20:00 ET queda en su día, con horario de verano o sin él) y +2 h para
//! futuros de CME y el resto (abren 17:00 CT del día anterior y fechan la sesión por el día en que
//! cierran).

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;

/// Corrimientos del día de sesión por tipo de contrato (`[session]` de `config/terminal.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionOffsets {
    pub stock: TimeDelta,
    pub other: TimeDelta,
}

impl SessionOffsets {
    /// Valida los corrimientos en horas: cada uno entre -12 y 12. El error nombra el campo.
    pub fn from_hours(stock: i64, other: i64) -> Result<Self, (&'static str, String)> {
        for (field, h) in [
            ("session.stock_offset_hours", stock),
            ("session.other_offset_hours", other),
        ] {
            if !(-12..=12).contains(&h) {
                return Err((field, "tiene que estar entre -12 y 12".into()));
            }
        }
        Ok(Self {
            stock: TimeDelta::hours(stock),
            other: TimeDelta::hours(other),
        })
    }

    /// Lee solo la tabla `[session]` de un TOML (el resto del archivo se ignora).
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            session: RawSession,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawSession {
            stock_offset_hours: i64,
            other_offset_hours: i64,
        }
        let r = toml::from_str::<Raw>(text)
            .map_err(|e| e.to_string())?
            .session;
        Self::from_hours(r.stock_offset_hours, r.other_offset_hours)
            .map_err(|(field, problem)| format!("{field}: {problem}"))
    }

    /// El corrimiento de una acción o ETF (`stock = true`) o de cualquier otro contrato.
    pub fn offset(&self, stock: bool) -> TimeDelta {
        if stock { self.stock } else { self.other }
    }
}

/// Día de sesión de una vela intradía: la fecha de `start + offset`.
pub fn session_date(start: DateTime<Utc>, offset: TimeDelta) -> NaiveDate {
    (start + offset).date_naive()
}

/// Primer instante (UTC) del día de sesión `day`: una vela con inicio anterior es de una sesión
/// anterior ([`session_date`] menor que `day`).
pub fn session_start(day: NaiveDate, offset: TimeDelta) -> DateTime<Utc> {
    day.and_hms_opt(0, 0, 0)
        .expect("medianoche existe siempre")
        .and_utc()
        - offset
}

/// Último menos referencia.
pub fn change(last: Decimal, reference: Decimal) -> Decimal {
    last - reference
}

/// Variación porcentual, redondeada a 2 decimales. `None` con referencia 0.
pub fn change_pct(last: Decimal, reference: Decimal) -> Option<Decimal> {
    if reference.is_zero() {
        return None;
    }
    Some(((last - reference) / reference * Decimal::ONE_HUNDRED).round_dp(2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    const TERMINAL: &str = include_str!("../../../../config/terminal.toml");

    #[test]
    fn reads_the_session_of_the_terminal_config() {
        let o = SessionOffsets::from_toml(TERMINAL).unwrap();
        assert_eq!(o.offset(true), TimeDelta::hours(-5));
        assert_eq!(o.offset(false), TimeDelta::hours(2));
        let bad = TERMINAL.replace("other_offset_hours = 2", "other_offset_hours = 13");
        assert_ne!(bad, TERMINAL);
        assert!(
            SessionOffsets::from_toml(&bad)
                .unwrap_err()
                .contains("other_offset_hours")
        );
        assert!(SessionOffsets::from_toml("[x]\na = 1").is_err());
    }

    #[test]
    fn session_dates_and_their_first_instant() {
        let fut = TimeDelta::hours(2);
        let stk = TimeDelta::hours(-5);
        // CME: la sesión del 25 abre el 24 a las 23:00 UTC (17:00 CT en invierno).
        assert_eq!(
            session_date(t("2026-11-24T23:00:00Z"), fut).to_string(),
            "2026-11-25"
        );
        assert_eq!(
            session_date(t("2026-11-24T21:59:00Z"), fut).to_string(),
            "2026-11-24"
        );
        assert_eq!(
            session_date(t("2026-11-25T00:59:00Z"), stk).to_string(),
            "2026-11-24"
        );
        let day = "2026-11-25".parse().unwrap();
        assert_eq!(session_start(day, fut), t("2026-11-24T22:00:00Z"));
        assert_eq!(session_start(day, stk), t("2026-11-25T05:00:00Z"));
        // Coherentes: lo de antes del primer instante es de otra sesión.
        for off in [fut, stk] {
            let s = session_start(day, off);
            assert_eq!(session_date(s, off), day);
            assert!(session_date(s - TimeDelta::minutes(1), off) < day);
        }
    }

    #[test]
    fn change_and_percent() {
        assert_eq!(change(dec!(210), dec!(200)), dec!(10));
        assert_eq!(change_pct(dec!(210), dec!(200)), Some(dec!(5.00)));
        assert_eq!(change_pct(dec!(2), dec!(3)), Some(dec!(-33.33)));
        assert_eq!(change_pct(dec!(2), dec!(0)), None);
    }
}

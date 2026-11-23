//! De qué velas sale la liquidez (#259): las del CFD de IG o las de los futuros de Databento.
//! Configuración en `config/candle_sources.toml`, validada al cargar.
//!
//! Lo que se calcula con los futuros se guarda bajo un **instrumento derivado**
//! (`<instrumento><id_suffix>`, por ejemplo `US500.FUT`). Así las tablas de niveles, FVG y
//! desequilibrio nunca mezclan las dos fuentes, y el cálculo es el mismo para las dos.

use std::collections::BTreeMap;

use cfds_core::{Candle, Instrument, InstrumentId};
use chrono::{DateTime, Datelike, Days, NaiveDate, Utc, Weekday};
use serde::Deserialize;

use crate::Session;

/// Una fuente de velas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CandleSource {
    /// Velas del CFD de IG (`candles`).
    Ig,
    /// Velas armadas desde los futuros (`futures_candles`).
    Futures,
}

impl CandleSource {
    pub const ALL: [CandleSource; 2] = [CandleSource::Ig, CandleSource::Futures];

    /// El nombre en la query del gateway y en los contratos (`ig`, `futures`).
    pub fn as_str(self) -> &'static str {
        match self {
            CandleSource::Ig => "ig",
            CandleSource::Futures => "futures",
        }
    }

    /// Lee `ig` o `futures` (también `futuros`, el nombre del flag en español).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ig" => Some(CandleSource::Ig),
            "futures" | "futuros" => Some(CandleSource::Futures),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandleSources {
    pub ig_label: String,
    /// Valor de `source` en `futures_candles` (hoy `databento`).
    pub futures_source: String,
    pub futures_label: String,
    pub id_suffix: String,
    /// Meses de vencimiento de los futuros (1–12).
    pub expiry_months: Vec<u32>,
    /// Qué día el continuo ya es el contrato nuevo, por instrumento (CFD).
    pub roll: BTreeMap<InstrumentId, RollDay>,
}

/// Qué día el contrato continuo `c.0` pasa al contrato nuevo (#269).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollDay {
    /// El día del vencimiento todavía es del contrato viejo (CME).
    AfterExpiryDay,
    /// El día del vencimiento ya es del contrato nuevo (Eurex).
    OnExpiryDay,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    ig: RawIg,
    futures: RawFutures,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIg {
    label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFutures {
    source: String,
    label: String,
    id_suffix: String,
    expiry_months: Vec<u32>,
    roll: BTreeMap<InstrumentId, RollDay>,
}

impl CandleSources {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let r: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let label_ok = |l: &str| (1..=80).contains(&l.chars().count());
        if !label_ok(&r.ig.label) || !label_ok(&r.futures.label) {
            return Err("las etiquetas tienen que tener de 1 a 80 caracteres".into());
        }
        let b = r.futures.source.as_bytes();
        if b.is_empty()
            || b.len() > 32
            || !b[0].is_ascii_lowercase()
            || !b
                .iter()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
        {
            return Err(format!("futures.source inválido: {:?}", r.futures.source));
        }
        let s = r.futures.id_suffix.as_bytes();
        if !(2..=6).contains(&s.len())
            || !matches!(s[0], b'.' | b'_' | b'-')
            || !s[1..]
                .iter()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return Err(format!(
                "futures.id_suffix tiene que ser un separador y mayúsculas (por ejemplo .FUT): {:?}",
                r.futures.id_suffix
            ));
        }
        let months = &r.futures.expiry_months;
        if months.is_empty()
            || months.iter().any(|m| !(1..=12).contains(m))
            || months.windows(2).any(|w| w[0] >= w[1])
        {
            return Err("futures.expiry_months: meses de 1 a 12, en orden y sin repetir".into());
        }
        Ok(Self {
            ig_label: r.ig.label,
            futures_source: r.futures.source,
            futures_label: r.futures.label,
            id_suffix: r.futures.id_suffix,
            expiry_months: r.futures.expiry_months,
            roll: r.futures.roll,
        })
    }

    pub fn label(&self, source: CandleSource) -> &str {
        match source {
            CandleSource::Ig => &self.ig_label,
            CandleSource::Futures => &self.futures_label,
        }
    }

    /// Bajo qué instrumento se guarda lo calculado con `source` para `base` (el CFD).
    pub fn instrument_for(
        &self,
        source: CandleSource,
        base: &InstrumentId,
    ) -> Result<InstrumentId, String> {
        match source {
            CandleSource::Ig => Ok(base.clone()),
            CandleSource::Futures => format!("{base}{}", self.id_suffix)
                .parse()
                .map_err(|e| format!("{e}")),
        }
    }

    /// El instrumento derivado para la tabla `instruments` (las tablas de liquidez la
    /// referencian): el mismo del CFD, con el id derivado y la fuente en el nombre.
    pub fn derived_instrument(
        &self,
        source: CandleSource,
        base: &Instrument,
    ) -> Result<Instrument, String> {
        let id = self.instrument_for(source, &base.id)?;
        let name = match source {
            CandleSource::Ig => base.name.clone(),
            CandleSource::Futures => {
                let n = format!("{} ({})", base.name, self.futures_label);
                n.chars().take(100).collect()
            }
        };
        Ok(Instrument {
            id,
            name,
            ..base.clone()
        })
    }
}

/// Tercer viernes de `year`-`month`.
fn third_friday(year: i32, month: u32) -> Option<NaiveDate> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let to_friday =
        (Weekday::Fri.num_days_from_monday() + 7 - first.weekday().num_days_from_monday()) % 7;
    first.checked_add_days(Days::new(u64::from(to_friday) + 14))
}

impl CandleSources {
    /// Día de vencimiento de `year`-`month`: el tercer viernes o, si la bolsa no abre ese día
    /// (feriado del calendario de `session`), el día hábil anterior. Si el calendario no cubre la
    /// fecha, el tercer viernes.
    pub fn expiry_day(session: &Session, year: i32, month: u32) -> Option<NaiveDate> {
        let mut d = third_friday(year, month)?;
        for _ in 0..7 {
            match session.hours(d) {
                Some(None) => d = d.pred_opt()?,
                _ => return Some(d),
            }
        }
        Some(d)
    }

    /// Cambios de contrato del continuo de `base` dentro de `m15` (velas en orden): el inicio de
    /// la primera vela del contrato nuevo en cada vencimiento (#269). Es un calendario público:
    /// no depende de los ids de contrato de la fuente.
    pub fn roll_instants(
        &self,
        base: &InstrumentId,
        session: &Session,
        m15: &[Candle],
    ) -> Result<Vec<DateTime<Utc>>, String> {
        let rule = *self
            .roll
            .get(base)
            .ok_or_else(|| format!("{base} no está en [futures.roll] de candle_sources.toml"))?;
        let (Some(first), Some(last)) = (m15.first(), m15.last()) else {
            return Ok(Vec::new());
        };
        let (from, to) = (first.start.date_naive(), last.start.date_naive());
        let mut out = Vec::new();
        for year in from.year()..=to.year() {
            for &month in &self.expiry_months {
                let Some(expiry) = Self::expiry_day(session, year, month) else {
                    continue;
                };
                let boundary = match rule {
                    RollDay::OnExpiryDay => expiry,
                    RollDay::AfterExpiryDay => match expiry.succ_opt() {
                        Some(d) => d,
                        None => continue,
                    },
                };
                let k = m15.partition_point(|c| c.start.date_naive() < boundary);
                if k > 0 && k < m15.len() {
                    out.push(m15[k].start);
                }
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../../../config/candle_sources.toml");

    #[test]
    fn the_repository_config_names_both_sources() {
        let c = CandleSources::from_toml(REAL).unwrap();
        assert_eq!(c.label(CandleSource::Ig), "IG CFD");
        assert_eq!(
            c.label(CandleSource::Futures),
            "futuros CME/Eurex (base distinta al CFD)"
        );
        assert_eq!(c.futures_source, "databento");
        let us500: InstrumentId = "US500".parse().unwrap();
        assert_eq!(
            c.instrument_for(CandleSource::Futures, &us500).unwrap(),
            "US500.FUT".parse::<InstrumentId>().unwrap()
        );
        assert_eq!(c.instrument_for(CandleSource::Ig, &us500).unwrap(), us500);
        assert_eq!(CandleSource::parse("futuros"), Some(CandleSource::Futures));
        assert_eq!(CandleSource::parse("ig"), Some(CandleSource::Ig));
        assert_eq!(CandleSource::parse("cfd"), None);
    }

    #[test]
    fn rejects_invalid_values() {
        for (from, to) in [
            (r#"id_suffix = ".FUT""#, r#"id_suffix = "fut""#),
            (r#"id_suffix = ".FUT""#, r#"id_suffix = ".FUTURES""#),
            (r#"source = "databento""#, r#"source = "Databento""#),
            (r#"label = "IG CFD""#, r#"label = """#),
            ("expiry_months = [3, 6, 9, 12]", "expiry_months = [3, 13]"),
            ("expiry_months = [3, 6, 9, 12]", "expiry_months = [6, 3]"),
            (r#"DE40 = "on_expiry_day""#, r#"DE40 = "monday""#),
            ("[futures]", "[futuros]"),
        ] {
            let text = REAL.replace(from, to);
            assert_ne!(text, REAL, "{from}");
            assert!(CandleSources::from_toml(&text).is_err(), "{to}");
        }
    }
}

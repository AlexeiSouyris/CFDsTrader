//! De qué velas sale la liquidez (#259): las del CFD de IG, las de los futuros de Databento o
//! (#630) las barras de los futuros micro de IBKR (`ibkr_bars`) de los instrumentos con mapeo.
//! Configuración en `config/candle_sources.toml`, validada al cargar.
//!
//! Lo que se calcula con los futuros se guarda bajo un **instrumento derivado**
//! (`<instrumento><id_suffix>`, por ejemplo `US500.FUT`). Así las tablas de niveles, FVG y
//! desequilibrio nunca mezclan las dos fuentes, y el cálculo es el mismo para las dos.

use std::collections::BTreeMap;

use cfds_core::{Candle, FuturesCandle, Instrument, InstrumentId};
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
    /// Barras de los futuros micro de IBKR (`ibkr_bars`, #630). Sin diarias.
    Ibkr,
}

impl CandleSource {
    pub const ALL: [CandleSource; 3] =
        [CandleSource::Ig, CandleSource::Futures, CandleSource::Ibkr];

    /// El nombre en la query del gateway y en los contratos (`ig`, `futures`).
    pub fn as_str(self) -> &'static str {
        match self {
            CandleSource::Ig => "ig",
            CandleSource::Futures => "futures",
            CandleSource::Ibkr => "ibkr",
        }
    }

    /// Lee `ig`, `futures` (también `futuros`, el nombre del flag en español) o `ibkr`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ig" => Some(CandleSource::Ig),
            "futures" | "futuros" => Some(CandleSource::Futures),
            "ibkr" => Some(CandleSource::Ibkr),
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
    /// Zona de la ventana de las diarias de los futuros (#282), la misma que las de IG.
    pub daily_tz: chrono_tz::Tz,
    /// Meses de vencimiento de los futuros (1–12).
    pub expiry_months: Vec<u32>,
    /// Qué día el continuo ya es el contrato nuevo, por instrumento (CFD).
    pub roll: BTreeMap<InstrumentId, RollDay>,
    /// Tasa que el precio del futuro ya descuenta en su base (#467).
    pub basis: FuturesBasisConfig,
    /// `[ibkr]` (#630): las barras de IBKR como fuente de la liquidez.
    pub ibkr: IbkrCandles,
}

/// `[ibkr]` de `config/candle_sources.toml` (#630).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrCandles {
    /// Valor de `source` en `ibkr_bars` (el de `[load] source` de `config/ibkr.toml`).
    pub source: String,
    /// Qué precio (`what_to_show` de `ibkr_bars`).
    pub what_to_show: String,
    pub label: String,
    pub id_suffix: String,
    /// Instrumento de `ibkr_bars` de cada CFD (por ejemplo US100 → MNQ). Sin entrada, sin velas.
    pub instruments: BTreeMap<InstrumentId, String>,
}

/// Qué parte de la base del futuro se descuenta de la financiación del CFD (#467).
///
/// Dentro de un contrato, el futuro converge al contado: su deriva ya resta `r − q` (tasa menos
/// dividendos). El CFD de IG al contado paga la financiación completa y recibe el ajuste por
/// dividendos. Con las velas del futuro, sumar la financiación completa cuenta la tasa dos veces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BasisMode {
    /// Se descuenta `r`: IG ajusta los dividendos en los CFD de índices al contado, así que los
    /// dividendos que trae la base son los mismos que recibe el CFD.
    Rate,
    /// Se descuenta `r − q`: para un broker que no ajusta dividendos. Necesita una serie de
    /// dividendos con fuente por instrumento.
    RateMinusDividends,
}

/// Un tramo de una serie diaria de `reference_rates`: vale desde `from` (el primero, desde
/// siempre) hasta el `from` del tramo siguiente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateSegment {
    pub series: String,
    pub from: Option<NaiveDate>,
}

/// Series de la base de un futuro: la tasa y, con [`BasisMode::RateMinusDividends`], los
/// dividendos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasisSpec {
    pub rate: Vec<RateSegment>,
    pub dividends: Option<Vec<RateSegment>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuturesBasisConfig {
    pub mode: BasisMode,
    /// Por instrumento (CFD). Un instrumento sin entrada no se puede simular con futuros.
    pub instruments: BTreeMap<InstrumentId, BasisSpec>,
}

/// Qué día el contrato continuo `c.0` pasa al contrato nuevo (#269).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollDay {
    /// El día del vencimiento todavía es del contrato viejo (CME).
    AfterExpiryDay,
    /// El día del vencimiento ya es del contrato nuevo (Eurex).
    OnExpiryDay,
    /// Sin calendario: el contrato cambia donde cambia el contrato de las velas (#449). Para el
    /// continuo por volumen (`GC.v.0`), que no vence en una fecha fija. Ver
    /// [`CandleSources::roll_instants_with_contracts`].
    ContractChange,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    ig: RawIg,
    futures: RawFutures,
    ibkr: RawIbkr,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIbkr {
    source: String,
    what_to_show: String,
    label: String,
    id_suffix: String,
    instruments: BTreeMap<InstrumentId, String>,
}

/// Un sufijo de instrumento derivado: un separador y mayúsculas o dígitos (por ejemplo `.FUT`).
fn suffix_ok(s: &str) -> bool {
    let s = s.as_bytes();
    (2..=6).contains(&s.len())
        && matches!(s[0], b'.' | b'_' | b'-')
        && s[1..]
            .iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

fn ibkr_config(r: RawIbkr, futures_suffix: &str) -> Result<IbkrCandles, String> {
    if !(1..=80).contains(&r.label.chars().count()) {
        return Err("ibkr.label: de 1 a 80 caracteres".into());
    }
    if !suffix_ok(&r.id_suffix) || r.id_suffix == futures_suffix {
        return Err(format!(
            "ibkr.id_suffix: un separador y mayúsculas, distinto del de futures: {:?}",
            r.id_suffix
        ));
    }
    let id_ok = |s: &str| {
        let b = s.as_bytes();
        !b.is_empty()
            && b.len() <= 32
            && b.iter()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    };
    if !id_ok(&r.source) || !id_ok(&r.what_to_show) {
        return Err("ibkr.source y ibkr.what_to_show: texto corto sin espacios".into());
    }
    // Sin entradas vale: ningún CFD tiene velas de IBKR.
    if r.instruments.values().any(|v| !id_ok(v)) {
        return Err("ibkr.instruments: el id de ibkr_bars de cada CFD".into());
    }
    Ok(IbkrCandles {
        source: r.source,
        what_to_show: r.what_to_show,
        label: r.label,
        id_suffix: r.id_suffix,
        instruments: r.instruments,
    })
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
    daily_timezone: String,
    expiry_months: Vec<u32>,
    roll: BTreeMap<InstrumentId, RollDay>,
    basis: RawBasis,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBasis {
    mode: BasisMode,
    rates: BTreeMap<InstrumentId, Vec<RawSegment>>,
    #[serde(default)]
    dividends: BTreeMap<InstrumentId, Vec<RawSegment>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSegment {
    series: String,
    from: Option<String>,
}

/// Tramos en orden: el primero sin `from`, los demás con fechas crecientes; nombres como en
/// `contracts/reference_rate.v1` (`^[A-Z][A-Z0-9_]{0,31}$`).
fn segments(what: &str, raw: Vec<RawSegment>) -> Result<Vec<RateSegment>, String> {
    if raw.is_empty() {
        return Err(format!("{what}: hace falta al menos una serie"));
    }
    let mut out: Vec<RateSegment> = Vec::with_capacity(raw.len());
    for (i, r) in raw.into_iter().enumerate() {
        let b = r.series.as_bytes();
        if b.is_empty()
            || b.len() > 32
            || !b[0].is_ascii_uppercase()
            || !b
                .iter()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == b'_')
        {
            return Err(format!("{what}: serie inválida {:?}", r.series));
        }
        let from = match (i, r.from) {
            (0, None) => None,
            (0, Some(_)) => return Err(format!("{what}: el primer tramo no lleva `from`")),
            (_, None) => {
                return Err(format!(
                    "{what}: cada tramo después del primero lleva `from`"
                ));
            }
            (_, Some(d)) => Some(
                NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                    .map_err(|_| format!("{what}: fecha inválida {d:?}"))?,
            ),
        };
        if let (Some(prev), Some(f)) = (out.last().and_then(|s| s.from), from)
            && f <= prev
        {
            return Err(format!(
                "{what}: los tramos tienen que ir en orden de fecha"
            ));
        }
        out.push(RateSegment {
            series: r.series,
            from,
        });
    }
    Ok(out)
}

fn basis_config(raw: RawBasis) -> Result<FuturesBasisConfig, String> {
    let RawBasis {
        mode,
        rates,
        mut dividends,
    } = raw;
    if mode == BasisMode::Rate && !dividends.is_empty() {
        return Err(
            "futures.basis.dividends solo se usa con mode = \"rate_minus_dividends\"".into(),
        );
    }
    if let Some(id) = dividends.keys().find(|id| !rates.contains_key(*id)) {
        return Err(format!(
            "futures.basis.dividends.{id} sin futures.basis.rates.{id}"
        ));
    }
    let mut instruments = BTreeMap::new();
    for (id, segs) in rates {
        let rate = segments(&format!("futures.basis.rates.{id}"), segs)?;
        let dividends = match (mode, dividends.remove(&id)) {
            (BasisMode::Rate, _) => None,
            (BasisMode::RateMinusDividends, Some(d)) => {
                Some(segments(&format!("futures.basis.dividends.{id}"), d)?)
            }
            (BasisMode::RateMinusDividends, None) => {
                return Err(format!(
                    "futures.basis.dividends.{id}: falta (mode = \"rate_minus_dividends\")"
                ));
            }
        };
        instruments.insert(id, BasisSpec { rate, dividends });
    }
    Ok(FuturesBasisConfig { mode, instruments })
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
        if !suffix_ok(&r.futures.id_suffix) {
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
        let daily_tz: chrono_tz::Tz = r.futures.daily_timezone.parse().map_err(|_| {
            format!(
                "futures.daily_timezone desconocida: {}",
                r.futures.daily_timezone
            )
        })?;
        let basis = basis_config(r.futures.basis)?;
        let ibkr = ibkr_config(r.ibkr, &r.futures.id_suffix)?;
        Ok(Self {
            ig_label: r.ig.label,
            daily_tz,
            futures_source: r.futures.source,
            futures_label: r.futures.label,
            id_suffix: r.futures.id_suffix,
            expiry_months: r.futures.expiry_months,
            roll: r.futures.roll,
            basis,
            ibkr,
        })
    }

    pub fn label(&self, source: CandleSource) -> &str {
        match source {
            CandleSource::Ig => &self.ig_label,
            CandleSource::Futures => &self.futures_label,
            CandleSource::Ibkr => &self.ibkr.label,
        }
    }

    /// El instrumento de `ibkr_bars` del CFD `base` (#630), si tiene mapeo.
    pub fn ibkr_instrument(&self, base: &InstrumentId) -> Option<&str> {
        self.ibkr.instruments.get(base).map(String::as_str)
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
            CandleSource::Ibkr => format!("{base}{}", self.ibkr.id_suffix)
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
            CandleSource::Futures | CandleSource::Ibkr => {
                let n = format!("{} ({})", base.name, self.label(source));
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
        if rule == RollDay::ContractChange {
            return Err(format!(
                "{base}: la regla contract_change necesita el contrato de cada vela \
                 (roll_instants_with_contracts)"
            ));
        }
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
                    RollDay::ContractChange => continue,
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

    /// Como [`CandleSources::roll_instants`], con el contrato de cada vela (en orden). Con las
    /// reglas de calendario da lo mismo; con [`RollDay::ContractChange`] da
    /// [`contract_changes`].
    pub fn roll_instants_with_contracts(
        &self,
        base: &InstrumentId,
        session: &Session,
        m15: &[FuturesCandle],
    ) -> Result<Vec<DateTime<Utc>>, String> {
        match self.roll.get(base) {
            Some(RollDay::ContractChange) => Ok(contract_changes(m15)),
            _ => {
                let candles: Vec<Candle> = m15.iter().map(|c| c.candle.clone()).collect();
                self.roll_instants(base, session, &candles)
            }
        }
    }
}

/// Cambios de contrato observados en `m15` (velas en orden): el inicio de cada vela cuyo
/// contrato es distinto del de la vela anterior (#449).
///
/// No mira el futuro: un cambio en `R` sale solo de las velas hasta `R`, así que llegar datos
/// nuevos nunca agrega, mueve ni borra un cambio ya visto (no repinta). La primera vela no es
/// un cambio (no hay anterior).
pub fn contract_changes(m15: &[FuturesCandle]) -> Vec<DateTime<Utc>> {
    m15.windows(2)
        .filter(|w| w[0].contract_id != w[1].contract_id)
        .map(|w| w[1].candle.start)
        .collect()
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
        // IBKR (#630): US100 → MNQ y US500 → MES, bajo su propio sufijo; DE40 sin mapeo.
        assert_eq!(CandleSource::parse("ibkr"), Some(CandleSource::Ibkr));
        assert_eq!(c.ibkr_instrument(&us500), Some("MES"));
        assert_eq!(c.ibkr_instrument(&"US100".parse().unwrap()), Some("MNQ"));
        assert_eq!(c.ibkr_instrument(&"DE40".parse().unwrap()), None);
        assert_eq!(
            c.instrument_for(CandleSource::Ibkr, &us500).unwrap(),
            "US500.IBKR".parse::<InstrumentId>().unwrap()
        );
        assert_eq!(c.ibkr.source, "ibkr");
        assert_eq!(c.ibkr.what_to_show, "TRADES");
    }

    #[test]
    fn the_ibkr_source_is_the_one_the_loader_saves() {
        let ibkr: toml::Table =
            toml::from_str(include_str!("../../../../config/ibkr.toml")).unwrap();
        let c = CandleSources::from_toml(REAL).unwrap();
        assert_eq!(
            ibkr["load"]["source"].as_str(),
            Some(c.ibkr.source.as_str())
        );
    }

    #[test]
    fn the_ibkr_section_is_validated() {
        for (from, to) in [
            ("id_suffix = \".IBKR\"", "id_suffix = \".FUT\""),
            ("id_suffix = \".IBKR\"", "id_suffix = \"IBKR\""),
            ("US100 = \"MNQ\"", "US100 = \"M NQ\""),
            ("what_to_show = \"TRADES\"", "what_to_show = \"\""),
            (
                "what_to_show = \"TRADES\"",
                "what_to_show = \"TRADES\"\nnope = 1",
            ),
        ] {
            let bad = REAL.replacen(from, to, 1);
            assert_ne!(bad, REAL, "{from}");
            assert!(CandleSources::from_toml(&bad).is_err(), "{to}");
        }
    }

    #[test]
    fn futures_dailies_use_the_same_day_as_ig_dailies() {
        let ig: toml::Table = toml::from_str(include_str!("../../../../config/ig.toml")).unwrap();
        let prices = ig
            .values()
            .filter_map(|v| v.as_table())
            .find_map(|t| t.get("prices_timezone"))
            .and_then(|v| v.as_str())
            .unwrap();
        let c = CandleSources::from_toml(REAL).unwrap();
        assert_eq!(c.daily_tz.name(), prices);
    }

    #[test]
    fn the_repository_config_has_the_basis_rate_of_each_index_future() {
        let c = CandleSources::from_toml(REAL).unwrap();
        assert_eq!(c.basis.mode, BasisMode::Rate);
        let us500 = &c.basis.instruments[&"US500".parse::<InstrumentId>().unwrap()];
        assert_eq!(
            us500.rate,
            vec![
                RateSegment {
                    series: "EFFR".into(),
                    from: None
                },
                RateSegment {
                    series: "SOFR".into(),
                    from: NaiveDate::from_ymd_opt(2018, 4, 2)
                },
            ]
        );
        assert_eq!(us500.dividends, None);
        let de40 = &c.basis.instruments[&"DE40".parse::<InstrumentId>().unwrap()];
        assert_eq!(de40.rate[0].series, "ESTR");
        // Cada índice con regla de vencimiento tiene su tasa; el oro queda para su Issue (#418).
        for id in c.roll.keys().filter(|id| id.to_string() != "XAUUSD") {
            assert!(c.basis.instruments.contains_key(id), "{id}");
        }
    }

    #[test]
    fn rate_minus_dividends_reads_the_dividend_series() {
        let text = REAL
            .replace(r#"mode = "rate""#, r#"mode = "rate_minus_dividends""#)
            .replace(
                "[futures.basis.rates]",
                "[futures.basis.dividends]\nDE40 = [{ series = \"DIV_DE\" }]\n[futures.basis.rates]",
            );
        // Faltan los dividendos de los demás instrumentos.
        assert!(CandleSources::from_toml(&text).is_err());
        let only_de40 = text
            .lines()
            .filter(|l| !l.starts_with("US"))
            .collect::<Vec<_>>()
            .join("\n");
        let c = CandleSources::from_toml(&only_de40).unwrap();
        let de40 = &c.basis.instruments[&"DE40".parse::<InstrumentId>().unwrap()];
        assert_eq!(de40.dividends.as_ref().unwrap()[0].series, "DIV_DE");
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
            (
                r#"daily_timezone = "Europe/Madrid""#,
                r#"daily_timezone = "Madrid""#,
            ),
            ("[futures]", "[futuros]"),
            (r#"mode = "rate""#, r#"mode = "tasa""#),
            (
                r#"DE40 = [{ series = "ESTR" }]"#,
                r#"DE40 = [{ series = "estr" }]"#,
            ),
            (
                r#"DE40 = [{ series = "ESTR" }]"#,
                r#"DE40 = [{ series = "ESTR", from = "2019-10-01" }]"#,
            ),
            (r#"DE40 = [{ series = "ESTR" }]"#, "DE40 = []"),
            (
                r#"US30 = [{ series = "EFFR" }, { series = "SOFR", from = "2018-04-02" }]"#,
                r#"US30 = [{ series = "EFFR" }, { series = "SOFR" }]"#,
            ),
            (
                r#"US30 = [{ series = "EFFR" }, { series = "SOFR", from = "2018-04-02" }]"#,
                r#"US30 = [{ series = "EFFR" }, { series = "SOFR", from = "2018-13-02" }]"#,
            ),
            (
                r#"US30 = [{ series = "EFFR" }, { series = "SOFR", from = "2018-04-02" }]"#,
                r#"US30 = [{ series = "EFFR" }, { series = "SOFR", from = "2018-04-02" }, { series = "X", from = "2018-04-01" }]"#,
            ),
            // Con "rate" no se aceptan dividendos (se ignorarían en silencio).
            (
                r#"DE40 = [{ series = "ESTR" }]"#,
                "DE40 = [{ series = \"ESTR\" }]\n[futures.basis.dividends]\nDE40 = [{ series = \"DIV\" }]",
            ),
            // Con "rate_minus_dividends", cada instrumento necesita su serie de dividendos.
            (r#"mode = "rate""#, r#"mode = "rate_minus_dividends""#),
        ] {
            let text = REAL.replace(from, to);
            assert_ne!(text, REAL, "{from}");
            assert!(CandleSources::from_toml(&text).is_err(), "{to}");
        }
    }
}

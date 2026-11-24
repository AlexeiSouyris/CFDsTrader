//! Escenarios de financiación para estudios de sensibilidad (#405).
//!
//! Cuando el catálogo todavía no confirma el tipo de referencia de un instrumento (queda
//! `pending` en `config/instruments.toml`), un estudio puede correr con un tipo **rotulado**
//! (por ejemplo €STR o SOFR) sin cambiar `config/`: el reemplazo es **solo en memoria** y el
//! resultado no es oficial. La tasa administrativa, la base de días y el resto de los costos
//! siguen siendo los del catálogo.
//!
//! - [`apply_funding_scenario`] pone el nombre del tipo en una copia del catálogo.
//! - [`rates_from_csv`] lee una serie diaria de una fuente oficial (`fecha,tasa`), con una hora
//!   de publicación conservadora: sin look-ahead (CLAUDE.md §5b).

use cfds_core::{InstrumentId, ReferenceRate};
use cfds_market_data::{Catalog, Sourced};
use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveTime, Utc, Weekday};
use rust_decimal::Decimal;

/// Fuente con la que queda rotulado el reemplazo en el catálogo en memoria.
pub const SCENARIO_SOURCE: &str = "escenario-de-estudio-no-oficial";

/// Pone `series` como tipo de referencia de `instrument` en `catalog` (en memoria). Si
/// `[funding_benchmarks]` no tiene `series`, lo agrega con `value_pct`: solo para que el catálogo
/// quede completo, porque la corrida usa la serie diaria. Devuelve el nombre que había antes
/// (`None` si estaba pendiente).
pub fn apply_funding_scenario(
    catalog: &mut Catalog,
    instrument: &InstrumentId,
    series: &str,
    value_pct: Decimal,
) -> Result<Option<String>, String> {
    let spec = catalog
        .instruments
        .get_mut(instrument)
        .ok_or_else(|| format!("{instrument} no está en el catálogo"))?;
    let before = spec.terms.funding_benchmark.value().cloned();
    spec.terms.funding_benchmark = Sourced::Known {
        value: series.to_owned(),
        source: SCENARIO_SOURCE.to_owned(),
    };
    catalog
        .funding_benchmarks
        .entry(series.to_owned())
        .or_insert_with(|| Sourced::Known {
            value: value_pct,
            source: SCENARIO_SOURCE.to_owned(),
        });
    Ok(before)
}

/// Suma `n` días hábiles (lunes a viernes) a `day`.
pub fn add_weekdays(day: NaiveDate, n: u32) -> NaiveDate {
    let mut d = day;
    let mut left = n;
    while left > 0 {
        d = d.checked_add_days(Days::new(1)).unwrap_or(d);
        if !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
            left -= 1;
        }
    }
    d
}

/// Serie diaria `series` desde un CSV `fecha,tasa_anual_pct` (una cabecera opcional como primera línea con datos; líneas
/// vacías y las que empiezan con `#` se ignoran). Cada valor cuenta desde `publish_time` UTC del
/// día que cae `lag_weekdays` días hábiles después de su fecha: con un rezago igual o mayor al
/// real de la fuente, nunca se usa antes de estar publicado. Error si una línea no se puede leer
/// o si no queda ningún valor.
pub fn rates_from_csv(
    series: &str,
    text: &str,
    lag_weekdays: u32,
    publish_time: NaiveTime,
) -> Result<Vec<ReferenceRate>, String> {
    let mut out = Vec::new();
    let mut first = true;
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split(',').map(str::trim);
        let (Some(date), Some(rate), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!("línea {}: se esperaba `fecha,tasa`: {line}", i + 1));
        };
        let is_first = std::mem::replace(&mut first, false);
        let Ok(ref_date) = date.parse::<NaiveDate>() else {
            if is_first {
                continue; // cabecera
            }
            return Err(format!("línea {}: fecha inválida: {date}", i + 1));
        };
        let rate_pct: Decimal = rate
            .parse()
            .map_err(|_| format!("línea {}: tasa inválida: {rate}", i + 1))?;
        let published = add_weekdays(ref_date, lag_weekdays).and_time(publish_time);
        out.push(ReferenceRate {
            series: series.to_owned(),
            ref_date,
            published_at: DateTime::<Utc>::from_naive_utc_and_offset(published, Utc),
            rate_pct,
        });
    }
    if out.is_empty() {
        return Err(format!("el CSV de {series} no tiene valores"));
    }
    Ok(out)
}

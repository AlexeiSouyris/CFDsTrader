//! Flags de estudio compartidos por `walk_forward` y `backtest_run` (#405, #421). Todo se aplica
//! **en memoria**: `config/` no cambia y el resultado no es oficial.
//!
//! - `--escenario-financiacion ESTR`: el tipo de referencia €STR, con la serie guardada en
//!   `reference_rates`.
//! - `--escenario-financiacion <ETIQUETA>=<archivo.csv>`: el tipo `ETIQUETA` con la serie diaria
//!   de un CSV de fuente oficial (`fecha,tasa`). Cada valor cuenta desde las 13:00 UTC de dos días
//!   hábiles después de su fecha (BCE y NY Fed publican el día hábil siguiente, a la mañana): así
//!   nunca se usa antes de estar publicado.
//! - `--supuestos <archivo.toml>`: costos del broker supuestos ([`StudyAssumptions`]) que
//!   completan los `pending` del catálogo, sin pisar ningún dato con fuente.

use cfds_backtest::{DailyRates, StudyAssumptions, rates_from_csv};

use super::prepare::{FundingScenario, Study};

/// Saca de `args` el flag `name` y su valor.
fn take(args: Vec<String>, name: &str) -> Result<(Option<String>, Vec<String>), String> {
    match args.iter().position(|a| a == name) {
        None => Ok((None, args)),
        Some(i) => {
            let v = args
                .get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{name} necesita un valor"))?;
            let rest = [&args[..i], args.get(i + 2..).unwrap_or(&[])].concat();
            Ok((Some(v), rest))
        }
    }
}

fn funding_scenario(arg: &str) -> Result<FundingScenario, Box<dyn std::error::Error>> {
    match arg.split_once('=') {
        None if arg == "ESTR" => Ok(FundingScenario {
            series: arg.to_owned(),
            rates: None,
        }),
        Some((label, path))
            if !label.is_empty()
                && label
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') =>
        {
            let text = std::fs::read_to_string(path)?;
            let at = chrono::NaiveTime::from_hms_opt(13, 0, 0).ok_or("hora inválida")?;
            let rates = rates_from_csv(label, &text, 2, at)?;
            let daily = DailyRates::new(rates).ok_or("serie vacía")?;
            Ok(FundingScenario {
                series: label.to_owned(),
                rates: Some(daily),
            })
        }
        _ => Err(format!(
            "escenario desconocido: {arg} (ESTR | <ETIQUETA EN MAYÚSCULAS>=<archivo.csv>)"
        )
        .into()),
    }
}

/// Lee los flags de estudio y devuelve el estudio (si hay alguno) y el resto de los argumentos.
pub fn parse(
    args: Vec<String>,
) -> Result<(Option<Study>, Vec<String>), Box<dyn std::error::Error>> {
    let (funding, args) = take(args, "--escenario-financiacion")?;
    let (assumed, args) = take(args, "--supuestos")?;
    let funding = funding.as_deref().map(funding_scenario).transpose()?;
    let assumptions = assumed
        .map(|path| -> Result<_, Box<dyn std::error::Error>> {
            Ok(StudyAssumptions::from_toml(&std::fs::read_to_string(
                &path,
            )?)?)
        })
        .transpose()?;
    let study = (funding.is_some() || assumptions.is_some()).then_some(Study {
        funding,
        assumptions,
    });
    Ok((study, args))
}

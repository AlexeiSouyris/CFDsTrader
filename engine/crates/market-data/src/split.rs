//! Separación de datos en entrenamiento, validación y prueba fuera de muestra (spec §15, #16).
//!
//! Reglas anti-fuga:
//! - El tramo **fuera de muestra** son los últimos meses de datos y no se entrega a la
//!   investigación: [`DataSplit::research`] lo excluye siempre.
//! - En cada ventana walk-forward, la validación empieza exactamente donde termina el
//!   entrenamiento: nunca se valida con datos anteriores a los que se usaron para ajustar.
//! - Una vela pertenece a un tramo solo si **empieza y cierra** dentro de él.

use cfds_core::Candle;
use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SplitError {
    #[error("config/data_split.toml inválida: {0}")]
    Config(String),
    #[error(
        "no alcanzan los datos: hace falta al menos una ventana de {needed} meses antes del tramo fuera de muestra"
    )]
    NotEnoughData { needed: u32 },
}

/// Un tramo de tiempo `[start, end)` con cortes a las 00:00 UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Period {
    /// La vela pertenece al tramo si empieza y **cierra** dentro de él.
    pub fn contains(&self, c: &Candle) -> bool {
        self.start <= c.start && c.end() <= self.end
    }

    pub fn select<'a>(&self, candles: &'a [Candle]) -> Vec<&'a Candle> {
        candles.iter().filter(|c| self.contains(c)).collect()
    }
}

/// Una ventana walk-forward: se ajusta con `train` y se evalúa con `validation`, que la sigue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub train: Period,
    pub validation: Period,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSplit {
    pub windows: Vec<Window>,
    pub out_of_sample: Period,
}

impl DataSplit {
    /// Velas que se pueden usar para investigar y ajustar: todas las que cierran antes del
    /// tramo fuera de muestra. Es la única puerta de entrada a los datos para la fase 3 a 4b.
    pub fn research<'a>(&self, candles: &'a [Candle]) -> Vec<&'a Candle> {
        candles
            .iter()
            .filter(|c| c.end() <= self.out_of_sample.start)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitConfig {
    pub out_of_sample_months: u32,
    pub train_months: u32,
    pub validation_months: u32,
    pub step_months: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    out_of_sample: RawOos,
    walk_forward: RawWf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOos {
    months: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWf {
    train_months: u32,
    validation_months: u32,
    step_months: u32,
}

impl SplitConfig {
    pub fn from_toml(text: &str) -> Result<Self, SplitError> {
        let raw: Raw = toml::from_str(text).map_err(|e| SplitError::Config(e.to_string()))?;
        let cfg = Self {
            out_of_sample_months: raw.out_of_sample.months,
            train_months: raw.walk_forward.train_months,
            validation_months: raw.walk_forward.validation_months,
            step_months: raw.walk_forward.step_months,
        };
        if [
            cfg.out_of_sample_months,
            cfg.train_months,
            cfg.validation_months,
            cfg.step_months,
        ]
        .contains(&0)
        {
            return Err(SplitError::Config(
                "todos los meses tienen que ser mayores que 0".into(),
            ));
        }
        Ok(cfg)
    }
}

fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d) // el día 1 siempre existe
}

fn midnight(d: NaiveDate) -> DateTime<Utc> {
    d.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc()
}

fn add_months(d: NaiveDate, n: u32) -> Option<NaiveDate> {
    d.checked_add_months(Months::new(n))
}

/// Planifica la separación para datos que van de `first` a `last` (inicios de la primera y la
/// última vela). Las ventanas empiezan en el primer mes completo de datos.
pub fn plan(
    first: DateTime<Utc>,
    last: DateTime<Utc>,
    cfg: &SplitConfig,
) -> Result<DataSplit, SplitError> {
    let window_months = cfg.train_months + cfg.validation_months;
    let not_enough = SplitError::NotEnoughData {
        needed: window_months,
    };

    let last_month = month_start(last.date_naive());
    let oos_start = last_month
        .checked_sub_months(Months::new(cfg.out_of_sample_months))
        .ok_or(not_enough.clone())?;
    let first_day = first.date_naive();
    let mut s = if first_day.day() == 1 && first.time() == chrono::NaiveTime::MIN {
        first_day
    } else {
        add_months(month_start(first_day), 1).ok_or(not_enough.clone())?
    };

    let mut windows = Vec::new();
    while let Some(val_end) = add_months(s, window_months).filter(|e| *e <= oos_start) {
        let train_end = add_months(s, cfg.train_months).ok_or(not_enough.clone())?;
        windows.push(Window {
            train: Period {
                start: midnight(s),
                end: midnight(train_end),
            },
            validation: Period {
                start: midnight(train_end),
                end: midnight(val_end),
            },
        });
        s = add_months(s, cfg.step_months).ok_or(not_enough.clone())?;
    }
    if windows.is_empty() {
        return Err(not_enough);
    }
    Ok(DataSplit {
        windows,
        out_of_sample: Period {
            start: midnight(oos_start),
            // Hasta el fin del mes de la última vela: cubre todos los datos disponibles.
            end: midnight(add_months(last_month, 1).ok_or(not_enough)?),
        },
    })
}

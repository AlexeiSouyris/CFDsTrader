//! Sensibilidad de parámetros de la Fase 5 (ROADMAP, #298).
//!
//! ```text
//! para cada ventana del walk-forward, SOLO con su entrenamiento:
//!   para cada parámetro de config/evaluation.toml [[sensitivity]]:
//!     para cada valor: el recorrido con ese valor y todo lo demás como en config
//! estabilidad por parámetro: retorno y Sharpe medios por valor (sobre las ventanas con datos),
//!   la diferencia entre el mayor y el menor retorno medio, y si todos tienen el mismo signo
//! ```
//! La prueba de cada ventana y el fuera de muestra nunca participan (`research_period`). No se
//! elige ningún "mejor" valor ni se cambia la estrategia: solo se mide qué tan estables son las
//! métricas alrededor de los valores de config. Elegir valores nuevos es del dueño.

use cfds_market_data::DataSplit;
use cfds_risk::RiskConfig;
use cfds_strategy::StrategyConfig;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::evaluation::mean;
use crate::{
    BacktestConfig, EvalError, EvaluationConfig, Inputs, Metrics, Setup, evaluate_period,
    research_period,
};

/// Qué archivo de `config/` toca un parámetro de la sensibilidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ParamFile {
    #[serde(rename = "strategy.toml")]
    Strategy,
    #[serde(rename = "backtest.toml")]
    Backtest,
    #[serde(rename = "risk.toml")]
    Risk,
}

impl ParamFile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Strategy => "strategy.toml",
            Self::Backtest => "backtest.toml",
            Self::Risk => "risk.toml",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensitivityParam {
    pub file: ParamFile,
    /// Clave con puntos dentro del archivo, por ejemplo `score.min_score`.
    pub key: String,
    pub values: Vec<toml::Value>,
}

/// Texto de los archivos de config que la sensibilidad puede variar.
#[derive(Debug, Clone, Copy)]
pub struct ConfigTexts<'a> {
    pub strategy: &'a str,
    pub backtest: &'a str,
    pub risk: &'a str,
}

/// El valor tal como se muestra y se guarda (los decimales van como texto en config).
pub fn value_text(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `text` con `key` (con puntos) reemplazada por `value`. La clave tiene que existir y el valor
/// ser del mismo tipo: la sensibilidad nunca agrega parámetros ni cambia su forma.
pub fn override_toml(text: &str, key: &str, value: &toml::Value) -> Result<String, EvalError> {
    let mut table: toml::Table =
        toml::from_str(text).map_err(|e| EvalError::Config(e.to_string()))?;
    let missing = || EvalError::Config(format!("la clave {key} no existe"));
    let mut parts = key.split('.').peekable();
    let mut current = &mut table;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            let slot = current.get_mut(part).ok_or_else(missing)?;
            if slot.type_str() != value.type_str() {
                return Err(EvalError::Config(format!(
                    "{key} es {} y el valor de la sensibilidad es {}",
                    slot.type_str(),
                    value.type_str()
                )));
            }
            *slot = value.clone();
        } else {
            current = current
                .get_mut(part)
                .and_then(toml::Value::as_table_mut)
                .ok_or_else(missing)?;
        }
    }
    toml::to_string(&table).map_err(|e| EvalError::Config(e.to_string()))
}

/// `base` con un parámetro de la sensibilidad cambiado. Todo lo demás queda igual (también lo
/// que la corrida pisó desde la línea de comandos, como el reinicio del kill switch).
pub fn setup_with(
    base: &Setup,
    texts: &ConfigTexts,
    param: &SensitivityParam,
    value: &toml::Value,
) -> Result<Setup, EvalError> {
    let bad = |e: String| EvalError::Config(format!("{} {}: {e}", param.file.name(), param.key));
    let mut s = base.clone();
    match param.file {
        ParamFile::Strategy => {
            let text = override_toml(texts.strategy, &param.key, value)?;
            s.strategy.strategy =
                StrategyConfig::from_toml(&text).map_err(|e| bad(e.to_string()))?;
        }
        ParamFile::Backtest => {
            let text = override_toml(texts.backtest, &param.key, value)?;
            let mut b = BacktestConfig::from_toml(&text).map_err(|e| bad(e.to_string()))?;
            b.kill_switch_reset_days = base.backtest.kill_switch_reset_days;
            s.backtest = b;
        }
        ParamFile::Risk => {
            let text = override_toml(texts.risk, &param.key, value)?;
            s.risk = RiskConfig::from_toml(&text).map_err(|e| bad(e.to_string()))?;
        }
    }
    Ok(s)
}

/// Un punto de la sensibilidad: un valor de un parámetro en el entrenamiento de una ventana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensitivityPoint {
    pub file: ParamFile,
    pub key: String,
    pub value: String,
    pub fold: usize,
    pub metrics: Option<Metrics>,
}

/// Cada parámetro de `cfg.sensitivity` variado solo, en el entrenamiento de cada ventana. La
/// prueba de la ventana y el fuera de muestra no se tocan.
pub fn sensitivity(
    inputs: &Inputs,
    split: &DataSplit,
    base: &Setup,
    texts: &ConfigTexts,
    cfg: &EvaluationConfig,
) -> Result<Vec<SensitivityPoint>, EvalError> {
    let mut out = Vec::new();
    for (fold, w) in split.windows.iter().enumerate() {
        let train = research_period(w.train, split)?;
        for param in &cfg.sensitivity {
            for value in &param.values {
                let setup = setup_with(base, texts, param, value)?;
                let r = evaluate_period(inputs, train, &setup, cfg.warmup, Decimal::ZERO)?;
                out.push(SensitivityPoint {
                    file: param.file,
                    key: param.key.clone(),
                    value: value_text(value),
                    fold,
                    metrics: r.metrics,
                });
            }
        }
    }
    Ok(out)
}

/// Cómo cambian las métricas de un parámetro a lo largo de sus valores (media sobre las
/// ventanas con datos). Es una medida de estabilidad: no elige ningún valor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stability {
    pub file: ParamFile,
    pub key: String,
    /// Por valor, en el orden de config: (valor, retorno medio, Sharpe medio, operaciones).
    pub by_value: Vec<(String, Option<Decimal>, Option<Decimal>, usize)>,
    /// Máximo − mínimo de los retornos medios.
    pub return_spread: Option<Decimal>,
    /// Todos los retornos medios del mismo signo (uno en cero cuenta como cambio).
    pub same_sign: bool,
}

pub fn stability(points: &[SensitivityPoint], cfg: &EvaluationConfig) -> Vec<Stability> {
    cfg.sensitivity
        .iter()
        .map(|param| {
            let by_value: Vec<_> = param
                .values
                .iter()
                .map(|v| {
                    let v = value_text(v);
                    let ms: Vec<&Metrics> = points
                        .iter()
                        .filter(|p| p.file == param.file && p.key == param.key && p.value == v)
                        .filter_map(|p| p.metrics.as_ref())
                        .collect();
                    let r: Vec<Decimal> = ms.iter().map(|m| m.total_return).collect();
                    let s: Vec<Decimal> = ms.iter().filter_map(|m| m.sharpe).collect();
                    let trades = ms.iter().map(|m| m.trades).sum();
                    (v, mean(&r), mean(&s), trades)
                })
                .collect();
            let means: Vec<Decimal> = by_value.iter().filter_map(|b| b.1).collect();
            let return_spread = match (means.iter().max(), means.iter().min()) {
                (Some(a), Some(b)) => Some(a - b),
                _ => None,
            };
            let same_sign = !means.is_empty()
                && (means.iter().all(|r| *r > Decimal::ZERO)
                    || means.iter().all(|r| *r < Decimal::ZERO));
            Stability {
                file: param.file,
                key: param.key.clone(),
                by_value,
                return_spread,
                same_sign,
            }
        })
        .collect()
}

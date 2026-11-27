//! La curva en vivo de una variante (`curve` de `lab_job_event.v1`, #854): la misma curva del
//! informe (capital compuesto desde 1, mes a mes, con [`super::sections::curve`]), empezando en 1
//! y reducida a pocos puntos para que el SSE siga liviano. Solo para mostrar mientras corre el
//! trabajo: exploratorio, no vota y no se usa para ningún cálculo.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::sections::{curve, num};

/// `curve` de `lab_job_event.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrialCurve {
    /// Índice de la variante en el pedido (desde 0).
    pub variant: u32,
    /// Meses de la curva completa, antes de reducirla.
    pub months: u32,
    /// Capital compuesto desde 1, como texto decimal.
    pub equity: Vec<String>,
}

impl TrialCurve {
    /// La curva de `monthly` (retornos mensuales de la variante, como en el informe) con como
    /// mucho `max_points` puntos. Un mes sin valor finito corta la curva ahí (nunca se inventa).
    pub fn from_monthly(variant: usize, monthly: &[f64], max_points: usize) -> Self {
        let (points, _, _) = curve(monthly);
        let mut equity = vec![Decimal::ONE.to_string()];
        for (eq, _) in points {
            match num(Some(eq)) {
                Value::String(s) => equity.push(s),
                _ => break,
            }
        }
        Self {
            variant: u32::try_from(variant).unwrap_or(u32::MAX),
            months: u32::try_from(monthly.len()).unwrap_or(u32::MAX),
            equity: downsample(&equity, max_points.max(2)),
        }
    }
}

/// Como mucho `max` elementos de `xs`, parejos y siempre con el primero y el último. Con `max` ≥
/// largo devuelve todo; con `max` 0, nada.
pub fn downsample<T: Clone>(xs: &[T], max: usize) -> Vec<T> {
    let n = xs.len();
    if n <= max {
        return xs.to_vec();
    }
    if max <= 1 {
        return xs
            .last()
            .filter(|_| max == 1)
            .cloned()
            .into_iter()
            .collect();
    }
    let (last, steps) = (n - 1, max - 1);
    // índice redondeado de j * last / steps: 0 en el primero y last en el último
    (0..max)
        .filter_map(|j| xs.get((j * last + steps / 2) / steps).cloned())
        .collect()
}

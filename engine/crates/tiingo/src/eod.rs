//! Respuesta de `GET /tiingo/daily/<ticker>/prices` (documentación oficial, leída el 23-11-2026):
//! una lista de filas con `date`, `close` (sin ajustar), `divCash` (dividendo con fecha ex en
//! `date`) y `splitFactor`, entre otros campos que acá no se usan.
//!
//! Las filas viven **solo en memoria** (ADR 0009): no implementan `Serialize` a propósito.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;

use crate::TiingoError;

/// Una fila diaria del ETF. Solo existe en memoria durante la comparación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EodRow {
    pub day: NaiveDate,
    pub close: Decimal,
    pub div_cash: Decimal,
    pub split_factor: Decimal,
}

/// Convierte el cuerpo JSON en filas, en orden de fecha y sin repetir. Cualquier cosa inesperada
/// es un error (kill switch): no se adivina.
pub fn parse_eod(symbol: &str, body: &[u8]) -> Result<Vec<EodRow>, TiingoError> {
    let shape = |reason: String| TiingoError::Shape {
        symbol: symbol.to_owned(),
        reason,
    };
    let v: Value = serde_json::from_slice(body).map_err(|e| shape(format!("no es JSON ({e})")))?;
    let rows = v
        .as_array()
        .ok_or_else(|| shape("no es una lista".into()))?;
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let obj = row
            .as_object()
            .ok_or_else(|| shape(format!("fila {i}: no es un objeto")))?;
        let date = obj
            .get("date")
            .and_then(Value::as_str)
            .ok_or_else(|| shape(format!("fila {i}: falta date")))?;
        // El formato de `date` no está documentado: se acepta "AAAA-MM-DD" con o sin hora,
        // y si trae hora tiene que ser la medianoche.
        let (d, rest) = date.split_at(date.len().min(10));
        if !(rest.is_empty() || rest.starts_with("T00:00:00")) {
            return Err(shape(format!("fila {i}: date con formato inesperado")));
        }
        let day = NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| shape(format!("fila {i}: date con formato inesperado")))?;
        let num = |name: &str| -> Result<Decimal, TiingoError> {
            let n = obj
                .get(name)
                .and_then(Value::as_number)
                .ok_or_else(|| shape(format!("fila {i}: falta {name} o no es un número")))?;
            // `Number` guarda el texto más corto que representa el número: se pasa a decimal
            // exacto desde ese texto, nunca con aritmética de float.
            n.to_string()
                .parse::<Decimal>()
                .or_else(|_| Decimal::from_scientific(&n.to_string()))
                .map_err(|_| shape(format!("fila {i}: {name} no es un decimal")))
        };
        let close = num("close")?;
        let div_cash = num("divCash")?;
        let split_factor = num("splitFactor")?;
        if close <= Decimal::ZERO || div_cash < Decimal::ZERO || split_factor <= Decimal::ZERO {
            return Err(shape(format!("fila {i}: valores fuera de rango")));
        }
        out.push(EodRow {
            day,
            close,
            div_cash,
            split_factor,
        });
    }
    out.sort_by_key(|r| r.day);
    if out.windows(2).any(|w| w[0].day == w[1].day) {
        return Err(shape("fechas repetidas".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    // Respuesta SINTÉTICA, armada a mano: no son precios de Tiingo.
    const BODY: &str = r#"[
      {"date":"2026-01-06T00:00:00.000Z","close":101.5,"high":1,"low":1,"open":1,"volume":1,
       "adjClose":1,"adjHigh":1,"adjLow":1,"adjOpen":1,"adjVolume":1,"divCash":0.25,"splitFactor":1.0},
      {"date":"2026-01-05","close":100,"divCash":0,"splitFactor":1}
    ]"#;

    #[test]
    fn parses_sorted_with_exact_decimals() {
        let rows = parse_eod("SPY", BODY.as_bytes()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].day, NaiveDate::from_ymd_opt(2026, 1, 5).unwrap());
        assert_eq!(rows[1].close, dec!(101.5));
        assert_eq!(rows[1].div_cash, dec!(0.25));
    }

    #[test]
    fn anything_unexpected_is_a_shape_error() {
        for bad in [
            "{}",
            "no json",
            r#"[{"date":"2026-01-05","close":"100","divCash":0,"splitFactor":1}]"#,
            r#"[{"date":"2026-01-05","close":100,"splitFactor":1}]"#,
            r#"[{"date":"05/01/2026","close":100,"divCash":0,"splitFactor":1}]"#,
            r#"[{"date":"2026-01-05T15:00:00Z","close":100,"divCash":0,"splitFactor":1}]"#,
            r#"[{"date":"2026-01-05","close":-1,"divCash":0,"splitFactor":1}]"#,
            r#"[{"date":"2026-01-05","close":1,"divCash":0,"splitFactor":1},{"date":"2026-01-05","close":1,"divCash":0,"splitFactor":1}]"#,
        ] {
            let e = parse_eod("SPY", bad.as_bytes()).unwrap_err();
            assert!(matches!(e, TiingoError::Shape { .. }), "{bad}");
        }
    }
}

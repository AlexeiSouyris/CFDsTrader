//! Decimales del contrato: string con dígitos, punto opcional y más dígitos
//! (`^\d+(\.\d+)?$`). Sin signo, sin exponente, sin espacios. Así Rust acepta
//! exactamente lo mismo que los JSON Schema de `contracts/`.

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serializer, de::Error};

fn is_contract_decimal(s: &str) -> bool {
    let mut parts = s.splitn(2, '.');
    let int = parts.next().unwrap_or("");
    let frac = parts.next();
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    digits(int) && frac.is_none_or(digits)
}

fn parse<E: Error>(s: &str) -> Result<Decimal, E> {
    if !is_contract_decimal(s) {
        return Err(E::custom(format!(
            "decimal fuera del formato del contrato: {s:?}"
        )));
    }
    s.parse::<Decimal>().map_err(E::custom)
}

pub fn serialize<S: Serializer>(d: &Decimal, s: S) -> Result<S::Ok, S::Error> {
    if d.is_sign_negative() {
        return Err(serde::ser::Error::custom(
            "el contrato no admite decimales negativos",
        ));
    }
    s.serialize_str(&d.to_string())
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
    parse(&String::deserialize(d)?)
}

/// Variante con signo (`^-?\d+(\.\d+)?$`), para resultados que pueden ser negativos
/// (P/L, financiación que cobra un corto).
pub mod signed {
    use super::*;

    pub fn serialize<S: Serializer>(d: &Decimal, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&d.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
        let text = String::deserialize(d)?;
        match text.strip_prefix('-') {
            Some(rest) => parse::<D::Error>(rest).map(|v| -v),
            None => parse(&text),
        }
    }
}

/// Variante para campos opcionales (`null` en JSON).
pub mod option {
    use super::*;

    pub fn serialize<S: Serializer>(d: &Option<Decimal>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => super::serialize(d, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
        Option::<String>::deserialize(d)?
            .map(|s| parse(&s))
            .transpose()
    }
}

/// Variante con signo para campos opcionales (por ejemplo un Sharpe negativo o indefinido).
pub mod signed_option {
    use super::*;

    pub fn serialize<S: Serializer>(d: &Option<Decimal>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => super::signed::serialize(d, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
        Option::<String>::deserialize(d)?
            .map(|text| match text.strip_prefix('-') {
                Some(rest) => parse::<D::Error>(rest).map(|v| -v),
                None => parse(&text),
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::is_contract_decimal;

    #[test]
    fn matches_the_contract_pattern() {
        for ok in ["0", "1", "26747.80", "0.000001", "786"] {
            assert!(is_contract_decimal(ok), "debería aceptar {ok:?}");
        }
        for bad in [
            "", "-1", "+1", "1.", ".5", "1e5", " 1", "1,5", "1.2.3", "NaN",
        ] {
            assert!(!is_contract_decimal(bad), "debería rechazar {bad:?}");
        }
    }
}

#[cfg(test)]
mod signed_tests {
    use rust_decimal::{Decimal, dec};
    use serde::Deserialize;

    #[derive(Deserialize, serde::Serialize, PartialEq, Debug)]
    struct W(#[serde(with = "super::signed")] Decimal);

    #[test]
    fn signed_decimals_roundtrip_and_reject_malformed_signs() {
        for v in [dec!(-12.15), dec!(0), dec!(3.5)] {
            let json = serde_json::to_string(&W(v)).unwrap();
            assert_eq!(serde_json::from_str::<W>(&json).unwrap(), W(v));
        }
        for bad in [r#""--1""#, r#""+1""#, r#""-""#, r#""1e3""#, r#""- 1""#] {
            assert!(serde_json::from_str::<W>(bad).is_err(), "{bad}");
        }
    }
}

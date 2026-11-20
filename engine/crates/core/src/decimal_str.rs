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

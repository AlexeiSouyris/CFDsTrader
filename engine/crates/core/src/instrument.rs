//! Instrumentos. Contrato: `contracts/instrument.v1.schema.json`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::money::Currency;

/// Error al construir un identificador de instrumento.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "id de instrumento inválido: {0:?} (1-32 caracteres A-Z, 0-9, '.', '_' o '-', empezando por letra o número)"
)]
pub struct InvalidInstrumentId(pub String);

/// Identificador interno y estable de un instrumento (por ejemplo `US100`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InstrumentId(String);

impl InstrumentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for InstrumentId {
    type Err = InvalidInstrumentId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let valid_char = |c: char| c.is_ascii_uppercase() || c.is_ascii_digit();
        let ok = (1..=32).contains(&s.len())
            && s.chars().next().is_some_and(valid_char)
            && s.chars()
                .all(|c| valid_char(c) || matches!(c, '.' | '_' | '-'));
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(InvalidInstrumentId(s.to_owned()))
        }
    }
}

impl TryFrom<String> for InstrumentId {
    type Error = InvalidInstrumentId;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<InstrumentId> for String {
    fn from(id: InstrumentId) -> Self {
        id.0
    }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Tipo de subyacente del CFD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Index,
    Share,
    /// Materia prima (ADR 0013), por ejemplo el oro.
    Commodity,
}

impl AssetClass {
    /// Texto del contrato (`"index"`, `"share"`, `"commodity"`): el mismo en JSON y en la base de datos.
    pub fn as_str(self) -> &'static str {
        match self {
            AssetClass::Index => "index",
            AssetClass::Share => "share",
            AssetClass::Commodity => "commodity",
        }
    }
}

impl FromStr for AssetClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "index" => Ok(AssetClass::Index),
            "share" => Ok(AssetClass::Share),
            "commodity" => Ok(AssetClass::Commodity),
            other => Err(format!("tipo de instrumento desconocido: {other:?}")),
        }
    }
}

/// Un instrumento operable o de contexto. Los parámetros del broker
/// (epic, tamaños, costos) viven en `config/`, no acá.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instrument {
    pub id: InstrumentId,
    pub name: String,
    pub asset_class: AssetClass,
    pub currency: Currency,
    /// Zona horaria IANA de la bolsa del subyacente (por ejemplo `America/New_York`).
    pub exchange_timezone: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_ids() {
        for ok in ["US100", "DE40", "AAPL", "BRK.B", "X", "EU_STOXX-50"] {
            assert!(ok.parse::<InstrumentId>().is_ok(), "debería aceptar {ok:?}");
        }
    }

    #[test]
    fn asset_classes_round_trip_as_text() {
        for c in [AssetClass::Index, AssetClass::Share, AssetClass::Commodity] {
            assert_eq!(c.as_str().parse::<AssetClass>(), Ok(c));
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(json, format!("\"{}\"", c.as_str()));
        }
        assert!("metal".parse::<AssetClass>().is_err());
    }

    #[test]
    fn rejects_invalid_ids() {
        let too_long = "A".repeat(33);
        for bad in ["", "us100", "-US100", ".X", "US 100", too_long.as_str()] {
            assert!(
                bad.parse::<InstrumentId>().is_err(),
                "debería rechazar {bad:?}"
            );
        }
    }
}

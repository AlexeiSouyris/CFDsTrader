//! Dinero con decimales exactos (CLAUDE.md §4: nunca `float` para dinero).

use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Error de dominio del módulo de dinero.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyError {
    #[error("código de moneda inválido: {0:?} (se esperan 3 letras mayúsculas, ISO 4217)")]
    InvalidCurrency(String),
    #[error("no se pueden operar montos en monedas distintas: {left} y {right}")]
    CurrencyMismatch { left: Currency, right: Currency },
}

/// Código de moneda ISO 4217 (por ejemplo `EUR`, `USD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Currency([u8; 3]);

impl Currency {
    pub const EUR: Currency = Currency(*b"EUR");
    pub const USD: Currency = Currency(*b"USD");

    pub fn as_str(&self) -> &str {
        // Solo se construye con ASCII mayúsculas, así que siempre es UTF-8 válido.
        std::str::from_utf8(&self.0).unwrap_or("???")
    }
}

impl FromStr for Currency {
    type Err = MoneyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        match bytes {
            [a, b, c] if bytes.iter().all(u8::is_ascii_uppercase) => Ok(Currency([*a, *b, *c])),
            _ => Err(MoneyError::InvalidCurrency(s.to_owned())),
        }
    }
}

impl TryFrom<String> for Currency {
    type Error = MoneyError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Currency> for String {
    fn from(c: Currency) -> Self {
        c.as_str().to_owned()
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Un monto en una moneda concreta. Sumar o restar monedas distintas es un error,
/// nunca una conversión implícita.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money {
    pub amount: Decimal,
    pub currency: Currency,
}

impl Money {
    pub fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    pub fn zero(currency: Currency) -> Self {
        Self::new(Decimal::ZERO, currency)
    }

    pub fn checked_add(self, other: Money) -> Result<Money, MoneyError> {
        self.same_currency(&other)?;
        Ok(Money::new(self.amount + other.amount, self.currency))
    }

    pub fn checked_sub(self, other: Money) -> Result<Money, MoneyError> {
        self.same_currency(&other)?;
        Ok(Money::new(self.amount - other.amount, self.currency))
    }

    fn same_currency(&self, other: &Money) -> Result<(), MoneyError> {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err(MoneyError::CurrencyMismatch {
                left: self.currency,
                right: other.currency,
            })
        }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    #[test]
    fn parses_valid_currency_codes() {
        assert_eq!("EUR".parse::<Currency>(), Ok(Currency::EUR));
        assert_eq!(
            "USD".parse::<Currency>().map(|c| c.to_string()),
            Ok("USD".into())
        );
    }

    #[test]
    fn rejects_invalid_currency_codes() {
        for bad in ["eur", "EU", "EURO", "", "E1R", "ÉUR"] {
            assert!(bad.parse::<Currency>().is_err(), "debería rechazar {bad:?}");
        }
    }

    #[test]
    fn decimal_sums_are_exact() {
        // Con float, 0.1 + 0.2 != 0.3. Con Decimal sí es exacto.
        let a = Money::new(dec!(0.1), Currency::EUR);
        let b = Money::new(dec!(0.2), Currency::EUR);
        assert_eq!(a.checked_add(b), Ok(Money::new(dec!(0.3), Currency::EUR)));
    }

    #[test]
    fn subtraction_works_and_can_go_negative() {
        let a = Money::new(dec!(100), Currency::EUR);
        let b = Money::new(dec!(260.50), Currency::EUR);
        assert_eq!(
            a.checked_sub(b),
            Ok(Money::new(dec!(-160.50), Currency::EUR))
        );
    }

    #[test]
    fn mixing_currencies_is_an_error() {
        let eur = Money::new(dec!(10), Currency::EUR);
        let usd = Money::new(dec!(10), Currency::USD);
        assert_eq!(
            eur.checked_add(usd),
            Err(MoneyError::CurrencyMismatch {
                left: Currency::EUR,
                right: Currency::USD
            })
        );
    }
}

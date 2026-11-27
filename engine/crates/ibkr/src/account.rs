//! La cuenta paper esperada (de `.env`) y la regla de las cuentas de la sesión (ADR 0017 §2.2).
//!
//! La regla para reconocer una cuenta paper por su forma (el prefijo `DU` u otra) está a confirmar
//! en la documentación oficial: hasta entonces solo vale el ID exacto que puso el dueño en `.env`.

use std::fmt;

/// ID de la cuenta paper. No es una credencial, pero es un dato personal: su `Debug` y su
/// `Display` lo tapan salvo los dos últimos caracteres.
#[derive(Clone, PartialEq, Eq)]
pub struct PaperAccountId(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PaperAccountError {
    #[error("falta la variable {0} con el ID de la cuenta paper (ver .env.example): no se conecta")]
    Missing(String),
    #[error("la variable {0} no parece un ID de cuenta (1-32 letras o números): no se conecta")]
    Malformed(String),
}

impl PaperAccountId {
    /// Lee la variable `var` con `env` (en producción, `std::env::var`). Sin ella, no hay
    /// conexión.
    pub fn from_env(
        var: &str,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, PaperAccountError> {
        let raw = env(var).ok_or_else(|| PaperAccountError::Missing(var.to_owned()))?;
        let v = raw.trim();
        if v.is_empty() {
            return Err(PaperAccountError::Missing(var.to_owned()));
        }
        if v.len() > 32 || !v.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(PaperAccountError::Malformed(var.to_owned()));
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Tapa un ID de cuenta para mostrarlo: solo quedan los dos últimos caracteres.
pub fn mask(account: &str) -> String {
    let n = account.chars().count();
    let tail: String = account.chars().skip(n.saturating_sub(2)).collect();
    format!("***{tail}")
}

impl fmt::Debug for PaperAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PaperAccountId({})", mask(&self.0))
    }
}

impl fmt::Display for PaperAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&mask(&self.0))
    }
}

/// `Ok` solo si la sesión informa al menos una cuenta y **todas** son la cuenta paper esperada.
/// El error dice el motivo con los IDs tapados.
pub fn check_session_accounts(session: &[String], paper: &PaperAccountId) -> Result<(), String> {
    if session.is_empty() {
        return Err("IB Gateway no informó ninguna cuenta en la sesión".into());
    }
    let others: Vec<String> = session
        .iter()
        .filter(|a| a.trim() != paper.as_str())
        .map(|a| mask(a.trim()))
        .collect();
    if others.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "la sesión tiene cuentas que no son la paper de .env ({}): ¿sesión real?",
            others.join(", ")
        ))
    }
}

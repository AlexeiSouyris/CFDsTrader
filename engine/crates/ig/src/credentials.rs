//! Credenciales de la cuenta demo. Solo desde variables de entorno y nunca visibles en
//! `Debug`, logs ni errores (CLAUDE.md §5).

/// Texto secreto: su `Debug` y su `Display` no muestran el valor.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// El único acceso al valor. Usarlo solo para armar el pedido.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub username: Secret,
    pub password: Secret,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("falta la variable de entorno {0}")]
pub struct MissingEnv(pub &'static str);

impl Credentials {
    /// `IG_USERNAME` e `IG_PASSWORD`.
    pub fn from_env() -> Result<Self, MissingEnv> {
        Ok(Self {
            username: required("IG_USERNAME")?,
            password: required("IG_PASSWORD")?,
        })
    }
}

/// `IG_API_KEY`, si está. Sin ella, el header lo tiene que poner el proxy del entorno
/// (las sesiones cloud lo inyectan solo).
pub fn api_key_from_env() -> Option<Secret> {
    std::env::var("IG_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(Secret)
}

fn required(name: &'static str) -> Result<Secret, MissingEnv> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(Secret)
        .ok_or(MissingEnv(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_never_print_their_value() {
        let s = Secret::new("muy-secreto");
        assert_eq!(format!("{s} {s:?}"), "*** Secret(***)");
        assert_eq!(s.expose(), "muy-secreto");
        let c = Credentials {
            username: Secret::new("usuario"),
            password: Secret::new("clave"),
        };
        let shown = format!("{c:?}");
        assert!(
            !shown.contains("usuario") && !shown.contains("clave"),
            "{shown}"
        );
    }
}

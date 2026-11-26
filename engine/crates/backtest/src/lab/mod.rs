//! Laboratorio de estrategias (#710, `docs/design/2026-11-laboratorio.md`): una estrategia
//! descrita con bloques (`strategy_spec.v1`) que el motor valida contra su catálogo
//! (`strategy_catalog.v1`) y después corre con el arnés de investigación (`research`).
//!
//! ```text
//! contratos:  spec (spec), catálogo (catalog) y resultado de la validación con la spec tipada
//!             (validation)
//! catálogo:   bloques y parámetros del código + rangos de config/lab.toml + riesgo y distancia
//!             del stop de config/risk.toml                                   → blocks::catalog
//! validación: forma del contrato, bloques y parámetros del catálogo, topes, riesgo ≤ config,
//!             costos ≥ catálogo, período antes del fuera de muestra (research_period)
//!                                                         → validate_spec → ValidSpec + hashes
//! planes:     señales con velas cerradas (decisión al cierre de t, entrada en t + 1), stop en
//!             ATR, salida por tiempo o sesión, objetivo en R → plans_from_spec → research::Plan
//! hash:       SHA-256 de la spec canónica (claves ordenadas, decimales normalizados, sin
//!             espacios); familia = mismo instrumento y mismos tipos de bloque (PENDIENTE §9.2)
//! ```
//! Funciones puras, sin base de datos ni red. La UI nunca calcula nada de esto.

pub mod blocks;
pub mod catalog;
pub mod plans;
pub mod spec;
pub mod validate;
pub mod validation;

pub use blocks::{LabConfig, catalog};
pub use catalog::StrategyCatalog;
pub use plans::{LabSeries, plans_from_spec};
pub use spec::{StrategySpec, canonical_json, family_sha256, sha256_hex, spec_sha256};
pub use validate::{validate_spec, validate_spec_json};
pub use validation::{
    Average, EntryCondition, EntryFilter, ErrorCode, ExitRule, SpecError, SpecValidation, StopRule,
    ValidSpec,
};

//! Laboratorio de estrategias (#710, `docs/design/2026-11-laboratorio.md`): una estrategia
//! descrita con bloques (`strategy_spec.v1`) que el motor valida contra su catálogo
//! (`strategy_catalog.v1`) y después corre con el arnés de investigación (`research`).
//!
//! ```text
//! contratos:  spec (spec), catálogo (catalog) y resultado de la validación con la spec tipada
//!             (validation)
//! hash:       SHA-256 de la spec canónica (claves ordenadas, decimales normalizados, sin
//!             espacios); familia = mismo instrumento y mismos tipos de bloque (PENDIENTE §9.2)
//! ```
//! Funciones puras, sin base de datos ni red. La UI nunca calcula nada de esto.

pub mod catalog;
pub mod spec;
pub mod validation;

pub use catalog::StrategyCatalog;
pub use spec::{StrategySpec, canonical_json, family_sha256, sha256_hex, spec_sha256};
pub use validation::{
    Average, EntryCondition, EntryFilter, ErrorCode, ExitRule, SpecError, SpecValidation, StopRule,
    ValidSpec,
};

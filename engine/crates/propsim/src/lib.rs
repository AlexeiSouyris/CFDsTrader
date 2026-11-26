//! Simulador de cuentas fondeadas (Academia, `docs/design/2026-11-academia.md` §6, #802): reglas
//! de una prop firm como funciones puras y el recorrido de una lista de operaciones por ellas.
//!
//! ```text
//! reglas:    prop_rules.v1 (plantilla con valores en blanco) → RulesDoc::validate → Rules
//!            (montos en dinero, porcentajes como fracción, zona y hora de corte del día)
//!            → rules
//! días:      (cierre UTC, R) × riesgo por operación en dinero → días de la firma → walk
//! etapas:    fases de evaluación (objetivo, días mínimos y máximos, consistencia) y fondeada
//!            hasta el primer cobro; pérdida diaria y caída máxima fija o que sigue al máximo al
//!            cierre del día; intradía = no simulable (no se aproxima)       → walk::run_stage
//! pedido:    prop_sim_request.v1 → prepare + recorrido determinista → prop_sim_result.v1
//!            → request
//! ```
//!
//! Dinero con `Decimal`. Plantillas en `config/propsim.toml`: solo la genérica en blanco; una
//! firma real necesita su reglamento publicado (enlace y fecha). La UI no calcula nada de esto.
//! Educativo: pagar una evaluación real queda fuera de la app (CLAUDE.md §5).

pub mod request;
pub mod rules;
pub mod walk;

pub use request::{PropSimError, PropSimRequest, run};
pub use rules::{PropsimConfig, Rules, RulesDoc};
pub use walk::{DayRef, End, FailRule, StageResult, Walk, firm_day, group_days, run_stage, walk};

//! Estrategia (Fase 4): funciones puras que convierten la evidencia de una vela cerrada en un
//! puntaje de confluencia (spec §6) y un stop estructural (spec §8).
//!
//! Este crate no mira velas: recibe la evidencia ya calculada por indicadores, patrones y
//! liquidez. Así cada regla se prueba sola, con casos construidos a mano.

pub mod config;
pub mod score;
pub mod stop;

pub use cfds_liquidity::Side;
pub use config::{ScoreConfig, StopConfig, StrategyConfig};
pub use score::{Component, Evidence, Score, ScoreError, Trigger, TriggerKind, score};
pub use stop::{risk_per_unit, structural_stop};

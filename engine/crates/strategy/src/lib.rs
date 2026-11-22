//! Estrategia (Fase 4): funciones puras que convierten la evidencia de una vela cerrada en un
//! puntaje de confluencia (spec §6) y un stop estructural (spec §8).
//!
//! El puntaje y el stop no miran velas: reciben la evidencia ya calculada, y así cada regla
//! se prueba sola. `evidence` arma esa evidencia desde las velas cerradas de 15m y 1h.

pub mod config;
pub mod evidence;
pub mod mtf;
pub mod news;
pub mod score;
pub mod stop;

pub use cfds_liquidity::Side;
pub use config::{EvidenceConfig, NewsConfig, ScoreConfig, StopConfig, StrategyConfig};
pub use evidence::{Analysis, Configs, Signal};
pub use mtf::align_closed;
pub use news::{
    BacktestInterpreter, InterpreterCutoff, exit_alerts, news_adjustment, single_adjustment,
};
pub use score::{Component, Evidence, Score, ScoreError, Trigger, TriggerKind, score};
pub use stop::{risk_per_unit, structural_stop};

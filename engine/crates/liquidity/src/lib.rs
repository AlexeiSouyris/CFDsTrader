//! Liquidez en el marco ICT/SMC (spec §5.4). Definiciones objetivas y parametrizadas
//! (`config/liquidity.toml`); todo se evalúa en velas **cerradas** y con pivotes **confirmados**.
//!
//! La spec describe los largos; los cortos son el espejo exacto.

pub mod config;
pub mod fvg;
pub mod order_blocks;
pub mod rejection;
pub mod structure;
pub mod sweep;

pub use config::LiquidityConfig;
pub use fvg::{Fvg, fvg_touches_at, fvgs_up_to};
pub use order_blocks::{OrderBlock, order_block_touches_at, order_blocks};
pub use rejection::{Rejection, RejectionInputs, RejectionVia, rejection_state_at};
pub use structure::{BreakKind, StructureBreak, recent_choch, structure_breaks};
pub use sweep::{Sweep, sweeps_at};

/// Lado de la operación que sugiere una señal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Long,
    Short,
}

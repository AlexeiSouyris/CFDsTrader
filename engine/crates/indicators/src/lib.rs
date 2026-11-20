//! Indicadores técnicos. Cada indicador vive en su propio módulo, con la fórmula
//! exacta documentada (ROADMAP fase 3b). Python y la UI no los recalculan.
//!
//! Convención: cada función recibe una serie de velas **cerradas** y devuelve un
//! valor por vela, alineado por índice. `None` = todavía en calentamiento.

pub mod atr;

pub use atr::{true_range, wilder_atr};

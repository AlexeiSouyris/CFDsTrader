//! Vencimientos de los futuros en el backtest con la fuente de futuros (#267).
//!
//! Las velas de los futuros no se ajustan por vencimiento (ADR 0011): cuando cambia el contrato,
//! el precio salta por la diferencia entre los dos contratos, y ese salto no es un movimiento
//! del mercado. Por eso, en cada cambio de contrato:
//!
//! ```text
//! sea R el inicio de la primera vela del contrato nuevo e i la última vela anterior a R:
//!   force_close[i] = true      una operación abierta se cierra al cierre de i, con el contrato
//!                              viejo (salida "cierre forzado", igual que el cierre de la semana)
//!   entries[i]     = false     no se decide en i: se entraría en la apertura de i+1, ya con
//!                              el contrato nuevo y un stop calculado con precios del viejo
//!   R es un error de datos      el calentamiento empieza de nuevo después de R (spec §3, §7.8):
//!                              los indicadores no mezclan precios de dos contratos
//! ```
//!
//! Se eligió **cerrar antes del vencimiento** y no marcar y excluir las operaciones, porque así
//! ninguna operación del diario cruza un salto que no existió, y el resto de la corrida no
//! cambia. Saber en `i` que el contrato vence no es mirar el futuro: los vencimientos se
//! publican con meses de anticipación, y el contrato continuo `c.0` pasa al siguiente en el
//! vencimiento.

use cfds_core::Candle;
use chrono::{DateTime, Utc};

/// Aplica los cambios de contrato `rolls` (inicios de la primera vela de cada contrato nuevo, en
/// orden) a las marcas del recorrido. `entries` y `force_close` tienen que estar alineados con
/// `m15`. Devuelve los índices de las velas cerradas a la fuerza.
pub fn mark_rollovers(
    m15: &[Candle],
    rolls: &[DateTime<Utc>],
    entries: &mut [bool],
    force_close: &mut [bool],
    data_errors: &mut Vec<DateTime<Utc>>,
) -> Vec<usize> {
    let mut closed = Vec::new();
    for &r in rolls {
        let next = m15.partition_point(|c| c.start < r);
        if next == 0 || next >= m15.len() {
            continue; // el vencimiento cae fuera de la serie
        }
        let i = next - 1;
        if let Some(f) = force_close.get_mut(i) {
            *f = true;
        }
        if let Some(e) = entries.get_mut(i) {
            *e = false;
        }
        data_errors.push(r);
        closed.push(i);
    }
    data_errors.sort();
    data_errors.dedup();
    closed
}

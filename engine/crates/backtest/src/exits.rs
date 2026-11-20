//! Salidas de una operación, vela por vela (spec §8 y §12).
//!
//! ```text
//! la operación entra en la apertura de la vela `entry_index` (t+1); esa vela ya cuenta
//! en cada vela, en este orden:
//!   1. stop  (si en la misma vela se tocan stop y TP, primero el stop: supuesto conservador)
//!   2. TP1 = entrada + 2R → se cierra el 50 % y el stop pasa a la entrada
//!      (si en esa misma vela el precio vuelve a la entrada, se cierra el resto ahí)
//!   3. al cierre: cierre forzado (viernes / fin de sesión), o salida por tiempo si a las
//!      32 velas no tocó +1R
//!   4. al cierre, con TP1 hecho: trailing = max(stop, máximo desde TP1 − 2 × ATR), vale
//!      desde la vela siguiente
//! gap: si la apertura ya saltó el stop o el TP, se ejecuta a la apertura.
//! ```
//! Los cortos se resuelven reflejando los precios (p → −p): son el espejo exacto por
//! construcción.

use cfds_core::Candle;
use cfds_strategy::Side;
use rust_decimal::Decimal;

use crate::ExitConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trade {
    pub side: Side,
    /// Vela de entrada (t+1); la operación entra en su apertura.
    pub entry_index: usize,
    pub entry: Decimal,
    pub stop: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    Stop,
    Tp1,
    /// El stop en la entrada, después de TP1.
    BreakEven,
    Trailing,
    Time,
    /// Cierre de viernes o de fin de sesión.
    Forced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fill {
    pub index: usize,
    pub price: Decimal,
    /// Fracción de la posición original que se cierra.
    pub fraction: Decimal,
    pub reason: ExitReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub fills: Vec<Fill>,
    /// Fracción que sigue abierta al terminar las velas (0 si se cerró todo).
    pub open_fraction: Decimal,
}

impl Outcome {
    /// Resultado en puntos por unidad de tamaño de lo ya cerrado (sin costos).
    pub fn realized_points(&self, trade: &Trade) -> Decimal {
        let sign = match trade.side {
            Side::Long => Decimal::ONE,
            Side::Short => Decimal::NEGATIVE_ONE,
        };
        self.fills
            .iter()
            .map(|f| f.fraction * (f.price - trade.entry) * sign)
            .sum()
    }
}

#[derive(Clone, Copy)]
struct Bar {
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
}

fn bar(c: &Candle, side: Side) -> Bar {
    match side {
        Side::Long => Bar {
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
        },
        Side::Short => Bar {
            open: -c.open,
            high: -c.low,
            low: -c.high,
            close: -c.close,
        },
    }
}

/// Simula la operación desde `entry_index`. `atr` y `force_close` están alineados con
/// `candles` (un `force_close` más corto se toma como `false`). `None` si el stop no está del
/// lado correcto de la entrada (la operación no tiene R).
pub fn simulate(
    trade: &Trade,
    candles: &[Candle],
    atr: &[Option<Decimal>],
    force_close: &[bool],
    cfg: &ExitConfig,
) -> Option<Outcome> {
    let flip = |p: Decimal| match trade.side {
        Side::Long => p,
        Side::Short => -p,
    };
    // Todo en "espacio largo": arriba es a favor.
    let entry = flip(trade.entry);
    let mut stop = flip(trade.stop);
    let r = entry - stop;
    if r <= Decimal::ZERO {
        return None;
    }
    let tp1 = entry + cfg.tp1_r * r;
    let one_r = entry + cfg.time_exit_min_r * r;

    let mut fills = Vec::new();
    let mut remaining = Decimal::ONE;
    let mut stop_reason = ExitReason::Stop;
    let mut best_since_tp1: Option<Decimal> = None;
    let mut reached_min_r = false;
    let mut fill = |index: usize, price: Decimal, fraction: Decimal, reason: ExitReason| {
        fills.push(Fill {
            index,
            price: flip(price),
            fraction,
            reason,
        });
    };

    for (i, c) in candles.iter().enumerate().skip(trade.entry_index) {
        let b = bar(c, trade.side);

        // 1. Stop (con gap: a la apertura si ya lo saltó).
        if b.low <= stop {
            fill(i, b.open.min(stop), remaining, stop_reason);
            remaining = Decimal::ZERO;
            break;
        }

        // 2. TP1.
        if best_since_tp1.is_none() && b.high >= tp1 {
            let part = cfg.tp1_fraction;
            fill(i, b.open.max(tp1), part, ExitReason::Tp1);
            remaining -= part;
            best_since_tp1 = Some(b.high);
            stop = entry;
            stop_reason = ExitReason::BreakEven;
            // Conservador: si en la misma vela el precio volvió a la entrada, se cierra ahí.
            if b.low <= stop {
                fill(i, stop, remaining, stop_reason);
                remaining = Decimal::ZERO;
                break;
            }
        }
        reached_min_r |= b.high >= one_r;

        // 3. Salidas al cierre.
        if force_close.get(i).copied().unwrap_or(false) {
            fill(i, b.close, remaining, ExitReason::Forced);
            remaining = Decimal::ZERO;
            break;
        }
        if !reached_min_r && i + 1 - trade.entry_index == cfg.time_exit_bars {
            fill(i, b.close, remaining, ExitReason::Time);
            remaining = Decimal::ZERO;
            break;
        }

        // 4. Trailing, al cierre, para la vela siguiente.
        if let Some(best) = best_since_tp1 {
            let best = best.max(b.high);
            best_since_tp1 = Some(best);
            if let Some(a) = atr.get(i).copied().flatten() {
                let trail = best - cfg.trailing_atr * a;
                if trail > stop {
                    stop = trail;
                    stop_reason = ExitReason::Trailing;
                }
            }
        }
    }
    Some(Outcome {
        fills,
        open_fraction: remaining,
    })
}

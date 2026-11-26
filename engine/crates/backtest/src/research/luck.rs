//! Test de suerte general (#729): ¿le gana la estrategia a entrar al azar con **las mismas
//! salidas y el mismo tamaño**? Es el modelo nulo de entradas al azar (Basso): solo cambia
//! cuándo y de qué lado se entra; todo lo demás queda igual. Extraído del programa 4 (§7.2 y
//! §13.2 de su pre-registro), que ahora lo usa con su salida planificada.
//!
//! ```text
//! por sesión con operaciones reales: n = sus operaciones, con sus distancias de stop en orden
//! velas candidatas de la sesión: ejecución en j con la decisión permitida en j − 1 y una salida
//!   planificada (la da quien llama: la misma regla de salida de la estrategia)
//! cada simulación (SplitMix64, semilla fija):
//!   n velas distintas al azar (Fisher–Yates parcial), en orden; lado al azar con la proporción
//!   de largos de la estrategia; una posición a la vez dentro de la sesión
//!   stop a la misma distancia, objetivo en R si la estrategia lo tiene, mismo recorrido
//!   (simulate_leg) y mismos costos de ejecución (fill_price)
//! resultado: (R neto medio, retorno total con `risk` por operación) de la estrategia y de cada
//!   simulación; percentil = % de simulaciones por debajo de la estrategia
//! ```
//! Solo reporte: no vota nada por sí mismo.

use std::collections::BTreeMap;

use cfds_core::Candle;
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use super::program4::SplitMix64;
use super::{StudyTrade, decision_allowed, simulate_leg};
use crate::{Execution, Setup, fill_price};

/// R neto de una ejecución: `(salida − entrada) × lado / distancia`, con los precios ejecutados.
fn net_r(side: Side, entry_price: Decimal, exit_price: Decimal, distance: Decimal) -> Decimal {
    let s = match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    };
    (exit_price - entry_price) * s / distance
}

/// R neto medio y retorno total con `risk` por operación (∏(1 + risk·R) − 1).
pub fn r_summary(rs: &[Decimal], risk: Decimal) -> (Decimal, Decimal) {
    if rs.is_empty() {
        return (Decimal::ZERO, Decimal::ZERO);
    }
    let mean = rs.iter().sum::<Decimal>() / Decimal::from(rs.len());
    let total = rs
        .iter()
        .fold(Decimal::ONE, |acc, r| acc * (Decimal::ONE + risk * r))
        - Decimal::ONE;
    (mean, total)
}

/// R neto de una operación ejecutada, medido como en el test de suerte (sin financiación).
pub fn trade_net_r(t: &StudyTrade) -> Option<Decimal> {
    let d = (t.entry_mid - t.stop).abs();
    (d > Decimal::ZERO).then(|| net_r(t.side, t.entry_price, t.exit_price, d))
}

/// Resultado del test de suerte: la estrategia y cada simulación, `(R neto medio, retorno total)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuckTest {
    pub strategy: (Decimal, Decimal),
    pub simulations: Vec<(Decimal, Decimal)>,
}

impl LuckTest {
    /// Porcentaje de simulaciones con un valor menor que el de la estrategia (R medio, total).
    pub fn percentiles(&self) -> (Decimal, Decimal) {
        let n = Decimal::from(self.simulations.len().max(1));
        let below = |f: &dyn Fn(&(Decimal, Decimal)) -> Decimal, v: Decimal| {
            Decimal::ONE_HUNDRED
                * Decimal::from(self.simulations.iter().filter(|s| f(s) < v).count())
                / n
        };
        (
            below(&|s| s.0, self.strategy.0),
            below(&|s| s.1, self.strategy.1),
        )
    }
}

/// Las velas y marcas de la serie, alineadas con `m15`.
#[derive(Debug, Clone, Copy)]
pub struct LuckSeries<'a> {
    pub m15: &'a [Candle],
    pub session: &'a Session,
    /// Marcas de entrada de la preparación (la decisión en `j − 1` tiene que estar permitida).
    pub entries: &'a [bool],
    pub force_close: &'a [bool],
}

/// Cuántas simulaciones, con qué semilla y con qué objetivo en R (`None` = sin objetivo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuckConfig {
    pub simulations: usize,
    pub seed: u64,
    pub target_r: Option<Decimal>,
}

/// Test de suerte (ver el módulo). `planned_exit(j)` es la vela de la salida planificada de una
/// ejecución en `j`, o `None` si la regla de la estrategia no deja entrar ahí. `risk` es el
/// riesgo por operación del retorno total.
pub fn luck_test(
    s: &LuckSeries,
    planned_exit: impl Fn(usize) -> Option<usize>,
    cfg: &LuckConfig,
    trades: &[StudyTrade],
    setup: &Setup,
    risk: Decimal,
) -> LuckTest {
    let m15 = s.m15;
    let real: Vec<Decimal> = trades.iter().filter_map(trade_net_r).collect();
    let strategy = r_summary(&real, risk);
    let longs = trades.iter().filter(|t| t.side == Side::Long).count();
    // Por sesión: las distancias de las operaciones reales, en orden.
    let mut per_day: BTreeMap<NaiveDate, Vec<Decimal>> = BTreeMap::new();
    for t in trades {
        let d = (t.entry_mid - t.stop).abs();
        if d > Decimal::ZERO {
            per_day
                .entry(s.session.trading_day(t.entry_time))
                .or_default()
                .push(d);
        }
    }
    // Velas de ejecución permitidas por sesión, con su salida planificada.
    let mut candidates: BTreeMap<NaiveDate, Vec<(usize, usize)>> = BTreeMap::new();
    for j in 1..m15.len() {
        if !decision_allowed(m15, s.entries, j - 1) {
            continue;
        }
        if let Some(exit) = planned_exit(j) {
            let day = s.session.trading_day(m15[j].start);
            if per_day.contains_key(&day) {
                candidates.entry(day).or_default().push((j, exit));
            }
        }
    }
    let mut rng = SplitMix64::new(cfg.seed);
    let mut simulations = Vec::with_capacity(cfg.simulations);
    for _ in 0..cfg.simulations {
        let mut rs = Vec::new();
        for (day, distances) in &per_day {
            let Some(pool) = candidates.get(day) else {
                continue;
            };
            // n velas distintas (Fisher–Yates parcial), en orden.
            let mut idx: Vec<usize> = (0..pool.len()).collect();
            let n = distances.len().min(pool.len());
            for k in 0..n {
                let r = k + rng.below(pool.len() - k);
                idx.swap(k, r);
            }
            let mut picked: Vec<(usize, usize)> = idx[..n].iter().map(|&k| pool[k]).collect();
            picked.sort_unstable();
            let mut busy_until: Option<usize> = None;
            for (k, (entry_index, exit_index)) in picked.into_iter().enumerate() {
                let side = if rng.chance(longs, trades.len()) {
                    Side::Long
                } else {
                    Side::Short
                };
                if busy_until.is_some_and(|b| entry_index <= b) {
                    continue;
                }
                let distance = distances[k];
                let entry_mid = m15[entry_index].open;
                let (stop, target) = match side {
                    Side::Long => (
                        entry_mid - distance,
                        cfg.target_r.map(|r| entry_mid + r * distance),
                    ),
                    Side::Short => (
                        entry_mid + distance,
                        cfg.target_r.map(|r| entry_mid - r * distance),
                    ),
                };
                let Some((out_index, exit_mid, _)) = simulate_leg(
                    side,
                    entry_index,
                    stop,
                    target,
                    exit_index,
                    m15,
                    s.force_close,
                ) else {
                    continue;
                };
                let c = &setup.backtest.costs;
                let entry_price = fill_price(side, Execution::Entry, entry_mid, &setup.costs, c);
                let exit_price = fill_price(side, Execution::Exit, exit_mid, &setup.costs, c);
                rs.push(net_r(side, entry_price, exit_price, distance));
                busy_until = Some(out_index);
            }
        }
        simulations.push(r_summary(&rs, risk));
    }
    LuckTest {
        strategy,
        simulations,
    }
}

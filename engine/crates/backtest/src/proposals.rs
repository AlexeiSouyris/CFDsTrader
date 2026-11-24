//! Propuestas de la estrategia y su etiqueta de ML (spec de ML v1 §1, #390), **sin P/L**.
//!
//! Recorre la serie de 15m con la misma decisión que el backtest (`choose`: velas cerradas,
//! decisión al cierre de `t` y entrada en la apertura de `t+1`) y el mismo riesgo (`check`), y
//! etiqueta cada propuesta con las mismas salidas (`simulate`):
//!
//! - `y = 1` si toca TP1 antes que el stop; `y = 0` si sale por el stop (también con gap en la
//!   entrada), por tiempo o por un cierre forzado (decisión abierta 1 de la spec de ML).
//! - Sin resolver: los datos terminan antes de saber el resultado. No cuenta en ninguna clase.
//!
//! Diferencias con el backtest, a propósito (solo se cuentan señales):
//! - Precios medios, sin spread, slippage, comisión ni financiación: no se calcula ningún P/L.
//!   TP1 y el stop se miden desde la apertura de `t+1` a precio medio.
//! - La cuenta es siempre la inicial: sin posiciones abiertas, sin drawdown ni pérdida diaria
//!   (dependen del P/L). Se aplican los filtros que no dependen del P/L: distancia del stop en
//!   ATR, margen, tamaño mínimo del broker.
//! - [`Overlap::OneAtATime`] es la regla del backtest (una posición por instrumento: la próxima
//!   decisión es al cierre de la vela de salida). [`Overlap::EveryCandle`] cuenta cada vela
//!   con señal aunque haya una propuesta anterior sin resolver.

use std::collections::BTreeMap;

use cfds_core::Candle;
use cfds_indicators::wilder_atr;
use cfds_risk::{Account, InstrumentLimits, Rejection, Request, RiskConfig, check};
use cfds_strategy::{Analysis, Component, Configs, ScoreError, Side, TriggerKind};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{ExitConfig, ExitReason, Trade, choose, simulate};

/// Todo lo fijo para contar propuestas de un instrumento.
#[derive(Debug, Clone)]
pub struct ProposalRules {
    pub instrument: cfds_core::InstrumentId,
    pub strategy: Configs,
    pub risk: RiskConfig,
    pub limits: InstrumentLimits,
    pub exits: ExitConfig,
    /// Capital de la cuenta (constante: no hay P/L), en la moneda de la cuenta.
    pub equity: Decimal,
    /// Moneda del instrumento por unidad de la moneda de la cuenta.
    pub account_to_instrument: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlap {
    /// Como el backtest: no se decide hasta la vela de salida de la propuesta anterior.
    OneAtATime,
    /// Toda vela con señal, aunque la anterior siga abierta.
    EveryCandle,
}

/// Resultado de una propuesta para la etiqueta de ML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// Tocó TP1 antes que el stop (`y = 1`).
    Tp1,
    /// `y = 0`, con el motivo de la salida (stop, tiempo o cierre forzado).
    Miss(ExitReason),
    /// Los datos terminan sin TP1 ni salida: no entra en ninguna clase.
    Unresolved,
}

impl Label {
    /// `Some(true)` = clase 1, `Some(false)` = clase 0, `None` = sin resolver.
    pub fn y(self) -> Option<bool> {
        match self {
            Label::Tp1 => Some(true),
            Label::Miss(_) => Some(false),
            Label::Unresolved => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Vela de la señal (`t`).
    pub index: usize,
    /// Cierre de la vela de la señal.
    pub signal_at: DateTime<Utc>,
    pub side: Side,
    pub score: i32,
    /// Qué sumó cada señal al puntaje (spec §6).
    pub breakdown: Vec<(Component, i32)>,
    /// Gatillos, sin repetir y en orden.
    pub triggers: Vec<TriggerKind>,
    /// Entrada de referencia: el cierre de la vela de la señal.
    pub reference: Decimal,
    pub stop: Decimal,
    /// TP1 desde la referencia: referencia ± `tp1_r` × R (como `TradeRecord::tp1`).
    pub tp1: Decimal,
    /// Tamaño y apalancamiento que aprobó el riesgo con la cuenta en su capital inicial.
    pub size: Decimal,
    pub leverage: Decimal,
    pub label: Label,
    /// Vela en la que se conoce el resultado (la de la salida final, o la última de la serie).
    pub resolved_index: usize,
    /// Fin de esa vela.
    pub resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProposalReport {
    pub proposals: Vec<Proposal>,
    /// Señales candidatas que el riesgo rechazó, por motivo.
    pub rejected: BTreeMap<String, usize>,
}

/// Propuestas de la serie de 15m (ver el módulo). `entries`, `force_close` y `data_errors` son
/// las mismas marcas que recibe el backtest (`run_checked`); un `entries` vacío deja entrar en
/// todas las velas. Sin noticias: el ajuste del puntaje es 0.
pub fn proposals(
    m15: &[Candle],
    h1: &[Candle],
    entries: &[bool],
    force_close: &[bool],
    data_errors: &[DateTime<Utc>],
    rules: &ProposalRules,
    overlap: Overlap,
) -> Result<ProposalReport, ScoreError> {
    let analysis = Analysis::new(m15, h1, rules.strategy).with_data_errors(data_errors.to_vec());
    let atr = wilder_atr(m15, rules.strategy.patterns.candles.atr_period);
    let account = Account {
        equity: rules.equity,
        peak_equity: rules.equity,
        day_start_equity: rules.equity,
        free_margin: rules.equity,
        kill_switch: false,
        open_positions: Vec::new(),
    };
    let mut report = ProposalReport::default();
    let mut busy_until = 0usize;
    // La vela t+1 tiene que existir para entrar.
    for t in 0..m15.len().saturating_sub(1) {
        let in_window = entries.get(t).copied().unwrap_or(true);
        if t < busy_until || !in_window {
            continue;
        }
        let Some(signal) = choose(&analysis, t, |_| 0)? else {
            continue;
        };
        let Some(atr_t) = atr[t] else {
            continue;
        };
        let request = Request {
            instrument: rules.instrument.clone(),
            entry: signal.reference,
            stop: signal.stop,
            atr: atr_t,
            account_to_instrument: rules.account_to_instrument,
        };
        let sizing = match check(&request, &account, &rules.limits, &rules.risk) {
            Ok(s) => s,
            Err(reason) => {
                let name = match reason {
                    Rejection::InvalidInput(_) => "InvalidInput".to_owned(),
                    other => format!("{other:?}"),
                };
                *report.rejected.entry(name).or_insert(0) += 1;
                continue;
            }
        };
        let trade = Trade {
            side: signal.side,
            entry_index: t + 1,
            entry: m15[t + 1].open,
            stop: signal.stop,
        };
        let last = m15.len() - 1;
        let (label, resolved_index) = match simulate(&trade, m15, &atr, force_close, &rules.exits) {
            // La apertura de t+1 ya saltó el stop (gap): sale por el stop en la entrada.
            None => (Label::Miss(ExitReason::Stop), t + 1),
            Some(o) => {
                let hit_tp1 = o.fills.iter().any(|f| f.reason == ExitReason::Tp1);
                let end = if o.open_fraction > Decimal::ZERO {
                    last
                } else {
                    o.fills.last().map_or(last, |f| f.index)
                };
                let label = if hit_tp1 {
                    Label::Tp1
                } else if o.open_fraction > Decimal::ZERO {
                    Label::Unresolved
                } else {
                    // Sin TP1, la única salida es la final (stop, tiempo o forzada).
                    o.fills
                        .last()
                        .map_or(Label::Unresolved, |f| Label::Miss(f.reason))
                };
                (label, end)
            }
        };
        if overlap == Overlap::OneAtATime {
            busy_until = resolved_index.max(t + 1);
        }
        let mut triggers: Vec<TriggerKind> = signal.triggers.iter().map(|g| g.kind).collect();
        triggers.sort();
        triggers.dedup();
        report.proposals.push(Proposal {
            index: t,
            signal_at: m15[t].end(),
            side: signal.side,
            score: signal.score.total,
            breakdown: signal.score.breakdown.clone(),
            triggers,
            reference: signal.reference,
            stop: signal.stop,
            tp1: signal.reference
                + side_sign(signal.side)
                    * rules.exits.tp1_r
                    * (signal.reference - signal.stop).abs(),
            size: sizing.size,
            leverage: sizing.leverage,
            label,
            resolved_index,
            resolved_at: m15[resolved_index].end(),
        });
    }
    Ok(report)
}

fn side_sign(side: Side) -> Decimal {
    match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    }
}

/// Conteo de un grupo de propuestas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub total: usize,
    /// Clase 1 (TP1 antes que el stop).
    pub y1: usize,
    /// Clase 0.
    pub y0: usize,
    pub unresolved: usize,
}

impl Tally {
    pub fn add(&mut self, label: Label) {
        self.total += 1;
        match label.y() {
            Some(true) => self.y1 += 1,
            Some(false) => self.y0 += 1,
            None => self.unresolved += 1,
        }
    }

    /// Llega al mínimo para entrenar (spec de ML §6): `min_total` propuestas con clase y
    /// `min_per_class` de cada clase.
    pub fn meets(&self, min_total: usize, min_per_class: usize) -> bool {
        self.y1 + self.y0 >= min_total && self.y1 >= min_per_class && self.y0 >= min_per_class
    }
}

/// Cuenta las propuestas por la clave que devuelve `key`.
pub fn tally_by<K: Ord>(
    proposals: &[Proposal],
    key: impl Fn(&Proposal) -> K,
) -> BTreeMap<K, Tally> {
    let mut out = BTreeMap::new();
    for p in proposals {
        out.entry(key(p))
            .or_insert_with(Tally::default)
            .add(p.label);
    }
    out
}

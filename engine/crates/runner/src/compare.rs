//! Comparación paper vs backtest del mismo período (fase 8, #315): el criterio de aprobación de
//! la fase ("la diferencia está explicada").
//!
//! ```text
//! paper:    registro del broker simulado (órdenes + informes) → operaciones valuadas por el broker
//! backtest: el recorrido sobre las mismas velas cerradas y la misma config
//! emparejar por vela de decisión (created_at de la orden = cierre de la vela de la señal)
//!   emparejadas → diferencias de entrada, salida, tamaño y P/L
//!   faltan en paper (el backtest operó) · sobran en paper (el backtest no operó)
//! diferencia total = P/L paper − P/L backtest, repartida por causa (la suma da exacto):
//!   cierre de fin de semana (el backtest cerró por Forced y paper no: desde #334 el broker
//!   aplica la misma regla, así que solo aparece si le falta la sesión del instrumento)
//!   precio de ejecución · tamaño · comisión · swap (en las emparejadas)
//!   rechazo del riesgo (un lado rechazó lo que el otro operó)
//!   falta en paper / sobra en paper (sin un rechazo que lo explique)
//! ```
//! Montos en la moneda del instrumento.

use std::collections::BTreeMap;

use cfds_backtest::{CloseReason, ExitReason, Report, TradeRecord};
use cfds_broker::PaperTrade;
use cfds_core::candle::utc_z;
use cfds_core::decimal_str;
use cfds_core::{Candle, InstrumentId, Order, OrderReport, OrderSide, OrderStatus};
use cfds_liquidity::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

/// Un lado de una operación emparejada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Leg {
    #[serde(with = "utc_z")]
    pub entry_time: DateTime<Utc>,
    #[serde(with = "decimal_str")]
    pub entry_price: Decimal,
    /// Hora de la última salida.
    #[serde(with = "utc_z")]
    pub exit_time: DateTime<Utc>,
    /// Precio de salida promedio, ponderado por la fracción cerrada.
    #[serde(with = "decimal_str")]
    pub exit_price: Decimal,
    #[serde(with = "decimal_str")]
    pub size: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub commission: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub swap: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub net_pnl: Decimal,
}

/// Paper − backtest de una operación emparejada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegDiff {
    #[serde(with = "decimal_str::signed")]
    pub entry_price: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub exit_price: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub size: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub net_pnl: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Matched {
    #[serde(with = "utc_z")]
    pub decision_at: DateTime<Utc>,
    pub side: OrderSide,
    pub order_id: String,
    pub paper: Leg,
    pub backtest: Leg,
    pub difference: LegDiff,
    /// El backtest la cerró por el cierre de fin de semana y el broker simulado no.
    pub weekend_close: bool,
}

/// Una operación del backtest que no está en paper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Missing {
    #[serde(with = "utc_z")]
    pub decision_at: DateTime<Utc>,
    pub side: OrderSide,
    #[serde(with = "decimal_str::signed")]
    pub backtest_net_pnl: Decimal,
    /// `no_order` (el runner no mandó la orden) o el motivo del rechazo del broker.
    pub paper: String,
}

/// Una operación de paper que el backtest no hizo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extra {
    #[serde(with = "utc_z")]
    pub decision_at: DateTime<Utc>,
    pub side: OrderSide,
    pub order_id: String,
    #[serde(with = "decimal_str::signed")]
    pub paper_net_pnl: Decimal,
    /// `no_trade` (el backtest no operó esa vela) o `rejected:<motivo>` del riesgo.
    pub backtest: String,
}

/// La diferencia total repartida por causa. La suma es exactamente `difference`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Causes {
    #[serde(with = "decimal_str::signed")]
    pub fill_price: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub size: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub commission: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub swap: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub weekend_close: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub risk_rejection: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub missing_in_paper: Decimal,
    #[serde(with = "decimal_str::signed")]
    pub extra_in_paper: Decimal,
}

impl Causes {
    pub fn total(&self) -> Decimal {
        self.fill_price
            + self.size
            + self.commission
            + self.swap
            + self.weekend_close
            + self.risk_rejection
            + self.missing_in_paper
            + self.extra_in_paper
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Totals {
    pub trades: usize,
    #[serde(with = "decimal_str::signed")]
    pub net_pnl: Decimal,
}

/// `paper_comparison.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comparison {
    pub comparison_id: String,
    pub instrument_id: InstrumentId,
    #[serde(with = "utc_z")]
    pub from: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub to: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub created_at: DateTime<Utc>,
    pub paper: Totals,
    pub backtest: Totals,
    /// P/L paper − P/L backtest.
    #[serde(with = "decimal_str::signed")]
    pub difference: Decimal,
    pub causes: Causes,
    pub matched: Vec<Matched>,
    pub missing: Vec<Missing>,
    pub extra: Vec<Extra>,
    /// Órdenes de paper rechazadas sin operación del backtest en esa vela, por motivo (por
    /// ejemplo `instrument_already_open`: el runner decide en cada vela; el backtest no decide
    /// con una posición abierta). No mueven el P/L.
    pub paper_rejections: BTreeMap<String, usize>,
}

fn order_side(s: Side) -> OrderSide {
    match s {
        Side::Long => OrderSide::Long,
        Side::Short => OrderSide::Short,
    }
}

fn code<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

/// Los montos se redondean a 8 decimales antes de compararlos: así las sumas por causa dan
/// exactamente la diferencia total (`Decimal` tiene 28 dígitos y los productos los agotan).
const DP: u32 = 8;

fn r(d: Decimal) -> Decimal {
    d.round_dp(DP)
}

impl Leg {
    /// Bruto: neto + comisión + swap.
    fn gross(&self) -> Decimal {
        self.net_pnl + self.commission + self.swap
    }
}

fn paper_leg(p: &PaperTrade) -> Leg {
    let last = p.exits.last();
    Leg {
        entry_time: p.opened_at,
        entry_price: p.entry_fill,
        exit_time: last.map_or(p.opened_at, |e| e.time),
        exit_price: r(p.exits.iter().map(|e| e.price * e.fraction).sum()),
        size: r(p.size),
        commission: r(p.commission),
        swap: r(p.funding),
        net_pnl: r(p.net_pnl),
    }
}

fn backtest_leg(t: &TradeRecord) -> Leg {
    let last = t.exits.last();
    Leg {
        entry_time: t.entry_time,
        entry_price: t.entry_price,
        exit_time: last.map_or(t.entry_time, |e| e.time),
        exit_price: r(t.exits.iter().map(|e| e.price * e.fraction).sum()),
        size: r(t.size),
        commission: r(t.commission),
        swap: r(t.funding),
        net_pnl: r(t.net_pnl),
    }
}

/// Bruto por unidad de tamaño (`0` sin tamaño).
fn per_unit(gross: Decimal, size: Decimal) -> Decimal {
    if size.is_zero() {
        Decimal::ZERO
    } else {
        gross / size
    }
}

/// Lo que entra a la comparación.
pub struct Inputs<'a> {
    pub instrument: &'a InstrumentId,
    /// Período `[from, to)` de las decisiones (cierre de la vela de la señal).
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    /// Registro del broker simulado (todas las órdenes; se filtran instrumento y período).
    pub journal: &'a [(Order, OrderReport)],
    /// Operaciones valuadas por el broker ([`cfds_broker::PaperBroker::trade_outcomes`]).
    pub paper: &'a [PaperTrade],
    /// El backtest sobre las mismas velas de 15m (`m15`) y la misma config.
    pub backtest: &'a Report,
    pub m15: &'a [Candle],
    pub created_at: DateTime<Utc>,
}

/// Compara paper con el backtest. Pura: misma entrada, misma salida.
pub fn compare(i: &Inputs<'_>) -> Comparison {
    let in_period = |t: DateTime<Utc>| t >= i.from && t < i.to;
    let orders: BTreeMap<DateTime<Utc>, &(Order, OrderReport)> = i
        .journal
        .iter()
        .filter(|(o, _)| &o.instrument_id == i.instrument && in_period(o.created_at))
        .map(|j| (j.0.created_at, j))
        .collect();
    let paper_by_order: BTreeMap<&str, &PaperTrade> =
        i.paper.iter().map(|p| (p.order_id.as_str(), p)).collect();
    let trades: BTreeMap<DateTime<Utc>, &TradeRecord> = i
        .backtest
        .trades
        .iter()
        .filter(|t| in_period(t.signal_at))
        .map(|t| (t.signal_at, t))
        .collect();
    let bt_rejected: BTreeMap<DateTime<Utc>, String> = i
        .backtest
        .rejected
        .iter()
        .filter_map(|r| {
            let at = i.m15.get(r.index)?.end();
            Some((at, format!("rejected:{:?}", r.reason)))
        })
        .collect();

    let mut causes = Causes::default();
    let (mut matched, mut missing, mut extra) = (Vec::new(), Vec::new(), Vec::new());
    let mut paper_rejections: BTreeMap<String, usize> = BTreeMap::new();
    let mut paper_total = Totals {
        trades: 0,
        net_pnl: Decimal::ZERO,
    };

    for (at, (order, report)) in &orders {
        let filled = report.status == OrderStatus::Filled;
        let paper = paper_by_order.get(order.order_id.as_str()).copied();
        if let (true, Some(p)) = (filled, paper) {
            paper_total.trades += 1;
            paper_total.net_pnl += r(p.net_pnl);
        }
        let bt = trades
            .get(at)
            .filter(|t| order_side(t.side) == order.side)
            .copied();
        match (filled, paper, bt) {
            (true, Some(p), Some(t)) => {
                let (pl, bl) = (paper_leg(p), backtest_leg(t));
                let diff = pl.net_pnl - bl.net_pnl;
                let weekend = t
                    .exits
                    .iter()
                    .any(|e| e.reason == CloseReason::Exit(ExitReason::Forced));
                if weekend {
                    causes.weekend_close += diff;
                } else {
                    // Tamaño: la diferencia de tamaño al bruto por unidad del backtest; el resto
                    // del bruto es precio de ejecución (entrada y salida).
                    let size = r((pl.size - bl.size) * per_unit(bl.gross(), bl.size));
                    causes.size += size;
                    causes.fill_price += pl.gross() - bl.gross() - size;
                    causes.commission -= pl.commission - bl.commission;
                    causes.swap -= pl.swap - bl.swap;
                }
                matched.push(Matched {
                    decision_at: *at,
                    side: order.side,
                    order_id: order.order_id.to_string(),
                    difference: LegDiff {
                        entry_price: pl.entry_price - bl.entry_price,
                        exit_price: pl.exit_price - bl.exit_price,
                        size: pl.size - bl.size,
                        net_pnl: diff,
                    },
                    paper: pl,
                    backtest: bl,
                    weekend_close: weekend,
                });
            }
            (true, Some(p), None) => {
                let backtest = bt_rejected
                    .get(at)
                    .cloned()
                    .unwrap_or_else(|| "no_trade".into());
                if backtest.starts_with("rejected:") {
                    causes.risk_rejection += r(p.net_pnl);
                } else {
                    causes.extra_in_paper += r(p.net_pnl);
                }
                extra.push(Extra {
                    decision_at: *at,
                    side: order.side,
                    order_id: order.order_id.to_string(),
                    paper_net_pnl: r(p.net_pnl),
                    backtest,
                });
            }
            _ if !filled && bt.is_none() => {
                let reason = report.rejection.as_ref().map_or("unknown".into(), code);
                *paper_rejections.entry(reason).or_default() += 1;
            }
            _ => {}
        }
    }

    let mut backtest_total = Totals {
        trades: 0,
        net_pnl: Decimal::ZERO,
    };
    for (at, t) in &trades {
        backtest_total.trades += 1;
        backtest_total.net_pnl += r(t.net_pnl);
        if matched.iter().any(|m| m.decision_at == *at) {
            continue;
        }
        let paper = match orders.get(at) {
            Some((o, rep))
                if rep.status == OrderStatus::Rejected && o.side == order_side(t.side) =>
            {
                let reason = rep.rejection.as_ref().map_or("unknown".into(), code);
                causes.risk_rejection -= r(t.net_pnl);
                reason
            }
            _ => {
                causes.missing_in_paper -= r(t.net_pnl);
                "no_order".into()
            }
        };
        missing.push(Missing {
            decision_at: *at,
            side: order_side(t.side),
            backtest_net_pnl: r(t.net_pnl),
            paper,
        });
    }

    let difference = paper_total.net_pnl - backtest_total.net_pnl;
    Comparison {
        comparison_id: format!(
            "paper-vs-backtest:{}:{}:{}:{}",
            i.instrument,
            i.from.format("%Y%m%dT%H%MZ"),
            i.to.format("%Y%m%dT%H%MZ"),
            i.created_at.format("%Y%m%dT%H%M%SZ")
        ),
        instrument_id: i.instrument.clone(),
        from: i.from,
        to: i.to,
        created_at: i.created_at,
        paper: paper_total,
        backtest: backtest_total,
        difference,
        causes,
        matched,
        missing,
        extra,
        paper_rejections,
    }
}

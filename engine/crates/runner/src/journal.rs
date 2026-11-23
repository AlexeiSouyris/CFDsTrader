//! El diario del runner de paper (#333): cada propuesta ejecutada como `proposal.v1` y, cuando la
//! operación terminó, su resultado como `outcome.v1`. Así el informe de aprendizaje (fase 8b) y
//! la calibración tienen datos de paper.
//!
//! - **Idempotente:** el id de la propuesta sale de la decisión (`cfds_journal::proposal_id` con
//!   el prefijo del runner como `run_id`) y va en la orden (`order.proposal_id`): un reinicio
//!   propone lo mismo con el mismo id, y el diario no duplica nada.
//! - **Sin look-ahead:** el resultado existe solo cuando la operación cerró entera con velas ya
//!   cerradas ([`paper_outcome`] da `None` mientras siga abierta).

use cfds_backtest::CloseReason;
use cfds_broker::PaperTrade;
use cfds_core::Order;
use cfds_journal::{
    BreakdownItem, Exit, Outcome, OutcomeKind, Proposal, Reason, ShownProbability, Source,
};
use cfds_liquidity::Side;
use cfds_strategy::Signal;
use rust_decimal::Decimal;

/// La propuesta de paper de una orden ya ejecutada. `size` y `leverage` son los del riesgo; la
/// probabilidad, la que se mostró al proponer.
pub fn paper_proposal(
    run_id: &str,
    order: &Order,
    signal: &Signal,
    tp1_r: Decimal,
    size: Decimal,
    leverage: Decimal,
    shown: Option<ShownProbability>,
) -> Proposal {
    let s = match signal.side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    };
    Proposal {
        id: cfds_journal::proposal_id(run_id, &order.instrument_id, order.created_at, signal.side),
        source: Source::Paper,
        run_id: run_id.to_owned(),
        instrument_id: order.instrument_id.clone(),
        side: signal.side.into(),
        signal_at: order.created_at,
        reference: signal.reference,
        stop: signal.stop,
        tp1: signal.reference + s * tp1_r * (signal.reference - signal.stop).abs(),
        size,
        leverage,
        score: signal.score.total,
        breakdown: signal
            .score
            .breakdown
            .iter()
            .map(|&(c, points)| BreakdownItem {
                component: c.into(),
                points,
            })
            .collect(),
        triggers: signal
            .triggers
            .iter()
            .map(|t| cfds_strategy::Component::Trigger(t.kind).into())
            .collect(),
        shown,
    }
}

/// El resultado simulado de una operación de paper, **solo si ya terminó**: todas sus salidas
/// salieron de velas cerradas y no queda nada abierto. Mientras siga abierta, `None`.
pub fn paper_outcome(proposal_id: &str, trade: &PaperTrade) -> Option<Outcome> {
    if trade.exits.is_empty() || trade.exits.iter().any(|e| e.end_of_data) {
        return None;
    }
    let exits: Vec<Exit> = trade
        .exits
        .iter()
        .map(|e| {
            Some(Exit {
                at: e.time,
                price: e.price,
                fraction: e.fraction,
                reason: CloseReason::Exit(e.reason?).into(),
            })
        })
        .collect::<Option<_>>()?;
    Some(Outcome {
        proposal_id: proposal_id.to_owned(),
        kind: OutcomeKind::Simulated,
        entry_at: trade.opened_at,
        entry_price: trade.entry_fill,
        hit_tp1: exits.iter().any(|e| e.reason == Reason::Tp1),
        hit_stop: exits.iter().any(|e| e.reason == Reason::Stop),
        exit_reason: exits.last().map_or(Reason::EndOfData, |e| e.reason),
        exits,
        commission: trade.commission,
        funding: trade.funding,
        gross_pnl: trade.gross_pnl,
        net_pnl: trade.net_pnl,
    })
}

//! Estados emocionales de precio de la spec §9.1 (#220), juntos y listos para guardar
//! (`emotional_state.v1`). **No calcula nada nuevo:** reúne lo que ya existe en el lenguaje y el
//! módulo que pide la spec.
//!
//! | Estado | Dónde | Velas |
//! |---|---|---|
//! | FOMO de precio y clímax de venta | `cfds_regime::price_emotions_with_values` | 1h |
//! | Rechazo | `cfds_liquidity::rejection_state_at` (patrones de velas, niveles, FVG, OB y sweeps) | 15m y 1h |
//!
//! - Un registro por cada vela **cerrada** que se le pasa, también sin estados (`states` vacío),
//!   con `as_of` = cierre de la vela. Las entradas tienen que ser solo velas cerradas.
//! - Sin repintado: cada detector usa solo lo confirmado al cierre de `t` (hay test: el estado de
//!   `t` es el mismo con la serie completa que cortada en `t`).
//! - La distribución no se calcula (pendiente de volumen real, spec §9.1).
//! - Informativo: no suma al puntaje hasta validarse en la fase 5 y votarse.

use std::collections::BTreeMap;

use cfds_core::{ActiveEmotion, Candle, EmotionBasis, EmotionKind, EmotionSource, EmotionalState};
use cfds_indicators::{trend, wilder_atr};
use cfds_liquidity::{
    Rejection, RejectionInputs, RejectionVia, Side, order_blocks, rejection_state_at,
    structure_breaks,
};
use cfds_patterns::confirmed_pivots;
use cfds_regime::{EmotionConfig, PriceEmotion, PriceEmotionValues, price_emotions_with_values};
use rust_decimal::Decimal;

use crate::{Analysis, Configs};

/// Decimales con que se muestran la EMA, el ATR y el estiramiento (solo para leer).
const SHOWN_DP: u32 = 4;

fn shown(d: Decimal) -> String {
    d.round_dp(SHOWN_DP).normalize().to_string()
}

fn price_state(kind: PriceEmotion, v: &PriceEmotionValues, cfg: &EmotionConfig) -> ActiveEmotion {
    let state = match kind {
        PriceEmotion::Fomo => EmotionKind::Fomo,
        PriceEmotion::SellingClimax => EmotionKind::SellingClimax,
    };
    let values = BTreeMap::from([
        ("close".to_owned(), v.close.normalize().to_string()),
        ("ema".to_owned(), shown(v.ema)),
        ("atr".to_owned(), shown(v.atr)),
        ("stretch".to_owned(), shown(v.stretch)),
        (
            "atr_multiple".to_owned(),
            cfg.atr_multiple.normalize().to_string(),
        ),
        ("lookback".to_owned(), cfg.lookback.to_string()),
    ]);
    ActiveEmotion { state, values }
}

fn via_name(v: RejectionVia) -> &'static str {
    match v {
        RejectionVia::Level => "level",
        RejectionVia::Fvg => "fvg",
        RejectionVia::OrderBlock => "order_block",
        RejectionVia::Sweep => "sweep",
    }
}

/// Un estado `rejection` por lado (largo primero), con sus motivos en orden fijo.
fn rejection_states(found: &[Rejection]) -> Vec<ActiveEmotion> {
    [Side::Long, Side::Short]
        .into_iter()
        .filter_map(|side| {
            let mut via: Vec<RejectionVia> = found
                .iter()
                .filter(|r| r.side == side)
                .map(|r| r.via)
                .collect();
            if via.is_empty() {
                return None;
            }
            via.sort();
            via.dedup();
            let side = match side {
                Side::Long => "long",
                Side::Short => "short",
            };
            let via = via.into_iter().map(via_name).collect::<Vec<_>>().join(",");
            Some(ActiveEmotion {
                state: EmotionKind::Rejection,
                values: BTreeMap::from([
                    ("side".to_owned(), side.to_owned()),
                    ("via".to_owned(), via),
                ]),
            })
        })
        .collect()
}

/// Rechazos de cada vela de 1h cerrada: la misma receta que la de 15m de [`Analysis`] (ATR de los
/// patrones, pivotes confirmados, rupturas con la tendencia de 1h y order blocks), sobre la serie
/// de 1h. El sweep es contra los pivotes de la misma serie (`cfds_liquidity::rejection`).
pub fn h1_rejections(h1: &[Candle], cfg: &Configs) -> Vec<Vec<Rejection>> {
    let atr = wilder_atr(h1, cfg.patterns.candles.atr_period);
    let pivots = confirmed_pivots(h1, cfg.patterns.pivots.strength);
    let tr = trend(h1, &cfg.trend);
    let breaks = structure_breaks(h1, &pivots, &tr);
    let ob_atr = if cfg.liquidity.atr_period == cfg.patterns.candles.atr_period {
        atr.clone()
    } else {
        wilder_atr(h1, cfg.liquidity.atr_period)
    };
    let obs = order_blocks(h1, &breaks, &ob_atr, &cfg.liquidity);
    let inputs = RejectionInputs {
        candles: h1,
        atr: &atr,
        pivots: &pivots,
        order_blocks: &obs,
        h1,
        h1_pivots: &pivots,
    };
    (0..h1.len())
        .map(|t| rejection_state_at(&inputs, t, &cfg.patterns, &cfg.liquidity))
        .collect()
}

/// Los estados de precio de cada vela cerrada de `m15` (rechazo) y de `h1` (FOMO, clímax de
/// venta y rechazo), en ese orden.
pub fn price_emotional_states(
    m15: &[Candle],
    h1: &[Candle],
    cfg: &Configs,
    emotions: &EmotionConfig,
) -> Vec<EmotionalState> {
    let mut out = Vec::with_capacity(m15.len() + h1.len());
    if let Some(first) = m15.first() {
        let analysis = Analysis::new(m15, h1, *cfg);
        out.extend(m15.iter().enumerate().map(|(t, c)| EmotionalState {
            instrument_id: first.instrument_id.clone(),
            source: EmotionSource::Price,
            basis: EmotionBasis::M15,
            as_of: c.end(),
            states: rejection_states(&analysis.rejections_at(t)),
        }));
    }
    let price = price_emotions_with_values(h1, emotions);
    let rejections = h1_rejections(h1, cfg);
    out.extend(h1.iter().enumerate().map(|(t, c)| {
        let mut states: Vec<ActiveEmotion> = price[t]
            .iter()
            .map(|(kind, v)| price_state(*kind, v, emotions))
            .collect();
        states.extend(rejection_states(&rejections[t]));
        EmotionalState {
            instrument_id: c.instrument_id.clone(),
            source: EmotionSource::Price,
            basis: EmotionBasis::H1,
            as_of: c.end(),
            states,
        }
    }));
    out
}

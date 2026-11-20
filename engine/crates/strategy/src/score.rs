//! Puntaje de confluencia (spec §6).
//!
//! ```text
//! si no hay NINGÚN gatillo en t → no hay señal
//! tendencia 1h a favor +2 | en contra −3 | neutral 0
//! precio del lado correcto de la SMA50 de 15m      +1
//! martillo o envolvente +1 · ruptura de doble suelo +2 · rechazo en soporte +1 · sweep +2
//! toque de FVG u order block +1 · CHoCH reciente +1 · noticias de −2 a +2
//! puntaje ≥ 5 → candidata a propuesta (después vienen los filtros de §7)
//! ```
//! Cada gatillo suma una sola vez aunque aparezca dos veces (por ejemplo, martillo y envolvente).

use cfds_indicators::Trend;
use rust_decimal::Decimal;

use crate::{ScoreConfig, Side};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TriggerKind {
    /// Martillo, estrella fugaz o envolvente.
    CandlePattern,
    /// Ruptura del neckline de un doble suelo o techo.
    DoubleBreak,
    /// Rechazo en un soporte o resistencia.
    LevelRejection,
    /// Sweep de liquidez externa (pivote de 1h).
    Sweep,
}

/// Un gatillo en `t`, con el precio de la estructura que define su stop (spec §8):
/// mínimo de la vela gatillo (y de la anterior si es envolvente), min(S1, S2), la mecha del
/// sweep o el nivel del soporte. En cortos, los máximos equivalentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trigger {
    pub kind: TriggerKind,
    pub structure: Decimal,
}

/// Lo que se sabe al cierre de la vela `t` para una dirección. Lo arma quien llama, con
/// velas cerradas y después del calentamiento: sin tendencia o sin SMA no hay evidencia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub side: Side,
    pub trend_1h: Trend,
    /// El cierre está del lado correcto de la SMA de 15m (arriba para largos).
    pub sma_side: bool,
    pub triggers: Vec<Trigger>,
    pub fvg_or_order_block: bool,
    pub recent_choch: bool,
    /// Ajuste por noticias (spec §9); 0 si no hay noticias relevantes.
    pub news: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Component {
    Trend,
    SmaSide,
    Trigger(TriggerKind),
    FvgOrOrderBlock,
    RecentChoch,
    News,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Score {
    pub total: i32,
    /// Qué sumó cada cosa (la tendencia aparece siempre, aunque sume 0).
    pub breakdown: Vec<(Component, i32)>,
    /// `total ≥ min_score`: pasa a los filtros de la spec §7.
    pub candidate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScoreError {
    NewsOutOfRange { news: i32, min: i32, max: i32 },
}

/// `Ok(None)` si no hay ningún gatillo: sin gatillo no hay señal.
pub fn score(e: &Evidence, cfg: &ScoreConfig) -> Result<Option<Score>, ScoreError> {
    if e.news < cfg.news_min || e.news > cfg.news_max {
        return Err(ScoreError::NewsOutOfRange {
            news: e.news,
            min: cfg.news_min,
            max: cfg.news_max,
        });
    }
    let mut kinds: Vec<TriggerKind> = e.triggers.iter().map(|t| t.kind).collect();
    kinds.sort();
    kinds.dedup();
    if kinds.is_empty() {
        return Ok(None);
    }

    let trend = match (e.side, e.trend_1h) {
        (_, Trend::Neutral) => 0,
        (Side::Long, Trend::Up) | (Side::Short, Trend::Down) => cfg.trend_with,
        (Side::Long, Trend::Down) | (Side::Short, Trend::Up) => cfg.trend_against,
    };
    let mut breakdown = vec![(Component::Trend, trend)];
    let mut add = |on: bool, c: Component, w: i32| {
        if on && w != 0 {
            breakdown.push((c, w));
        }
    };
    add(e.sma_side, Component::SmaSide, cfg.sma_side);
    for k in kinds {
        let w = match k {
            TriggerKind::CandlePattern => cfg.candle_pattern,
            TriggerKind::DoubleBreak => cfg.double_break,
            TriggerKind::LevelRejection => cfg.level_rejection,
            TriggerKind::Sweep => cfg.sweep,
        };
        add(true, Component::Trigger(k), w);
    }
    add(
        e.fvg_or_order_block,
        Component::FvgOrOrderBlock,
        cfg.fvg_or_order_block,
    );
    add(e.recent_choch, Component::RecentChoch, cfg.recent_choch);
    add(true, Component::News, e.news);

    let total = breakdown.iter().map(|(_, w)| w).sum();
    Ok(Some(Score {
        total,
        breakdown,
        candidate: total >= cfg.min_score,
    }))
}

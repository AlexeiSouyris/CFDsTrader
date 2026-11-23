//! Arma la evidencia de cada vela cerrada de 15m (spec §5 y §6) y la convierte en señal.
//!
//! `Analysis::new` calcula una sola vez todo lo que depende de la serie (ATR, SMA, tendencia
//! alineada, pivotes, rupturas y order blocks). Cada uno de esos cálculos es causal: su valor en
//! `t` solo usa velas hasta `t`. Por eso la señal en `t` es la misma con la serie completa que con
//! solo los datos disponibles al cierre de `t` (hay un test que lo verifica en cada vela).

use cfds_core::Candle;
use cfds_indicators::{Trend, TrendConfig, sma, trend, wilder_atr};
use cfds_liquidity::{
    LiquidityConfig, OrderBlock, StructureBreak, fvg_touches_at, order_block_touches_at,
    order_blocks, recent_choch, structure_breaks, sweeps_at,
};
use cfds_patterns::{
    CandlePattern, DoubleKind, LevelKind, PatternsConfig, Pivot, candle_patterns_at,
    confirmed_pivots, double_at, levels_at, rejections_at,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{
    Evidence, Score, ScoreError, Side, StrategyConfig, Trigger, TriggerKind, align_closed,
    risk_per_unit, score, structural_stop,
};

/// Todas las configuraciones que usa la estrategia, ya validadas.
#[derive(Debug, Clone, Copy)]
pub struct Configs {
    pub patterns: PatternsConfig,
    pub liquidity: LiquidityConfig,
    pub trend: TrendConfig,
    pub strategy: StrategyConfig,
}

/// Señal de una dirección al cierre de `t`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub side: Side,
    pub index: usize,
    pub score: Score,
    /// Precio de referencia: el cierre de `t` (la simulación entra en la apertura de `t+1`).
    pub reference: Decimal,
    pub stop: Decimal,
    /// R con respecto a `reference`.
    pub r: Decimal,
    pub triggers: Vec<Trigger>,
}

pub struct Analysis<'a> {
    m15: &'a [Candle],
    h1: &'a [Candle],
    cfg: Configs,
    atr: Vec<Option<Decimal>>,
    sma: Vec<Option<Decimal>>,
    trend: Vec<Option<Trend>>,
    /// Índice de la última vela de 1h cerrada en cada vela de 15m.
    h1_closed: Vec<Option<usize>>,
    /// Momentos desde los que hubo un error de datos (spec §7.8 y §12), ordenados.
    data_errors: Vec<DateTime<Utc>>,
    pivots: Vec<Pivot>,
    h1_pivots: Vec<Pivot>,
    breaks: Vec<StructureBreak>,
    obs: Vec<OrderBlock>,
}

impl<'a> Analysis<'a> {
    pub fn new(m15: &'a [Candle], h1: &'a [Candle], cfg: Configs) -> Self {
        let atr = wilder_atr(m15, cfg.patterns.candles.atr_period);
        let h1_trend = trend(h1, &cfg.trend);
        let h1_closed = align_closed(h1, m15);
        let trend: Vec<Option<Trend>> = h1_closed
            .iter()
            .map(|j| j.and_then(|j| h1_trend[j]))
            .collect();
        let strength = cfg.patterns.pivots.strength;
        let pivots = confirmed_pivots(m15, strength);
        let breaks = structure_breaks(m15, &pivots, &trend);
        let ob_atr = if cfg.liquidity.atr_period == cfg.patterns.candles.atr_period {
            atr.clone()
        } else {
            wilder_atr(m15, cfg.liquidity.atr_period)
        };
        let obs = order_blocks(m15, &breaks, &ob_atr, &cfg.liquidity);
        Self {
            m15,
            h1,
            cfg,
            sma: sma(m15, cfg.strategy.evidence.sma_period),
            h1_pivots: confirmed_pivots(h1, strength),
            atr,
            trend,
            h1_closed,
            data_errors: Vec::new(),
            pivots,
            breaks,
            obs,
        }
    }

    /// ATR de las velas de 15m, alineado por índice (solo lectura, para exportar: #81).
    pub fn atr(&self) -> &[Option<Decimal>] {
        &self.atr
    }

    /// SMA de 15m de la evidencia (`sma_period`), alineada por índice.
    pub fn sma(&self) -> &[Option<Decimal>] {
        &self.sma
    }

    /// Tendencia de la última vela de 1h cerrada en cada vela de 15m.
    pub fn trend_1h(&self) -> &[Option<Trend>] {
        &self.trend
    }

    /// Pivotes confirmados de 15m.
    pub fn pivots(&self) -> &[Pivot] {
        &self.pivots
    }

    /// Rechazo de la spec §9.1 (#220) en la vela de 15m `t`, con las entradas que ya armó el
    /// análisis (ATR de los patrones, pivotes, order blocks y pivotes de 1h para el sweep).
    pub fn rejections_at(&self, t: usize) -> Vec<cfds_liquidity::Rejection> {
        cfds_liquidity::rejection_state_at(
            &cfds_liquidity::RejectionInputs {
                candles: self.m15,
                atr: &self.atr,
                pivots: &self.pivots,
                order_blocks: &self.obs,
                h1: self.h1,
                h1_pivots: &self.h1_pivots,
            },
            t,
            &self.cfg.patterns,
            &self.cfg.liquidity,
        )
    }

    /// Errores de datos del instrumento (`cfds_market_data::check_series`): después de cada uno,
    /// el calentamiento empieza de nuevo y hacen falta otra vez `min_m15_candles` y
    /// `min_h1_candles` velas válidas, **posteriores** al error (spec §3, §7.8 y §12).
    pub fn with_data_errors(mut self, mut errors: Vec<DateTime<Utc>>) -> Self {
        errors.sort();
        self.data_errors = errors;
        self
    }

    /// `true` cuando en `t` ya pasó el calentamiento de la spec §3: hay `min_m15_candles` velas
    /// de 15m (hasta `t` inclusive) y `min_h1_candles` velas de 1h cerradas, todas posteriores
    /// al último error de datos conocido al cierre de `t`.
    pub fn warmed_up(&self, t: usize) -> bool {
        let Some(c) = self.m15.get(t) else {
            return false;
        };
        let e = &self.cfg.strategy.evidence;
        // Último error conocido al cierre de `t` (una vela en curso o futura no cuenta).
        let known = self.data_errors.partition_point(|x| *x < c.end());
        let after = |series: &[Candle]| match known.checked_sub(1) {
            Some(k) => series.partition_point(|x| x.start <= self.data_errors[k]),
            None => 0,
        };
        let m15_count = t + 1 - after(&self.m15[..=t]);
        let h1_count = self.h1_closed[t].map_or(0, |j| j + 1 - after(&self.h1[..=j]));
        m15_count >= e.min_m15_candles && h1_count >= e.min_h1_candles
    }

    /// Evidencia al cierre de `t` para `side`. `None` durante el calentamiento (spec §3, y
    /// mientras falten la SMA o la tendencia de 1h) o si `t` está fuera de la serie. Los patrones
    /// que necesitan ATR no aparecen hasta que el ATR existe.
    pub fn evidence_at(&self, t: usize, side: Side, news: i32) -> Option<Evidence> {
        let c = self.m15.get(t)?;
        if !self.warmed_up(t) {
            return None;
        }
        let trend_1h = self.trend[t]?;
        let sma = self.sma[t]?;
        let long = side == Side::Long;
        let p = &self.cfg.patterns;
        let mut triggers = Vec::new();

        for pat in candle_patterns_at(self.m15, t, &self.atr, &p.candles) {
            if pat.is_long() != long {
                continue;
            }
            let mut bars = vec![c];
            if matches!(
                pat,
                CandlePattern::BullishEngulfing | CandlePattern::BearishEngulfing
            ) {
                bars.push(&self.m15[t - 1]);
            }
            let structure = if long {
                bars.iter().map(|b| b.low).min()
            } else {
                bars.iter().map(|b| b.high).max()
            };
            triggers.extend(structure.map(|structure| Trigger {
                kind: TriggerKind::CandlePattern,
                structure,
            }));
        }

        let double = if long {
            DoubleKind::Bottom
        } else {
            DoubleKind::Top
        };
        for d in double_at(self.m15, t, &self.pivots, &self.atr, &p.double) {
            if d.kind == double {
                triggers.push(Trigger {
                    kind: TriggerKind::DoubleBreak,
                    structure: d.extreme(),
                });
            }
        }

        let level = if long {
            LevelKind::Support
        } else {
            LevelKind::Resistance
        };
        let levels = levels_at(t, &self.pivots, &p.levels);
        for l in rejections_at(self.m15, t, &levels, &p.levels) {
            if l.kind == level {
                triggers.push(Trigger {
                    kind: TriggerKind::LevelRejection,
                    structure: l.price,
                });
            }
        }

        if sweeps_at(c, self.h1, &self.h1_pivots)
            .iter()
            .any(|s| s.side == side)
        {
            triggers.push(Trigger {
                kind: TriggerKind::Sweep,
                // La mecha del sweep (spec §8).
                structure: if long { c.low } else { c.high },
            });
        }

        let liq = &self.cfg.liquidity;
        let fvg_or_order_block = fvg_touches_at(self.m15, t, liq)
            .iter()
            .any(|f| f.side == side)
            || order_block_touches_at(self.m15, t, &self.obs)
                .iter()
                .any(|o| o.side == side);

        Some(Evidence {
            side,
            trend_1h,
            sma_side: if long { c.close > sma } else { c.close < sma },
            triggers,
            fvg_or_order_block,
            recent_choch: recent_choch(&self.breaks, t, side, liq),
            news,
        })
    }

    /// Señal al cierre de `t` para `side`: `Ok(None)` si no hay evidencia, no hay gatillo o no
    /// se puede definir el riesgo (stop del lado equivocado).
    pub fn signal_at(&self, t: usize, side: Side, news: i32) -> Result<Option<Signal>, ScoreError> {
        let Some(e) = self.evidence_at(t, side, news) else {
            return Ok(None);
        };
        let Some(score) = score(&e, &self.cfg.strategy.score)? else {
            return Ok(None);
        };
        let (Some(atr), reference) = (self.atr[t], self.m15[t].close) else {
            return Ok(None);
        };
        let Some(stop) = structural_stop(side, &e.triggers, atr, &self.cfg.strategy.stop) else {
            return Ok(None);
        };
        let Some(r) = risk_per_unit(side, reference, stop) else {
            return Ok(None);
        };
        Ok(Some(Signal {
            side,
            index: t,
            score,
            reference,
            stop,
            r,
            triggers: e.triggers,
        }))
    }
}

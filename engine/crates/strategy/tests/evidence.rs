//! Ensamblado de la evidencia vela por vela (#47): calentamiento, coherencia y, sobre todo,
//! que la señal en `t` no cambie al llegar datos nuevos (sin repintado ni look-ahead).

use cfds_core::{Candle, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_patterns::PatternsConfig;
use cfds_strategy::{Analysis, Configs, ScoreError, Side, StrategyConfig, TriggerKind};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn configs() -> Configs {
    Configs {
        patterns: PatternsConfig::from_toml(include_str!("../../../../config/patterns.toml"))
            .unwrap(),
        liquidity: LiquidityConfig::from_toml(include_str!("../../../../config/liquidity.toml"))
            .unwrap(),
        trend: TrendConfig::from_toml(include_str!("../../../../config/indicators.toml")).unwrap(),
        strategy: StrategyConfig::from_toml(include_str!("../../../../config/strategy.toml"))
            .unwrap(),
    }
}

fn t0() -> DateTime<Utc> {
    "2026-01-05T00:00:00Z".parse().unwrap()
}

/// Caminata aleatoria determinística (LCG) de velas de 15m, con mechas y regímenes que cambian
/// de dirección, para que aparezcan patrones de todo tipo.
fn random_walk(n: usize, seed: u64) -> Vec<Candle> {
    let mut state = seed;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let mut close = dec!(1000);
    (0..n)
        .map(|i| {
            let drift = if (i / 150) % 2 == 0 { 1 } else { -1 };
            let open = close;
            let step = Decimal::new(next(41) - 20 + drift * 2, 1);
            close = open + step;
            let up = Decimal::new(next(25), 1);
            let down = Decimal::new(next(25), 1);
            Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0() + TimeDelta::minutes(15 * i as i64),
                open,
                high: open.max(close) + up,
                low: open.min(close) - down,
                close,
                volume: None,
            }
        })
        .collect()
}

/// Velas de 1h armadas con grupos de 4 velas de 15m completas.
fn resample_h1(m15: &[Candle]) -> Vec<Candle> {
    m15.chunks_exact(4)
        .map(|g| Candle {
            instrument_id: g[0].instrument_id.clone(),
            timeframe: Timeframe::H1,
            start: g[0].start,
            open: g[0].open,
            high: g.iter().map(|c| c.high).max().unwrap(),
            low: g.iter().map(|c| c.low).min().unwrap(),
            close: g[3].close,
            volume: None,
        })
        .collect()
}

#[test]
fn the_signal_at_t_only_uses_data_available_at_the_close_of_t() {
    let m15 = random_walk(1200, 7);
    let h1 = resample_h1(&m15);
    let full = Analysis::new(&m15, &h1, configs());
    let mut signals = 0;
    for t in 0..m15.len() {
        let cut_m15 = &m15[..=t];
        let closed_h1: Vec<Candle> = h1
            .iter()
            .filter(|k| k.end() <= m15[t].end())
            .cloned()
            .collect();
        let past = Analysis::new(cut_m15, &closed_h1, configs());
        for side in [Side::Long, Side::Short] {
            let a = full.signal_at(t, side, 0).unwrap();
            assert_eq!(a, past.signal_at(t, side, 0).unwrap(), "t = {t}, {side:?}");
            signals += usize::from(a.is_some());
        }
    }
    // La serie tiene que producir señales; si no, el test no estaría probando nada.
    assert!(signals > 20, "solo {signals} señales");
}

#[test]
fn every_kind_of_trigger_shows_up_in_a_long_series() {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let a = Analysis::new(&m15, &h1, configs());
    let mut seen = std::collections::HashSet::new();
    for t in 0..m15.len() {
        for side in [Side::Long, Side::Short] {
            if let Some(s) = a.signal_at(t, side, 0).unwrap() {
                seen.extend(s.triggers.iter().map(|g| g.kind));
            }
        }
    }
    for k in [
        TriggerKind::CandlePattern,
        TriggerKind::DoubleBreak,
        TriggerKind::LevelRejection,
        TriggerKind::Sweep,
    ] {
        assert!(seen.contains(&k), "nunca apareció {k:?}");
    }
}

#[test]
fn signals_are_coherent() {
    let m15 = random_walk(1500, 3);
    let h1 = resample_h1(&m15);
    let a = Analysis::new(&m15, &h1, configs());
    for (t, c) in m15.iter().enumerate() {
        for side in [Side::Long, Side::Short] {
            let Some(s) = a.signal_at(t, side, 0).unwrap() else {
                continue;
            };
            assert_eq!((s.index, s.side, s.reference), (t, side, c.close));
            assert!(!s.triggers.is_empty());
            assert!(s.r > Decimal::ZERO);
            match side {
                Side::Long => {
                    assert_eq!(s.reference - s.stop, s.r);
                    // El stop queda por debajo de toda la estructura de los gatillos.
                    assert!(s.triggers.iter().all(|g| s.stop < g.structure));
                }
                Side::Short => {
                    assert_eq!(s.stop - s.reference, s.r);
                    assert!(s.triggers.iter().all(|g| s.stop > g.structure));
                }
            }
            assert_eq!(s.score.candidate, s.score.total >= 5);
        }
    }
}

#[test]
fn no_evidence_during_warmup() {
    let m15 = random_walk(600, 5);
    let h1 = resample_h1(&m15);
    let a = Analysis::new(&m15, &h1, configs());
    // La tendencia de 1h necesita 50 + 5 velas de 1h cerradas: 55 × 4 velas de 15m.
    for t in 0..(55 * 4 - 1) {
        for side in [Side::Long, Side::Short] {
            assert_eq!(a.evidence_at(t, side, 0), None, "t = {t}");
        }
    }
    assert!(a.evidence_at(55 * 4 - 1, Side::Long, 0).is_some());
    // Sin velas de 1h no hay tendencia, y sin tendencia no hay evidencia.
    let none = Analysis::new(&m15, &[], configs());
    assert!((0..m15.len()).all(|t| none.evidence_at(t, Side::Long, 0).is_none()));
    // Fuera de la serie, tampoco.
    assert_eq!(a.evidence_at(m15.len(), Side::Long, 0), None);
}

#[test]
fn news_out_of_range_is_an_error_not_a_silent_clamp() {
    let m15 = random_walk(1200, 7);
    let h1 = resample_h1(&m15);
    let a = Analysis::new(&m15, &h1, configs());
    let t = (0..m15.len())
        .find(|&t| a.signal_at(t, Side::Long, 0).unwrap().is_some())
        .unwrap();
    assert!(matches!(
        a.signal_at(t, Side::Long, 3),
        Err(ScoreError::NewsOutOfRange { .. })
    ));
    let plus = a.signal_at(t, Side::Long, 2).unwrap().unwrap();
    let zero = a.signal_at(t, Side::Long, 0).unwrap().unwrap();
    assert_eq!(plus.score.total, zero.score.total + 2);
}

// ---------- Cada pieza de la evidencia, contra los crates que la calculan ----------

use cfds_liquidity::fvg_touches_at;
use cfds_patterns::{CandlePattern, DoubleKind, candle_patterns_at, confirmed_pivots, double_at};

fn each_evidence(
    seed: u64,
    mut check: impl FnMut(&[Candle], usize, Side, &cfds_strategy::Evidence),
) {
    let m15 = random_walk(2000, seed);
    let h1 = resample_h1(&m15);
    let a = Analysis::new(&m15, &h1, configs());
    for t in 0..m15.len() {
        for side in [Side::Long, Side::Short] {
            if let Some(e) = a.evidence_at(t, side, 0) {
                check(&m15, t, side, &e);
            }
        }
    }
}

fn structures(e: &cfds_strategy::Evidence, kind: TriggerKind) -> Vec<Decimal> {
    e.triggers
        .iter()
        .filter(|g| g.kind == kind)
        .map(|g| g.structure)
        .collect()
}

#[test]
fn candle_triggers_follow_the_patterns_of_their_side() {
    let cfg = configs();
    let mut engulfings = 0;
    each_evidence(13, |m15, t, side, e| {
        let atr = cfds_indicators::wilder_atr(m15, cfg.patterns.candles.atr_period);
        let pats: Vec<CandlePattern> = candle_patterns_at(m15, t, &atr, &cfg.patterns.candles)
            .into_iter()
            .filter(|p| p.is_long() == (side == Side::Long))
            .collect();
        let got = structures(e, TriggerKind::CandlePattern);
        assert_eq!(got.is_empty(), pats.is_empty(), "t = {t}, {side:?}");
        for (p, s) in pats.iter().zip(&got) {
            let (c, prev) = (&m15[t], &m15[t - 1]);
            let expected = match (p, side) {
                (CandlePattern::BullishEngulfing, _) => c.low.min(prev.low),
                (CandlePattern::BearishEngulfing, _) => c.high.max(prev.high),
                (_, Side::Long) => c.low,
                (_, Side::Short) => c.high,
            };
            engulfings += usize::from(
                matches!(
                    p,
                    CandlePattern::BullishEngulfing | CandlePattern::BearishEngulfing
                ) && expected != c.low
                    && expected != c.high,
            );
            assert_eq!(*s, expected, "t = {t}, {p:?}");
        }
    });
    // Envolventes cuyo stop depende de la vela anterior: si no hay, el test no prueba eso.
    assert!(engulfings > 0);
}

#[test]
fn double_and_sweep_triggers_use_their_structure() {
    let cfg = configs();
    let (mut doubles, mut sweeps) = (0, 0);
    each_evidence(11, |m15, t, side, e| {
        let pivots = confirmed_pivots(m15, cfg.patterns.pivots.strength);
        let atr = cfds_indicators::wilder_atr(m15, cfg.patterns.candles.atr_period);
        let kind = match side {
            Side::Long => DoubleKind::Bottom,
            Side::Short => DoubleKind::Top,
        };
        let expected: Vec<Decimal> = double_at(m15, t, &pivots, &atr, &cfg.patterns.double)
            .iter()
            .filter(|d| d.kind == kind)
            .map(|d| d.extreme())
            .collect();
        assert_eq!(structures(e, TriggerKind::DoubleBreak), expected, "t = {t}");
        doubles += expected.len();
        for s in structures(e, TriggerKind::Sweep) {
            let wick = match side {
                Side::Long => m15[t].low,
                Side::Short => m15[t].high,
            };
            assert_eq!(s, wick, "t = {t}");
            sweeps += 1;
        }
    });
    assert!(
        doubles > 0 && sweeps > 0,
        "{doubles} dobles, {sweeps} sweeps"
    );
}

#[test]
fn sma_side_and_fvg_context_match_their_definitions() {
    let cfg = configs();
    let (mut fvgs, mut short_sma) = (0, 0);
    each_evidence(7, |m15, t, side, e| {
        let sma = cfds_indicators::sma(m15, cfg.strategy.evidence.sma_period)[t].unwrap();
        let expected = match side {
            Side::Long => m15[t].close > sma,
            Side::Short => m15[t].close < sma,
        };
        assert_eq!(e.sma_side, expected, "t = {t}, {side:?}");
        short_sma += usize::from(side == Side::Short && expected);
        if fvg_touches_at(m15, t, &cfg.liquidity)
            .iter()
            .any(|f| f.side == side)
        {
            // Un toque de FVG alcanza, aunque no haya order block.
            assert!(e.fvg_or_order_block, "t = {t}, {side:?}");
            fvgs += 1;
        }
    });
    assert!(fvgs > 0 && short_sma > 0);
}

#[test]
fn with_1h_history_the_15m_sma_still_needs_its_warmup() {
    let walk = random_walk(1000, 5);
    let h1 = resample_h1(&walk);
    // Las velas de 15m empiezan cuando la tendencia de 1h ya existe.
    let m15 = &walk[400..];
    let a = Analysis::new(m15, &h1, configs());
    for t in 0..49 {
        assert_eq!(a.evidence_at(t, Side::Long, 0), None, "t = {t}");
    }
    assert!(a.evidence_at(49, Side::Long, 0).is_some());
}

#[test]
fn level_sweep_and_choch_evidence_match_their_definitions() {
    use cfds_liquidity::{recent_choch, structure_breaks, sweeps_at};
    use cfds_patterns::{LevelKind, levels_at, rejections_at};
    let cfg = configs();
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let a = Analysis::new(&m15, &h1, cfg);
    let pivots = confirmed_pivots(&m15, cfg.patterns.pivots.strength);
    let h1_pivots = confirmed_pivots(&h1, cfg.patterns.pivots.strength);
    let h1_trend = cfds_indicators::trend(&h1, &cfg.trend);
    let trend: Vec<_> = cfds_strategy::align_closed(&h1, &m15)
        .into_iter()
        .map(|j| j.and_then(|j| h1_trend[j]))
        .collect();
    let breaks = structure_breaks(&m15, &pivots, &trend);
    let (mut levels_seen, mut sweeps_seen, mut chochs_seen) = (0, 0, 0);
    for t in 0..m15.len() {
        for side in [Side::Long, Side::Short] {
            let Some(e) = a.evidence_at(t, side, 0) else {
                continue;
            };
            let kind = match side {
                Side::Long => LevelKind::Support,
                Side::Short => LevelKind::Resistance,
            };
            let levels = levels_at(t, &pivots, &cfg.patterns.levels);
            let expected: Vec<Decimal> = rejections_at(&m15, t, &levels, &cfg.patterns.levels)
                .iter()
                .filter(|l| l.kind == kind)
                .map(|l| l.price)
                .collect();
            assert_eq!(
                structures(&e, TriggerKind::LevelRejection),
                expected,
                "t = {t}"
            );
            levels_seen += expected.len();

            let swept = sweeps_at(&m15[t], &h1, &h1_pivots)
                .iter()
                .any(|s| s.side == side);
            assert_eq!(
                !structures(&e, TriggerKind::Sweep).is_empty(),
                swept,
                "t = {t}, {side:?}"
            );
            sweeps_seen += usize::from(swept);

            let choch = recent_choch(&breaks, t, side, &cfg.liquidity);
            assert_eq!(e.recent_choch, choch, "t = {t}, {side:?}");
            chochs_seen += usize::from(choch);
        }
    }
    assert!(levels_seen > 0 && sweeps_seen > 0 && chochs_seen > 0);
}

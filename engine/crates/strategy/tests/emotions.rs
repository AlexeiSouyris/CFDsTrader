//! Estados emocionales de precio juntos (`emotional_state.v1`, spec §9.1, #220): sin repintado,
//! valores de la regla y forma del contrato. Sin red ni servicios.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_core::{Candle, EmotionBasis, EmotionKind, EmotionSource, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_patterns::PatternsConfig;
use cfds_regime::EmotionConfig;
use cfds_strategy::{Configs, StrategyConfig, price_emotional_states};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn configs() -> Configs {
    Configs {
        patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
        liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
        trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
        strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
    }
}

fn t0() -> DateTime<Utc> {
    "2026-01-05T00:00:00Z".parse().unwrap()
}

/// Caminata aleatoria determinística (LCG) de 15m con mechas y tramos que cambian de dirección.
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
            close = open + Decimal::new(next(41) - 20 + drift * 2, 1);
            let (up, down) = (Decimal::new(next(25), 1), Decimal::new(next(25), 1));
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

fn real_emotions() -> EmotionConfig {
    EmotionConfig::from_toml(cfg!("regime.toml")).unwrap()
}

#[test]
fn one_record_per_closed_candle_and_it_never_repaints() {
    let m15 = random_walk(1200, 7);
    let h1 = resample_h1(&m15);
    let (cfg, em) = (configs(), real_emotions());
    let full = price_emotional_states(&m15, &h1, &cfg, &em);
    assert_eq!(full.len(), m15.len() + h1.len());
    assert!(full.iter().all(|s| s.source == EmotionSource::Price));
    let active = full.iter().filter(|s| !s.states.is_empty()).count();
    assert!(active > 0, "la caminata tiene que activar algún estado");
    // Cortando la serie en varios momentos, lo ya calculado no cambia.
    for cut in [200, 480, 801, 1200] {
        let m = &m15[..cut];
        let h = resample_h1(m);
        let end = m[cut - 1].end();
        let part = price_emotional_states(m, &h, &cfg, &em);
        let known: Vec<_> = full.iter().filter(|s| s.as_of <= end).collect();
        let part_known: Vec<_> = part.iter().filter(|s| s.as_of <= end).collect();
        assert_eq!(part_known, known, "corte en {cut}");
    }
}

#[test]
fn fomo_and_climax_carry_their_rule_values() {
    // EMA(3), ATR(2), ventana 3, umbral 0,5 ATR: en t4 EMA = 105 y ATR = 6,5 (cuentas a mano,
    // las mismas de cfds-regime): (110 − 105) / 6,5 ≈ 0,7692.
    let em = EmotionConfig {
        ema_period: 3,
        atr_period: 2,
        atr_multiple: dec!(0.5),
        lookback: 3,
    };
    let h1: Vec<Candle> = [100, 100, 100, 100, 110]
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let c = Decimal::from(c);
            Candle {
                instrument_id: "US500".parse().unwrap(),
                timeframe: Timeframe::H1,
                start: t0() + TimeDelta::hours(i as i64),
                open: c,
                high: c + Decimal::ONE,
                low: c - Decimal::ONE,
                close: c,
                volume: None,
            }
        })
        .collect();
    let rows = price_emotional_states(&[], &h1, &configs(), &em);
    assert_eq!(rows.len(), 5);
    let last = &rows[4];
    assert_eq!(last.basis, EmotionBasis::H1);
    assert_eq!(last.as_of, t0() + TimeDelta::hours(5));
    let fomo = last
        .states
        .iter()
        .find(|s| s.state == EmotionKind::Fomo)
        .expect("FOMO en t4");
    assert_eq!(
        fomo.values,
        BTreeMap::from(
            [
                ("close", "110"),
                ("ema", "105"),
                ("atr", "6.5"),
                ("stretch", "0.7692"),
                ("atr_multiple", "0.5"),
                ("lookback", "3"),
            ]
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
        )
    );
    assert!(rows[..4].iter().all(|r| r.states.is_empty()));
}

#[test]
fn every_record_matches_the_contract() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/emotional_state.v1.schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    let m15 = random_walk(800, 11);
    let h1 = resample_h1(&m15);
    let rows = price_emotional_states(&m15, &h1, &configs(), &real_emotions());
    let mut seen = std::collections::BTreeSet::new();
    for r in &rows {
        let doc = serde_json::to_value(r).unwrap();
        let errors: Vec<String> = v.iter_errors(&doc).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{errors:?}\n{doc}");
        let back: cfds_core::EmotionalState = serde_json::from_value(doc).unwrap();
        assert_eq!(&back, r);
        seen.extend(r.states.iter().map(|s| s.state));
    }
    assert!(seen.contains(&EmotionKind::Rejection), "{seen:?}");
}

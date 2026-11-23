//! FOMO de precio y clímax de venta en 1h (spec §9.1, #220). Sin red ni servicios.

use cfds_core::{Candle, Timeframe};
use cfds_regime::{EmotionConfig, PriceEmotion, price_emotions};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const CONFIG: &str = include_str!("../../../../config/regime.toml");

/// Velas de 1h seguidas con esos cierres, rango ±1 alrededor del cierre (apertura = cierre).
fn h1(closes: &[i64]) -> Vec<Candle> {
    let t0: DateTime<Utc> = "2026-11-16T14:00:00Z".parse().unwrap();
    closes
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let c = Decimal::from(c);
            Candle {
                instrument_id: "US500".parse().unwrap(),
                timeframe: Timeframe::H1,
                start: t0 + TimeDelta::hours(i as i64),
                open: c,
                high: c + Decimal::ONE,
                low: c - Decimal::ONE,
                close: c,
                volume: None,
            }
        })
        .collect()
}

/// EMA(3), ATR(2), ventana de 3 velas: cuentas a mano.
fn small(multiple: Decimal) -> EmotionConfig {
    EmotionConfig {
        ema_period: 3,
        atr_period: 2,
        atr_multiple: multiple,
        lookback: 3,
    }
}

#[test]
fn the_real_config_has_the_spec_values() {
    let cfg = EmotionConfig::from_toml(CONFIG).unwrap();
    assert_eq!(
        (
            cfg.ema_period,
            cfg.atr_period,
            cfg.atr_multiple,
            cfg.lookback
        ),
        (20, 14, dec!(3), 20)
    );
    for (old, new) in [
        ("atr_multiple = \"3\"", "atr_multiple = \"0\""),
        ("lookback = 20", "lookback = 0"),
        ("ema_period = 20", "ema_periodo = 20"),
    ] {
        assert!(EmotionConfig::from_toml(&CONFIG.replace(old, new)).is_err());
    }
}

#[test]
fn fomo_and_selling_climax_by_hand() {
    // t4: EMA = 105, ATR = (2 + 11) / 2 = 6,5 → (110 − 105) / 6,5 ≈ 0,77.
    let up = price_emotions(&h1(&[100, 100, 100, 100, 110]), &small(dec!(0.5)));
    assert_eq!(up, [None, None, None, None, Some(PriceEmotion::Fomo)]);
    let down = price_emotions(&h1(&[100, 100, 100, 100, 90]), &small(dec!(0.5)));
    assert_eq!(down[4], Some(PriceEmotion::SellingClimax));
    // Con 1 ATR de umbral no alcanza.
    assert_eq!(
        price_emotions(&h1(&[100, 100, 100, 100, 110]), &small(dec!(1)))[4],
        None
    );
}

#[test]
fn it_must_be_the_highest_close_of_the_window() {
    // t3: 112 es el máximo → FOMO. t4: 110 sigue estirado, pero 112 está en la ventana.
    let e = price_emotions(&h1(&[100, 100, 100, 112, 110]), &small(dec!(0.1)));
    assert_eq!((e[3], e[4]), (Some(PriceEmotion::Fomo), None));
    // Un empate con un cierre anterior cuenta como el más alto.
    let tie = price_emotions(&h1(&[100, 100, 100, 112, 112]), &small(dec!(0.01)));
    assert_eq!(tie[4], Some(PriceEmotion::Fomo));
}

#[test]
fn warms_up_and_needs_a_positive_atr() {
    // Sin rango ni movimiento el ATR es 0: no hay estado.
    let flat: Vec<Candle> = h1(&[100; 6])
        .into_iter()
        .map(|mut c| {
            c.high = c.close;
            c.low = c.close;
            c
        })
        .collect();
    assert!(
        price_emotions(&flat, &small(dec!(0.1)))
            .iter()
            .all(Option::is_none)
    );
    // Con menos de `lookback` velas, nada.
    let cfg = EmotionConfig {
        lookback: 10,
        ..small(dec!(0.1))
    };
    assert!(
        price_emotions(&h1(&[100, 100, 100, 120]), &cfg)
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn never_repaints() {
    let closes: Vec<i64> = (0..80)
        .map(|i| 100 + (i * 37 % 23) - (i % 7) * 2 + i / 3)
        .collect();
    let candles = h1(&closes);
    let cfg = small(dec!(0.2));
    let full = price_emotions(&candles, &cfg);
    assert!(full.iter().any(Option::is_some));
    for t in 0..candles.len() {
        assert_eq!(price_emotions(&candles[..=t], &cfg)[t], full[t]);
    }
}

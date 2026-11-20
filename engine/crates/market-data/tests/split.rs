//! Separación de datos sin fugas (Issue #16), con la configuración real de `config/`.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{ExchangeCalendar, Period, SessionCalendar, SplitConfig, SplitError, plan};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::dec;

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn cfg() -> SplitConfig {
    SplitConfig::from_toml(include_str!("../../../../config/data_split.toml")).unwrap()
}

fn candle(start: DateTime<Utc>) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start,
        open: dec!(1),
        high: dec!(1),
        low: dec!(1),
        close: dec!(1),
        volume: None,
    }
}

#[test]
fn plans_the_expected_windows_and_out_of_sample_period() {
    // Datos desde el lunes 4/1/2021 hasta hoy (20/11/2026).
    let split = plan(
        utc("2021-01-04T14:30:00Z"),
        utc("2026-11-20T20:45:00Z"),
        &cfg(),
    )
    .unwrap();

    // Fuera de muestra: los 12 meses completos antes del mes de la última vela, más ese mes.
    assert_eq!(
        split.out_of_sample,
        Period {
            start: utc("2025-11-01T00:00:00Z"),
            end: utc("2026-12-01T00:00:00Z")
        }
    );
    // Primera ventana: arranca en el primer mes completo (febrero 2021): 24 meses + 3.
    let w = split.windows[0];
    assert_eq!(
        (w.train.start, w.train.end),
        (utc("2021-02-01T00:00:00Z"), utc("2023-02-01T00:00:00Z"))
    );
    assert_eq!(
        (w.validation.start, w.validation.end),
        (utc("2023-02-01T00:00:00Z"), utc("2023-05-01T00:00:00Z"))
    );
    // Avanza de a 3 meses mientras la validación termine antes del fuera de muestra:
    // inicios feb-2021 … ago-2023 → 11 ventanas; la última valida hasta noviembre de 2025.
    assert_eq!(split.windows.len(), 11);
    assert_eq!(
        split.windows.last().unwrap().validation.end,
        split.out_of_sample.start
    );
}

#[test]
fn every_window_validates_strictly_after_it_trains_and_before_out_of_sample() {
    let split = plan(
        utc("2021-01-04T14:30:00Z"),
        utc("2026-11-20T20:45:00Z"),
        &cfg(),
    )
    .unwrap();
    for w in &split.windows {
        assert!(w.train.start < w.train.end);
        assert_eq!(
            w.train.end, w.validation.start,
            "validación pegada al entrenamiento, sin solaparse"
        );
        assert!(
            w.validation.end <= split.out_of_sample.start,
            "ninguna ventana toca el fuera de muestra"
        );
    }
}

#[test]
fn no_real_candle_lands_in_two_sets() {
    // Todas las velas de 15m reales de Nueva York, de 2021 a hoy, según el calendario del #9.
    let config = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let us100: InstrumentId = "US100".parse().unwrap();
    let cal =
        ExchangeCalendar::load(&config, HashMap::from([(us100.clone(), "XNYS".into())])).unwrap();
    let starts = cal
        .expected_starts(
            &us100,
            Timeframe::M15,
            utc("2021-01-04T00:00:00Z"),
            utc("2026-11-21T00:00:00Z"),
        )
        .unwrap();
    let candles: Vec<Candle> = starts.into_iter().map(candle).collect();
    assert!(candles.len() > 30_000);

    let split = plan(candles[0].start, candles.last().unwrap().start, &cfg()).unwrap();
    let research = split.research(&candles);
    let oos = split.out_of_sample.select(&candles);

    // Investigación + fuera de muestra = todas las velas, sin repetir ninguna.
    assert_eq!(research.len() + oos.len(), candles.len());
    assert!(
        research
            .iter()
            .all(|c| c.end() <= split.out_of_sample.start)
    );
    assert!(oos.iter().all(|c| c.start >= split.out_of_sample.start));

    for w in &split.windows {
        let train = w.train.select(&candles);
        let validation = w.validation.select(&candles);
        assert!(!train.is_empty() && !validation.is_empty());
        let last_train = train.last().unwrap().end();
        let first_val = validation.first().unwrap().start;
        assert!(
            last_train <= first_val,
            "el entrenamiento termina antes de que empiece la validación"
        );
        assert!(validation.iter().all(|c| !w.train.contains(c)));
    }
}

#[test]
fn a_candle_belongs_to_a_period_only_if_it_closes_inside_it() {
    let jan = Period {
        start: utc("2025-01-01T00:00:00Z"),
        end: utc("2025-02-01T00:00:00Z"),
    };
    let feb = Period {
        start: utc("2025-02-01T00:00:00Z"),
        end: utc("2025-03-01T00:00:00Z"),
    };
    let last_of_jan = candle(utc("2025-01-31T23:45:00Z")); // cierra 00:00 del 1/2: es de enero
    let first_of_feb = candle(utc("2025-02-01T00:00:00Z"));
    let straddling = candle(utc("2025-01-31T23:50:00Z")); // cerraría 00:05 del 1/2: de ninguno
    assert!(jan.contains(&last_of_jan) && !feb.contains(&last_of_jan));
    assert!(!jan.contains(&first_of_feb) && feb.contains(&first_of_feb));
    assert!(!jan.contains(&straddling) && !feb.contains(&straddling));
}

#[test]
fn out_of_sample_is_never_handed_to_research() {
    let split = plan(
        utc("2021-01-04T14:30:00Z"),
        utc("2026-11-20T20:45:00Z"),
        &cfg(),
    )
    .unwrap();
    let before = candle(split.out_of_sample.start - TimeDelta::minutes(15));
    let inside = candle(split.out_of_sample.start);
    let both = [before.clone(), inside];
    assert_eq!(split.research(&both), vec![&before]);
}

#[test]
fn rejects_too_little_data_and_invalid_config() {
    // 2 años de datos no alcanzan para 12 meses fuera de muestra + una ventana de 27 meses.
    let err = plan(
        utc("2024-11-04T14:30:00Z"),
        utc("2026-11-20T20:45:00Z"),
        &cfg(),
    );
    assert_eq!(err, Err(SplitError::NotEnoughData { needed: 27 }));

    let zero = include_str!("../../../../config/data_split.toml")
        .replace("step_months = 3", "step_months = 0");
    assert!(SplitConfig::from_toml(&zero).is_err());
    let typo =
        include_str!("../../../../config/data_split.toml").replace("train_months", "train_meses");
    assert!(SplitConfig::from_toml(&typo).is_err());
}

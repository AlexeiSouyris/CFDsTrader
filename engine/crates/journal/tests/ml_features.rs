//! Variables de ML de una propuesta (`contracts/ml_features.v1`, docs/spec/ml-v1.md §3 y §8,
//! #158): contrato, tercio de volatilidad (fixture compartido con Python), franja de sesión, y
//! que no miren el futuro ni repinten.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_backtest::TradeRecord;
use cfds_core::{Candle, DailyIndicator, InstrumentId, Timeframe};
use cfds_journal::ml_features::{
    FeaturesConfig, MlFeatures, SessionBand, VolTercile, atr14_m15, from_backtest, session_band,
    vol_tercile,
};
use cfds_market_data::{ExchangeCalendar, Session};
use cfds_strategy::{Side, TriggerKind};
use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc, Weekday};
use rust_decimal::{Decimal, dec};
use serde_json::{Value, json};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn us500() -> InstrumentId {
    "US500".parse().unwrap()
}

fn nyse() -> Session {
    let cal = ExchangeCalendar::load(
        &root().join("config"),
        HashMap::from([(us500(), "XNYS".to_string())]),
    )
    .unwrap();
    cal.session_for(&us500()).unwrap().clone()
}

fn daily(day: NaiveDate, hour: u32, vol: Option<Decimal>) -> DailyIndicator {
    DailyIndicator {
        instrument_id: us500(),
        day,
        published_at: day.and_hms_opt(hour, 0, 0).unwrap().and_utc(),
        atr_period: 14,
        atr: None,
        realized_vol_window: 20,
        realized_vol: vol,
    }
}

#[test]
fn features_validate_against_their_contract_and_read_back_identical() {
    let path = root().join("contracts/ml_features.v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let f = MlFeatures {
        proposal_id: "bt-1:US500:2025-11-19T15:00:00Z:long".into(),
        as_of: at("2025-11-19T15:00:00Z"),
        atr14_m15: dec!(4.25),
        realized_vol_tercile: VolTercile::Mid,
        session_band: SessionBand::Open,
    };
    let j = serde_json::to_value(&f).unwrap();
    assert!(validator.is_valid(&j), "{j}");
    assert_eq!(j["atr14_m15"], json!("4.25"));
    assert_eq!(j["realized_vol_tercile"], json!("mid"));
    assert_eq!(serde_json::from_value::<MlFeatures>(j).unwrap(), f);
}

#[test]
fn the_vol_tercile_matches_the_fixture_shared_with_python() {
    let path = root().join("intelligence/tests/fixtures/vol_tercile.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let hour = u32::try_from(doc["published_hour_utc"].as_u64().unwrap()).unwrap();
    let mut day: NaiveDate = doc["start"].as_str().unwrap().parse().unwrap();
    let mut series = Vec::new();
    for v in doc["realized_vol"].as_array().unwrap() {
        while matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
            day = day.succ_opt().unwrap();
        }
        series.push(daily(day, hour, v.as_str().map(|s| s.parse().unwrap())));
        day = day.succ_opt().unwrap();
    }
    let years = u32::try_from(doc["years"].as_u64().unwrap()).unwrap();
    let queries = doc["queries"].as_array().unwrap();
    assert!(queries.len() > 30);
    for q in queries {
        let got = vol_tercile(&series, at(q["as_of"].as_str().unwrap()), years);
        assert_eq!(serde_json::to_value(got).unwrap(), q["expected"], "{q}");
    }
}

#[test]
fn a_daily_value_counts_only_from_its_publication() {
    let d = |s: &str| s.parse::<NaiveDate>().unwrap();
    let mut series = vec![
        daily(d("2024-01-02"), 21, Some(dec!(0.01))),
        daily(d("2024-01-03"), 21, Some(dec!(0.02))),
        daily(d("2024-01-04"), 21, Some(dec!(0.03))),
    ];
    series.push(daily(d("2025-01-03"), 21, Some(dec!(0.05))));
    // Antes de publicarse el del 03-01-2025 rige el del 04-01-2024, sin un año de historia.
    assert_eq!(vol_tercile(&series, at("2025-01-03T20:59:00Z"), 1), None);
    // Publicado: la historia del año anterior (03-01-2024 y 04-01-2024) es toda menor.
    assert_eq!(
        vol_tercile(&series, at("2025-01-03T21:00:00Z"), 1),
        Some(VolTercile::High)
    );
}

#[test]
fn the_session_band_splits_the_session_in_thirds() {
    let s = nyse();
    // 19-11-2025, Nueva York 09:30–16:00 EST = 14:30–21:00 UTC (6 h 30 min, tercios de 130 min).
    let band = |t: &str| session_band(&s, at(t));
    assert_eq!(band("2025-11-19T14:30:00Z"), None); // la apertura misma: ninguna vela cerró
    assert_eq!(band("2025-11-19T14:45:00Z"), Some(SessionBand::Open));
    assert_eq!(band("2025-11-19T16:40:00Z"), Some(SessionBand::Open)); // 130 min: borde
    assert_eq!(band("2025-11-19T16:45:00Z"), Some(SessionBand::Mid));
    assert_eq!(band("2025-11-19T18:50:00Z"), Some(SessionBand::Mid)); // 260 min: borde
    assert_eq!(band("2025-11-19T19:00:00Z"), Some(SessionBand::Close));
    assert_eq!(band("2025-11-19T21:00:00Z"), Some(SessionBand::Close));
    assert_eq!(band("2025-11-19T21:15:00Z"), None); // después del cierre
    assert_eq!(band("2025-11-22T15:00:00Z"), None); // sábado
    assert_eq!(band("2025-11-27T15:00:00Z"), None); // Acción de Gracias
    // Cierre temprano (13:00 EST): la sesión dura 3 h 30 min y el último tercio empieza antes.
    assert_eq!(band("2025-11-28T17:00:00Z"), Some(SessionBand::Close));
    assert_eq!(band("2025-11-28T18:15:00Z"), None);
    // Fuera de lo que cubre el calendario no se supone nada.
    assert_eq!(band("2035-11-19T15:00:00Z"), None);
}

/// Velas de 15m sintéticas de US500, todas seguidas, desde `start`.
fn candles(start: &str, n: usize, shift: Decimal) -> Vec<Candle> {
    (0..n)
        .map(|i| {
            let base = Decimal::from(5000 + (i * 37) % 23) + shift;
            Candle {
                instrument_id: us500(),
                timeframe: Timeframe::M15,
                start: at(start) + TimeDelta::minutes(15 * i64::try_from(i).unwrap()),
                open: base,
                high: base + Decimal::from(3 + i % 5),
                low: base - Decimal::from(2 + i % 3),
                close: base + Decimal::ONE,
                volume: None,
            }
        })
        .collect()
}

fn trade(m15: &[Candle], t: usize) -> TradeRecord {
    TradeRecord {
        side: Side::Long,
        signal_index: t,
        signal_at: m15[t].end(),
        score: 6,
        breakdown: vec![],
        triggers: vec![TriggerKind::Sweep],
        reference: m15[t].close,
        tp1: m15[t].close + dec!(10),
        entry_index: t + 1,
        entry_time: m15[t].end(),
        entry_mid: m15[t].close,
        entry_price: m15[t].close,
        stop: m15[t].close - dec!(5),
        size: Decimal::ONE,
        leverage: Decimal::ONE,
        margin: Decimal::ONE,
        exits: vec![],
        commission: Decimal::ZERO,
        funding: Decimal::ZERO,
        mid_pnl: Decimal::ZERO,
        gross_pnl: Decimal::ZERO,
        net_pnl: Decimal::ZERO,
    }
}

/// Dos años de diarias con volatilidad creciente: el último día queda en el tercio alto.
fn history() -> Vec<DailyIndicator> {
    let mut day: NaiveDate = "2023-11-01".parse().unwrap();
    let mut out = Vec::new();
    let mut i = 0;
    while day < "2025-11-19".parse().unwrap() {
        if !matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
            out.push(daily(day, 21, Some(Decimal::new(100 + i, 4))));
            i += 1;
        }
        day = day.succ_opt().unwrap();
    }
    out
}

#[test]
fn features_use_nothing_after_the_signal_and_never_repaint() {
    let s = nyse();
    let cfg = FeaturesConfig {
        vol_history_years: 1,
    };
    // La vela 20 cierra a las 17:15 UTC: 165 min después de la apertura (tercio medio).
    let m15 = candles("2025-11-19T12:00:00Z", 40, Decimal::ZERO);
    let t = 20;
    let tr = trade(&m15, t);
    let feats = |m15: &[Candle], daily: &[DailyIndicator]| {
        from_backtest("bt-ml", &us500(), &tr, &atr14_m15(m15), daily, &s, &cfg)
    };
    let daily = history();
    let base = feats(&m15, &daily).expect("todas las variables");
    assert_eq!(base.as_of, at("2025-11-19T17:15:00Z"));
    assert_eq!(base.proposal_id, "bt-ml:US500:2025-11-19T17:15:00Z:long");
    assert_eq!(base.session_band, SessionBand::Mid);
    assert_eq!(base.realized_vol_tercile, VolTercile::High);
    assert!(base.atr14_m15 > Decimal::ZERO);

    // §8.1: cambiar todas las velas posteriores a t no cambia nada.
    let mut changed = m15.clone();
    for (c, other) in changed[t + 1..]
        .iter_mut()
        .zip(&candles("2025-11-19T12:00:00Z", 40, dec!(900))[t + 1..])
    {
        *c = other.clone();
    }
    assert_eq!(feats(&changed, &daily), Some(base.clone()));
    // Un diario publicado después de as_of (el del mismo día) tampoco.
    let mut later = daily.clone();
    later.push(daily_after());
    assert_eq!(feats(&m15, &later), Some(base.clone()));

    // §8.2: llegan velas nuevas y la propuesta ya armada no cambia.
    let longer = candles("2025-11-19T12:00:00Z", 80, Decimal::ZERO);
    assert_eq!(longer[..40], m15[..]);
    assert_eq!(feats(&longer, &later), Some(base));
}

fn daily_after() -> DailyIndicator {
    daily("2025-11-19".parse().unwrap(), 21, Some(dec!(0.0001)))
}

#[test]
fn a_missing_feature_means_no_row() {
    let s = nyse();
    let cfg = FeaturesConfig {
        vol_history_years: 1,
    };
    let m15 = candles("2025-11-19T12:00:00Z", 40, Decimal::ZERO);
    // Calentamiento del ATR (menos de 14 velas antes).
    let early = trade(&m15, 5);
    assert_eq!(
        from_backtest(
            "bt",
            &us500(),
            &early,
            &atr14_m15(&m15),
            &history(),
            &s,
            &cfg
        ),
        None
    );
    // Sin historia diaria suficiente.
    let tr = trade(&m15, 20);
    let short: Vec<_> = history().split_off(400);
    assert_eq!(
        from_backtest("bt", &us500(), &tr, &atr14_m15(&m15), &short, &s, &cfg),
        None
    );
    // Fuera de la sesión: la vela 20 cierra a las 13:15 UTC, antes de la apertura.
    let before_open = candles("2025-11-19T08:00:00Z", 40, Decimal::ZERO);
    let tr = trade(&before_open, 20);
    assert_eq!(
        from_backtest(
            "bt",
            &us500(),
            &tr,
            &atr14_m15(&before_open),
            &history(),
            &s,
            &cfg
        ),
        None
    );
}

#[test]
fn the_config_reads_its_section_of_ml_toml() {
    let text = std::fs::read_to_string(root().join("config/ml.toml")).unwrap();
    let cfg = FeaturesConfig::from_toml(&text).unwrap();
    assert_eq!(cfg.vol_history_years, 3);
    assert!(FeaturesConfig::from_toml("[features]\nvol_history_years = 0").is_err());
    assert!(FeaturesConfig::from_toml("[features]\nvol_history_years = 3\nx = 1").is_err());
}

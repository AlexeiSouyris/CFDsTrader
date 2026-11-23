//! Variables de ML de una propuesta (`contracts/ml_features.v1`, docs/spec/ml-v1.md §3 y §8,
//! #158): contrato, tercio de volatilidad guardado (solo publicado), franja de sesión, y que no
//! miren el futuro ni repinten.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_backtest::TradeRecord;
use cfds_core::{Candle, DailyVolTercile, InstrumentId, Timeframe};
use cfds_indicators::vol_terciles;
use cfds_journal::ml_features::{
    MlFeatures, SessionBand, VolTercile, atr14_m15, from_backtest, session_band, vol_tercile,
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

fn daily(day: NaiveDate, tercile: Option<VolTercile>) -> DailyVolTercile {
    DailyVolTercile {
        instrument_id: us500(),
        day,
        published_at: day.and_hms_opt(21, 0, 0).unwrap().and_utc(),
        realized_vol_window: 20,
        history_years: 1,
        tercile,
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
fn a_daily_tercile_counts_only_from_its_publication() {
    let d = |s: &str| s.parse::<NaiveDate>().unwrap();
    let series = vec![
        daily(d("2025-01-02"), Some(VolTercile::Low)),
        daily(d("2025-01-03"), Some(VolTercile::High)),
        daily(d("2025-01-06"), None),
    ];
    assert_eq!(vol_tercile(&series, at("2025-01-02T20:59:00Z")), None);
    assert_eq!(
        vol_tercile(&series, at("2025-01-03T20:59:00Z")),
        Some(VolTercile::Low)
    );
    assert_eq!(
        vol_tercile(&series, at("2025-01-03T21:00:00Z")),
        Some(VolTercile::High)
    );
    // El fin de semana rige el del viernes; el lunes sin tercio (calentamiento) no tiene dato.
    assert_eq!(
        vol_tercile(&series, at("2025-01-05T12:00:00Z")),
        Some(VolTercile::High)
    );
    assert_eq!(vol_tercile(&series, at("2025-01-06T21:00:00Z")), None);
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

/// Dos años de tercios (de un año de historia) con volatilidad creciente, como los guarda
/// `cfds-ingest`: el último día queda en el tercio alto y el primer año, sin dato.
fn history() -> Vec<DailyVolTercile> {
    let mut day: NaiveDate = "2023-11-01".parse().unwrap();
    let mut series = Vec::new();
    let mut i = 0;
    while day < "2025-11-19".parse().unwrap() {
        if !matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
            series.push((day, Some(Decimal::new(100 + i, 4))));
            i += 1;
        }
        day = day.succ_opt().unwrap();
    }
    let terciles = vol_terciles(&series, 1);
    series
        .iter()
        .zip(terciles)
        .map(|((d, _), t)| daily(*d, t))
        .collect()
}

#[test]
fn features_use_nothing_after_the_signal_and_never_repaint() {
    let s = nyse();
    // La vela 20 cierra a las 17:15 UTC: 165 min después de la apertura (tercio medio).
    let m15 = candles("2025-11-19T12:00:00Z", 40, Decimal::ZERO);
    let t = 20;
    let tr = trade(&m15, t);
    let feats = |m15: &[Candle], daily: &[DailyVolTercile]| {
        from_backtest("bt-ml", &us500(), &tr, &atr14_m15(m15), daily, &s)
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

fn daily_after() -> DailyVolTercile {
    daily("2025-11-19".parse().unwrap(), Some(VolTercile::Low))
}

#[test]
fn a_missing_feature_means_no_row() {
    let s = nyse();
    let m15 = candles("2025-11-19T12:00:00Z", 40, Decimal::ZERO);
    // Calentamiento del ATR (menos de 14 velas antes).
    let early = trade(&m15, 5);
    assert_eq!(
        from_backtest("bt", &us500(), &early, &atr14_m15(&m15), &history(), &s),
        None
    );
    // Sin tercio publicado ese día (historia diaria insuficiente).
    let tr = trade(&m15, 20);
    let short: Vec<_> = history()
        .into_iter()
        .map(|d| DailyVolTercile { tercile: None, ..d })
        .collect();
    assert_eq!(
        from_backtest("bt", &us500(), &tr, &atr14_m15(&m15), &short, &s),
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
            &s
        ),
        None
    );
}

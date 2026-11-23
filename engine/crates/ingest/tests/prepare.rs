//! `prepare`: lo que pasa antes de tocar la base, sin red ni servicios.

use std::collections::HashMap;
use std::path::PathBuf;

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_ingest::{IngestError, IngestRequest, prepare};
use cfds_market_data::{ExchangeCalendar, IssueKind, ValidationConfig};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn config_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config")
}

fn calendar() -> ExchangeCalendar {
    let map = HashMap::from([("US100".parse().unwrap(), "XNYS".to_string())]);
    ExchangeCalendar::load(&config_dir(), map).unwrap()
}

fn cfg() -> ValidationConfig {
    ValidationConfig::from_toml(include_str!("../../../../config/data_validation.toml")).unwrap()
}

fn open_time() -> DateTime<Utc> {
    "2026-11-25T14:30:00Z".parse().unwrap() // apertura de Nueva York ese día
}

fn us100() -> InstrumentId {
    "US100".parse().unwrap()
}

/// `n` velas de 15m contiguas desde la apertura, sanas.
fn series(n: usize) -> Vec<Candle> {
    (0..n)
        .map(|i| {
            let base = dec!(26700) + Decimal::from(i);
            Candle {
                instrument_id: us100(),
                timeframe: Timeframe::M15,
                start: open_time() + TimeDelta::minutes(15 * i as i64),
                open: base,
                high: base + dec!(6),
                low: base - dec!(4),
                close: base + dec!(1),
                volume: None,
            }
        })
        .collect()
}

fn req() -> IngestRequest {
    IngestRequest {
        instrument: us100(),
        timeframe: Timeframe::M15,
        from: open_time(),
        to: open_time() + TimeDelta::days(1),
    }
}

#[test]
fn keeps_closed_candles_and_drops_the_one_still_open() {
    let candles = series(5);
    let now = candles[4].start + TimeDelta::minutes(5); // la quinta vela está en curso
    let (closed, skipped) = prepare(&req(), candles.clone(), &calendar(), &cfg(), now).unwrap();
    assert_eq!(closed, candles[..4].to_vec());
    assert_eq!(skipped, 1);
}

#[test]
fn an_open_candle_followed_by_closed_ones_is_a_broken_response() {
    let c = series(3);
    let disordered = vec![c[0].clone(), c[2].clone(), c[1].clone()];
    let now = c[1].end() + TimeDelta::minutes(1); // c[2] sigue abierta, c[1] ya cerró
    let err = prepare(&req(), disordered, &calendar(), &cfg(), now).unwrap_err();
    assert!(
        matches!(err, IngestError::OpenCandleNotLast { .. }),
        "{err}"
    );
}

#[test]
fn rejects_candles_that_were_not_requested() {
    let mut other = series(2);
    other[1].instrument_id = "DE40".parse().unwrap();
    let now = other[1].end();
    let err = prepare(&req(), other, &calendar(), &cfg(), now).unwrap_err();
    assert!(matches!(err, IngestError::Unexpected { .. }), "{err}");

    let mut outside = series(2);
    outside[0].start = req().from - TimeDelta::minutes(15);
    let err = prepare(&req(), outside, &calendar(), &cfg(), now).unwrap_err();
    assert!(matches!(err, IngestError::Unexpected { .. }), "{err}");
}

#[test]
fn any_data_problem_rejects_the_whole_batch() {
    let mut bad = series(6);
    bad[3].high = bad[3].low - dec!(1);
    let now = bad[5].end();
    match prepare(&req(), bad, &calendar(), &cfg(), now) {
        Err(IngestError::Rejected(issues)) => {
            assert!(matches!(issues[0].kind, IssueKind::InconsistentOhlc { .. }));
        }
        other => panic!("debería rechazar el lote: {other:?}"),
    }
}

#[test]
fn a_missing_candle_inside_the_session_rejects_the_batch() {
    let mut gap = series(6);
    gap.remove(2);
    let now = gap.last().unwrap().end();
    match prepare(&req(), gap, &calendar(), &cfg(), now) {
        Err(IngestError::Rejected(issues)) => {
            assert!(matches!(
                issues[0].kind,
                IssueKind::MissingCandles { count: 1, .. }
            ));
        }
        other => panic!("debería rechazar el lote: {other:?}"),
    }
}

#[test]
fn a_big_jump_does_not_reject_the_batch_it_is_decided_later() {
    // #271: 20 velas sanas (ATR ≈ 10) y un salto de 1.000 puntos que se sostiene. Se guarda; si
    // fue un pico o un movimiento real lo decide `check_series` al usar la serie.
    let mut up = series(24);
    for c in &mut up[20..] {
        c.open += dec!(1000);
        c.high += dec!(1000);
        c.low += dec!(1000);
        c.close += dec!(1000);
    }
    let now = up.last().unwrap().end();
    let (closed, _) = prepare(&req(), up.clone(), &calendar(), &cfg(), now).unwrap();
    assert_eq!(closed, up);
}

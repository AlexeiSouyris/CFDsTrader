//! Contraste de SMA, EMA y ATR contra referencias independientes (#666, plan B2 de
//! `docs/analysis/2026-11-autoanalisis.md`).
//!
//! `fixtures/reference_indicators.csv` tiene 300 velas **sintéticas** y los valores de dos
//! referencias, calculados fuera del motor con `tools/verify/make_indicator_fixture.py`:
//! - `pine_*`: las fórmulas publicadas de TradingView (`ta.sma`, `ta.ema`, `ta.atr`) en
//!   aritmética exacta, redondeadas a 20 decimales. Tienen que coincidir desde la primera vela.
//! - `talib_*`: TA-Lib 0.8.3 en `float`. SMA y EMA, desde la primera vela; el ATR de TA-Lib
//!   arranca una vela después, así que se compara cuando la diferencia inicial ya se borró.
//!
//! Los golden tests contra una exportación real de TradingView se suman siguiendo
//! `docs/guides/golden-tradingview.md`.

use std::str::FromStr;

use cfds_core::{Candle, Timeframe};
use cfds_indicators::{ema, sma, wilder_atr};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;

const FIXTURE: &str = include_str!("fixtures/reference_indicators.csv");

struct Row {
    candle: Candle,
    cols: Vec<Option<Decimal>>,
}

fn load() -> (Vec<String>, Vec<Row>) {
    let mut lines = FIXTURE.lines();
    let header: Vec<String> = lines
        .next()
        .unwrap()
        .split(',')
        .map(str::to_owned)
        .collect();
    let t0: DateTime<Utc> = "2026-01-05T00:00:00Z".parse().unwrap();
    let rows = lines
        .enumerate()
        .map(|(i, line)| {
            let cols: Vec<Option<Decimal>> = line
                .split(',')
                .map(|v| (!v.is_empty()).then(|| Decimal::from_str(v).unwrap()))
                .collect();
            let (high, low, close) = (cols[1].unwrap(), cols[2].unwrap(), cols[3].unwrap());
            Row {
                candle: Candle {
                    instrument_id: "US500".parse().unwrap(),
                    timeframe: Timeframe::H1,
                    start: t0 + TimeDelta::hours(i as i64),
                    open: close,
                    high,
                    low,
                    close,
                    volume: None,
                },
                cols,
            }
        })
        .collect();
    (header, rows)
}

fn column(header: &[String], rows: &[Row], name: &str) -> Vec<Option<Decimal>> {
    let k = header.iter().position(|h| h == name).unwrap();
    rows.iter().map(|r| r.cols[k]).collect()
}

/// Compara desde la vela `from`: los dos tienen valor (o los dos no) y la diferencia relativa
/// no pasa de `tol`.
fn assert_close(
    name: &str,
    mine: &[Option<Decimal>],
    reference: &[Option<Decimal>],
    from: usize,
    tol: Decimal,
) {
    assert_eq!(mine.len(), reference.len());
    for i in from..mine.len() {
        match (mine[i], reference[i]) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                let rel = (a - b).abs() / b.abs();
                assert!(
                    rel <= tol,
                    "{name} vela {i}: motor {a} referencia {b} (relativa {rel})"
                );
            }
            (a, b) => {
                panic!("{name} vela {i}: calentamiento distinto (motor {a:?}, referencia {b:?})")
            }
        }
    }
}

#[test]
fn matches_the_published_tradingview_formulas_from_the_first_candle() {
    let (header, rows) = load();
    let candles: Vec<Candle> = rows.iter().map(|r| r.candle.clone()).collect();
    let tol = Decimal::new(1, 18);
    assert_close(
        "SMA(10)",
        &sma(&candles, 10),
        &column(&header, &rows, "pine_sma10"),
        0,
        tol,
    );
    assert_close(
        "EMA(10)",
        &ema(&candles, 10),
        &column(&header, &rows, "pine_ema10"),
        0,
        tol,
    );
    assert_close(
        "ATR(14)",
        &wilder_atr(&candles, 14),
        &column(&header, &rows, "pine_atr14"),
        0,
        tol,
    );
}

#[test]
fn matches_talib() {
    let (header, rows) = load();
    let candles: Vec<Candle> = rows.iter().map(|r| r.candle.clone()).collect();
    // TA-Lib calcula en `float` (unos 15 dígitos): la tolerancia es la de un `f64`.
    let tol = Decimal::new(1, 12);
    assert_close(
        "SMA(10)",
        &sma(&candles, 10),
        &column(&header, &rows, "talib_sma10"),
        0,
        tol,
    );
    assert_close(
        "EMA(10)",
        &ema(&candles, 10),
        &column(&header, &rows, "talib_ema10"),
        0,
        tol,
    );

    // ATR: TA-Lib da su primer valor en la vela 14 (el motor y TradingView, en la 13). Con el
    // suavizado de Wilder la diferencia inicial se multiplica por 13/14 en cada vela: desde la
    // vela 250 ya es menor que 1e-8 en términos relativos.
    let atr = wilder_atr(&candles, 14);
    let ref_atr = column(&header, &rows, "talib_atr14");
    assert!(atr[13].is_some() && ref_atr[13].is_none() && ref_atr[14].is_some());
    assert_close("ATR(14)", &atr, &ref_atr, 250, Decimal::new(1, 8));
}

/// Golden test contra una exportación real de TradingView (`docs/guides/golden-tradingview.md`).
/// La exportación tiene precios de un proveedor: **no se commitea** (el repo es público), así que
/// este test lee la ruta de `CFDS_TV_GOLDEN` y solo corre a mano:
///
/// ```text
/// CFDS_TV_GOLDEN=~/cfds-golden/us500_1h.csv cargo test -p cfds-indicators --test reference -- --ignored
/// ```
///
/// Usa las columnas `open`, `high`, `low` y `close` de la exportación y compara las que estén de
/// `sma50`, `ema10`, `ema365` y `atr14` (títulos de los `plot` del script de la guía). SMA,
/// desde su primer valor; EMA y ATR, desde la vela 400, porque TradingView las siembra en la
/// primera vela de su historia y la exportación puede empezar después.
#[test]
#[ignore = "necesita una exportación local de TradingView (ver docs/guides/golden-tradingview.md)"]
fn matches_a_local_tradingview_export() {
    let Ok(path) = std::env::var("CFDS_TV_GOLDEN") else {
        panic!("falta CFDS_TV_GOLDEN con la ruta del CSV exportado de TradingView");
    };
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines = text.lines();
    let header: Vec<String> = lines
        .next()
        .unwrap()
        .split(',')
        .map(|h| h.trim().trim_matches('"').to_owned())
        .collect();
    let parse = |v: &str| -> Option<Decimal> {
        let v = v.trim().trim_matches('"');
        Decimal::from_str(v)
            .or_else(|_| Decimal::from_scientific(v))
            .ok()
    };
    let table: Vec<Vec<Option<Decimal>>> =
        lines.map(|l| l.split(',').map(parse).collect()).collect();
    let col = |name: &str| -> Option<Vec<Option<Decimal>>> {
        let k = header.iter().position(|h| h == name)?;
        Some(table.iter().map(|r| r.get(k).copied().flatten()).collect())
    };
    let (high, low, close) = (
        col("high").unwrap(),
        col("low").unwrap(),
        col("close").unwrap(),
    );
    let t0: DateTime<Utc> = "2000-01-01T00:00:00Z".parse().unwrap();
    let candles: Vec<Candle> = (0..table.len())
        .map(|i| Candle {
            instrument_id: "US500".parse().unwrap(),
            timeframe: Timeframe::H1,
            start: t0 + TimeDelta::hours(i as i64),
            open: close[i].unwrap(),
            high: high[i].unwrap(),
            low: low[i].unwrap(),
            close: close[i].unwrap(),
            volume: None,
        })
        .collect();
    let tol = Decimal::new(1, 6);
    let mut compared = 0;
    let checks: [(&str, Vec<Option<Decimal>>, usize); 4] = [
        ("sma50", sma(&candles, 50), 0),
        ("ema10", ema(&candles, 10), 400),
        ("ema365", ema(&candles, 365), 400),
        ("atr14", wilder_atr(&candles, 14), 400),
    ];
    for (name, mine, from) in checks {
        let Some(reference) = col(name) else { continue };
        for i in from..candles.len() {
            if let (Some(a), Some(b)) = (mine[i], reference[i]) {
                let rel = (a - b).abs() / b.abs();
                assert!(rel <= tol, "{name} vela {i}: motor {a} TradingView {b}");
            }
        }
        compared += 1;
    }
    assert!(
        compared > 0,
        "la exportación no tiene ninguna columna sma50/ema10/ema365/atr14"
    );
}

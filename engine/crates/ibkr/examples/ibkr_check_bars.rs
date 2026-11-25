//! Diagnóstico de **solo lectura** de `ibkr_bars` (#563): lista las velas de 15m y 1h guardadas
//! cuyo OHLC no coincide con el agregado de las velas de 1m guardadas de la misma serie (mismo
//! contrato y precio), con la misma función que usa `ibkr_load` antes de guardar
//! (`cfds_ibkr::load::check_against_minutes`): una vela se revisa solo si hay velas de 1m dentro de
//! su período y las de 1m guardadas llegan después de su cierre. Compara los cuatro precios y marca
//! cuáles rechazaría hoy `ibkr_load` (`Compare::before_storing`: sin la apertura; en acciones, solo
//! el cierre). No escribe nada.
//!
//! ```text
//! cd engine && set -a && . ../.env && set +a
//! cargo run -q -p cfds-ibkr --example ibkr_check_bars            # resumen por serie y rangos
//! cargo run -q -p cfds-ibkr --example ibkr_check_bars -- --rows   # además, cada vela distinta
//! ```
//!
//! Salida: 0 todo coincide; 1 error; 5 hay velas que no coinciden.

use std::process::ExitCode;

use cfds_ibkr::load::{Coherence, Compare, Ohlc, check_against_minutes};
use cfds_ibkr::{BarSize, ContractKind};
use cfds_storage::{IbkrBarRow, IbkrSeries, PgStore};
use chrono::{DateTime, Utc};

fn ohlc(b: &IbkrBarRow) -> Ohlc {
    Ohlc {
        open: b.open,
        high: b.high,
        low: b.low,
        close: b.close,
    }
}

fn show(o: &Ohlc) -> String {
    format!("O {} H {} L {} C {}", o.open, o.high, o.low, o.close)
}

/// Rangos de velas consecutivas (separadas por exactamente `step`).
fn ranges(
    starts: &[DateTime<Utc>],
    step: chrono::TimeDelta,
) -> Vec<(DateTime<Utc>, DateTime<Utc>, usize)> {
    let mut out: Vec<(DateTime<Utc>, DateTime<Utc>, usize)> = Vec::new();
    for &s in starts {
        match out.last_mut() {
            Some((_, last, n)) if *last + step == s => {
                *last = s;
                *n += 1;
            }
            _ => out.push((s, s, 1)),
        }
    }
    out
}

#[tokio::main]
async fn main() -> ExitCode {
    let rows_flag = std::env::args().any(|a| a == "--rows");
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("falta DATABASE_URL");
        return ExitCode::from(1);
    };
    let store = match PgStore::connect(&url).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let all = match store.ibkr_series().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let far_past: DateTime<Utc> = DateTime::UNIX_EPOCH;
    let far_future: DateTime<Utc> = "2100-01-01T00:00:00Z".parse().expect("fecha fija");
    let mut total_bad = 0usize;
    for key in all.iter().filter(|k| k.bar_size != "1m") {
        let Ok(size) = key.bar_size.parse::<BarSize>() else {
            eprintln!("tamaño desconocido: {}", key.bar_size);
            return ExitCode::from(1);
        };
        let series = key.as_series();
        let minute_series = IbkrSeries {
            bar_size: "1m",
            ..series.clone()
        };
        let read = async {
            let bars = store
                .ibkr_bars_between(&series, far_past, far_future)
                .await?;
            let last_minute = store.last_ibkr_bar_start(&minute_series).await?;
            let minutes = match (bars.first(), bars.last()) {
                (Some(a), Some(b)) => {
                    store
                        .ibkr_bars_between(&minute_series, a.start, b.start + size.duration())
                        .await?
                }
                _ => Vec::new(),
            };
            Ok::<_, cfds_storage::StorageError>((bars, last_minute, minutes))
        };
        let (bars, last_minute, minutes) = match read.await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        };
        let minutes: Vec<(DateTime<Utc>, Ohlc)> =
            minutes.iter().map(|m| (m.start, ohlc(m))).collect();
        let (mut checked, mut bad) = (0usize, Vec::new());
        // Las 1m están ordenadas: cada vela mira solo su tramo.
        let mut lo = 0usize;
        for b in &bars {
            while lo < minutes.len() && minutes[lo].0 < b.start {
                lo += 1;
            }
            let end = b.start + size.duration();
            let hi = lo + minutes[lo..].partition_point(|(t, _)| *t < end);
            let kind = if b.kind == "stock" {
                ContractKind::Stock
            } else {
                ContractKind::Future
            };
            let check =
                |c| check_against_minutes(b.start, size, ohlc(b), &minutes[lo..hi], last_minute, c);
            match check(Compare::All) {
                Coherence::NotChecked => {}
                Coherence::Matches => checked += 1,
                Coherence::Differs { expected } => {
                    checked += 1;
                    let refused = check(Compare::before_storing(kind)) != Coherence::Matches;
                    bad.push((b, expected, refused));
                }
            }
        }
        println!(
            "{} {} ({}) {} {}: {} velas, {} revisadas, {} no coinciden ({} las rechazaría ibkr_load)",
            key.source,
            key.instrument_id,
            key.contract,
            key.bar_size,
            key.what_to_show,
            bars.len(),
            checked,
            bad.len(),
            bad.iter().filter(|(_, _, r)| *r).count()
        );
        let starts: Vec<DateTime<Utc>> = bad.iter().map(|(b, _, _)| b.start).collect();
        for (a, z, n) in ranges(&starts, size.duration()) {
            println!("  {} … {} ({n})", a.to_rfc3339(), z.to_rfc3339());
        }
        if rows_flag {
            for (b, expected, refused) in &bad {
                println!(
                    "    {} guardada {} | 1m {}{}",
                    b.start.to_rfc3339(),
                    show(&ohlc(b)),
                    show(expected),
                    if *refused {
                        " | ibkr_load la rechaza"
                    } else {
                        ""
                    }
                );
            }
        }
        total_bad += bad.len();
    }
    println!("total: {total_bad} velas no coinciden");
    if total_bad > 0 {
        ExitCode::from(5)
    } else {
        ExitCode::SUCCESS
    }
}

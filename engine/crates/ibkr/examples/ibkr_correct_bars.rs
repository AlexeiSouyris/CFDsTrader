//! Corrige las velas de 15m y 1h de `ibkr_bars` que se guardaron con datos a medias (#563, #575):
//! calcula, para cada una que no coincide con el agregado de sus velas de 1m guardadas (misma serie:
//! mismo contrato y precio), el OHLC correcto con ese agregado, y lo **agrega** a
//! `ibkr_bar_corrections`. Nunca edita ni borra `ibkr_bars` (es de solo agregar).
//!
//! Misma revisión que `ibkr_check_bars` y que `ibkr_load` antes de guardar
//! (`cfds_ibkr::load::check_against_minutes`, comparando los cuatro precios): una vela se revisa
//! solo si hay velas de 1m dentro de su período y las de 1m guardadas llegan después de su cierre.
//! Las lecturas ya devuelven la corrección más reciente, así que correrlo otra vez no agrega nada.
//!
//! Guardas: solo los instrumentos de `--instruments` (obligatorio) y nunca acciones ni ETF (en
//! QQQ las diferencias tienen otra causa, #563). Una vela cuyo volumen guardado es **mayor** que la
//! suma de sus velas de 1m no se corrige (las de 1m podrían estar incompletas): se informa.
//!
//! No aplica migraciones: la tabla tiene que existir (migración 0049).
//!
//! ```text
//! cd engine && set -a && . ../.env && set +a
//! cargo run -q -p cfds-ibkr --example ibkr_correct_bars -- --instruments MNQ,MES        # muestra
//! cargo run -q -p cfds-ibkr --example ibkr_correct_bars -- --instruments MNQ,MES --run  # guarda
//! ```
//!
//! Salida: 0 bien (sin `--run`, aunque haya velas para corregir); 1 error o argumentos.

use std::process::ExitCode;

use cfds_ibkr::BarSize;
use cfds_ibkr::load::{Coherence, Compare, Ohlc, check_against_minutes};
use cfds_storage::{IbkrBarCorrection, IbkrBarRow, IbkrSeries, PgStore};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

const REASON: &str = "#563: ibkr_load la guardó con datos a medias (atraso de los datos de IBKR); \
                      OHLC = agregado de las velas de 1m guardadas de la misma serie";
const CORRECTION_SOURCE: &str = "ibkr_1m_aggregate";

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

/// `--instruments A,B` (obligatorio) y `--run`.
fn args() -> Result<(Vec<String>, bool), String> {
    let mut instruments = Vec::new();
    let mut run = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--run" => run = true,
            "--instruments" => {
                let v = it
                    .next()
                    .ok_or("--instruments necesita una lista: MNQ,MES")?;
                instruments.extend(
                    v.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                );
            }
            other => return Err(format!("argumento desconocido: {other}")),
        }
    }
    if instruments.is_empty() {
        return Err("falta --instruments (por ejemplo MNQ,MES)".into());
    }
    Ok((instruments, run))
}

#[tokio::main]
async fn main() -> ExitCode {
    let (instruments, run) = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("falta DATABASE_URL");
        return ExitCode::from(1);
    };
    match correct(&url, &instruments, run).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

async fn correct(url: &str, instruments: &[String], run: bool) -> Result<(), String> {
    let store = PgStore::connect(url).await.map_err(|e| e.to_string())?;
    let all = store.ibkr_series().await.map_err(|e| e.to_string())?;
    let far_past: DateTime<Utc> = DateTime::UNIX_EPOCH;
    let far_future: DateTime<Utc> = "2100-01-01T00:00:00Z".parse().expect("fecha fija");
    let mut fixes: Vec<IbkrBarCorrection> = Vec::new();
    for key in all
        .iter()
        .filter(|k| k.bar_size != "1m" && instruments.contains(&k.instrument_id))
    {
        let size: BarSize = key.bar_size.parse()?;
        let series = key.as_series();
        let minute_series = IbkrSeries {
            bar_size: "1m",
            ..series.clone()
        };
        let bars = store
            .ibkr_bars_between(&series, far_past, far_future)
            .await
            .map_err(|e| e.to_string())?;
        if bars.iter().any(|b| b.kind == "stock") {
            println!(
                "{} {} ({}) {}: acción o ETF, no se corrige (#563)",
                key.source, key.instrument_id, key.contract, key.bar_size
            );
            continue;
        }
        let last_minute = store
            .last_ibkr_bar_start(&minute_series)
            .await
            .map_err(|e| e.to_string())?;
        let minutes = match (bars.first(), bars.last()) {
            (Some(a), Some(b)) => store
                .ibkr_bars_between(&minute_series, a.start, b.start + size.duration())
                .await
                .map_err(|e| e.to_string())?,
            _ => Vec::new(),
        };
        let (mut found, mut skipped) = (0usize, 0usize);
        // Las 1m están ordenadas: cada vela mira solo su tramo.
        let mut lo = 0usize;
        for b in &bars {
            while lo < minutes.len() && minutes[lo].start < b.start {
                lo += 1;
            }
            let end = b.start + size.duration();
            let hi = lo + minutes[lo..].partition_point(|m| m.start < end);
            let inside = &minutes[lo..hi];
            let pairs: Vec<(DateTime<Utc>, Ohlc)> =
                inside.iter().map(|m| (m.start, ohlc(m))).collect();
            let Coherence::Differs { expected } =
                check_against_minutes(b.start, size, ohlc(b), &pairs, last_minute, Compare::All)
            else {
                continue;
            };
            // Volumen: la suma de las de 1m, si todas lo tienen.
            let volume: Option<Decimal> = inside.iter().map(|m| m.volume).sum();
            if let (Some(stored), Some(sum)) = (b.volume, volume)
                && stored > sum
            {
                println!(
                    "  {} {}: NO se corrige: volumen guardado {stored} > suma de las 1m {sum} \
                     (las de 1m podrían estar incompletas)",
                    key.bar_size,
                    b.start.to_rfc3339()
                );
                skipped += 1;
                continue;
            }
            println!(
                "  {} {} {}: guardada {} vol {:?} -> 1m {} vol {:?}",
                key.instrument_id,
                key.bar_size,
                b.start.to_rfc3339(),
                show(&ohlc(b)),
                b.volume,
                show(&expected),
                volume
            );
            found += 1;
            fixes.push(IbkrBarCorrection {
                source: b.source.clone(),
                instrument_id: b.instrument_id.clone(),
                contract: b.contract.clone(),
                bar_size: b.bar_size.clone(),
                what_to_show: b.what_to_show.clone(),
                start: b.start,
                open: expected.open,
                high: expected.high,
                low: expected.low,
                close: expected.close,
                volume,
                reason: REASON.into(),
                correction_source: CORRECTION_SOURCE.into(),
            });
        }
        println!(
            "{} {} ({}) {} {}: {} velas, {found} para corregir, {skipped} sin corregir",
            key.source,
            key.instrument_id,
            key.contract,
            key.bar_size,
            key.what_to_show,
            bars.len()
        );
    }
    if !run {
        println!(
            "total: {} para corregir (sin --run no se guarda nada)",
            fixes.len()
        );
        return Ok(());
    }
    let n = store
        .insert_ibkr_bar_corrections(&fixes)
        .await
        .map_err(|e| e.to_string())?;
    println!("total: {n} correcciones agregadas a ibkr_bar_corrections");
    Ok(())
}

//! Carga incremental de velas **cerradas** de IBKR a PostgreSQL (`ibkr_bars`, #536), contra
//! IB Gateway **paper**, solo lectura.
//!
//! ```text
//! cd engine && set -a && . ../.env && set +a
//! # qué pediría (lee la base y la primera vela de IBKR, no guarda nada):
//! cargo run -q -p cfds-ibkr --example ibkr_load -- MNQ 1h --month 202612
//! # guardar:
//! cargo run -q -p cfds-ibkr --example ibkr_load -- MNQ 1h --month 202612 --run
//! # desde una fecha (si no hay nada guardado de la serie; si hay, sigue desde la última):
//! cargo run -q -p cfds-ibkr --example ibkr_load -- QQQ 15m --from 2026-11-01T00:00:00Z --run
//! ```
//!
//! Serie = instrumento de `config/ibkr.toml` + contrato + tamaño (1m, 15m o 1h) + precio. Sigue
//! desde la última vela guardada de esa serie; si no hay, desde `--from` o `[load].backfill_days`
//! atrás. Respeta `[load]`: ventanas por tamaño, pausa entre pedidos y tope por corrida. Cada
//! ventana se guarda en una transacción, idempotente: repetir no cambia nada y una vela distinta a
//! la guardada (repintado) para la corrida. El futuro continuo y las diarias no se guardan.
//! `IBKR_CLIENT_ID` cambia el `client_id` de la configuración.
//!
//! Solo velas **completas** (#563): los datos de IBKR sin suscripción llegan con unos 10 minutos de
//! atraso, así que la última vela de cada respuesta se guarda solo si cerró hace al menos
//! `[load].data_delay_secs` (las anteriores sí, porque IBKR ya devolvió una después). Si queda una
//! esperando, la corrida para ahí (sin huecos) y la sigue la próxima. Antes de guardar una de 15m o
//! 1h se compara con el agregado de las de 1m guardadas de ese período (si las hay; máximo, mínimo y
//! cierre, o solo el cierre en acciones: ver `Compare::before_storing`): si no coincide, no se guarda
//! esa ni las siguientes, se avisa y la salida es 4.
//!
//! Salida: 0 bien; 1 error; 2 kill switch; 3 IBKR avisó violación de ritmo (pacing): se paró;
//! 4 una vela no coincide con sus velas de 1m guardadas: no se guardó.
//!
//! **Aviso de carga trabada (#607):** con `--run`, cada corrida anota si IBKR respondió (con o sin
//! velas) o no (tiempo agotado, conexión) en el archivo de `[load_health].state_file`. Cuando una
//! serie lleva demasiado sin respuesta (`[load_health]`), guarda **una vez** un evento en
//! `ops_events` (lo muestra la app en Operación) y manda una notificación de macOS; cuando IBKR
//! vuelve a responder, otra vez cada uno. Los errores de la base o de la configuración no cuentan.

use std::process::ExitCode;

use cfds_core::InstrumentId;
use cfds_ibkr::health::{self, RunOutcome, Transition};
use cfds_ibkr::load::{
    Coherence, Compare, Ohlc, OnError, check_against_minutes, check_storable, classify,
    closed_until, complete_prefix, resume_from, windows,
};
use cfds_ibkr::{BarSize, ContractChoice, IbkrConfig, RawBar, SessionError, WhatToShow, connect};
use cfds_storage::{IbkrBarRow, IbkrSeries, OpsEventRecord, PgStore, StorageError};
use chrono::{DateTime, Utc};

const CONFIG: &str = include_str!("../../../../config/ibkr.toml");

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("{msg}");
    ExitCode::from(1)
}

fn ohlc(
    open: rust_decimal::Decimal,
    high: rust_decimal::Decimal,
    low: rust_decimal::Decimal,
    close: rust_decimal::Decimal,
) -> Ohlc {
    Ohlc {
        open,
        high,
        low,
        close,
    }
}

/// Cuántas de `bars` (de 15m o 1h) coinciden con sus velas de 1m guardadas, desde el principio; la
/// primera que no coincide, con el agregado esperado. Sin velas de 1m que cubran un período, esa
/// vela no se revisa.
async fn coherent_prefix(
    store: &PgStore,
    series: &IbkrSeries<'_>,
    size: BarSize,
    compare: Compare,
    bars: &[RawBar],
) -> Result<(usize, Option<(DateTime<Utc>, Ohlc, Ohlc)>), StorageError> {
    let (Some(first), Some(last)) = (bars.first(), bars.last()) else {
        return Ok((0, None));
    };
    if size == BarSize::M1 {
        return Ok((bars.len(), None));
    }
    let minute_series = IbkrSeries {
        bar_size: BarSize::M1.as_str(),
        ..series.clone()
    };
    let last_minute = store.last_ibkr_bar_start(&minute_series).await?;
    let minutes: Vec<(DateTime<Utc>, Ohlc)> = store
        .ibkr_bars_between(&minute_series, first.start, last.start + size.duration())
        .await?
        .into_iter()
        .map(|m| (m.start, ohlc(m.open, m.high, m.low, m.close)))
        .collect();
    for (i, b) in bars.iter().enumerate() {
        let got = ohlc(b.open, b.high, b.low, b.close);
        if let Coherence::Differs { expected } =
            check_against_minutes(b.start, size, got, &minutes, last_minute, compare)
        {
            return Ok((i, Some((b.start, got, expected))));
        }
    }
    Ok((bars.len(), None))
}

fn arg(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

/// Raíz del repo (el ejemplo se compila adentro del repo; el cron corre desde `engine/`).
const REPO_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let run = args.iter().any(|a| a == "--run");
    let (code, outcome) = load(&args, run).await;
    if let (true, Some(outcome), Some(series)) = (run, outcome, series_name(&args)) {
        record_health(&series, &outcome).await;
    }
    code
}

/// "MNQ 1m": el nombre de la serie para la racha.
fn series_name(args: &[String]) -> Option<String> {
    let id = args.first()?.parse::<InstrumentId>().ok()?;
    let size = args.get(1)?.parse::<BarSize>().ok()?;
    Some(format!("{id} {}", size.as_str()))
}

/// Suma la corrida a la racha y, si empezó o terminó un corte, avisa una vez (#607). Nunca cambia
/// la salida de la carga: si algo de esto falla, lo dice y sigue.
async fn record_health(series: &str, outcome: &RunOutcome) {
    let Ok(Some(cfg)) = IbkrConfig::from_toml(CONFIG).map(|c| c.load_health) else {
        return;
    };
    let path = std::path::Path::new(REPO_ROOT).join(&cfg.state_file);
    let (state, problem) = health::read_state(&path);
    if let Some(p) = problem {
        eprintln!(
            "aviso de carga trabada: no se leyó {} ({p}); la racha empieza de nuevo",
            path.display()
        );
    }
    let now = Utc::now();
    let (next, transition) = health::step(&state, series, outcome, now, &cfg);
    if let Err(e) = health::write_state(&path, &next) {
        eprintln!(
            "aviso de carga trabada: no se guardó {}: {e}",
            path.display()
        );
    }
    let mut fields = serde_json::Map::new();
    let (level, target, event, message) = match transition {
        Transition::None => return,
        Transition::Stalled {
            since,
            series,
            failures,
            error,
        } => {
            fields.insert("since".into(), since.to_rfc3339().into());
            fields.insert("series".into(), series.join(", ").into());
            fields.insert("failures".into(), failures.into());
            fields.insert("error".into(), error.into());
            (
                "error",
                "alert",
                "ibkr_stalled",
                health::stalled_message(since, now),
            )
        }
        Transition::Recovered { since } => {
            fields.insert("since".into(), since.to_rfc3339().into());
            (
                "info",
                "ibkr_load",
                "ibkr_recovered",
                health::recovered_message(since, now),
            )
        }
    };
    // `message` primero: es lo que muestra la app.
    let mut ordered = serde_json::Map::new();
    ordered.insert("message".into(), message.clone().into());
    ordered.extend(fields);
    eprintln!("AVISO {event}: {message}");
    if cfg.notify_macos {
        notify_macos("CFDsTrader", &message);
    }
    let record = OpsEventRecord {
        ts: now,
        level: level.into(),
        target: target.into(),
        event: event.into(),
        fields: ordered,
    };
    let saved = match std::env::var("DATABASE_URL") {
        Err(_) => Err("falta DATABASE_URL".to_owned()),
        Ok(url) => match PgStore::connect(&url).await {
            Err(e) => Err(e.to_string()),
            Ok(store) => store
                .insert_ops_events(&[record])
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
        },
    };
    if let Err(e) = saved {
        eprintln!("aviso de carga trabada: no se guardó el evento en ops_events: {e}");
    }
}

/// Notificación de macOS con `osascript`. El texto va como argumento (no dentro del script), así
/// ninguna comilla lo rompe. Fuera de macOS no hace nada.
fn notify_macos(title: &str, message: &str) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let status = std::process::Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display notification (item 1 of argv) with title (item 2 of argv)",
            "-e",
            "end run",
            message,
            title,
        ])
        .status();
    if !matches!(status, Ok(s) if s.success()) {
        eprintln!("aviso de carga trabada: no salió la notificación de macOS ({status:?})");
    }
}

/// La carga. Devuelve la salida y, si se llegó a hablar con IBKR, si respondió o no (#607).
async fn load(args: &[String], run: bool) -> (ExitCode, Option<RunOutcome>) {
    let mut cfg = match IbkrConfig::from_toml(CONFIG) {
        Ok(c) => c,
        Err(e) => return (fail(e), None),
    };
    if let Some(id) = std::env::var("IBKR_CLIENT_ID")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        cfg.client_id = id;
    }
    let (Some(id), Some(size)) = (
        args.first().and_then(|s| s.parse::<InstrumentId>().ok()),
        args.get(1).and_then(|s| s.parse::<BarSize>().ok()),
    ) else {
        return (
            fail("uso: ibkr_load ID TAMAÑO [--month AAAAMM] [--from RFC3339] [--run]"),
            None,
        );
    };
    let Some(spec) = cfg.instrument(&id).cloned() else {
        return (fail(format!("{id} no está en config/ibkr.toml")), None);
    };
    if let Err(e) = check_storable(spec.kind, size) {
        return (fail(format!("{id} {}: {e}", size.as_str())), None);
    }
    let choice = arg(args, "--month").map_or(ContractChoice::Front, ContractChoice::Month);
    let from_arg = match arg(args, "--from").map(|s| s.parse::<DateTime<Utc>>()) {
        None => None,
        Some(Ok(t)) => Some(t),
        Some(Err(e)) => return (fail(format!("--from: {e}")), None),
    };
    let what = match spec.what_to_show.unwrap_or(cfg.what_to_show) {
        WhatToShow::Trades => "TRADES",
        WhatToShow::Midpoint => "MIDPOINT",
    };
    let load = cfg.load.clone();
    let Some(window) = load.window(size) else {
        return (fail("tamaño sin ventana de carga"), None);
    };
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return (fail("falta DATABASE_URL"), None);
    };
    let store = match PgStore::connect(&url).await {
        Ok(s) => s,
        Err(e) => return (fail(e), None),
    };

    let session = match connect(cfg, |k| std::env::var(k).ok()).await {
        Ok(s) => s,
        Err(e) => return (fail(&e), health::connect_outcome(&e)),
    };
    let now = Utc::now();
    // Qué contrato es y desde cuándo tiene datos IBKR.
    let (contract, head) = match session.head_timestamp(&id, choice.clone(), now).await {
        Ok(h) => h,
        Err(e @ SessionError::KillSwitch(_)) => {
            if let SessionError::KillSwitch(t) = &e {
                eprintln!("KILL SWITCH ({}): {}", t.kind.as_str(), t.reason);
            }
            return (ExitCode::from(2), health::session_outcome(&e));
        }
        Err(e) => {
            session.close().await;
            return (fail(format!("{id}: {e}")), health::session_outcome(&e));
        }
    };
    let series = IbkrSeries {
        source: &load.source,
        instrument_id: id.as_str(),
        contract: &contract,
        bar_size: size.as_str(),
        what_to_show: what,
    };
    let last = match store.last_ibkr_bar_start(&series).await {
        Ok(l) => l,
        Err(e) => {
            session.close().await;
            return (fail(e), Some(RunOutcome::Answered));
        }
    };
    let from = resume_from(
        last,
        from_arg.unwrap_or(now - load.backfill),
        Some(head),
        size,
    );
    let to = closed_until(now, size);
    let plan = windows(from, to, window, load.max_requests);
    println!(
        "{id} ({contract}) {} {what}: última guardada {}, se pide [{}, {}) en {} pedidos (tope {}, pausa {} s)",
        size.as_str(),
        last.map_or("ninguna".into(), |t| t.to_rfc3339()),
        from.to_rfc3339(),
        plan.last().map_or(from, |w| w.1).to_rfc3339(),
        plan.len(),
        load.max_requests,
        load.pause.as_secs()
    );
    if !run {
        println!("sin --run: no se pide ni se guarda nada");
        session.close().await;
        return (ExitCode::SUCCESS, Some(RunOutcome::Answered));
    }

    let (mut inserted, mut unchanged) = (0, 0);
    let mut code = ExitCode::SUCCESS;
    // IBKR ya respondió el primer pedido (head_timestamp); un pedido de velas puede cambiarlo.
    let mut outcome = RunOutcome::Answered;
    for (i, (a, b)) in plan.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(load.pause).await;
        }
        let got = match session
            .closed_bars(&id, choice.clone(), size, *a, *b, Utc::now())
            .await
        {
            Ok(g) => g,
            Err(e @ SessionError::KillSwitch(_)) => {
                if let SessionError::KillSwitch(t) = &e {
                    eprintln!("KILL SWITCH ({}): {}", t.kind.as_str(), t.reason);
                }
                return (ExitCode::from(2), health::session_outcome(&e));
            }
            Err(e) => match classify(&e) {
                OnError::Empty => {
                    println!("  [{}, {}): sin velas", a.to_rfc3339(), b.to_rfc3339());
                    continue;
                }
                OnError::Pacing => {
                    eprintln!("IBKR avisó violación de ritmo: se para la corrida ({e})");
                    code = ExitCode::from(3);
                    break;
                }
                OnError::Stop => {
                    eprintln!("{id}: {e}");
                    if let Some(o) = health::session_outcome(&e) {
                        outcome = o;
                    }
                    code = ExitCode::from(1);
                    break;
                }
            },
        };
        if got.contract != contract {
            eprintln!(
                "IBKR cambió de contrato ({contract} → {}): se para; volvé a correr",
                got.contract
            );
            code = ExitCode::from(1);
            break;
        }
        // Solo las completas (#563): la última puede tener datos a medias por el atraso de IBKR.
        let complete = complete_prefix(&got.bars, size, Utc::now(), load.data_delay);
        let waiting = got.bars.get(complete).map(|b| b.start);
        let (keep, differs) = match coherent_prefix(
            &store,
            &series,
            size,
            Compare::before_storing(spec.kind),
            &got.bars[..complete],
        )
        .await
        {
            Ok((n, d)) => (&got.bars[..n], d),
            Err(e) => {
                eprintln!("no se pudieron leer las velas de 1m guardadas: {e}");
                code = ExitCode::from(1);
                break;
            }
        };
        let rows: Vec<IbkrBarRow> = keep
            .iter()
            .map(|b| IbkrBarRow {
                source: load.source.clone(),
                instrument_id: id.to_string(),
                kind: spec.kind.as_str().to_owned(),
                contract: contract.clone(),
                bar_size: size.as_str().to_owned(),
                what_to_show: what.to_owned(),
                start: b.start,
                open: b.open,
                high: b.high,
                low: b.low,
                close: b.close,
                volume: b.volume,
            })
            .collect();
        match store.insert_ibkr_bars(&rows).await {
            Ok(r) => {
                inserted += r.inserted;
                unchanged += r.unchanged;
                println!(
                    "  [{}, {}): {} nuevas, {} ya estaban",
                    a.to_rfc3339(),
                    b.to_rfc3339(),
                    r.inserted,
                    r.unchanged
                );
            }
            Err(e) => {
                eprintln!("no se guardó la ventana: {e}");
                code = ExitCode::from(1);
                break;
            }
        }
        if let Some((start, got, expected)) = differs {
            eprintln!(
                "{id} ({contract}) {} {}: IBKR dio O {} H {} L {} C {}, pero sus velas de 1m \
                 guardadas dan O {} H {} L {} C {}: no se guarda esa ni las siguientes (#563)",
                size.as_str(),
                start.to_rfc3339(),
                got.open,
                got.high,
                got.low,
                got.close,
                expected.open,
                expected.high,
                expected.low,
                expected.close
            );
            code = ExitCode::from(4);
            break;
        }
        if let Some(start) = waiting {
            println!(
                "  la vela de {} espera: puede tener datos a medias (atraso de IBKR); sigue la \
                 próxima corrida",
                start.to_rfc3339()
            );
            break;
        }
    }
    println!("total: {inserted} nuevas, {unchanged} ya estaban");
    session.close().await;
    (code, Some(outcome))
}

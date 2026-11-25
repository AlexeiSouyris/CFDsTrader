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
//! Salida: 0 bien; 1 error; 2 kill switch; 3 IBKR avisó violación de ritmo (pacing): se paró.

use std::process::ExitCode;

use cfds_core::InstrumentId;
use cfds_ibkr::load::{OnError, check_storable, classify, closed_until, resume_from, windows};
use cfds_ibkr::{BarSize, ContractChoice, IbkrConfig, SessionError, WhatToShow, connect};
use cfds_storage::{IbkrBarRow, IbkrSeries, PgStore};
use chrono::{DateTime, Utc};

const CONFIG: &str = include_str!("../../../../config/ibkr.toml");

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("{msg}");
    ExitCode::from(1)
}

fn arg(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let run = args.iter().any(|a| a == "--run");
    let mut cfg = match IbkrConfig::from_toml(CONFIG) {
        Ok(c) => c,
        Err(e) => return fail(e),
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
        return fail("uso: ibkr_load ID TAMAÑO [--month AAAAMM] [--from RFC3339] [--run]");
    };
    let Some(spec) = cfg.instrument(&id).cloned() else {
        return fail(format!("{id} no está en config/ibkr.toml"));
    };
    if let Err(e) = check_storable(spec.kind, size) {
        return fail(format!("{id} {}: {e}", size.as_str()));
    }
    let choice = arg(&args, "--month").map_or(ContractChoice::Front, ContractChoice::Month);
    let from_arg = match arg(&args, "--from").map(|s| s.parse::<DateTime<Utc>>()) {
        None => None,
        Some(Ok(t)) => Some(t),
        Some(Err(e)) => return fail(format!("--from: {e}")),
    };
    let what = match spec.what_to_show.unwrap_or(cfg.what_to_show) {
        WhatToShow::Trades => "TRADES",
        WhatToShow::Midpoint => "MIDPOINT",
    };
    let load = cfg.load.clone();
    let Some(window) = load.window(size) else {
        return fail("tamaño sin ventana de carga");
    };
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return fail("falta DATABASE_URL");
    };
    let store = match PgStore::connect(&url).await {
        Ok(s) => s,
        Err(e) => return fail(e),
    };

    let session = match connect(cfg, |k| std::env::var(k).ok()).await {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let now = Utc::now();
    // Qué contrato es y desde cuándo tiene datos IBKR.
    let (contract, head) = match session.head_timestamp(&id, choice.clone(), now).await {
        Ok(h) => h,
        Err(SessionError::KillSwitch(t)) => {
            eprintln!("KILL SWITCH ({}): {}", t.kind.as_str(), t.reason);
            return ExitCode::from(2);
        }
        Err(e) => {
            session.close().await;
            return fail(format!("{id}: {e}"));
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
            return fail(e);
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
        return ExitCode::SUCCESS;
    }

    let (mut inserted, mut unchanged) = (0, 0);
    let mut code = ExitCode::SUCCESS;
    for (i, (a, b)) in plan.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(load.pause).await;
        }
        let got = match session
            .closed_bars(&id, choice.clone(), size, *a, *b, Utc::now())
            .await
        {
            Ok(g) => g,
            Err(SessionError::KillSwitch(t)) => {
                eprintln!("KILL SWITCH ({}): {}", t.kind.as_str(), t.reason);
                return ExitCode::from(2);
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
        let rows: Vec<IbkrBarRow> = got
            .bars
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
    }
    println!("total: {inserted} nuevas, {unchanged} ya estaban");
    session.close().await;
    code
}

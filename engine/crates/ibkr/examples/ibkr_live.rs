//! Medición a mano de los precios por streaming de IBKR (#558), contra IB Gateway **paper**, solo
//! lectura. Se suscribe a **un** instrumento con el tipo de dato pedido durante unos segundos e
//! imprime qué manda IBKR (tipo de dato, contrato, atraso medido, cantidad de precios) y las velas
//! de 1m que arma `cfds_ibkr::live` (en formación y cerradas).
//!
//! ```text
//! cd engine && set -a && . ../.env && set +a
//! cargo run -q -p cfds-ibkr --example ibkr_live -- MNQ live 90
//! cargo run -q -p cfds-ibkr --example ibkr_live -- QQQ delayed 60
//! # todo [live] como lo corre el gateway (en vivo y, sin permiso, demorados), SEGUNDOS segundos:
//! cargo run -q -p cfds-ibkr --example ibkr_live -- --feed 120
//! ```
//!
//! Usa `config/ibkr.toml` y sus guardas (puerto, cuenta paper, kill switch) con el `client_id` de
//! `[live]` (o `IBKR_CLIENT_ID`). No guarda nada ni muestra cuentas ni saldos.
//!
//! Salida: 0 bien; 1 error o rechazo de IBKR (se imprime); 2 kill switch.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use cfds_core::InstrumentId;
use cfds_ibkr::live_feed::{self, FeedStatus, LiveSink, QuoteInfo};
use cfds_ibkr::{
    BarUpdate, IbkrConfig, LiveDataType, LiveTimeframe, QuoteEvent, QuoteState, SessionError, Trip,
    connect,
};
use chrono::{SecondsFormat, TimeDelta, Utc};
use tokio::sync::mpsc;

const CONFIG: &str = include_str!("../../../../config/ibkr.toml");

fn usage() -> ExitCode {
    eprintln!("uso: ibkr_live ID live|delayed SEGUNDOS | --feed SEGUNDOS");
    ExitCode::from(1)
}

/// Imprime lo que manda `live_feed` (sin cuentas ni saldos): estado, tipo de dato y velas cerradas.
struct Print;

impl LiveSink for Print {
    fn status(&self, status: FeedStatus, at: chrono::DateTime<Utc>) {
        println!(
            "{} estado {} {:?}",
            at.format("%H:%M:%S"),
            status.as_str(),
            status.reason()
        );
    }
    fn quote(&self, id: &InstrumentId, info: &QuoteInfo, at: chrono::DateTime<Utc>) {
        println!(
            "{} {id}: contrato {:?}, tipo {:?}, atraso {:?} s, rechazo {:?}",
            at.format("%H:%M:%S"),
            info.contract,
            info.data_type.map(|d| d.as_str()),
            info.delay.map(|d| d.num_seconds()),
            info.rejected
        );
    }
    fn bar(&self, id: &InstrumentId, _: &QuoteInfo, u: &BarUpdate) {
        if !u.forming {
            let b = &u.bar;
            println!(
                "{id} {} cerrada {} O {} H {} L {} C {} ({} precios{})",
                b.timeframe.as_str(),
                b.start.to_rfc3339_opts(SecondsFormat::Secs, true),
                b.open,
                b.high,
                b.low,
                b.close,
                b.ticks,
                if b.partial { ", parcial" } else { "" }
            );
        }
    }
    fn trip(&self, t: &Trip) {
        println!("kill switch: {}", t.reason);
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--feed") {
        let Some(secs) = args.get(1).and_then(|s| s.parse::<u64>().ok()) else {
            return usage();
        };
        let cfg = match IbkrConfig::from_toml(CONFIG) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        };
        let run = live_feed::run(cfg, |k| std::env::var(k).ok(), Arc::new(Print));
        let _ = tokio::time::timeout(Duration::from_secs(secs), run).await;
        return ExitCode::SUCCESS;
    }
    let (Some(id), Some(kind), Some(secs)) = (
        args.first().and_then(|s| s.parse::<InstrumentId>().ok()),
        args.get(1),
        args.get(2).and_then(|s| s.parse::<u64>().ok()),
    ) else {
        return usage();
    };
    let data_type = match kind.as_str() {
        "live" => LiveDataType::Live,
        "delayed" => LiveDataType::Delayed,
        _ => return usage(),
    };
    let mut cfg = match IbkrConfig::from_toml(CONFIG) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let Some(live) = cfg.live.clone() else {
        eprintln!("config/ibkr.toml no tiene [live]");
        return ExitCode::from(1);
    };
    cfg.client_id = std::env::var("IBKR_CLIENT_ID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(live.client_id);
    let session = match connect(cfg, |k| std::env::var(k).ok()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let (tx, mut rx) = mpsc::channel(1024);
    let mut state = QuoteState::new(&[LiveTimeframe::M1], live.pending_wait);
    let (mut lasts, mut times, mut types) = (0u32, 0u32, Vec::new());
    let mut max_delay = TimeDelta::zero();
    let deadline = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(deadline);
    let stream = session.stream_quotes(&id, data_type, tx, Utc::now());
    tokio::pin!(stream);
    let mut clock = tokio::time::interval(Duration::from_secs(1));
    let t = |d: chrono::DateTime<Utc>| d.to_rfc3339_opts(SecondsFormat::Secs, true);
    let result = loop {
        let updates = tokio::select! {
            r = &mut stream => break r,
            () = &mut deadline => break Ok(()),
            Some(ev) = rx.recv() => {
                match &ev {
                    QuoteEvent::Contract(c) => println!("contrato: {c}"),
                    QuoteEvent::DataType(d) => {
                        println!("tipo de dato que informa IBKR: {}", d.as_str());
                        types.push(*d);
                    }
                    QuoteEvent::Last { .. } => lasts += 1,
                    QuoteEvent::LastTime { at, received } => {
                        times += 1;
                        max_delay = max_delay.max(*received - *at);
                        if times <= 3 {
                            println!("hora del último operado {} (llegó {}, atraso {} s)", t(*at), t(*received), (*received - *at).num_seconds());
                        }
                    }
                }
                state.on_event(ev)
            }
            _ = clock.tick() => state.on_clock(Utc::now()),
        };
        for u in updates.iter().filter(|u| !u.forming) {
            let b = &u.bar;
            println!(
                "vela 1m cerrada {} O {} H {} L {} C {} ({} precios)",
                t(b.start),
                b.open,
                b.high,
                b.low,
                b.close,
                b.ticks
            );
        }
    };
    if let Some(b) = state.bar(LiveTimeframe::M1) {
        println!(
            "vela 1m en formación {} O {} H {} L {} C {} ({} precios)",
            t(b.start),
            b.open,
            b.high,
            b.low,
            b.close,
            b.ticks
        );
    }
    println!(
        "resumen: pedido {}, informado {:?}, {} últimos, {} horas, atraso máx {} s",
        data_type.as_str(),
        types.iter().map(|d| d.as_str()).collect::<Vec<_>>(),
        lasts,
        times,
        max_delay.num_seconds()
    );
    session.disconnect().await;
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e @ SessionError::KillSwitch(_)) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

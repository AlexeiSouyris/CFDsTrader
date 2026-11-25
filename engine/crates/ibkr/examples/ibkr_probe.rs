//! Medición a mano de la historia que da IBKR (#536), contra IB Gateway **paper**, solo lectura.
//! Un pedido por corrida: así el ritmo (pacing) lo controla quien lo llama.
//!
//! ```text
//! cd engine && set -a && . ../.env && set +a
//! # primera vela que tiene IBKR de un instrumento de config/ibkr.toml:
//! cargo run -q -p cfds-ibkr --example ibkr_probe -- --head MNQ
//! # velas cerradas de un tamaño en [desde, hasta) (UTC, RFC 3339):
//! cargo run -q -p cfds-ibkr --example ibkr_probe -- --bars MNQ 1h 2026-11-20T00:00:00Z 2026-11-21T00:00:00Z
//! # lo mismo de un mes concreto (puede estar vencido): --month 202609
//! ```
//!
//! Usa `config/ibkr.toml` y sus guardas (puerto, cuenta paper, kill switch); el `client_id` se
//! puede cambiar con `IBKR_CLIENT_ID` para no chocar con otro programa conectado. Solo imprime la
//! cantidad de velas, la primera y la última: no guarda nada ni muestra cuentas ni saldos.
//!
//! Salida: 0 bien; 1 error o rechazo de IBKR (se imprime); 2 kill switch.

use std::process::ExitCode;

use cfds_core::InstrumentId;
use cfds_ibkr::{BarSize, ContractChoice, IbkrConfig, SessionError, connect};
use chrono::{DateTime, Utc};

const CONFIG: &str = include_str!("../../../../config/ibkr.toml");

fn usage() -> ExitCode {
    eprintln!(
        "uso: ibkr_probe --head ID [--month AAAAMM] | --bars ID TAMAÑO DESDE HASTA [--month AAAAMM]"
    );
    ExitCode::from(1)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cfg = match IbkrConfig::from_toml(CONFIG) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    if let Some(id) = std::env::var("IBKR_CLIENT_ID")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        cfg.client_id = id;
    }
    let choice = match args.iter().position(|a| a == "--month") {
        Some(i) => match args.get(i + 1) {
            Some(m) => ContractChoice::Month(m.clone()),
            None => return usage(),
        },
        None => ContractChoice::Front,
    };
    let Some(id) = args.get(1).and_then(|s| s.parse::<InstrumentId>().ok()) else {
        return usage();
    };
    let session = match connect(cfg, |k| std::env::var(k).ok()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let now = Utc::now();
    let out = match args.first().map(String::as_str) {
        Some("--head") => session
            .head_timestamp(&id, choice, now)
            .await
            .map(|(c, t)| println!("{id} ({c}): primera vela {}", t.to_rfc3339())),
        Some("--bars") => {
            let size = args.get(2).and_then(|s| s.parse::<BarSize>().ok());
            let from = args.get(3).and_then(|s| s.parse::<DateTime<Utc>>().ok());
            let to = args.get(4).and_then(|s| s.parse::<DateTime<Utc>>().ok());
            let (Some(size), Some(from), Some(to)) = (size, from, to) else {
                session.close().await;
                return usage();
            };
            session
                .closed_bars(&id, choice, size, from, to, now)
                .await
                .map(|got| {
                    let fmt = |b: Option<&cfds_ibkr::RawBar>| {
                        b.map(|b| format!("{} C {}", b.start.to_rfc3339(), b.close))
                            .unwrap_or_else(|| "-".into())
                    };
                    println!(
                        "{id} ({}), {}: {} velas cerradas; primera {}; última {}",
                        got.contract,
                        size.as_str(),
                        got.bars.len(),
                        fmt(got.bars.first()),
                        fmt(got.bars.last())
                    );
                })
        }
        _ => {
            session.close().await;
            return usage();
        }
    };
    match out {
        Ok(()) => {
            session.close().await;
            ExitCode::SUCCESS
        }
        Err(SessionError::KillSwitch(t)) => {
            eprintln!("KILL SWITCH ({}): {}", t.kind.as_str(), t.reason);
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("{id}: {e}");
            session.close().await;
            ExitCode::from(1)
        }
    }
}

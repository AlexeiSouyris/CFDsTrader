//! Prueba a mano contra IB Gateway **paper**, solo lectura (ADR 0017 etapa 1, #534).
//!
//! ```text
//! cd engine && set -a && . ../.env && set +a
//! cargo run -q -p cfds-ibkr --example ibkr_read                 # cuentas, resumen y velas, y sale
//! cargo run -q -p cfds-ibkr --example ibkr_read -- --watch      # además vigila la conexión
//! cargo run -q -p cfds-ibkr --example ibkr_read -- --check-config  # valida la configuración, sin conectar
//! ```
//!
//! Antes: IB Gateway abierto con login paper hecho a mano, "Read-Only API" activado y
//! `IBKR_PAPER_ACCOUNT_ID` en `.env`. Lee `config/ibkr.toml` y `config/alerts.toml`. Solo imprime:
//! no guarda nada en la base. Los IDs de cuenta se muestran tapados.
//!
//! Salida: 0 si todo bien; 1 si no se pudo conectar o un pedido falló; 2 si se activó el kill
//! switch (cuenta que no es paper, desconexión o error de datos), con alerta al log (y a Telegram si
//! está configurado).

use std::process::ExitCode;
use std::sync::Arc;

use cfds_ibkr::{ConnectError, IbkrConfig, SessionError, Trip, connect};
use cfds_obs::{AlertConfig, Alerter, Logger, ReqwestPost, TelegramSink};
use chrono::Utc;

const CONFIG: &str = include_str!("../../../../config/ibkr.toml");
const ALERTS: &str = include_str!("../../../../config/alerts.toml");

async fn alert(alerts: &Alerter<ReqwestPost>, trip: &Trip) -> ExitCode {
    eprintln!("KILL SWITCH ({}): {}", trip.kind.as_str(), trip.reason);
    alerts.raise(trip.kind, &trip.reason, trip.at).await;
    ExitCode::from(2)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let watch = args.iter().any(|a| a == "--watch");
    let cfg = match IbkrConfig::from_toml(CONFIG) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    if args.iter().any(|a| a == "--check-config") {
        println!(
            "config/ibkr.toml válida: {}:{} (client_id {}), puertos prohibidos {:?}, velas {} de {}",
            cfg.host,
            cfg.port,
            cfg.client_id,
            cfg.forbidden_ports,
            cfg.timeframe.as_str(),
            cfg.instruments
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return ExitCode::SUCCESS;
    }
    let alerts = {
        let alert_cfg = match AlertConfig::from_toml(ALERTS) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("config/alerts.toml: {e}");
                return ExitCode::from(1);
            }
        };
        let http = match ReqwestPost::new(alert_cfg.telegram_timeout) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        };
        let telegram = TelegramSink::from_env(&alert_cfg, http, |n| std::env::var(n).ok());
        Alerter::new(alert_cfg, Arc::new(Logger::stderr()), telegram)
    };

    println!(
        "conectando a IB Gateway en {}:{} (solo lectura)...",
        cfg.host, cfg.port
    );
    let session = match connect(cfg, |k| std::env::var(k).ok()).await {
        Ok(s) => s,
        Err(ConnectError::Tripped(trip)) => return alert(&alerts, &trip).await,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    println!(
        "sesión abierta: la única cuenta es la paper de .env ({})",
        session.paper_account()
    );

    let mut failed = false;
    match session.account_summary(Utc::now()).await {
        Ok(s) => println!(
            "cuenta {}: tipo {:?}, moneda {:?}, valor neto {:?}, efectivo {:?}, disponible {:?}",
            s.account,
            s.account_type,
            s.currency,
            s.net_liquidation,
            s.total_cash,
            s.available_funds
        ),
        Err(SessionError::KillSwitch(trip)) => return alert(&alerts, &trip).await,
        Err(e) => {
            eprintln!("resumen de cuenta: {e}");
            failed = true;
        }
    }

    let cfg = session.config().clone();
    let now = Utc::now();
    for spec in &cfg.instruments {
        match session
            .closed_candles(&spec.id, cfg.timeframe, now - cfg.lookback, now, now)
            .await
        {
            Ok(got) => {
                println!(
                    "{} ({}), fuente {}: {} velas cerradas de {}",
                    spec.id,
                    got.contract,
                    got.source,
                    got.candles.len(),
                    cfg.timeframe.as_str()
                );
                for c in got.candles.iter().rev().take(3).rev() {
                    println!(
                        "  {} O {} H {} L {} C {} V {}",
                        c.start.format("%Y-%m-%d %H:%M UTC"),
                        c.open,
                        c.high,
                        c.low,
                        c.close,
                        c.volume
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".into())
                    );
                }
            }
            Err(SessionError::KillSwitch(trip)) => return alert(&alerts, &trip).await,
            Err(e) => {
                eprintln!("{}: {e}", spec.id);
                failed = true;
            }
        }
    }

    if watch {
        println!(
            "vigilando la conexión cada {} s (Ctrl+C para salir)...",
            cfg.watch_interval.as_secs()
        );
        loop {
            tokio::time::sleep(cfg.watch_interval).await;
            if let Err(e) = session.check_connection(Utc::now()).await {
                return match e {
                    SessionError::KillSwitch(trip) => alert(&alerts, &trip).await,
                    other => {
                        eprintln!("{other}");
                        ExitCode::from(1)
                    }
                };
            }
        }
    }

    session.close().await;
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

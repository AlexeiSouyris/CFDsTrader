//! Ensayo del runner de paper sobre velas guardadas (#406): el mismo modo del gateway
//! (`PgPaper::start` + `PgPaper::run_once` + `PgPaper::shutdown`), pero con un **reloj simulado**
//! que avanza vela a vela en vez de `Utc::now()`.
//!
//! ```text
//! DATABASE_URL=<base cfds_test_*> cargo run -p cfds-gateway --example runner_replay -- \
//!     <desde UTC> <hasta UTC> [--demora-seg N]
//! ```
//! - **Solo una base de pruebas:** se niega a correr si la base de `DATABASE_URL` no se llama
//!   `cfds_test_*`. Escribe el registro del broker simulado y el diario en esa base, como el
//!   gateway (sesión de solo lectura salvo sus escrituras declaradas).
//! - El reloj marca el cierre de cada vela de 15m entre `desde` y `hasta` (ambos incluidos) más
//!   `--demora-seg` (60 por omisión: el runner real mira cada 30 s). En cada marca corre un paso
//!   del runner. Al terminar, la parada segura del gateway en `hasta`.
//! - No cambia la lógica de decisión: es la misma `run_once` del gateway, con la config de
//!   `config/` (salvo `[runner] enabled`, que acá no se mira). Sin Telegram: las alertas van
//!   solo al log (salida de errores, una línea JSON por evento).
//! - Para ensayar un reinicio a mitad: correr `desde → medio` y después `medio → hasta`. La
//!   marca `medio` se repite a propósito: la misma decisión no tiene que duplicar nada.
//!
//! Nunca llama a IG. Solo paper.

use std::path::PathBuf;
use std::sync::Arc;

use cfds_backtest::BacktestConfig;
use cfds_broker::BrokerConfig;
use cfds_core::Timeframe;
use cfds_gateway::paper::{Ops, PaperConfig, PaperService, PgPaper};
use cfds_gateway::paper_runner::RunnerDeps;
use cfds_gateway::read_only_url;
use cfds_journal::pg::PgJournal;
use cfds_market_data::{Catalog, ExchangeCalendar};
use cfds_obs::{AlertConfig, Alerter, Logger, ReqwestPost, TelegramSink};
use cfds_risk::RiskConfig;
use cfds_storage::PgStore;
use chrono::{DateTime, DurationRound, TimeDelta, Utc};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// Las marcas del reloj: el cierre de cada vela de 15m en `[from, to]` más `lag`. Si `from` no
/// cae en un cierre, la primera marca es el cierre siguiente.
fn replay_clock(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    lag: TimeDelta,
) -> Result<Vec<DateTime<Utc>>, String> {
    let step = Timeframe::M15.duration();
    let mut t = from.duration_trunc(step).map_err(|e| e.to_string())?;
    if t < from {
        t += step;
    }
    let mut out = Vec::new();
    while t <= to {
        out.push(t + lag);
        t += step;
    }
    Ok(out)
}

/// El nombre de la base de una URL de PostgreSQL (sin parámetros).
fn database_name(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let path = rest.split_once('/')?.1;
    Some(path.split(['?', '#']).next().unwrap_or(path))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut lag = TimeDelta::seconds(60);
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--demora-seg" {
            let v: i64 = it.next().ok_or("--demora-seg N")?.parse()?;
            if !(0..=600).contains(&v) {
                return Err("--demora-seg: de 0 a 600".into());
            }
            lag = TimeDelta::seconds(v);
        } else {
            pos.push(a.as_str());
        }
    }
    let [from, to] = pos.as_slice() else {
        return Err("uso: <desde UTC> <hasta UTC> [--demora-seg N]".into());
    };
    let (from, to): (DateTime<Utc>, DateTime<Utc>) = (from.parse()?, to.parse()?);
    let ticks = replay_clock(from, to, lag)?;
    let (Some(&first), Some(&last)) = (ticks.first(), ticks.last()) else {
        return Err("no hay ningún cierre de vela de 15m en el período".into());
    };

    let url = std::env::var("DATABASE_URL")?;
    if !database_name(&url).is_some_and(|d| d.starts_with("cfds_test_")) {
        return Err("solo contra una base de pruebas cfds_test_* (nunca la real)".into());
    }

    let log = Arc::new(Logger::stderr());
    log.register_secret(&url);
    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let mut deps = RunnerDeps::from_repo_config(&catalog, &config_dir)?;
    let risk = RiskConfig::from_toml(cfg!("risk.toml"))?;
    let backtest = BacktestConfig::from_toml(cfg!("backtest.toml"))?;
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let broker_cfg = BrokerConfig::from_toml(
        cfg!("broker.toml"),
        risk,
        backtest.costs,
        backtest.exits,
        backtest.day_tz,
    )?
    .with_calendar(&calendar, catalog.instruments.keys())?;
    let paper_cfg = PaperConfig::from_toml(cfg!("gateway.toml"))?;
    // Sin Telegram: las alertas quedan en el log.
    let alerts = Alerter::new(
        AlertConfig::from_toml(cfg!("alerts.toml"))?,
        log.clone(),
        None::<TelegramSink<ReqwestPost>>,
    );
    let ops = Arc::new(Ops {
        log: log.clone(),
        alerts,
    });

    // La sesión de solo lectura del gateway: el runner escribe solo lo declarado.
    let store = PgStore::connect(&read_only_url(&url)).await?;
    deps.load_probability(&PgJournal::new(store.pool().clone()))
        .await?;
    let (paper, skipped) =
        PgPaper::start(store, &catalog, broker_cfg, &paper_cfg, ops, first).await?;
    for s in &skipped {
        eprintln!("saltado: {s}");
    }
    for (i, &now) in ticks.iter().enumerate() {
        if let Err(e) = paper.run_once(&deps, now).await {
            eprintln!("paso {i} ({now}): {e}");
        }
    }
    let flush = paper.shutdown(last).await?;
    let account = paper.account(last).await?;
    let positions = paper.positions(last).await?;
    println!(
        "{} pasos de {first} a {last}; parada segura limpia: {} (el kill switch y las \
         propuestas, en el log)",
        ticks.len(),
        flush.is_clean()
    );
    println!(
        "cuenta: capital {} · posiciones abiertas {}",
        account.equity,
        positions.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn the_clock_marks_every_close_plus_the_lag() {
        let v = replay_clock(
            t("2026-10-01T10:00:00Z"),
            t("2026-10-01T10:45:00Z"),
            TimeDelta::seconds(60),
        )
        .unwrap();
        assert_eq!(
            v,
            vec![
                t("2026-10-01T10:01:00Z"),
                t("2026-10-01T10:16:00Z"),
                t("2026-10-01T10:31:00Z"),
                t("2026-10-01T10:46:00Z"),
            ]
        );
    }

    #[test]
    fn a_start_between_closes_waits_for_the_next_close() {
        let v = replay_clock(
            t("2026-10-01T10:07:00Z"),
            t("2026-10-01T10:20:00Z"),
            TimeDelta::zero(),
        )
        .unwrap();
        assert_eq!(v, vec![t("2026-10-01T10:15:00Z")]);
    }

    #[test]
    fn an_empty_period_has_no_marks() {
        let v = replay_clock(
            t("2026-10-01T10:01:00Z"),
            t("2026-10-01T10:14:00Z"),
            TimeDelta::zero(),
        )
        .unwrap();
        assert!(v.is_empty());
    }

    #[test]
    fn only_test_databases_are_accepted() {
        assert_eq!(
            database_name("postgres://u:p@127.0.0.1:5432/cfds_test_runner?sslmode=disable"),
            Some("cfds_test_runner")
        );
        assert_eq!(
            database_name("postgres://u:p@127.0.0.1:5432/CDFsTrader"),
            Some("CDFsTrader")
        );
        assert_eq!(database_name("no es una url"), None);
    }
}

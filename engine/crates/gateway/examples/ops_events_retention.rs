//! Retención de los eventos de la operación (#337): borra de `ops_events` lo anterior a
//! `[ops] retention_days` de `config/gateway.toml`. Es el **único** que puede borrar: la tabla es
//! de solo agregar y el trigger solo lo deja con `SET LOCAL cfds.ops_events_retention = 'on'`,
//! que este job pide en su propia transacción. No usa IG ni toca el broker.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-gateway --example ops_events_retention [-- --dry-run]
//! ```
//! Con `--dry-run` solo dice desde cuándo borraría. Pensado para correr una vez por día (cron del
//! Mac mini). Necesita una sesión con permiso de escritura (no la de solo lectura del gateway).

use cfds_gateway::status::StatusConfig;
use cfds_storage::PgStore;
use chrono::{TimeDelta, Utc};

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = StatusConfig::from_toml(GATEWAY)?;
    let dry_run = std::env::args().skip(1).any(|a| a == "--dry-run");
    let cutoff = Utc::now() - TimeDelta::days(i64::from(cfg.retention_days));
    if dry_run {
        println!(
            "ops_events: borraría los eventos anteriores a {cutoff} ({} días)",
            cfg.retention_days
        );
        return Ok(());
    }
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    let n = store.purge_ops_events_before(cutoff).await?;
    println!(
        "ops_events: {n} eventos anteriores a {cutoff} borrados ({} días de retención)",
        cfg.retention_days
    );
    Ok(())
}

//! Aplica las migraciones de `infra/migrations/` que falten y muestra la versión de la base (#348).
//! No hace nada más.
//!
//! ```text
//! DATABASE_URL=... cargo run -q -p cfds-storage --example migrate
//! ```
//! El gateway **no** migra: su sesión de PostgreSQL es de solo lectura. Después de bajar código
//! con migraciones nuevas, se corre esto una vez (o cualquier carga que migre, como
//! `ig_backfill_daily`) y recién después se prende el gateway.

use cfds_storage::{PgStore, migrator};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let before = store.migration_version().await.ok().flatten();
    store.migrate().await?;
    let after = store.migration_version().await?;
    let known = migrator().migrations.iter().map(|m| m.version).max();
    println!(
        "migraciones: la base estaba en {}, ahora en {}; la última de infra/migrations es {}",
        before.map_or("ninguna".into(), |v| format!("{v:04}")),
        after.map_or("ninguna".into(), |v| format!("{v:04}")),
        known.map_or("ninguna".into(), |v| format!("{v:04}")),
    );
    Ok(())
}

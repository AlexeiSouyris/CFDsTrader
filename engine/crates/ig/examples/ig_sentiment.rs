//! Graba el sentimiento de clientes de IG (#27) cada `sentiment.interval_minutes`, para los
//! instrumentos del catálogo. Solo guarda el dato; no lo interpreta.
//!
//! ```text
//! cargo run -p cfds-ig --example ig_sentiment            # graba hasta Ctrl+C
//! cargo run -p cfds-ig --example ig_sentiment -- --once  # una consulta y sale
//! ```
//!
//! Con `DATABASE_URL`, guarda en `sentiment_observations` (solo agregar). Sin ella, muestra los
//! valores y no guarda nada. Un error de una consulta se informa y se reintenta en la próxima;
//! credenciales rechazadas cortan el proceso.

use cfds_ig::sentiment::{next_poll, observations};
use cfds_ig::{
    Credentials, IgConfig, IgProvider, ReqwestTransport, api_key_from_env, epics_from_catalog,
};
use cfds_market_data::{Catalog, ProviderError};
use cfds_storage::PgStore;
use chrono::{TimeDelta, Utc};

const CONFIG: &str = include_str!("../../../../config/ig.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let once = std::env::args().any(|a| a == "--once");
    let cfg = IgConfig::from_toml(CONFIG)?;
    let catalog = Catalog::from_toml(CATALOG)?;
    let interval = TimeDelta::from_std(cfg.sentiment_interval)?;
    let store = match std::env::var("DATABASE_URL") {
        Ok(url) => {
            let s = PgStore::connect(&url).await?;
            s.migrate().await?;
            for spec in catalog.instruments.values() {
                s.upsert_instrument(&spec.instrument).await?;
            }
            println!("guarda en PostgreSQL (sentiment_observations)");
            Some(s)
        }
        Err(_) => {
            println!("corrida en seco: sin DATABASE_URL no se guarda nada");
            None
        }
    };
    let epics = epics_from_catalog(&catalog);
    let instruments: Vec<_> = epics.keys().cloned().collect();
    let transport = ReqwestTransport::new(&cfg, api_key_from_env())?;
    let ig = IgProvider::new(transport, cfg, Credentials::from_env()?, epics);

    let result = loop {
        match ig.client_sentiment(&instruments).await {
            Ok(readings) => {
                let obs = observations(&readings, Utc::now());
                for (id, long) in &readings {
                    match long {
                        Some(v) => println!("{id}: {v} % largos"),
                        None => println!("{id}: IG no tiene dato (no se graba)"),
                    }
                }
                if let Some(s) = &store {
                    match s.insert_sentiment(&obs).await {
                        Ok(r) => println!("guardadas {} observaciones nuevas", r.inserted),
                        Err(e) => break Err(e.to_string()),
                    }
                }
            }
            Err(ProviderError::Unauthorized) => break Err("credenciales rechazadas".into()),
            Err(e) => println!("consulta fallida, se reintenta en la próxima: {e}"),
        }
        if once {
            break Ok(());
        }
        let wake = next_poll(Utc::now(), interval);
        tokio::time::sleep((wake - Utc::now()).to_std().unwrap_or_default()).await;
    };
    ig.logout().await?;
    result.map_err(Into::into)
}

//! Prueba manual contra la API demo (no corre en CI): baja velas y muestra el cupo restante.
//!
//! ```text
//! cargo run -p cfds-ig --example ig_history -- <INSTRUMENTO> <EPIC> <15m|1h> <desde UTC> <hasta UTC>
//! cargo run -p cfds-ig --example ig_history -- DE40 IX.D.DAX.IFE.IP 15m 2026-11-20T09:00:00Z 2026-11-20T10:00:00Z
//! ```
//! Cada vela gasta un punto del cupo semanal de IG (10.000): usar rangos cortos.

use std::collections::BTreeMap;

use cfds_ig::{Credentials, IgConfig, IgProvider, ReqwestTransport, api_key_from_env};
use cfds_market_data::MarketDataProvider;

const CONFIG: &str = include_str!("../../../../config/ig.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [instrument, epic, tf, from, to] = args.as_slice() else {
        return Err("uso: <INSTRUMENTO> <EPIC> <15m|1h> <desde UTC> <hasta UTC>".into());
    };
    let cfg = IgConfig::from_toml(CONFIG)?;
    let transport = ReqwestTransport::new(&cfg, api_key_from_env())?;
    let id: cfds_core::InstrumentId = instrument.parse()?;
    let epics = BTreeMap::from([(id.clone(), epic.clone())]);
    let ig = IgProvider::new(transport, cfg, Credentials::from_env()?, epics);

    let result = ig
        .history(&id, tf.parse()?, from.parse()?, to.parse()?)
        .await;
    ig.logout().await?;
    for c in result? {
        println!(
            "{} {} o={} h={} l={} c={}",
            c.start.to_rfc3339(),
            c.timeframe,
            c.open,
            c.high,
            c.low,
            c.close
        );
    }
    if let Some((left, resets)) = ig.allowance().await {
        println!(
            "cupo semanal restante: {left} puntos (se renueva en {} h)",
            resets.as_secs() / 3600
        );
    }
    Ok(())
}

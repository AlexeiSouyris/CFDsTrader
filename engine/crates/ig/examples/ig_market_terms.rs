//! Lee de la API demo lo que falta en `config/instruments.toml` (#67) y mide el spread.
//! No corre en CI.
//!
//! ```text
//! cargo run -p cfds-ig --example ig_market_terms -- <desde UTC> <hasta UTC>
//! cargo run -p cfds-ig --example ig_market_terms -- 2026-11-09T00:00:00Z 2026-11-21T00:00:00Z
//! ```
//!
//! Para cada instrumento con epic:
//! - condiciones del contrato (`GET /markets/{epic}` v3): moneda, valor por punto, mínimo;
//! - spread típico: mediana de `ask − bid` al cierre de las velas de 1h que caben enteras en la
//!   sesión regular de la bolsa (calendario de `config/sessions.toml`), y la mediana por hora UTC.
//!
//! Gasta del cupo semanal unas 24 velas de 1h por día del rango, por instrumento.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_core::Timeframe;
use cfds_ig::{
    Credentials, IgConfig, IgProvider, ReqwestTransport, api_key_from_env, epics_from_catalog,
    median,
};
use cfds_market_data::{Catalog, ExchangeCalendar};
use chrono::{DateTime, Timelike, Utc};

const CONFIG: &str = include_str!("../../../../config/ig.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [from, to] = args.as_slice() else {
        return Err("uso: <desde UTC> <hasta UTC>".into());
    };
    let (from, to): (DateTime<Utc>, DateTime<Utc>) = (from.parse()?, to.parse()?);
    let catalog = Catalog::from_toml(CATALOG)?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let cfg = IgConfig::from_toml(CONFIG)?;
    let transport = ReqwestTransport::new(&cfg, api_key_from_env())?;
    let epics = epics_from_catalog(&catalog);
    let ig = IgProvider::new(transport, cfg, Credentials::from_env()?, epics.clone());

    let result = report(&ig, &calendar, &epics, from, to).await;
    ig.logout().await?;
    if let Some((left, resets)) = ig.allowance().await {
        println!(
            "\ncupo semanal restante: {left} puntos (se renueva en {} h)",
            resets.as_secs() / 3600
        );
    }
    result
}

async fn report(
    ig: &IgProvider<ReqwestTransport>,
    calendar: &ExchangeCalendar,
    epics: &BTreeMap<cfds_core::InstrumentId, String>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<(), Box<dyn std::error::Error>> {
    for id in epics.keys() {
        let t = ig.market_terms(id).await?;
        println!(
            "\n[{id}] {} | {} | {} | moneda {} | valor por punto {} | mínimo {} | margen {:?} %",
            t.epic,
            t.name,
            t.instrument_type,
            t.currency,
            t.value_per_point,
            t.min_deal_size,
            t.margin_pct
        );

        let spreads = ig.close_spreads(id, Timeframe::H1, from, to).await?;
        let session = calendar
            .session_for(id)
            .ok_or("instrumento sin bolsa en el calendario")?;
        let in_session = session
            .candle_starts(Timeframe::H1, from, to)
            .ok_or("rango fuera del calendario de feriados")?;
        let typical: Vec<_> = spreads
            .iter()
            .filter(|(start, _)| in_session.contains(start))
            .map(|(_, s)| *s)
            .collect();
        println!(
            "  spread típico en sesión: {:?} puntos ({} velas de 1h; {} velas en total)",
            median(typical.clone()),
            typical.len(),
            spreads.len()
        );
        let mut by_hour: BTreeMap<u32, Vec<_>> = BTreeMap::new();
        for (start, s) in &spreads {
            by_hour.entry(start.hour()).or_default().push(*s);
        }
        let line: Vec<String> = by_hour
            .into_iter()
            .map(|(h, v)| format!("{h:02}h={}", median(v).unwrap_or_default()))
            .collect();
        println!("  mediana por hora UTC: {}", line.join(" "));
    }
    Ok(())
}

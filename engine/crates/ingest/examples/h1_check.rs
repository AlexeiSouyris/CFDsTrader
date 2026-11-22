//! Mide la vela de 1h armada con las de 15m (spec §3, #150) contra la de 1h guardada del
//! proveedor, con datos que **ya están** en PostgreSQL. No se conecta a IG: 0 puntos de cupo.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example h1_check -- <INSTRUMENTO> <desde UTC> <hasta UTC>
//! ```
//! Solo se comparan las horas completas de sesión según el calendario de la bolsa.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cfds_core::{InstrumentId, Timeframe};
use cfds_market_data::{Catalog, ExchangeCalendar, SessionCalendar, aggregate_h1, compare_h1};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const FIELDS: [&str; 4] = ["apertura", "máximo", "mínimo", "cierre"];
const SHOW: usize = 10;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [instrument, from, to] = args.as_slice() else {
        return Err("uso: <INSTRUMENTO> <desde UTC> <hasta UTC>".into());
    };
    let id: InstrumentId = instrument.parse()?;
    let (from, to): (DateTime<Utc>, DateTime<Utc>) = (from.parse()?, to.parse()?);
    let catalog = Catalog::from_toml(CATALOG)?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let session: BTreeSet<_> = calendar
        .expected_starts(&id, Timeframe::H1, from, to)
        .ok_or("el calendario de sesiones no cubre ese rango")?
        .into_iter()
        .collect();

    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let m15 = store.candles(&id, Timeframe::M15, from, to).await?;
    let mut ig = store.candles(&id, Timeframe::H1, from, to).await?;
    ig.retain(|c| session.contains(&c.start));
    let mut built = aggregate_h1(&m15)?;
    built.retain(|c| session.contains(&c.start));
    let r = compare_h1(&ig, &built);

    println!(
        "{id} {from} → {to}: {} horas de sesión; guardadas {} velas de 15m y {} de 1h de IG",
        session.len(),
        m15.len(),
        ig.len()
    );
    if r.compared == 0 {
        println!("no hay horas con las dos versiones: cargar 15m y 1h del mismo rango");
        return Ok(());
    }
    let pct =
        (Decimal::from(r.identical) * Decimal::ONE_HUNDRED / Decimal::from(r.compared)).round_dp(1);
    println!(
        "comparadas {}: {} idénticas ({pct} %), {} con alguna diferencia",
        r.compared,
        r.identical,
        r.differing.len()
    );
    for (k, (name, max)) in FIELDS.iter().zip(r.max_abs_diff).enumerate() {
        let n = r.differing.iter().filter(|(_, d)| !d[k].is_zero()).count();
        println!("  {name}: difiere en {n} horas, diferencia máxima {max} puntos");
    }
    for (start, d) in r.differing.iter().take(SHOW) {
        println!("  {start}: IG − armada = {d:?}");
    }
    if r.differing.len() > SHOW {
        println!("  … y {} más", r.differing.len() - SHOW);
    }
    if !r.only_provider.is_empty() {
        println!(
            "{} horas de IG sin las cuatro velas de 15m (no se pudieron armar)",
            r.only_provider.len()
        );
    }
    if !r.only_built.is_empty() {
        println!("{} horas armadas sin vela de 1h de IG", r.only_built.len());
    }
    Ok(())
}

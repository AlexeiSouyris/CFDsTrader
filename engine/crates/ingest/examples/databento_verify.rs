//! Verificación del histórico de futuros de Databento contra las diarias de IG (ADR 0011, #248).
//! No usa IG ni Databento: lee lo que ya está guardado en PostgreSQL.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example databento_verify -- [INSTRUMENTO] [desde] [hasta]
//! ```
//! Por omisión, DE40 y todo lo guardado. Arma la diaria del futuro dentro de cada diaria cerrada
//! de IG y muestra cuántos días se comparan, la correlación de los retornos diarios (sin los
//! saltos de vencimiento) y la base (cierre IG − cierre futuro): media y desvío, en total y por
//! contrato. También cuenta las velas de 15m y 1h que se arman y las que mezclarían contratos.

use cfds_core::Timeframe;
use cfds_market_data::{compare_daily, futures_candles};
use cfds_storage::PgStore;
use chrono::{NaiveDate, Utc};

const SOURCE: &str = "databento";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let id = args.first().map_or("DE40", String::as_str).parse()?;
    let now = Utc::now();
    let from: NaiveDate = args.get(1).map_or("2000-01-01", String::as_str).parse()?;
    let to: NaiveDate = match args.get(2) {
        Some(t) => t.parse()?,
        None => now.date_naive().succ_opt().ok_or("fecha")?,
    };
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    let ig: Vec<_> = store
        .daily_candles(&id, from, to)
        .await?
        .into_iter()
        .filter(|d| d.is_closed_at(now))
        .collect();
    let (Some(first), Some(last)) = (ig.first(), ig.last()) else {
        println!("{id}: sin diarias de IG guardadas entre {from} y {to}");
        return Ok(());
    };
    let bars = store
        .futures_bars(SOURCE, &id, first.start, last.end)
        .await?;
    println!(
        "{id}: {} diarias de IG ({} → {}) y {} barras de 1 minuto de {SOURCE}",
        ig.len(),
        first.day,
        last.day,
        bars.len()
    );
    if bars.is_empty() {
        println!("sin barras del futuro: bajarlas con `python -m cfds_intel.databento --run`");
        return Ok(());
    }
    let f = futures_candles(&bars, &id, now)?;
    for (tf, n) in [(Timeframe::M15, f.m15.len()), (Timeframe::H1, f.h1.len())] {
        let mixed = f.mixed.iter().filter(|(t, _)| *t == tf).count();
        println!("velas de {tf}: {n} armadas, {mixed} no (mezclarían dos contratos)");
    }
    let r = compare_daily(&ig, &bars);
    let rolled = r.days.iter().filter(|d| d.rolled).count();
    println!(
        "días comparados: {} ({} con vencimiento); días de IG sin futuro: {}",
        r.days.len(),
        rolled,
        r.missing.len()
    );
    let show = |x: Option<rust_decimal::Decimal>, dp: u32| {
        x.map_or("sin dato".to_string(), |v| v.round_dp(dp).to_string())
    };
    println!(
        "retornos diarios: {} pares, correlación {}",
        r.return_pairs,
        show(r.correlation, 4)
    );
    println!(
        "base (cierre IG − cierre futuro): media {}, desvío {}",
        show(r.basis_mean, 2),
        show(r.basis_std_dev, 2)
    );
    for c in &r.by_contract {
        println!(
            "  contrato {}: {} días, base media {}, desvío {}",
            c.contract_id,
            c.days,
            c.mean.round_dp(2),
            show(c.std_dev, 2)
        );
    }
    if let Some(d) = r.missing.first() {
        println!("primer día de IG sin futuro: {d}");
    }
    Ok(())
}

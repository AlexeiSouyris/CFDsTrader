//! Arma y guarda las velas de 15m y 1h de futuros desde las barras de 1 minuto ya bajadas de
//! Databento (ADR 0011, #259). No usa IG ni Databento: solo PostgreSQL.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-ingest --example futures_candles
//! ```
//! - Recorre mes por mes lo bajado de cada instrumento del catálogo (`futures_bar_loads`). Una
//!   ventana se arma solo si cerró **dentro de lo bajado** (y antes de ahora): así una ventana a
//!   medio bajar nunca se guarda incompleta.
//! - Lee solo las barras del dataset y símbolo de cada instrumento en `config/databento.toml`
//!   (#376): una fila ajena con el mismo instrumento no se mezcla.
//! - Ninguna vela mezcla contratos: si en la ventana hubo un vencimiento, no se arma y se avisa.
//! - Las guarda en `futures_candles`, aparte de las velas de IG (`candles`).
//! - Es idempotente: lo ya guardado e idéntico cuenta como "ya estaban". Una vela que diera
//!   distinto es repintado: frena con error y no guarda nada de ese mes.

use cfds_core::InstrumentId;
use cfds_market_data::{Catalog, FuturesFeed, FuturesFeeds, futures_candles};
use cfds_storage::PgStore;
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};

const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const DATABENTO: &str = include_str!("../../../../config/databento.toml");
const SOURCE: &str = "databento";

fn month_start(t: DateTime<Utc>) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(t.year(), t.month(), 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap_or_default(),
    )
}

fn next_month(t: DateTime<Utc>) -> DateTime<Utc> {
    let (y, m) = if t.month() == 12 {
        (t.year() + 1, 1)
    } else {
        (t.year(), t.month() + 1)
    };
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(y, m, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap_or_default(),
    )
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = Catalog::from_toml(CATALOG)?;
    let feeds = FuturesFeeds::from_toml(DATABENTO)?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    store.migrate().await?;
    let now = Utc::now();
    let mut failed = 0;
    for id in catalog.instruments.keys() {
        let Some(feed) = feeds.get(id) else {
            println!("{id}: sin futuro en config/databento.toml");
            continue;
        };
        // Un instrumento con datos que no cumplen las reglas no frena a los demás.
        if let Err(e) = build(&store, id, feed, now).await {
            failed += 1;
            eprintln!("⚠️  {id}: {e}");
        }
    }
    if failed > 0 {
        return Err(format!("{failed} instrumentos con errores").into());
    }
    Ok(())
}

async fn build(
    store: &PgStore,
    id: &InstrumentId,
    feed: &FuturesFeed,
    now: DateTime<Utc>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some((from, to)) = store
        .futures_loaded_range(SOURCE, &feed.dataset, &feed.symbol, id)
        .await?
    else {
        println!("{id}: sin barras de {SOURCE} bajadas");
        return Ok(());
    };
    let until = to.min(now);
    let (mut inserted, mut unchanged, mut m15, mut h1, mut mixed) = (0, 0, 0, 0, 0);
    let mut month = month_start(from);
    while month < until {
        let end = next_month(month).min(until);
        let bars = store
            .futures_bars(SOURCE, &feed.dataset, &feed.symbol, id, month, end)
            .await?;
        if !bars.is_empty() {
            let f = futures_candles(&bars, id, end)?;
            for (tf, t) in &f.mixed {
                eprintln!("⚠️  {id} {tf} {t}: vencimiento dentro de la ventana, no se arma");
            }
            let all: Vec<_> = f.m15.iter().chain(&f.h1).cloned().collect();
            let r = store
                .insert_futures_candles(SOURCE, &feed.dataset, &feed.symbol, &all)
                .await?;
            (inserted, unchanged) = (inserted + r.inserted, unchanged + r.unchanged);
            (m15, h1, mixed) = (m15 + f.m15.len(), h1 + f.h1.len(), mixed + f.mixed.len());
        }
        month = next_month(month);
    }
    println!(
        "{id}: {from} → {until}: {m15} velas de 15m y {h1} de 1h → {inserted} nuevas, \
         {unchanged} ya estaban; {mixed} ventanas no se armaron por un vencimiento"
    );
    Ok(())
}

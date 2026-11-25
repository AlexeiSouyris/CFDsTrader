//! Programa 4 (#561), decisión 14 del dueño: **conteo** de señales por año de H4.1 y H4.2 en la
//! validación concatenada del walk-forward, antes de congelar. Sin P/L, sin aciertos, sin
//! ninguna operación: no llama al riesgo ni a la ejecución. Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example programa_4_conteo -- \
//!     US100 --escenario-financiacion EUR=<archivo.csv>
//! ```

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/program4.rs"]
mod program4;
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use std::collections::BTreeMap;

use cfds_backtest::research::ResearchConfig;
use cfds_core::InstrumentId;
use cfds_market_data::{CandleSources, SplitConfig};
use cfds_storage::PgStore;
use chrono::{DateTime, Datelike, Utc};
use research_round::load;
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let from: DateTime<Utc> = "2011-01-01T00:00:00Z".parse()?;
    let cfg = ResearchConfig::from_toml(cfg!("research.toml"))?.program4;
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let id: InstrumentId = args
        .first()
        .ok_or("uso: US100 --escenario-financiacion …")?
        .parse()?;
    let l = load(
        &id,
        &args,
        from,
        Decimal::from(100_000),
        &sources,
        &store,
        &split_cfg,
    )
    .await?;
    let p = &l.p;
    let session = p.calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let (lo, hi) = (
        l.split.windows[0].validation.start,
        l.split.out_of_sample.start,
    );
    let inputs = program4::inputs(p, session, &cfg, &store, hi).await?;
    let plans = program4::plans(p, session, &inputs, &cfg)?;
    println!("══ conteo de señales (decisión en [{lo}, {hi})), sin resultados");
    for (name, ps) in ["H4.1", "H4.2"].iter().zip(&plans) {
        let mut by_year: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
        for x in ps {
            let at = p.m15[x.signal_index].end();
            if at < lo || at >= hi {
                continue;
            }
            let e = by_year.entry(at.year()).or_default();
            match x.side {
                cfds_strategy::Side::Long => e.0 += 1,
                cfds_strategy::Side::Short => e.1 += 1,
            }
        }
        let total: usize = by_year.values().map(|(a, b)| a + b).sum();
        println!("{name}: {total} señales en total");
        for (y, (a, b)) in &by_year {
            println!("  {y}: {} ({a} largas, {b} cortas)", a + b);
        }
        if total < cfg.min_trades {
            println!(
                "  ⚠️  {name}: menos de {} señales: NO se congela (decisión del dueño)",
                cfg.min_trades
            );
        }
    }
    Ok(())
}

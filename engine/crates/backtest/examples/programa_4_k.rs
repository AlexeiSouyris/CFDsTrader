//! Programa 4 (#561), §5.1 del pre-registro: calibración de k de H4.2. Imprime **solo** q (la
//! proporción de velas de 15m de la sesión de contado con cuerpo ≥ 15 puntos en el período de
//! referencia) y k (el cuantil 1 − q de cuerpo / ATR, redondeado). No calcula señales,
//! estructuras, operaciones ni resultados. Estudio: no guarda nada en la base.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
//! cargo run --release -p cfds-backtest --example programa_4_k -- \
//!     US100 --escenario-financiacion EUR=<archivo.csv>
//! ```
//! El escenario de financiación solo lo pide la preparación común; k no lo usa.

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/study.rs"]
mod study;

use cfds_backtest::research::ResearchConfig;
use cfds_backtest::research::program4::{body_atr_sample, equivalent_k, segment_atr};
use cfds_core::InstrumentId;
use cfds_market_data::CandleSource;
use chrono::{DateTime, Days, Utc};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (study, positional) = study::parse(std::env::args().skip(1).collect())?;
    let id: InstrumentId = positional
        .first()
        .ok_or("uso: US100 --escenario-financiacion <ETIQUETA>=<csv>")?
        .parse()?;
    let cfg = ResearchConfig::from_toml(cfg!("research.toml"))?.program4;
    let utc = |d: chrono::NaiveDate| -> Result<DateTime<Utc>, String> {
        d.and_hms_opt(0, 0, 0)
            .map(|t| t.and_utc())
            .ok_or_else(|| "fecha".to_owned())
    };
    let (from, to) = (utc(cfg.k_reference.0)?, utc(cfg.k_reference.1)?);
    // 60 días antes para el calentamiento del ATR y el calendario de vencimientos.
    let load_from = from.checked_sub_days(Days::new(60)).ok_or("fecha")?;
    let p = prepare::prepare_with(
        &id,
        CandleSource::Futures,
        (load_from, to),
        Decimal::from(100_000),
        None,
        true,
        study,
    )
    .await?;
    if p.m15.last().is_some_and(|c| c.start >= to) {
        return Err("se cargó una vela posterior al período de referencia".into());
    }
    let session = p.calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let rolls = p
        .setup
        .futures_basis
        .as_ref()
        .map(|b| b.rolls.clone())
        .ok_or("sin vencimientos")?;
    let atr = segment_atr(&p.m15, &rolls, cfg.atr_period);
    let sample = body_atr_sample(&p.m15, session, &atr, (from, to));
    let (q, k) = equivalent_k(&sample, cfg.body_points, cfg.k_step).ok_or("sin muestra")?;
    println!("══ calibración de k (§5.1), {id}, velas de sesión en [{from}, {to})");
    println!("velas en la muestra: {}", sample.len());
    println!(
        "q (cuerpo ≥ {} puntos) = {}",
        cfg.body_points,
        q.round_dp(6)
    );
    println!(
        "k (cuantil 1 − q de cuerpo / ATR, al {} más cercano) = {k}",
        cfg.k_step
    );
    Ok(())
}

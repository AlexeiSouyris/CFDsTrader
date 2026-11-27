//! Fuente de velas de la liquidez (#259), común a `liquidity_levels` y `liquidity_stats`.
//!
//! `--fuente ig` (por omisión) usa las velas del CFD de IG. `--fuente futuros` usa las armadas
//! desde los futuros de Databento (`futures_candles`) y guarda todo bajo el instrumento derivado
//! de `config/candle_sources.toml` (por ejemplo `US500.FUT`), que se registra en `instruments`.
//! `--fuente ibkr` (#630) usa las barras de IBKR (`ibkr_bars`) de los instrumentos con mapeo en
//! `[ibkr.instruments]` (por ejemplo US100 → MNQ) y guarda bajo `US100.IBKR`; los demás se saltean.
//! El cálculo es el mismo: solo cambian las velas de entrada y dónde queda el resultado.

use cfds_core::InstrumentId;
use cfds_market_data::{CandleSource, CandleSources, InstrumentSpec};
use cfds_storage::{CandleFeed, IbkrFeed, PgStore};

pub const SOURCES: &str = include_str!("../../../../../config/candle_sources.toml");

/// `--fuente ig|futuros|ibkr` de la línea de comandos (por omisión, `ig`).
pub fn source_from_args() -> Result<CandleSource, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().position(|a| a == "--fuente") {
        None => Ok(CandleSource::Ig),
        Some(i) => args
            .get(i + 1)
            .and_then(|s| CandleSource::parse(s))
            .ok_or_else(|| "uso: [--fuente ig|futuros|ibkr]".to_string()),
    }
}

/// De qué tabla leer y bajo qué instrumento guardar. `None`: esa fuente no tiene velas de `base`
/// (IBKR sin mapeo).
pub fn feed<'a>(
    sources: &'a CandleSources,
    source: CandleSource,
    base: &InstrumentId,
) -> Result<Option<(CandleFeed<'a>, InstrumentId)>, String> {
    let out = sources.instrument_for(source, base)?;
    let feed = match source {
        CandleSource::Ig => CandleFeed::Ig,
        CandleSource::Futures => CandleFeed::Futures(&sources.futures_source),
        CandleSource::Ibkr => match sources.ibkr_instrument(base) {
            Some(instrument) => CandleFeed::Ibkr(IbkrFeed {
                source: &sources.ibkr.source,
                instrument,
                what_to_show: &sources.ibkr.what_to_show,
            }),
            None => return Ok(None),
        },
    };
    Ok(Some((feed, out)))
}

/// Antes de guardar con futuros o IBKR: registra el instrumento derivado en `instruments`, que las
/// tablas de liquidez referencian. Con IG no hace nada.
pub async fn register(
    store: &PgStore,
    sources: &CandleSources,
    source: CandleSource,
    spec: &InstrumentSpec,
) -> Result<(), Box<dyn std::error::Error>> {
    if source != CandleSource::Ig {
        store
            .upsert_instrument(&sources.derived_instrument(source, &spec.instrument)?)
            .await?;
    }
    Ok(())
}

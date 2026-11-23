//! Filtro de noticias de alto impacto (spec §7.2) con el calendario guardado (#243), común a
//! `backtest_run` y `chart_export`.
//!
//! Carga de `economic_events` los eventos que pueden tocar alguna vela (hora dentro del período
//! más la ventana, publicados hasta el último cierre) y las lecturas de la ingesta, y devuelve
//! las entradas permitidas. Avisa cuántas velas bloquea y qué tramos no tienen calendario: ahí
//! no se sabe si había un evento y el filtro no bloquea.

use cfds_core::Candle;
use cfds_market_data::{NewsFilter, NewsFilterConfig};
use cfds_storage::PgStore;

/// Entradas de `entries` que además pasan el filtro de noticias.
pub async fn apply(
    store: &PgStore,
    m15: &[Candle],
    entries: &[bool],
    cfg: &NewsFilterConfig,
) -> Result<Vec<bool>, Box<dyn std::error::Error>> {
    let (Some(first), Some(last)) = (m15.first(), m15.last()) else {
        return Ok(entries.to_vec());
    };
    let until = last.end();
    let events = store
        .economic_events(first.end() - cfg.window, until + cfg.window, until)
        .await?;
    let sightings = store.economic_event_sightings(until).await?;
    let f = NewsFilter::new(m15, &events, &sightings, cfg);
    let uncovered = f.uncovered();
    if uncovered == m15.len() {
        eprintln!(
            "⚠️  filtro de noticias de alto impacto (§7.2) SIN DATOS: no hay calendario guardado \
             para ninguna vela del período (`python -m cfds_intel.calendar`, ADR 0010; no hay \
             histórico). El resultado no lo descuenta."
        );
    } else {
        eprintln!(
            "filtro de noticias (§7.2): {} eventos del calendario en el período; {} de {} velas \
             no dejan abrir",
            events.len(),
            f.blocked(),
            m15.len()
        );
        if uncovered > 0 {
            eprintln!(
                "⚠️  {uncovered} velas sin calendario (la ingesta no había empezado o estuvo \
                 parada más de {} días): ahí el filtro no descuenta nada. Tramos:",
                cfg.calendar_stale_after.num_days()
            );
            for (from, to) in f.uncovered_spans(m15) {
                eprintln!("    {from} → {to}");
            }
        }
    }
    Ok(entries
        .iter()
        .zip(&f.allowed)
        .map(|(e, n)| *e && *n)
        .collect())
}

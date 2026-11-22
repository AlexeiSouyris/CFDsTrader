//! Ingesta de velas (#28): el único camino por el que una vela entra al sistema.
//!
//! ```text
//! proveedor → solo velas cerradas → validación (#5, calendario #9) → PostgreSQL → Redis (#6)
//! ```
//!
//! Reglas:
//! - Las velas todavía abiertas se descartan: nunca se guardan ni se publican.
//! - Si la validación encuentra **cualquier** problema, no se guarda nada del lote y se
//!   devuelven los problemas. Nada se corrige en silencio.
//! - Se publica en Redis solo si se pide (modo en vivo), y siempre **después** de guardar.

pub mod daily_indicators;
pub mod gaps;

pub use daily_indicators::daily_indicators;
pub use gaps::{GapPlan, daily_gaps, intraday_gaps, missing_runs, missing_runs_by, session_days};

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{
    Issue, MarketDataProvider, ProviderError, SessionCalendar, ValidationConfig, validate_series,
};
use cfds_storage::{CandlePublisher, PgStore, Published, StorageError};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestRequest {
    pub instrument: InstrumentId,
    pub timeframe: Timeframe,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestReport {
    pub fetched: usize,
    pub skipped_open: usize,
    pub inserted: usize,
    pub unchanged: usize,
    pub published: usize,
    pub already_published: usize,
    /// Velas guardadas pero no publicadas porque llegaron después de otras más nuevas
    /// (por ejemplo, al rellenar un hueco). Quedan en la base; no van al stream en vivo.
    pub late_not_published: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("proveedor: {0}")]
    Provider(#[from] ProviderError),
    #[error("el proveedor devolvió una vela que no se pidió ({instrument} {timeframe} {start})")]
    Unexpected {
        instrument: InstrumentId,
        timeframe: Timeframe,
        start: DateTime<Utc>,
    },
    #[error("una vela abierta llegó antes que velas cerradas en {start}: respuesta desordenada")]
    OpenCandleNotLast { start: DateTime<Utc> },
    #[error("lote rechazado: {} problema(s) de datos; no se guardó nada", .0.len())]
    Rejected(Vec<Issue>),
    #[error("almacenamiento: {0}")]
    Storage(#[from] StorageError),
}

/// Paso puro (sin red ni base): controla lo que devolvió el proveedor, descarta la vela
/// abierta y valida. Devuelve las velas cerradas listas para guardar y cuántas se descartaron.
pub fn prepare(
    req: &IngestRequest,
    candles: Vec<Candle>,
    calendar: &impl SessionCalendar,
    cfg: &ValidationConfig,
    now: DateTime<Utc>,
) -> Result<(Vec<Candle>, usize), IngestError> {
    if let Some(c) = candles.iter().find(|c| {
        c.instrument_id != req.instrument
            || c.timeframe != req.timeframe
            || c.start < req.from
            || c.start >= req.to
    }) {
        return Err(IngestError::Unexpected {
            instrument: c.instrument_id.clone(),
            timeframe: c.timeframe,
            start: c.start,
        });
    }
    // Solo la última vela puede estar abierta. Una abierta en el medio es una respuesta rota.
    let closed_count = candles.iter().take_while(|c| c.is_closed_at(now)).count();
    if let Some(c) = candles[closed_count..].iter().find(|c| c.is_closed_at(now)) {
        return Err(IngestError::OpenCandleNotLast { start: c.start });
    }
    let skipped_open = candles.len() - closed_count;
    let mut closed = candles;
    closed.truncate(closed_count);

    let issues = validate_series(&closed, calendar, cfg);
    if !issues.is_empty() {
        return Err(IngestError::Rejected(issues));
    }
    Ok((closed, skipped_open))
}

/// Trae, valida, guarda y (si `publisher` es `Some`) publica las velas de `req`.
pub async fn ingest(
    provider: &impl MarketDataProvider,
    calendar: &impl SessionCalendar,
    cfg: &ValidationConfig,
    store: &PgStore,
    publisher: Option<&mut CandlePublisher>,
    req: &IngestRequest,
    now: DateTime<Utc>,
) -> Result<IngestReport, IngestError> {
    let candles = provider
        .history(&req.instrument, req.timeframe, req.from, req.to)
        .await?;
    let fetched = candles.len();
    let (closed, skipped_open) = prepare(req, candles, calendar, cfg, now)?;

    let stored = store.insert_candles(&closed).await?;
    let mut report = IngestReport {
        fetched,
        skipped_open,
        inserted: stored.inserted,
        unchanged: stored.unchanged,
        ..Default::default()
    };

    if let Some(p) = publisher {
        for c in &closed {
            match p.publish(c, now).await {
                Ok(Published::New { .. }) => report.published += 1,
                Ok(Published::AlreadyPublished { .. }) => report.already_published += 1,
                Err(StorageError::OutOfOrder { .. }) => report.late_not_published += 1,
                Err(e) => return Err(e.into()),
            }
        }
    }
    Ok(report)
}

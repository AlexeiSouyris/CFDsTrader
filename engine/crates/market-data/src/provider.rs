//! Proveedores de datos de mercado (#28). Cada broker o fuente implementa
//! [`MarketDataProvider`]; el resto del sistema no sabe de dónde vienen las velas.
//! IG lo implementa en el #8.

use std::future::Future;

use cfds_core::{Candle, InstrumentId, Timeframe};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("credenciales rechazadas por el proveedor")]
    Unauthorized,
    #[error("límite de uso alcanzado; reintentar después de {retry_after_secs:?} s")]
    RateLimited { retry_after_secs: Option<u64> },
    #[error("el proveedor no conoce {0}")]
    NotFound(String),
    #[error("error de red o del proveedor: {0}")]
    Transport(String),
    #[error("respuesta inesperada del proveedor: {0}")]
    InvalidResponse(String),
}

/// Fuente de velas históricas.
pub trait MarketDataProvider {
    /// Velas de `instrument` y `timeframe` con inicio en `[from, to)`, en orden.
    /// Puede incluir la vela todavía abierta: decide la ingesta, nunca el proveedor.
    fn history(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Candle>, ProviderError>> + Send;
}

/// Proveedor en memoria: devuelve las velas que se le cargan. Para tests y para cargar
/// datos ya descargados, sin red.
#[derive(Debug, Clone, Default)]
pub struct StaticProvider {
    candles: Vec<Candle>,
}

impl StaticProvider {
    pub fn new(candles: Vec<Candle>) -> Self {
        Self { candles }
    }
}

impl MarketDataProvider for StaticProvider {
    async fn history(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, ProviderError> {
        // Se devuelven en el orden en que se cargaron: si vienen desordenadas,
        // la validación tiene que detectarlo (no se ordenan en silencio).
        Ok(self
            .candles
            .iter()
            .filter(|c| &c.instrument_id == instrument && c.timeframe == timeframe)
            .filter(|c| from <= c.start && c.start < to)
            .cloned()
            .collect())
    }
}

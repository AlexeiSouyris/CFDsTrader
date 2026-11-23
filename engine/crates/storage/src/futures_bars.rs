//! Lectura de las barras de 1 minuto de futuros (`futures_bars_1m`, `futures_bar.v1`, #248).
//! Las escribe Python (`cfds_intel.databento`, #247); el motor solo las lee para agruparlas y
//! verificarlas contra IG.

use cfds_core::{FuturesBar, InstrumentId};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{PgStore, StorageError};

/// Fila: fuente, dataset, símbolo, contrato, inicio, OHLC y volumen.
type BarRow = (
    String,
    String,
    String,
    i64,
    DateTime<Utc>,
    Decimal,
    Decimal,
    Decimal,
    Decimal,
    i64,
);

impl PgStore {
    /// Barras de `source` del futuro que corresponde a `instrument`, con inicio en `[from, to)`,
    /// en orden.
    pub async fn futures_bars(
        &self,
        source: &str,
        instrument: &InstrumentId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<FuturesBar>, StorageError> {
        let rows: Vec<BarRow> = sqlx::query_as(
            "SELECT source, dataset, symbol, contract_id, start_at, open, high, low, close, volume
             FROM futures_bars_1m
             WHERE source = $1 AND instrument_id = $2 AND start_at >= $3 AND start_at < $4
             ORDER BY start_at",
        )
        .bind(source)
        .bind(instrument.as_str())
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;
        let corrupt = |what: &str| StorageError::CorruptRow(format!("futures_bars_1m: {what}"));
        let mut out = Vec::with_capacity(rows.len());
        for (source, dataset, symbol, contract, start, open, high, low, close, volume) in rows {
            if out
                .last()
                .is_some_and(|prev: &FuturesBar| prev.start == start)
            {
                return Err(corrupt("dos futuros para el mismo instrumento y minuto"));
            }
            out.push(FuturesBar {
                source,
                dataset,
                symbol,
                instrument_id: instrument.clone(),
                contract_id: u64::try_from(contract).map_err(|_| corrupt("contract_id"))?,
                start,
                open,
                high,
                low,
                close,
                volume: u64::try_from(volume).map_err(|_| corrupt("volume"))?,
            });
        }
        Ok(out)
    }
}

//! Velas cerradas de IBKR como fuente de datos (`ibkr_bars`, `ibkr_bar.v1`, #536), aparte de las
//! de IG y de Databento. Igual que [`PgStore::insert_candles`]: una vela ya guardada e idéntica
//! cuenta como `unchanged`; con otros valores es un conflicto (sería repintado) y no se guarda
//! nada del lote.
//!
//! Lecturas para el gateway (#551): solo velas **cerradas** a un instante (`start + tamaño <=
//! at`), de una sola serie (un contrato, un tamaño y un precio): nunca se mezclan contratos.
//!
//! Correcciones (#563, #575): `ibkr_bar_corrections` (de solo agregar) guarda el OHLC correcto de
//! velas mal guardadas. **Todas las lecturas** de este módulo devuelven la vela con su corrección
//! más reciente, si tiene; `insert_ibkr_bars` sigue comparando con lo guardado en `ibkr_bars`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{InsertReport, PgStore, StorageError};

/// Columnas de [`IbkrBarRow`], en orden, con la corrección más reciente si hay (ver
/// `from_bars!`). Una corrección sin volumen deja el volumen en NULL (no se conoce).
macro_rules! columns {
    () => {
        "b.source, b.instrument_id, b.kind, b.contract, b.bar_size, b.what_to_show, b.start_at, \
         COALESCE(c.c_open, b.open), COALESCE(c.c_high, b.high), COALESCE(c.c_low, b.low), \
         COALESCE(c.c_close, b.close), \
         CASE WHEN c.c_open IS NULL THEN b.volume ELSE c.c_volume END"
    };
}

/// `ibkr_bars` (alias `b`) con la corrección más reciente de cada vela (alias `c`, columnas
/// `c_…`; todo NULL si no tiene).
macro_rules! from_bars {
    () => {
        " FROM ibkr_bars b LEFT JOIN LATERAL (
             SELECT x.open AS c_open, x.high AS c_high, x.low AS c_low, x.close AS c_close,
                    x.volume AS c_volume
             FROM ibkr_bar_corrections x
             WHERE x.source = b.source AND x.instrument_id = b.instrument_id
               AND x.contract = b.contract AND x.bar_size = b.bar_size
               AND x.what_to_show = b.what_to_show AND x.start_at = b.start_at
             ORDER BY x.corrected_at DESC, x.id DESC
             LIMIT 1
         ) c ON true "
    };
}

/// Cierre de la vela (inicio + tamaño).
macro_rules! end_at {
    () => {
        "(start_at + CASE bar_size WHEN '1m' THEN interval '1 minute' \
         WHEN '15m' THEN interval '15 minutes' ELSE interval '1 hour' END)"
    };
}

/// Orden de los tamaños, del más chico al más grande.
macro_rules! size_rank {
    () => {
        "CASE bar_size WHEN '1m' THEN 1 WHEN '15m' THEN 2 ELSE 3 END"
    };
}

/// Una fila de `ibkr_bars`. Los textos ya validados por quien la arma (la base los vuelve a
/// revisar con sus CHECK).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrBarRow {
    pub source: String,
    pub instrument_id: String,
    pub kind: String,
    pub contract: String,
    pub bar_size: String,
    pub what_to_show: String,
    pub start: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
}

/// Una corrección de una vela ya guardada (`ibkr_bar_corrections`, #563): la clave de la vela,
/// el OHLC correcto, el motivo y de dónde sale. `volume` `None`: no se conoce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrBarCorrection {
    pub source: String,
    pub instrument_id: String,
    pub contract: String,
    pub bar_size: String,
    pub what_to_show: String,
    pub start: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
    pub reason: String,
    pub correction_source: String,
}

/// Qué serie: todas las velas de un contrato, un tamaño y un precio de una fuente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrSeries<'a> {
    pub source: &'a str,
    pub instrument_id: &'a str,
    pub contract: &'a str,
    pub bar_size: &'a str,
    pub what_to_show: &'a str,
}

/// Una serie con sus textos propios (para listar las series guardadas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrSeriesKey {
    pub source: String,
    pub instrument_id: String,
    pub contract: String,
    pub bar_size: String,
    pub what_to_show: String,
}

impl IbkrSeriesKey {
    pub fn as_series(&self) -> IbkrSeries<'_> {
        IbkrSeries {
            source: &self.source,
            instrument_id: &self.instrument_id,
            contract: &self.contract,
            bar_size: &self.bar_size,
            what_to_show: &self.what_to_show,
        }
    }
}

type Stored = (
    String,
    DateTime<Utc>,
    Decimal,
    Decimal,
    Decimal,
    Decimal,
    Option<Decimal>,
);

impl PgStore {
    /// Guarda velas de IBKR en una sola transacción: o entran todas, o ninguna.
    pub async fn insert_ibkr_bars(
        &self,
        rows: &[IbkrBarRow],
    ) -> Result<InsertReport, StorageError> {
        let mut tx = self.pool().begin().await?;
        let mut report = InsertReport::default();
        for r in rows {
            let inserted = sqlx::query(
                "INSERT INTO ibkr_bars (source, instrument_id, kind, contract, bar_size,
                     what_to_show, start_at, open, high, low, close, volume)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                 ON CONFLICT ON CONSTRAINT ibkr_bars_key DO NOTHING",
            )
            .bind(&r.source)
            .bind(&r.instrument_id)
            .bind(&r.kind)
            .bind(&r.contract)
            .bind(&r.bar_size)
            .bind(&r.what_to_show)
            .bind(r.start)
            .bind(r.open)
            .bind(r.high)
            .bind(r.low)
            .bind(r.close)
            .bind(r.volume)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                == 1;
            if inserted {
                report.inserted += 1;
                continue;
            }
            let stored: Stored = sqlx::query_as(
                "SELECT kind, start_at, open, high, low, close, volume FROM ibkr_bars
                 WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
                   AND what_to_show = $5 AND start_at = $6",
            )
            .bind(&r.source)
            .bind(&r.instrument_id)
            .bind(&r.contract)
            .bind(&r.bar_size)
            .bind(&r.what_to_show)
            .bind(r.start)
            .fetch_one(&mut *tx)
            .await?;
            let mine = (
                r.kind.clone(),
                r.start,
                r.open,
                r.high,
                r.low,
                r.close,
                r.volume,
            );
            if stored == mine {
                report.unchanged += 1;
            } else {
                return Err(StorageError::Conflict {
                    instrument: format!("{} ({})", r.instrument_id, r.contract),
                    timeframe: r.bar_size.clone(),
                    start: r.start,
                });
            }
        }
        tx.commit().await?;
        Ok(report)
    }

    /// Guarda correcciones en una sola transacción, todas con la misma `corrected_at` (la hora de la
    /// base): o entran todas, o ninguna. De solo agregar: nunca toca `ibkr_bars`. Cada una tiene que
    /// apuntar a una vela guardada (si no, falla la clave foránea y no entra ninguna).
    pub async fn insert_ibkr_bar_corrections(
        &self,
        rows: &[IbkrBarCorrection],
    ) -> Result<u64, StorageError> {
        let mut tx = self.pool().begin().await?;
        let mut inserted = 0;
        for r in rows {
            inserted += sqlx::query(
                "INSERT INTO ibkr_bar_corrections (source, instrument_id, contract, bar_size,
                     what_to_show, start_at, open, high, low, close, volume, reason,
                     correction_source)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            )
            .bind(&r.source)
            .bind(&r.instrument_id)
            .bind(&r.contract)
            .bind(&r.bar_size)
            .bind(&r.what_to_show)
            .bind(r.start)
            .bind(r.open)
            .bind(r.high)
            .bind(r.low)
            .bind(r.close)
            .bind(r.volume)
            .bind(&r.reason)
            .bind(&r.correction_source)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        }
        tx.commit().await?;
        Ok(inserted)
    }

    /// Inicio de la última vela guardada de la serie, si hay alguna (para seguir desde ahí).
    pub async fn last_ibkr_bar_start(
        &self,
        s: &IbkrSeries<'_>,
    ) -> Result<Option<DateTime<Utc>>, StorageError> {
        let last: Option<DateTime<Utc>> = sqlx::query_scalar(
            "SELECT max(start_at) FROM ibkr_bars
             WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
               AND what_to_show = $5",
        )
        .bind(s.source)
        .bind(s.instrument_id)
        .bind(s.contract)
        .bind(s.bar_size)
        .bind(s.what_to_show)
        .fetch_one(self.pool())
        .await?;
        Ok(last)
    }

    /// La última vela **cerrada** a `at` de un instrumento y una fuente, de cualquier contrato y
    /// precio; con `bar_size`, solo de ese tamaño. Entre velas que cierran a la vez gana la más
    /// chica y, después, la cargada más tarde (la del contrato vigente).
    pub async fn latest_closed_ibkr_bar(
        &self,
        source: &str,
        instrument_id: &str,
        bar_size: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, StorageError> {
        let row: Option<Row> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            from_bars!(),
            "WHERE source = $1 AND instrument_id = $2 AND ($3::text IS NULL OR bar_size = $3)
               AND ",
            end_at!(),
            " <= $4
             ORDER BY ",
            end_at!(),
            " DESC, ",
            size_rank!(),
            " ASC, inserted_at DESC
             LIMIT 1"
        ))
        .bind(source)
        .bind(instrument_id)
        .bind(bar_size)
        .bind(at)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(into_row))
    }

    /// Las últimas `limit` velas de la serie **cerradas** a `at`, de la más vieja a la más nueva.
    pub async fn ibkr_bars_closed_by(
        &self,
        s: &IbkrSeries<'_>,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<IbkrBarRow>, StorageError> {
        let mut rows: Vec<Row> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            from_bars!(),
            "WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
               AND what_to_show = $5 AND ",
            end_at!(),
            " <= $6
             ORDER BY start_at DESC
             LIMIT $7"
        ))
        .bind(s.source)
        .bind(s.instrument_id)
        .bind(s.contract)
        .bind(s.bar_size)
        .bind(s.what_to_show)
        .bind(at)
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?;
        rows.reverse();
        Ok(rows.into_iter().map(into_row).collect())
    }

    /// Las velas de la serie que empiezan en `[from, to)`, en orden (#563: revisar una vela de 15m
    /// o 1h contra sus velas de 1m). Solo lectura.
    pub async fn ibkr_bars_between(
        &self,
        s: &IbkrSeries<'_>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<IbkrBarRow>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            from_bars!(),
            "WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
               AND what_to_show = $5 AND start_at >= $6 AND start_at < $7
             ORDER BY start_at"
        ))
        .bind(s.source)
        .bind(s.instrument_id)
        .bind(s.contract)
        .bind(s.bar_size)
        .bind(s.what_to_show)
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(into_row).collect())
    }

    /// Velas de 15m o 1h de un instrumento de IBKR con inicio en `[from, to)`, en orden, como
    /// fuente de la liquidez (#630). Con su corrección más reciente, si tiene.
    ///
    /// Una sola serie empalmada, sin mezclar contratos en una vela y sin repintar: los contratos
    /// van en el orden en que se empezaron a guardar (`min(inserted_at)`; la carga guarda un
    /// vencimiento por vez), y una vela de un contrato se usa solo si empieza después de la última
    /// de todos los contratos anteriores. Así la historia que trae un contrato nuevo nunca
    /// reemplaza ni se mete entre velas ya usadas. El instrumento de las velas es el de IBKR;
    /// quien llama lo cambia por el suyo. Velas en curso: las filtra quien llama
    /// (`is_closed_at`); la carga solo guarda velas completas (#575).
    pub async fn ibkr_candles(
        &self,
        f: &crate::IbkrFeed<'_>,
        timeframe: cfds_core::Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<cfds_core::Candle>, StorageError> {
        let rows: Vec<Row> = sqlx::query_as(concat!(
            "WITH k AS (
                 SELECT contract, min(inserted_at) AS first_in, max(start_at) AS last_start
                 FROM ibkr_bars
                 WHERE source = $1 AND instrument_id = $2 AND what_to_show = $3 AND bar_size = $4
                 GROUP BY contract)
             SELECT ",
            columns!(),
            from_bars!(),
            "JOIN k ON k.contract = b.contract
             WHERE b.source = $1 AND b.instrument_id = $2 AND b.what_to_show = $3
               AND b.bar_size = $4 AND b.start_at >= $5 AND b.start_at < $6
               AND NOT EXISTS (
                 SELECT 1 FROM k AS e
                 WHERE (e.first_in, e.contract) < (k.first_in, k.contract)
                   AND e.last_start >= b.start_at)
             ORDER BY b.start_at"
        ))
        .bind(f.source)
        .bind(f.instrument)
        .bind(f.what_to_show)
        .bind(timeframe.as_str())
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter()
            .map(|r| {
                let r = into_row(r);
                Ok(cfds_core::Candle {
                    instrument_id: r
                        .instrument_id
                        .parse()
                        .map_err(|e| StorageError::CorruptRow(format!("{e}")))?,
                    timeframe,
                    start: r.start,
                    open: r.open,
                    high: r.high,
                    low: r.low,
                    close: r.close,
                    volume: r.volume,
                })
            })
            .collect()
    }

    /// Todas las series guardadas, ordenadas. Solo lectura.
    pub async fn ibkr_series(&self) -> Result<Vec<IbkrSeriesKey>, StorageError> {
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT DISTINCT source, instrument_id, contract, bar_size, what_to_show
             FROM ibkr_bars ORDER BY 1, 2, 3, 4, 5",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(source, instrument_id, contract, bar_size, what_to_show)| IbkrSeriesKey {
                    source,
                    instrument_id,
                    contract,
                    bar_size,
                    what_to_show,
                },
            )
            .collect())
    }

    /// La última vela de la serie que **empieza** antes de `before` (para el cierre de una sesión
    /// anterior: `before` es el primer instante de la sesión actual).
    pub async fn last_ibkr_bar_starting_before(
        &self,
        s: &IbkrSeries<'_>,
        before: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, StorageError> {
        let row: Option<Row> = sqlx::query_as(concat!(
            "SELECT ",
            columns!(),
            from_bars!(),
            "WHERE source = $1 AND instrument_id = $2 AND contract = $3 AND bar_size = $4
               AND what_to_show = $5 AND start_at < $6
             ORDER BY start_at DESC
             LIMIT 1"
        ))
        .bind(s.source)
        .bind(s.instrument_id)
        .bind(s.contract)
        .bind(s.bar_size)
        .bind(s.what_to_show)
        .bind(before)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(into_row))
    }
}

type Row = (
    String,
    String,
    String,
    String,
    String,
    String,
    DateTime<Utc>,
    Decimal,
    Decimal,
    Decimal,
    Decimal,
    Option<Decimal>,
);

fn into_row(r: Row) -> IbkrBarRow {
    IbkrBarRow {
        source: r.0,
        instrument_id: r.1,
        kind: r.2,
        contract: r.3,
        bar_size: r.4,
        what_to_show: r.5,
        start: r.6,
        open: r.7,
        high: r.8,
        low: r.9,
        close: r.10,
        volume: r.11,
    }
}

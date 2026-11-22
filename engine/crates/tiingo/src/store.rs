//! Registro de uso y veredictos en PostgreSQL (migración 0014, tablas de solo agregar). Nunca
//! precios ni números de Tiingo (ADR 0009).

use cfds_core::InstrumentId;
use cfds_storage::PgStore;
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgPool, Row};

use crate::TiingoError;
use crate::budget::Usage;
use crate::verify::DayVerdict;

/// Un pedido a Tiingo. `http_status` es `None` si no hubo respuesta completa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRecord {
    pub requested_at: DateTime<Utc>,
    pub endpoint: &'static str,
    pub symbol: String,
    pub http_status: Option<u16>,
    pub bytes: u64,
}

pub struct TiingoStore {
    pool: PgPool,
}

impl TiingoStore {
    /// Se conecta y aplica las migraciones que falten (como el resto del motor).
    pub async fn connect(database_url: &str) -> Result<Self, TiingoError> {
        let pg = PgStore::connect(database_url).await?;
        pg.migrate().await?;
        Ok(Self {
            pool: pg.pool().clone(),
        })
    }

    /// Uso hacia atrás desde `now`: 60 minutos, 24 horas y 31 días (ventanas móviles).
    pub async fn usage(&self, now: DateTime<Utc>) -> Result<Usage, TiingoError> {
        let row = sqlx::query(
            "SELECT
                count(*) FILTER (WHERE requested_at > $1) AS hour,
                count(*) FILTER (WHERE requested_at > $2) AS day,
                COALESCE(sum(bytes), 0)::BIGINT AS bytes,
                COALESCE(array_agg(DISTINCT symbol), '{}') AS symbols
             FROM tiingo_usage_log WHERE requested_at > $3",
        )
        .bind(now - TimeDelta::hours(1))
        .bind(now - TimeDelta::days(1))
        .bind(now - TimeDelta::days(31))
        .fetch_one(&self.pool)
        .await?;
        let n = |name: &str| -> Result<u64, TiingoError> {
            let v: i64 = row.try_get(name)?;
            Ok(u64::try_from(v).unwrap_or(0))
        };
        let symbols: Vec<String> = row.try_get("symbols")?;
        Ok(Usage {
            requests_last_hour: n("hour")?,
            requests_last_day: n("day")?,
            symbols_last_31_days: symbols.into_iter().collect(),
            bytes_last_31_days: n("bytes")?,
        })
    }

    pub async fn record_usage(&self, r: &UsageRecord) -> Result<(), TiingoError> {
        sqlx::query(
            "INSERT INTO tiingo_usage_log (requested_at, endpoint, symbol, http_status, bytes)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(r.requested_at)
        .bind(r.endpoint)
        .bind(&r.symbol)
        .bind(r.http_status.map(i32::from))
        .bind(i64::try_from(r.bytes).unwrap_or(i64::MAX))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Guarda los veredictos de una corrida (solo el veredicto: opción B), en una transacción.
    pub async fn record_verdicts(
        &self,
        instrument: &InstrumentId,
        etf: &str,
        verified_at: DateTime<Utc>,
        verdicts: &[DayVerdict],
    ) -> Result<(), TiingoError> {
        let mut tx = self.pool.begin().await?;
        for v in verdicts {
            sqlx::query(
                "INSERT INTO tiingo_verifications
                    (verified_at, instrument_id, day, etf, method, verdict)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(verified_at)
            .bind(instrument.as_str())
            .bind(v.day)
            .bind(etf)
            .bind(v.method.as_str())
            .bind(v.verdict.as_str())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

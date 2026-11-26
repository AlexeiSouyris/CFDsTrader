//! Laboratorio de estrategias en PostgreSQL (`infra/migrations/0053_lab.sql`, #774; la curva en
//! vivo de cada variante, `0055_lab_event_curve.sql`, #854): la cola de trabajos (`lab_jobs`), sus eventos (`lab_job_events`), los informes (`lab_reports`) y las
//! variantes probadas (`lab_trials`). Todo de solo agregar: el estado de un trabajo es su último
//! evento y un trabajo tiene un solo final (`done`, `failed` o `cancelled`).
//!
//! Lo usan el gateway (encolar, cancelar, leer) y el `lab_worker` (tomar, avanzar, guardar). El
//! gateway abre la sesión en solo lectura: cada escritura declara su propia transacción de
//! escritura y toca solo estas tablas. Dos escrituras sobre el mismo trabajo se ordenan con el
//! bloqueo de su fila de `lab_jobs` (`FOR UPDATE`, que no la modifica).

use cfds_backtest::lab::{
    JobStage, JobState, JobStep, LabJobEvent, LabJobRequest, LabTrial, job_id,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Postgres, Row, Transaction};

#[derive(Debug, thiserror::Error)]
pub enum LabStoreError {
    #[error("PostgreSQL: {0}")]
    Db(#[from] sqlx::Error),
    #[error("JSON del laboratorio: {0}")]
    Json(#[from] serde_json::Error),
    #[error("fila inválida en las tablas del laboratorio: {0}")]
    Corrupt(String),
    #[error("no existe el trabajo {0}")]
    UnknownJob(String),
}

/// Resultado de encolar: un trabajo nuevo o el que ya existía para el mismo pedido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enqueued {
    New(LabJobEvent),
    /// El mismo pedido ya está en cola, corriendo o listo: es el mismo trabajo (idempotente).
    Existing(LabJobEvent),
}

/// Resultado de agregar un paso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Appended {
    Added(LabJobEvent),
    /// El trabajo ya terminó (por ejemplo, se canceló): no se agrega nada. Trae el final.
    AlreadyEnded(LabJobEvent),
}

/// Un trabajo tomado por el worker: su evento de toma y el pedido guardado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimed {
    pub event: LabJobEvent,
    pub request: LabJobRequest,
}

/// Un informe para guardar con el final de su trabajo.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredReport {
    pub report_id: String,
    pub spec_sha256: String,
    pub family_sha256: String,
    /// `lab_report.v1`.
    pub report: Value,
    pub computed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PgLab {
    pool: PgPool,
}

/// Las columnas de un `lab_job_event.v1` (`e` = `lab_job_events`, `j` = `lab_jobs`).
macro_rules! event_columns {
    () => {
        "e.seq, e.job_id, e.state, e.stage, e.percent, e.variants_done, j.variants_total, \
         e.report_ids, e.error, e.curve::text AS curve, e.recorded_at"
    };
}

/// Texto de un enum del contrato (`"exploratory"`…), sin las comillas del JSON.
fn name<T: serde::Serialize>(v: &T) -> Result<String, LabStoreError> {
    match serde_json::to_value(v)? {
        Value::String(s) => Ok(s),
        other => Err(corrupt(other)),
    }
}

fn corrupt(what: impl std::fmt::Display) -> LabStoreError {
    LabStoreError::Corrupt(what.to_string())
}

fn row_to_event(r: &PgRow) -> Result<LabJobEvent, LabStoreError> {
    let state: String = r.try_get("state")?;
    let stage: Option<String> = r.try_get("stage")?;
    let curve: Option<String> = r.try_get("curve")?;
    let count = |name: &str| -> Result<u32, LabStoreError> {
        u32::try_from(r.try_get::<i32, _>(name)?).map_err(|_| corrupt(name))
    };
    Ok(LabJobEvent {
        job_id: r.try_get("job_id")?,
        seq: r.try_get("seq")?,
        state: JobState::parse(&state).ok_or_else(|| corrupt(&state))?,
        stage: stage
            .map(|s| JobStage::parse(&s).ok_or_else(|| corrupt(&s)))
            .transpose()?,
        percent: u8::try_from(r.try_get::<i16, _>("percent")?).map_err(|_| corrupt("percent"))?,
        variants_done: count("variants_done")?,
        variants_total: count("variants_total")?,
        report_ids: r.try_get("report_ids")?,
        error: r.try_get("error")?,
        curve: curve.as_deref().map(serde_json::from_str).transpose()?,
        recorded_at: r.try_get("recorded_at")?,
    })
}

async fn write_tx(pool: &PgPool) -> Result<Transaction<'static, Postgres>, LabStoreError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION READ WRITE")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// El último evento de `job_id` dentro de `tx` (o fuera, con el pool).
async fn last_in<'e, E>(exec: E, job_id: &str) -> Result<Option<LabJobEvent>, LabStoreError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let sql = concat!(
        "SELECT ",
        event_columns!(),
        " FROM lab_job_events e JOIN lab_jobs j USING (job_id)
         WHERE e.job_id = $1 ORDER BY e.seq DESC LIMIT 1"
    );
    let row = sqlx::query(sql).bind(job_id).fetch_optional(exec).await?;
    row.as_ref().map(row_to_event).transpose()
}

/// Agrega `step` dentro de `tx`, que ya tiene bloqueada la fila del trabajo.
async fn insert_step(
    tx: &mut Transaction<'static, Postgres>,
    job_id: &str,
    step: &JobStep,
    at: DateTime<Utc>,
) -> Result<LabJobEvent, LabStoreError> {
    let none: &[String] = &[];
    let (stage, percent, done, report_ids, error) = match step {
        JobStep::Queued | JobStep::Cancelled => (None, 0_i16, 0_i32, none, None),
        JobStep::Failed { error } => (None, 0, 0, none, Some(error.as_str())),
        JobStep::Running {
            stage,
            percent,
            variants_done,
            ..
        } => (
            Some(stage.as_str()),
            i16::from((*percent).min(100)),
            i32::try_from(*variants_done).unwrap_or(i32::MAX),
            none,
            None,
        ),
        JobStep::Done { report_ids } => (None, 100, 0, report_ids.as_slice(), None),
    };
    let curve = match step {
        JobStep::Running { curve: Some(c), .. } => Some(serde_json::to_string(c)?),
        _ => None,
    };
    // En `done` las variantes hechas son todas; en los demás finales, las del último paso.
    let sql = concat!(
        "WITH ins AS (
           INSERT INTO lab_job_events
             (job_id, state, stage, percent, variants_done, report_ids, error, curve, recorded_at)
           SELECT $1, $2, $3, $4,
                  CASE WHEN $2 = 'done' THEN j.variants_total
                       WHEN $2 = 'running' THEN $5
                       ELSE COALESCE((SELECT variants_done FROM lab_job_events
                                      WHERE job_id = $1 ORDER BY seq DESC LIMIT 1), 0) END,
                  $6, $7, $9::jsonb, $8
           FROM lab_jobs j WHERE j.job_id = $1
           RETURNING *)
         SELECT ",
        event_columns!(),
        " FROM ins e JOIN lab_jobs j USING (job_id)"
    );
    let row = sqlx::query(sql)
        .bind(job_id)
        .bind(step.state().as_str())
        .bind(stage)
        .bind(percent)
        .bind(done)
        .bind(report_ids)
        .bind(error)
        .bind(at)
        .bind(curve)
        .fetch_one(&mut **tx)
        .await?;
    row_to_event(&row)
}

/// Bloquea la fila del trabajo (sin modificarla). `false` si no existe.
async fn lock_job(
    tx: &mut Transaction<'static, Postgres>,
    job_id: &str,
) -> Result<bool, LabStoreError> {
    let row = sqlx::query("SELECT 1 FROM lab_jobs WHERE job_id = $1 FOR UPDATE")
        .bind(job_id)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(row.is_some())
}

impl PgLab {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Encola `request` (ya validado y normalizado). Si el mismo pedido ya tiene un trabajo que
    /// no falló ni se canceló, devuelve ese (idempotente); si no, crea el intento siguiente.
    pub async fn enqueue(
        &self,
        request: &LabJobRequest,
        request_sha256: &str,
        family_sha256: &str,
        now: DateTime<Utc>,
    ) -> Result<Enqueued, LabStoreError> {
        let mut tx = write_tx(&self.pool).await?;
        // Dos pedidos iguales a la vez: el segundo espera y ve el trabajo del primero.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('lab_jobs:' || $1))")
            .bind(request_sha256)
            .execute(&mut *tx)
            .await?;
        let latest = sqlx::query(
            "SELECT job_id, attempt FROM lab_jobs WHERE request_sha256 = $1
             ORDER BY attempt DESC LIMIT 1",
        )
        .bind(request_sha256)
        .fetch_optional(&mut *tx)
        .await?;
        let mut attempt = 1;
        if let Some(r) = latest {
            let id: String = r.try_get("job_id")?;
            let last = last_in(&mut *tx, &id)
                .await?
                .ok_or_else(|| corrupt(format!("trabajo {id} sin eventos")))?;
            if !matches!(last.state, JobState::Failed | JobState::Cancelled) {
                tx.commit().await?;
                return Ok(Enqueued::Existing(last));
            }
            attempt = r.try_get::<i32, _>("attempt")? + 1;
        }
        let id = job_id(
            request_sha256,
            u32::try_from(attempt).map_err(|_| corrupt("attempt"))?,
        );
        sqlx::query(
            "INSERT INTO lab_jobs
               (job_id, request_sha256, attempt, family_sha256, request, variants_total, created_at)
             VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7)",
        )
        .bind(&id)
        .bind(request_sha256)
        .bind(attempt)
        .bind(family_sha256)
        .bind(serde_json::to_string(request)?)
        .bind(i32::try_from(request.specs.len()).map_err(|_| corrupt("variants_total"))?)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let event = insert_step(&mut tx, &id, &JobStep::Queued, now).await?;
        tx.commit().await?;
        Ok(Enqueued::New(event))
    }

    /// Toma el trabajo en cola más viejo (con `started`). `None` si no hay. Un trabajo se toma
    /// una sola vez (índice único); dos workers a la vez se saltean (`SKIP LOCKED`).
    pub async fn claim_next(&self, now: DateTime<Utc>) -> Result<Option<Claimed>, LabStoreError> {
        let mut tx = write_tx(&self.pool).await?;
        let row = sqlx::query(
            "SELECT j.job_id, j.request::text AS request FROM lab_jobs j
             WHERE NOT EXISTS (SELECT 1 FROM lab_job_events e
                               WHERE e.job_id = j.job_id AND e.state <> 'queued')
             ORDER BY j.created_at, j.job_id LIMIT 1
             FOR UPDATE OF j SKIP LOCKED",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let id: String = row.try_get("job_id")?;
        let request: LabJobRequest = serde_json::from_str(&row.try_get::<String, _>("request")?)?;
        let step = JobStep::Running {
            stage: JobStage::Started,
            percent: 0,
            variants_done: 0,
            curve: None,
        };
        let event = insert_step(&mut tx, &id, &step, now).await?;
        tx.commit().await?;
        Ok(Some(Claimed { event, request }))
    }

    /// Agrega un paso (corriendo, falló o cancelado). Si el trabajo ya terminó no agrega nada y
    /// devuelve su final.
    pub async fn append(
        &self,
        job_id: &str,
        step: &JobStep,
        now: DateTime<Utc>,
    ) -> Result<Appended, LabStoreError> {
        let mut tx = write_tx(&self.pool).await?;
        if !lock_job(&mut tx, job_id).await? {
            return Err(LabStoreError::UnknownJob(job_id.to_owned()));
        }
        if let Some(last) = last_in(&mut *tx, job_id).await?
            && last.state.is_end()
        {
            tx.commit().await?;
            return Ok(Appended::AlreadyEnded(last));
        }
        let event = insert_step(&mut tx, job_id, step, now).await?;
        tx.commit().await?;
        Ok(Appended::Added(event))
    }

    /// Cancela `job_id` si todavía no terminó. `None` si el trabajo no existe.
    pub async fn cancel(
        &self,
        job_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<Appended>, LabStoreError> {
        match self.append(job_id, &JobStep::Cancelled, now).await {
            Ok(a) => Ok(Some(a)),
            Err(LabStoreError::UnknownJob(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Guarda los informes y las variantes probadas y agrega `done`, todo en una transacción:
    /// si el trabajo se canceló antes, no se guarda nada (`AlreadyEnded`). Un informe idéntico ya
    /// guardado (mismo `report_id`) no se duplica.
    pub async fn finish(
        &self,
        job_id: &str,
        reports: &[StoredReport],
        trials: &[LabTrial],
        now: DateTime<Utc>,
    ) -> Result<Appended, LabStoreError> {
        let mut tx = write_tx(&self.pool).await?;
        if !lock_job(&mut tx, job_id).await? {
            return Err(LabStoreError::UnknownJob(job_id.to_owned()));
        }
        if let Some(last) = last_in(&mut *tx, job_id).await?
            && last.state.is_end()
        {
            tx.commit().await?;
            return Ok(Appended::AlreadyEnded(last));
        }
        for r in reports {
            sqlx::query(
                "INSERT INTO lab_reports
                   (report_id, spec_sha256, family_sha256, job_id, report, computed_at)
                 VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                 ON CONFLICT (report_id) DO NOTHING",
            )
            .bind(&r.report_id)
            .bind(&r.spec_sha256)
            .bind(&r.family_sha256)
            .bind(job_id)
            .bind(serde_json::to_string(&r.report)?)
            .bind(r.computed_at)
            .execute(&mut *tx)
            .await?;
        }
        for t in trials {
            sqlx::query(
                "INSERT INTO lab_trials
                   (spec_sha256, family_sha256, instrument_id, timeframe, mode, catalog_version,
                    report_id, recorded_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(&t.spec_sha256)
            .bind(&t.family_sha256)
            .bind(t.instrument_id.as_str())
            .bind(t.timeframe.as_str())
            .bind(name(&t.mode)?)
            .bind(i32::try_from(t.catalog_version).map_err(|_| corrupt("catalog_version"))?)
            .bind(&t.report_id)
            .bind(t.recorded_at)
            .execute(&mut *tx)
            .await?;
        }
        let report_ids = reports.iter().map(|r| r.report_id.clone()).collect();
        let event = insert_step(&mut tx, job_id, &JobStep::Done { report_ids }, now).await?;
        tx.commit().await?;
        Ok(Appended::Added(event))
    }

    /// Marca como fallados los trabajos que quedaron corriendo (por ejemplo, el worker se
    /// reinició a mitad). Solo lo llama el worker al arrancar, cuando no hay otro corriendo.
    pub async fn fail_running(
        &self,
        error: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<LabJobEvent>, LabStoreError> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT e.job_id FROM lab_job_events e
             WHERE e.state = 'running'
               AND NOT EXISTS (SELECT 1 FROM lab_job_events f WHERE f.job_id = e.job_id
                               AND f.state IN ('done', 'failed', 'cancelled'))",
        )
        .fetch_all(&self.pool)
        .await?;
        let step = JobStep::Failed {
            error: error.to_owned(),
        };
        let mut failed = Vec::new();
        for id in ids {
            if let Appended::Added(e) = self.append(&id, &step, now).await? {
                failed.push(e);
            }
        }
        Ok(failed)
    }

    /// El último evento (estado actual) de `job_id`, o `None` si no existe.
    pub async fn last_event(&self, job_id: &str) -> Result<Option<LabJobEvent>, LabStoreError> {
        last_in(&self.pool, job_id).await
    }

    /// Los eventos de `job_id` con `seq` mayor que `after`, en orden.
    pub async fn events_after(
        &self,
        job_id: &str,
        after: i64,
    ) -> Result<Vec<LabJobEvent>, LabStoreError> {
        let sql = concat!(
            "SELECT ",
            event_columns!(),
            " FROM lab_job_events e JOIN lab_jobs j USING (job_id)
             WHERE e.job_id = $1 AND e.seq > $2 ORDER BY e.seq"
        );
        let rows = sqlx::query(sql)
            .bind(job_id)
            .bind(after)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_event).collect()
    }

    /// El pedido guardado de `job_id`.
    pub async fn request(&self, job_id: &str) -> Result<Option<LabJobRequest>, LabStoreError> {
        let text: Option<String> =
            sqlx::query_scalar("SELECT request::text FROM lab_jobs WHERE job_id = $1")
                .bind(job_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(text.map(|t| serde_json::from_str(&t)).transpose()?)
    }

    /// El informe `report_id` (`lab_report.v1`) tal como se guardó.
    pub async fn report(&self, report_id: &str) -> Result<Option<Value>, LabStoreError> {
        let text: Option<String> =
            sqlx::query_scalar("SELECT report::text FROM lab_reports WHERE report_id = $1")
                .bind(report_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(text.map(|t| serde_json::from_str(&t)).transpose()?)
    }

    /// Las variantes probadas de una familia, en el orden en que se guardaron.
    pub async fn family_trials(&self, family_sha256: &str) -> Result<Vec<LabTrial>, LabStoreError> {
        let rows = sqlx::query(
            "SELECT spec_sha256, family_sha256, instrument_id, timeframe, mode, catalog_version,
                    report_id, recorded_at
             FROM lab_trials WHERE family_sha256 = $1 ORDER BY seq",
        )
        .bind(family_sha256)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                let doc = serde_json::json!({
                    "spec_sha256": r.try_get::<String, _>("spec_sha256")?,
                    "family_sha256": r.try_get::<String, _>("family_sha256")?,
                    "instrument_id": r.try_get::<String, _>("instrument_id")?,
                    "timeframe": r.try_get::<String, _>("timeframe")?,
                    "mode": r.try_get::<String, _>("mode")?,
                    "catalog_version": r.try_get::<i32, _>("catalog_version")?,
                    "report_id": r.try_get::<String, _>("report_id")?,
                    "recorded_at": r
                        .try_get::<DateTime<Utc>, _>("recorded_at")?
                        .to_rfc3339_opts(SecondsFormat::AutoSi, true),
                });
                serde_json::from_value(doc).map_err(LabStoreError::from)
            })
            .collect()
    }
}

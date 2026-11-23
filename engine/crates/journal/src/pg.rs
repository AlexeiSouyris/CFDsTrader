//! Escritura y lectura del diario en PostgreSQL (`infra/migrations/0002_journal.sql`).
//!
//! Idempotente, como las velas: volver a guardar la misma entrada no duplica nada. Si ya hay
//! una entrada con el mismo id y **otro** contenido, es un error (`Conflict`): el diario nunca
//! se reescribe en silencio.

use rust_decimal::Decimal;
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Postgres, Row, Transaction};

use crate::curve::{CurveInvalid, RunCurve, RunDetail, RunTrade};
use crate::ml_features::MlFeatures;
use crate::walk_forward::WalkForwardReport;
use crate::{BacktestRun, Outcome, OutcomeKind, Proposal, RunInvalid};

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("PostgreSQL: {0}")]
    Db(#[from] sqlx::Error),
    #[error("JSON del diario: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ya hay una entrada {id} con otro contenido: el diario no se reescribe")]
    Conflict { id: String },
    #[error("corrida inválida: {0}")]
    InvalidRun(#[from] RunInvalid),
    #[error("curva inválida: {0}")]
    InvalidCurve(#[from] CurveInvalid),
    #[error("reporte de walk-forward inválido: report_id o config_sha256 con otro formato")]
    InvalidReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    Inserted,
    /// Ya estaba, idéntica.
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct PgJournal {
    pool: PgPool,
}

/// Texto de un enum del contrato (`"backtest"`, `"long"`...), sin las comillas del JSON.
fn name<T: serde::Serialize>(v: &T) -> Result<String, JournalError> {
    match serde_json::to_value(v)? {
        serde_json::Value::String(s) => Ok(s),
        other => Ok(other.to_string()),
    }
}

fn parse_name<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, JournalError> {
    Ok(serde_json::from_value(serde_json::Value::String(
        s.to_string(),
    ))?)
}

fn row_to_proposal(r: &PgRow) -> Result<Proposal, JournalError> {
    Ok(Proposal {
        id: r.try_get("id")?,
        source: parse_name(r.try_get::<&str, _>("source")?)?,
        run_id: r.try_get("run_id")?,
        instrument_id: serde_json::from_value(serde_json::Value::String(
            r.try_get("instrument_id")?,
        ))?,
        side: parse_name(r.try_get::<&str, _>("side")?)?,
        signal_at: r.try_get("signal_at")?,
        reference: r.try_get::<Decimal, _>("reference")?,
        stop: r.try_get("stop")?,
        tp1: r.try_get("tp1")?,
        size: r.try_get("size")?,
        leverage: r.try_get("leverage")?,
        score: r.try_get("score")?,
        breakdown: serde_json::from_str(r.try_get("breakdown")?)?,
        triggers: serde_json::from_str(r.try_get("triggers")?)?,
    })
}

fn row_to_outcome(r: &PgRow) -> Result<Outcome, JournalError> {
    Ok(Outcome {
        proposal_id: r.try_get("proposal_id")?,
        kind: parse_name(r.try_get::<&str, _>("kind")?)?,
        entry_at: r.try_get("entry_at")?,
        entry_price: r.try_get("entry_price")?,
        exits: serde_json::from_str(r.try_get("exits")?)?,
        commission: r.try_get("commission")?,
        funding: r.try_get("funding")?,
        gross_pnl: r.try_get("gross_pnl")?,
        net_pnl: r.try_get("net_pnl")?,
        hit_tp1: r.try_get("hit_tp1")?,
        hit_stop: r.try_get("hit_stop")?,
        exit_reason: parse_name(r.try_get::<&str, _>("exit_reason")?)?,
    })
}

/// Columnas en el orden que leen `row_to_*`. Macros para armar SQL fijo con `concat!`: sqlx
/// exige consultas estáticas, que no se puedan inyectar (igual que en `cfds-storage`).
macro_rules! select_proposal {
    () => {
        concat!(
            "SELECT id, source, run_id, instrument_id, side, signal_at, reference, stop, tp1, ",
            "size, leverage, score, breakdown::text AS breakdown, triggers::text AS triggers ",
            "FROM proposals WHERE id = $1"
        )
    };
}

macro_rules! select_outcome {
    () => {
        concat!(
            "SELECT proposal_id, kind, entry_at, entry_price, exits::text AS exits, commission, ",
            "funding, gross_pnl, net_pnl, hit_tp1, hit_stop, exit_reason ",
            "FROM outcomes WHERE proposal_id = $1 AND kind = $2"
        )
    };
}

impl PgJournal {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Guarda la propuesta y, si hay, su resultado, en una sola transacción.
    pub async fn record(
        &self,
        proposal: &Proposal,
        outcome: Option<&Outcome>,
    ) -> Result<Recorded, JournalError> {
        let mut tx = self.pool.begin().await?;
        let mut inserted = insert_proposal(&mut tx, proposal).await?;
        if let Some(o) = outcome {
            if o.proposal_id != proposal.id {
                return Err(JournalError::Conflict {
                    id: o.proposal_id.clone(),
                });
            }
            inserted |= insert_outcome(&mut tx, o).await?;
        }
        tx.commit().await?;
        Ok(if inserted {
            Recorded::Inserted
        } else {
            Recorded::Unchanged
        })
    }

    pub async fn proposal(&self, id: &str) -> Result<Option<Proposal>, JournalError> {
        sqlx::query(select_proposal!())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| row_to_proposal(&r))
            .transpose()
    }

    pub async fn outcome(
        &self,
        proposal_id: &str,
        kind: OutcomeKind,
    ) -> Result<Option<Outcome>, JournalError> {
        sqlx::query(select_outcome!())
            .bind(proposal_id)
            .bind(name(&kind)?)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| row_to_outcome(&r))
            .transpose()
    }
}

impl PgJournal {
    /// Guarda una corrida de backtest (#200). Idempotente: si ya estaba la misma corrida (todo
    /// igual salvo `started_at` y `finished_at`) no guarda nada y conserva la primera; con otro
    /// contenido es `Conflict`, porque reusar un `run_id` mezclaría corridas en el diario.
    pub async fn record_run(&self, run: &BacktestRun) -> Result<Recorded, JournalError> {
        run.validate()?;
        let mut tx = self.pool.begin().await?;
        let done = sqlx::query(
            "INSERT INTO backtest_runs (run_id, instruments, timeframe, from_at, to_at,
               config_sha256, initial_equity, costs, news, metrics, started_at, finished_at)
             VALUES ($1, $2::jsonb, $3, $4, $5, $6, $7, $8::jsonb, $9::jsonb, $10::jsonb, $11, $12)
             ON CONFLICT (run_id) DO NOTHING",
        )
        .bind(&run.run_id)
        .bind(serde_json::to_string(&run.instruments)?)
        .bind(run.timeframe.as_str())
        .bind(run.from)
        .bind(run.to)
        .bind(&run.config_sha256)
        .bind(run.initial_equity)
        .bind(serde_json::to_string(&run.costs)?)
        .bind(run.news.as_ref().map(serde_json::to_string).transpose()?)
        .bind(serde_json::to_string(&run.metrics)?)
        .bind(run.started_at)
        .bind(run.finished_at)
        .execute(&mut *tx)
        .await?;
        let inserted = done.rows_affected() == 1;
        if !inserted {
            let stored =
                select_run(&mut tx, &run.run_id)
                    .await?
                    .ok_or_else(|| JournalError::Conflict {
                        id: run.run_id.clone(),
                    })?;
            if !stored.same_run(run) {
                return Err(JournalError::Conflict {
                    id: run.run_id.clone(),
                });
            }
        }
        tx.commit().await?;
        Ok(if inserted {
            Recorded::Inserted
        } else {
            Recorded::Unchanged
        })
    }

    pub async fn run(&self, run_id: &str) -> Result<Option<BacktestRun>, JournalError> {
        let mut tx = self.pool.begin().await?;
        let run = select_run(&mut tx, run_id).await?;
        tx.commit().await?;
        Ok(run)
    }

    /// Las últimas `limit` corridas guardadas (las más nuevas primero, por `finished_at`), cada
    /// una armada y validada como `backtest_run.v1`.
    pub async fn runs(&self, limit: u32) -> Result<Vec<BacktestRun>, JournalError> {
        let mut tx = self.pool.begin().await?;
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT run_id FROM backtest_runs ORDER BY finished_at DESC, run_id LIMIT $1",
        )
        .bind(i64::from(limit))
        .fetch_all(&mut *tx)
        .await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(run) = select_run(&mut tx, &id).await? {
                out.push(run);
            }
        }
        tx.commit().await?;
        Ok(out)
    }
}

impl PgJournal {
    /// Guarda la curva de una corrida ya guardada (`backtest_run_curve.v1`, #299). Idempotente:
    /// la misma curva no guarda nada; otra distinta para la misma corrida es `Conflict`.
    pub async fn record_run_curve(&self, curve: &RunCurve) -> Result<Recorded, JournalError> {
        curve.validate()?;
        let mut tx = self.pool.begin().await?;
        let done = sqlx::query(
            "INSERT INTO backtest_run_curves (run_id, points, buy_and_hold)
             VALUES ($1, $2::jsonb, $3::jsonb)
             ON CONFLICT (run_id) DO NOTHING",
        )
        .bind(&curve.run_id)
        .bind(serde_json::to_string(&curve.points)?)
        .bind(serde_json::to_string(&curve.buy_and_hold)?)
        .execute(&mut *tx)
        .await?;
        let inserted = done.rows_affected() == 1;
        if !inserted && select_curve(&mut tx, &curve.run_id).await?.as_ref() != Some(curve) {
            return Err(JournalError::Conflict {
                id: curve.run_id.clone(),
            });
        }
        tx.commit().await?;
        Ok(if inserted {
            Recorded::Inserted
        } else {
            Recorded::Unchanged
        })
    }

    pub async fn run_curve(&self, run_id: &str) -> Result<Option<RunCurve>, JournalError> {
        let mut tx = self.pool.begin().await?;
        let curve = select_curve(&mut tx, run_id).await?;
        tx.commit().await?;
        Ok(curve)
    }

    /// Las operaciones de una corrida: sus propuestas (por hora de la señal) con su resultado
    /// simulado, si hay.
    pub async fn run_trades(&self, run_id: &str) -> Result<Vec<RunTrade>, JournalError> {
        let mut tx = self.pool.begin().await?;
        let trades = select_trades(&mut tx, run_id).await?;
        tx.commit().await?;
        Ok(trades)
    }

    /// El detalle de una corrida (`backtest_run_detail.v1`): la corrida, su curva (o `None` si
    /// no se guardó) y sus operaciones, leídos en una sola transacción. `None` si no existe.
    pub async fn run_detail(&self, run_id: &str) -> Result<Option<RunDetail>, JournalError> {
        let mut tx = self.pool.begin().await?;
        let Some(run) = select_run(&mut tx, run_id).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        let curve = select_curve(&mut tx, run_id).await?;
        let trades = select_trades(&mut tx, run_id).await?;
        tx.commit().await?;
        Ok(Some(RunDetail { run, curve, trades }))
    }
}

async fn select_curve(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
) -> Result<Option<RunCurve>, JournalError> {
    let Some(r) = sqlx::query(
        "SELECT run_id, points::text AS points, buy_and_hold::text AS buy_and_hold
         FROM backtest_run_curves WHERE run_id = $1",
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    let curve = RunCurve {
        run_id: r.try_get("run_id")?,
        points: serde_json::from_str(r.try_get("points")?)?,
        buy_and_hold: serde_json::from_str(r.try_get("buy_and_hold")?)?,
    };
    curve.validate()?;
    Ok(Some(curve))
}

async fn select_trades(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
) -> Result<Vec<RunTrade>, JournalError> {
    let rows = sqlx::query(concat!(
        "SELECT id, source, run_id, instrument_id, side, signal_at, reference, stop, tp1, ",
        "size, leverage, score, breakdown::text AS breakdown, triggers::text AS triggers ",
        "FROM proposals WHERE run_id = $1 ORDER BY signal_at, id"
    ))
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let proposal = row_to_proposal(&r)?;
        let outcome = sqlx::query(select_outcome!())
            .bind(&proposal.id)
            .bind(name(&OutcomeKind::Simulated)?)
            .fetch_optional(&mut **tx)
            .await?
            .map(|r| row_to_outcome(&r))
            .transpose()?;
        out.push(RunTrade { proposal, outcome });
    }
    Ok(out)
}

impl PgJournal {
    /// Guarda las variables de ML de una propuesta ya guardada (`ml_features.v1`, #158). La
    /// tabla es de solo agregar: volver a guardar las mismas no hace nada, y otras distintas
    /// para la misma propuesta son `Conflict` (spec §8: sin repintado).
    pub async fn record_ml_features(&self, f: &MlFeatures) -> Result<Recorded, JournalError> {
        let done = sqlx::query(
            "INSERT INTO ml_features (proposal_id, as_of, atr14_m15, realized_vol_tercile,
               session_band)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (proposal_id) DO NOTHING",
        )
        .bind(&f.proposal_id)
        .bind(f.as_of)
        .bind(f.atr14_m15)
        .bind(name(&f.realized_vol_tercile)?)
        .bind(name(&f.session_band)?)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 1 {
            return Ok(Recorded::Inserted);
        }
        if self.ml_features(&f.proposal_id).await?.as_ref() != Some(f) {
            return Err(JournalError::Conflict {
                id: f.proposal_id.clone(),
            });
        }
        Ok(Recorded::Unchanged)
    }

    pub async fn ml_features(&self, proposal_id: &str) -> Result<Option<MlFeatures>, JournalError> {
        let Some(r) = sqlx::query(
            "SELECT proposal_id, as_of, atr14_m15, realized_vol_tercile, session_band
             FROM ml_features WHERE proposal_id = $1",
        )
        .bind(proposal_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        Ok(Some(MlFeatures {
            proposal_id: r.try_get("proposal_id")?,
            as_of: r.try_get("as_of")?,
            atr14_m15: r.try_get::<Decimal, _>("atr14_m15")?.normalize(),
            realized_vol_tercile: parse_name(r.try_get::<&str, _>("realized_vol_tercile")?)?,
            session_band: parse_name(r.try_get::<&str, _>("session_band")?)?,
        }))
    }
}

/// Lee una corrida y la vuelve a armar como documento de `backtest_run.v1`, validado igual que
/// un mensaje del contrato.
async fn select_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: &str,
) -> Result<Option<BacktestRun>, JournalError> {
    let Some(r) = sqlx::query(
        "SELECT run_id, instruments::text AS instruments, timeframe, from_at, to_at,
                config_sha256, initial_equity, costs::text AS costs, news::text AS news,
                metrics::text AS metrics, started_at, finished_at
         FROM backtest_runs WHERE run_id = $1",
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(None);
    };
    let json = |col: &str| -> Result<serde_json::Value, JournalError> {
        let text: Option<String> = r.try_get(col)?;
        Ok(match text {
            Some(t) => serde_json::from_str(&t)?,
            None => serde_json::Value::Null,
        })
    };
    let utc = |col: &str| -> Result<String, JournalError> {
        let t: chrono::DateTime<chrono::Utc> = r.try_get(col)?;
        Ok(t.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true))
    };
    let equity: Decimal = r.try_get("initial_equity")?;
    let doc = serde_json::json!({
        "run_id": r.try_get::<String, _>("run_id")?,
        "instruments": json("instruments")?,
        "timeframe": r.try_get::<String, _>("timeframe")?,
        "from": utc("from_at")?,
        "to": utc("to_at")?,
        "config_sha256": r.try_get::<String, _>("config_sha256")?,
        "initial_equity": equity.to_string(),
        "costs": json("costs")?,
        "news": json("news")?,
        "metrics": json("metrics")?,
        "started_at": utc("started_at")?,
        "finished_at": utc("finished_at")?,
    });
    Ok(Some(serde_json::from_value(doc)?))
}

/// `true` si era nueva; si ya estaba, tiene que ser idéntica.
async fn insert_proposal(
    tx: &mut Transaction<'_, Postgres>,
    p: &Proposal,
) -> Result<bool, JournalError> {
    let done = sqlx::query(
        "INSERT INTO proposals (id, source, run_id, instrument_id, side, signal_at, reference,
           stop, tp1, size, leverage, score, breakdown, triggers)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13::jsonb, $14::jsonb)
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&p.id)
    .bind(name(&p.source)?)
    .bind(&p.run_id)
    .bind(p.instrument_id.as_str())
    .bind(name(&p.side)?)
    .bind(p.signal_at)
    .bind(p.reference)
    .bind(p.stop)
    .bind(p.tp1)
    .bind(p.size)
    .bind(p.leverage)
    .bind(p.score)
    .bind(serde_json::to_string(&p.breakdown)?)
    .bind(serde_json::to_string(&p.triggers)?)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 1 {
        return Ok(true);
    }
    let stored = sqlx::query(select_proposal!())
        .bind(&p.id)
        .fetch_one(&mut **tx)
        .await?;
    if row_to_proposal(&stored)? != *p {
        return Err(JournalError::Conflict { id: p.id.clone() });
    }
    Ok(false)
}

async fn insert_outcome(
    tx: &mut Transaction<'_, Postgres>,
    o: &Outcome,
) -> Result<bool, JournalError> {
    let done = sqlx::query(
        "INSERT INTO outcomes (proposal_id, kind, entry_at, entry_price, exits, commission,
           funding, gross_pnl, net_pnl, hit_tp1, hit_stop, exit_reason)
         VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (proposal_id, kind) DO NOTHING",
    )
    .bind(&o.proposal_id)
    .bind(name(&o.kind)?)
    .bind(o.entry_at)
    .bind(o.entry_price)
    .bind(serde_json::to_string(&o.exits)?)
    .bind(o.commission)
    .bind(o.funding)
    .bind(o.gross_pnl)
    .bind(o.net_pnl)
    .bind(o.hit_tp1)
    .bind(o.hit_stop)
    .bind(name(&o.exit_reason)?)
    .execute(&mut **tx)
    .await?;
    if done.rows_affected() == 1 {
        return Ok(true);
    }
    let stored = sqlx::query(select_outcome!())
        .bind(&o.proposal_id)
        .bind(name(&o.kind)?)
        .fetch_one(&mut **tx)
        .await?;
    if row_to_outcome(&stored)? != *o {
        return Err(JournalError::Conflict {
            id: o.proposal_id.clone(),
        });
    }
    Ok(false)
}

/// Reportes de la evaluación de la Fase 5 (`walk_forward_report.v1`, migración 0031, #298).
impl PgJournal {
    /// Guarda el reporte. Idempotente: el mismo reporte (salvo `computed_at`) no guarda nada; con
    /// otro contenido es `Conflict`.
    pub async fn record_walk_forward(
        &self,
        report: &WalkForwardReport,
    ) -> Result<Recorded, JournalError> {
        if !report.valid_id() {
            return Err(JournalError::InvalidReport);
        }
        let mut tx = self.pool.begin().await?;
        let done = sqlx::query(
            "INSERT INTO walk_forward_reports (report_id, instrument_id, candle_source,
               config_sha256, computed_at, report)
             VALUES ($1, $2, $3, $4, $5, $6::jsonb)
             ON CONFLICT (report_id) DO NOTHING",
        )
        .bind(&report.report_id)
        .bind(report.instrument_id.as_str())
        .bind(report.candle_source.as_str())
        .bind(&report.config_sha256)
        .bind(report.computed_at)
        .bind(serde_json::to_string(report)?)
        .execute(&mut *tx)
        .await?;
        let inserted = done.rows_affected() == 1;
        if !inserted {
            let stored = select_walk_forward(&mut tx, &report.report_id).await?;
            if !stored.is_some_and(|s| s.same_report(report)) {
                return Err(JournalError::Conflict {
                    id: report.report_id.clone(),
                });
            }
        }
        tx.commit().await?;
        Ok(if inserted {
            Recorded::Inserted
        } else {
            Recorded::Unchanged
        })
    }

    pub async fn walk_forward(
        &self,
        report_id: &str,
    ) -> Result<Option<WalkForwardReport>, JournalError> {
        let mut tx = self.pool.begin().await?;
        let r = select_walk_forward(&mut tx, report_id).await?;
        tx.commit().await?;
        Ok(r)
    }
}

async fn select_walk_forward(
    tx: &mut Transaction<'_, Postgres>,
    report_id: &str,
) -> Result<Option<WalkForwardReport>, JournalError> {
    let text: Option<String> =
        sqlx::query_scalar("SELECT report::text FROM walk_forward_reports WHERE report_id = $1")
            .bind(report_id)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(text.map(|t| serde_json::from_str(&t)).transpose()?)
}

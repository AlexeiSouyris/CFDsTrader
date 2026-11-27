//! Corredor de trabajos del laboratorio de estrategias (#775, `docs/design/2026-11-laboratorio.md`
//! §5): un **proceso aparte** (§9 punto 6, recomendado) que toma de a un trabajo de la cola
//! (`lab_jobs`, que llena el gateway), corre sus variantes con lo mismo que el ejemplo
//! `laboratorio` y guarda los informes, las variantes probadas y el final en PostgreSQL. Así un
//! backtest largo nunca frena al gateway ni al runner de paper.
//!
//! ```text
//! DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don REDIS_URL=redis://... \
//! cargo run --release -p cfds-backtest --example lab_worker -- [--once]
//! ```
//! - Con `--once` corre lo que haya en la cola y termina (para cron con `lockf`); sin él, mira la
//!   cola cada `[worker] poll_secs` de `config/lab.toml`.
//! - La sesión puede ser de solo lectura: cada escritura (solo en las tablas del laboratorio)
//!   declara su propia transacción de escritura (`cfds_journal::lab`).
//! - Velas de futuros **solo hasta el inicio del fuera de muestra** (como las rondas), nunca IG.
//!   Cada spec se vuelve a validar al tomarla (la separación de datos pudo moverse).
//! - Cada variante terminada manda su curva de capital reducida (`curve`, #854) en el paso de
//!   "corriendo variantes", para la vista en vivo de la UI.
//! - Cada paso (`lab_job_event.v1`) se guarda en `lab_job_events` y se publica en el Redis Stream
//!   `lab_jobs.v1.<job_id>`, que el gateway pasa a SSE. Sin Redis, sigue: el gateway lee de
//!   PostgreSQL.
//! - Cancelación y duración máxima (`[worker] max_minutes`) se revisan entre etapas. Un trabajo
//!   que quedó corriendo cuando el worker se cortó se marca como fallado al arrancar.
//! - Exploratorio: **no vota** y no suma al N del proyecto.
#[path = "common/lab_run.rs"]
mod lab_run;
#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/research_round.rs"]
mod research_round;
#[path = "common/study.rs"]
mod study;

use std::time::Instant;

use cfds_backtest::lab::{
    JobStage, JobStep, LabJobEvent, LabTrial, TrialCurve, ValidSpec, WorkerConfig, check_variants,
    family_variants, progress, validate_spec,
};
use cfds_journal::lab::{Appended, Claimed, PgLab, StoredReport};
use cfds_storage::{LabStream, PgStore};
use chrono::Utc;

use lab_run::LabEnv;

/// Por qué se corta un trabajo antes de terminar.
enum Stop {
    Cancelled,
    Failed(String),
}

impl<E: std::fmt::Display> From<E> for Stop {
    fn from(e: E) -> Self {
        Self::Failed(e.to_string())
    }
}

struct Worker {
    env: LabEnv,
    cfg: WorkerConfig,
    store: PgStore,
    lab: PgLab,
    redis_url: Option<String>,
    redis: Option<LabStream>,
}

impl Worker {
    /// Publica el evento en Redis. Si Redis falla, avisa y sigue (PostgreSQL ya lo tiene).
    async fn publish(&mut self, e: &LabJobEvent) {
        if self.redis.is_none()
            && let Some(url) = &self.redis_url
        {
            self.redis = LabStream::connect(url)
                .await
                .map_err(|err| eprintln!("⚠️  Redis: {err}"))
                .ok();
        }
        let Some(r) = self.redis.as_mut() else { return };
        let data = match serde_json::to_string(e) {
            Ok(d) => d,
            Err(err) => return eprintln!("⚠️  evento: {err}"),
        };
        let (ttl, maxlen) = (self.cfg.stream_ttl(), self.cfg.stream_maxlen);
        if let Err(err) = r.publish(&e.job_id, e.seq, &data, ttl, maxlen).await {
            eprintln!("⚠️  Redis: {err} (los eventos siguen en PostgreSQL)");
            self.redis = None;
        }
    }

    /// Agrega un paso de "corriendo". Si el trabajo ya terminó (se canceló), corta.
    async fn step(
        &mut self,
        job_id: &str,
        stage: JobStage,
        done: u32,
        total: u32,
        started: Instant,
    ) -> Result<(), Stop> {
        self.step_with(job_id, stage, done, total, started, None)
            .await
    }

    /// Como `step`, con la curva de la variante que acaba de terminar (#854).
    async fn step_with(
        &mut self,
        job_id: &str,
        stage: JobStage,
        done: u32,
        total: u32,
        started: Instant,
        curve: Option<TrialCurve>,
    ) -> Result<(), Stop> {
        if started.elapsed() > self.cfg.max_duration() {
            return Err(Stop::Failed(format!(
                "superó la duración máxima de {} minutos ([worker] max_minutes)",
                self.cfg.max_minutes
            )));
        }
        let step = JobStep::Running {
            stage,
            percent: progress(stage, done, total),
            variants_done: done,
            curve,
        };
        match self.lab.append(job_id, &step, Utc::now()).await? {
            Appended::Added(e) => {
                self.publish(&e).await;
                Ok(())
            }
            Appended::AlreadyEnded(_) => Err(Stop::Cancelled),
        }
    }

    /// Corre el trabajo y devuelve sus informes y variantes probadas, sin guardarlos.
    async fn run(
        &mut self,
        c: &Claimed,
        started: Instant,
    ) -> Result<(Vec<StoredReport>, Vec<LabTrial>), Stop> {
        let job = c.event.job_id.clone();
        let req = &c.request;
        let total = c.event.variants_total;
        let first = req.specs.first().ok_or("el pedido no tiene specs")?;
        let id = &first.market.instrument_id;
        let from = self.cfg.load_from()?;
        let split = self.env.split_for(&self.store, id, from).await?;
        let mut specs: Vec<ValidSpec> = Vec::new();
        for (k, s) in req.specs.iter().enumerate() {
            let v = validate_spec(s, &self.env.catalog, &split).map_err(|errors| {
                let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
                format!("la variante {} ya no es válida: {}", k + 1, msgs.join("; "))
            })?;
            specs.push(v);
        }
        check_variants(&specs, &self.env.catalog)?;
        self.step(&job, JobStage::LoadingCandles, 0, total, started)
            .await?;
        let args = self.cfg.study_args();
        let equity = req.initial_equity;
        let data =
            lab_run::load(&self.env, &specs, &args, from, equity, split, &self.store).await?;
        let mut runs = Vec::new();
        self.step(&job, JobStage::RunningVariants, 0, total, started)
            .await?;
        for (k, v) in specs.iter().enumerate() {
            let run = lab_run::run_variant(&data, v)?;
            // la curva de la variante apenas termina, para verla en vivo (#854)
            let curve = TrialCurve::from_monthly(k, &run.monthly, self.cfg.curve_max_points);
            let done = u32::try_from(k + 1).unwrap_or(u32::MAX);
            self.step_with(
                &job,
                JobStage::RunningVariants,
                done,
                total,
                started,
                Some(curve),
            )
            .await?;
            runs.push(run);
        }
        let variants: Vec<Vec<f64>> = runs.iter().map(|r| r.monthly.clone()).collect();
        let family = &specs[0].family_sha256;
        let past = self.lab.family_trials(family).await?;
        let current: Vec<&str> = specs.iter().map(|v| v.spec_sha256.as_str()).collect();
        let n = family_variants(&past, family, &current);
        let now = Utc::now();
        let (mut reports, mut trials) = (Vec::new(), Vec::new());
        for (k, (v, vr)) in specs.iter().zip(&runs).enumerate() {
            let done = u32::try_from(k).unwrap_or(u32::MAX);
            self.step(&job, JobStage::BuildingReports, done, total, started)
                .await?;
            let report = lab_run::report(&self.env, &data, v, vr, &variants, n, now)?;
            let report_id = report["report_id"]
                .as_str()
                .ok_or("el informe no tiene report_id")?
                .to_owned();
            trials.push(LabTrial::new(v, &report_id, now));
            reports.push(StoredReport {
                report_id,
                spec_sha256: v.spec_sha256.clone(),
                family_sha256: v.family_sha256.clone(),
                report,
                computed_at: now,
            });
        }
        self.step(&job, JobStage::Saving, total, total, started)
            .await?;
        Ok((reports, trials))
    }

    async fn process(&mut self, c: Claimed) -> Result<(), Box<dyn std::error::Error>> {
        let job = c.event.job_id.clone();
        println!(
            "── trabajo {} ({} variantes)",
            &job[..12],
            c.event.variants_total
        );
        self.publish(&c.event).await;
        let started = Instant::now();
        let end = match self.run(&c, started).await {
            Ok((reports, trials)) => self.lab.finish(&job, &reports, &trials, Utc::now()).await?,
            Err(Stop::Cancelled) => {
                println!("   cancelado: no se guarda nada");
                return Ok(());
            }
            Err(Stop::Failed(error)) => {
                eprintln!("   falló: {error}");
                let step = JobStep::Failed { error };
                self.lab.append(&job, &step, Utc::now()).await?
            }
        };
        let (Appended::Added(e) | Appended::AlreadyEnded(e)) = end;
        println!(
            "   {}: {} informes en {:.0?}",
            e.state.as_str(),
            e.report_ids.len(),
            started.elapsed()
        );
        self.publish(&e).await;
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let once = std::env::args().skip(1).any(|a| a == "--once");
    let env = LabEnv::load()?;
    let cfg = env.lab.worker.clone();
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let redis_url = std::env::var("REDIS_URL").ok();
    if redis_url.is_none() {
        eprintln!("⚠️  sin REDIS_URL: el progreso queda solo en PostgreSQL");
    }
    let mut w = Worker {
        lab: cfds_journal::lab::PgLab::new(store.pool().clone()),
        env,
        cfg,
        store,
        redis_url,
        redis: None,
    };
    // Un trabajo que quedó corriendo cuando el worker se cortó no termina nunca: falla.
    let error = "el lab_worker se cortó con el trabajo a medias; pedilo de nuevo";
    for e in w.lab.fail_running(error, Utc::now()).await? {
        println!(
            "trabajo {} marcado como fallado (quedó a medias)",
            &e.job_id[..12]
        );
        w.publish(&e).await;
    }
    println!(
        "lab_worker: cola de lab_jobs, de a un trabajo{}",
        if once { " (--once)" } else { "" }
    );
    loop {
        match w.lab.claim_next(Utc::now()).await? {
            Some(c) => w.process(c).await?,
            None if once => break,
            None => tokio::time::sleep(w.cfg.poll()).await,
        }
    }
    Ok(())
}

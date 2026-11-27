//! Laboratorio en PostgreSQL (#774): cola de trabajos, eventos, informes y variantes probadas.
//! Necesitan `DATABASE_URL` de una base de pruebas (`cfds_test…`); están marcados `#[ignore]` y
//! el CI los corre con `-- --ignored` (job de integración). Las tablas son de solo agregar: cada
//! test usa hashes propios de su corrida.

use cfds_backtest::lab::{
    JobStage, JobState, JobStep, LabJobRequest, LabTrial, TrialCurve, sha256_hex,
};
use cfds_journal::lab::{Appended, Enqueued, PgLab, StoredReport};
use cfds_storage::PgStore;
use cfds_storage::test_db::{test_database_url, unique_id};
use chrono::{DateTime, Utc};
use serde_json::json;

/// La cola es una sola: los tests que encolan y toman trabajos corren de a uno.
static QUEUE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn lab() -> PgLab {
    let store = PgStore::connect(&test_database_url()).await.unwrap();
    store.migrate().await.unwrap();
    PgLab::new(store.pool().clone())
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

/// Un hash propio de esta corrida.
fn fresh(tag: &str) -> String {
    sha256_hex(&unique_id(tag))
}

fn request(variants: usize) -> LabJobRequest {
    let spec = json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": "1h" },
        "direction": "long",
        "entry": [{ "type": "ma_cross", "params": { "average": "ema", "fast": 20, "slow": 50 } }],
        "filters": [],
        "stop": { "type": "atr", "params": { "period": 14, "k": "1.5" } },
        "exit": { "type": "time_target", "params": { "max_candles": 6 } },
        "risk_pct": "0.5",
        "cost_multiplier": "1",
        "period": { "from": "2020-01-01T00:00:00Z", "to": "2022-01-01T00:00:00Z" },
        "mode": "exploratory"
    });
    serde_json::from_value(json!({ "specs": vec![spec; variants], "initial_equity": "100000" }))
        .unwrap()
}

fn trial(spec: &str, family: &str, report: &str) -> LabTrial {
    serde_json::from_value(json!({
        "spec_sha256": spec, "family_sha256": family, "instrument_id": "US500",
        "timeframe": "1h", "mode": "exploratory", "catalog_version": 1, "report_id": report,
        "recorded_at": "2026-11-27T02:00:00.5Z"
    }))
    .unwrap()
}

fn report(id: &str, spec: &str, family: &str) -> StoredReport {
    StoredReport {
        report_id: id.to_owned(),
        spec_sha256: spec.to_owned(),
        family_sha256: family.to_owned(),
        report: json!({ "report_id": id, "label": "exploratorio, no vota" }),
        computed_at: at("2026-11-27T02:00:00Z"),
    }
}

/// Toma trabajos hasta llegar a `job_id` (otro test pudo dejar uno en cola un instante).
async fn claim(lab: &PgLab, job_id: &str) -> cfds_journal::lab::Claimed {
    for _ in 0..100 {
        let c = lab.claim_next(at("2026-11-27T01:01:00Z")).await.unwrap();
        match c {
            Some(c) if c.event.job_id == job_id => return c,
            Some(_) => continue,
            None => break,
        }
    }
    panic!("no se pudo tomar {job_id}");
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn a_job_goes_from_queued_to_done_with_its_reports_and_trials() {
    let _queue = QUEUE.lock().await;
    let lab = lab().await;
    let (req_hash, family) = (fresh("LJREQ"), fresh("LJFAM"));
    let req = request(2);
    let t0 = at("2026-11-27T01:00:00Z");
    let Enqueued::New(queued) = lab.enqueue(&req, &req_hash, &family, t0).await.unwrap() else {
        panic!("el primer pedido crea el trabajo");
    };
    assert_eq!(queued.state, JobState::Queued);
    assert_eq!((queued.variants_done, queued.variants_total), (0, 2));
    // Mismo pedido: mismo trabajo (idempotente), sin otro evento.
    let again = lab.enqueue(&req, &req_hash, &family, t0).await.unwrap();
    assert_eq!(again, Enqueued::Existing(queued.clone()));
    assert_eq!(
        lab.request(&queued.job_id).await.unwrap(),
        Some(req.clone())
    );

    let claimed = claim(&lab, &queued.job_id).await;
    assert_eq!(claimed.request, req);
    assert_eq!(claimed.event.stage, Some(JobStage::Started));
    // Un trabajo tomado no se vuelve a tomar.
    let next = lab.claim_next(t0).await.unwrap();
    assert!(next.is_none_or(|c| c.event.job_id != queued.job_id));

    // la curva de la variante que terminó (#854) se guarda y vuelve igual
    let curve = TrialCurve::from_monthly(0, &[0.01, -0.02, 0.03], 200);
    let step = JobStep::Running {
        stage: JobStage::RunningVariants,
        percent: 55,
        variants_done: 1,
        curve: Some(curve.clone()),
    };
    let Appended::Added(progress) = lab.append(&queued.job_id, &step, t0).await.unwrap() else {
        panic!("corriendo");
    };
    assert_eq!((progress.percent, progress.variants_done), (55, 1));
    assert_eq!(progress.curve, Some(curve));
    let replay = lab.events_after(&queued.job_id, 0).await.unwrap();
    assert_eq!(replay.last(), Some(&progress));

    let (s1, s2) = (fresh("LJS1"), fresh("LJS2"));
    let (r1, r2) = (fresh("LJR1"), fresh("LJR2"));
    let reports = [report(&r1, &s1, &family), report(&r2, &s2, &family)];
    let trials = [trial(&s1, &family, &r1), trial(&s2, &family, &r2)];
    let Appended::Added(done) = lab
        .finish(&queued.job_id, &reports, &trials, t0)
        .await
        .unwrap()
    else {
        panic!("listo");
    };
    assert_eq!(done.state, JobState::Done);
    assert_eq!(done.report_ids, vec![r1.clone(), r2.clone()]);
    assert_eq!((done.percent, done.variants_done), (100, 2));
    assert_eq!(
        lab.last_event(&queued.job_id).await.unwrap(),
        Some(done.clone())
    );
    assert_eq!(
        lab.report(&r1).await.unwrap().unwrap()["report_id"],
        json!(r1)
    );
    assert!(lab.report(&fresh("LJNONE")).await.unwrap().is_none());
    assert_eq!(lab.family_trials(&family).await.unwrap(), trials.to_vec());

    // Los eventos en orden, y desde un seq.
    let all = lab.events_after(&queued.job_id, 0).await.unwrap();
    let states: Vec<JobState> = all.iter().map(|e| e.state).collect();
    use JobState::{Done, Queued, Running};
    assert_eq!(states, vec![Queued, Running, Running, Done]);
    assert!(all.windows(2).all(|w| w[0].seq < w[1].seq));
    assert_eq!(
        lab.events_after(&queued.job_id, all[2].seq).await.unwrap(),
        vec![done.clone()]
    );

    // Terminado: ni cancelar ni volver a terminar agrega nada.
    let late = lab.cancel(&queued.job_id, t0).await.unwrap();
    assert_eq!(late, Some(Appended::AlreadyEnded(done.clone())));
    let twice = lab
        .finish(&queued.job_id, &reports, &trials, t0)
        .await
        .unwrap();
    assert_eq!(twice, Appended::AlreadyEnded(done.clone()));
    assert_eq!(lab.family_trials(&family).await.unwrap().len(), 2);
    // Pedirlo de nuevo devuelve el trabajo listo.
    let again = lab.enqueue(&req, &req_hash, &family, t0).await.unwrap();
    assert_eq!(again, Enqueued::Existing(done));
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn a_cancelled_job_saves_nothing_and_the_same_request_gets_a_new_attempt() {
    let _queue = QUEUE.lock().await;
    let lab = lab().await;
    let (req_hash, family) = (fresh("LJREQ"), fresh("LJFAM"));
    let req = request(1);
    let t0 = at("2026-11-27T01:00:00Z");
    let Enqueued::New(first) = lab.enqueue(&req, &req_hash, &family, t0).await.unwrap() else {
        panic!("nuevo");
    };
    let Some(Appended::Added(cancelled)) = lab.cancel(&first.job_id, t0).await.unwrap() else {
        panic!("cancelado");
    };
    assert_eq!(cancelled.state, JobState::Cancelled);
    // El worker termina tarde: no se guarda ningún informe ni variante.
    let (s, r) = (fresh("LJS"), fresh("LJR"));
    let late = lab
        .finish(
            &first.job_id,
            &[report(&r, &s, &family)],
            &[trial(&s, &family, &r)],
            t0,
        )
        .await
        .unwrap();
    assert_eq!(late, Appended::AlreadyEnded(cancelled.clone()));
    assert!(lab.report(&r).await.unwrap().is_none());
    assert!(lab.family_trials(&family).await.unwrap().is_empty());
    // El mismo pedido otra vez: intento 2, otro trabajo.
    let Enqueued::New(second) = lab.enqueue(&req, &req_hash, &family, t0).await.unwrap() else {
        panic!("intento nuevo");
    };
    assert_ne!(second.job_id, first.job_id);
    assert_eq!(second.job_id, cfds_backtest::lab::job_id(&req_hash, 2));
    // Un trabajo desconocido.
    assert_eq!(lab.cancel(&fresh("LJX"), t0).await.unwrap(), None);
    assert!(lab.last_event(&fresh("LJX")).await.unwrap().is_none());
    lab.cancel(&second.job_id, t0).await.unwrap();
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn running_jobs_fail_on_restart_and_the_tables_are_append_only() {
    let _queue = QUEUE.lock().await;
    let lab = lab().await;
    let (req_hash, family) = (fresh("LJREQ"), fresh("LJFAM"));
    let t0 = at("2026-11-27T01:00:00Z");
    let Enqueued::New(job) = lab
        .enqueue(&request(1), &req_hash, &family, t0)
        .await
        .unwrap()
    else {
        panic!("nuevo");
    };
    claim(&lab, &job.job_id).await;
    let failed = lab.fail_running("el worker se reinició", t0).await.unwrap();
    let mine = failed.iter().find(|e| e.job_id == job.job_id).unwrap();
    assert_eq!(mine.state, JobState::Failed);
    assert_eq!(mine.error.as_deref(), Some("el worker se reinició"));

    let store = PgStore::connect(&test_database_url()).await.unwrap();
    for sql in [
        "UPDATE lab_jobs SET attempt = 9 WHERE job_id = $1",
        "DELETE FROM lab_job_events WHERE job_id = $1",
    ] {
        let err = sqlx::query(sql)
            .bind(&job.job_id)
            .execute(store.pool())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("solo agregar"), "{err}");
    }
    // Un segundo final directo en la tabla lo frena el índice único.
    let dup = sqlx::query(
        "INSERT INTO lab_job_events (job_id, state, percent, variants_done, recorded_at)
         VALUES ($1, 'cancelled', 0, 0, now())",
    )
    .bind(&job.job_id)
    .execute(store.pool())
    .await;
    assert!(dup.is_err());
}

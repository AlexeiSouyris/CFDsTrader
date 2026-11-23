//! Diario en PostgreSQL (#62). Necesitan `DATABASE_URL`; están marcados `#[ignore]` para que
//! `cargo test` normal no los corra, y el CI los corre con `-- --ignored` (job de integración).

use cfds_backtest::{CloseReason, ExitReason, ExitRecord, TradeRecord};
use cfds_core::{AssetClass, Currency, Instrument};
use cfds_journal::pg::{JournalError, PgJournal, Recorded};
use cfds_journal::{Outcome, OutcomeKind, Proposal, from_backtest};
use cfds_storage::PgStore;
use cfds_strategy::{Component, Side, TriggerKind};
use chrono::{DateTime, Utc};
use rust_decimal::dec;

#[path = "common/run_sample.rs"]
mod run_sample;

const NEEDS: &str = "necesita PostgreSQL: DATABASE_URL";

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

/// Base migrada, con el instrumento cargado y sin entradas previas de `run_id`.
async fn journal(run_id: &str) -> PgJournal {
    let url = std::env::var("DATABASE_URL").expect(NEEDS);
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    store
        .upsert_instrument(&Instrument {
            id: "US100".parse().unwrap(),
            name: "US Tech 100".into(),
            asset_class: AssetClass::Index,
            currency: Currency::USD,
            exchange_timezone: "America/New_York".into(),
        })
        .await
        .unwrap();
    let pool = store.pool().clone();
    sqlx::query(
        "DELETE FROM outcomes WHERE proposal_id IN (SELECT id FROM proposals WHERE run_id = $1)",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM proposals WHERE run_id = $1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    PgJournal::new(pool)
}

fn entry(run_id: &str) -> (Proposal, Outcome) {
    let exit = |time: &str, price, reason| ExitRecord {
        index: 2,
        time: at(time),
        mid: price,
        price,
        fraction: dec!(0.5),
        reason: CloseReason::Exit(reason),
    };
    let record = TradeRecord {
        side: Side::Long,
        signal_index: 0,
        signal_at: at("2026-11-19T14:45:00Z"),
        score: 6,
        breakdown: vec![
            (Component::Trend, 2),
            (Component::Trigger(TriggerKind::Sweep), 2),
            (Component::News, -1),
        ],
        triggers: vec![TriggerKind::Sweep],
        reference: dec!(100),
        tp1: dec!(104),
        entry_index: 1,
        entry_time: at("2026-11-19T14:45:00Z"),
        entry_mid: dec!(100),
        entry_price: dec!(100.7),
        stop: dec!(98),
        size: dec!(2),
        leverage: dec!(0.2),
        margin: dec!(10),
        exits: vec![
            exit("2026-11-19T15:15:00Z", dec!(105.4), ExitReason::Tp1),
            exit("2026-11-19T15:30:00Z", dec!(99.30), ExitReason::BreakEven),
        ],
        commission: dec!(0),
        funding: dec!(-0.35),
        mid_pnl: dec!(5),
        gross_pnl: dec!(3.4),
        net_pnl: dec!(3.75),
    };
    from_backtest(run_id, &"US100".parse().unwrap(), &record)
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn an_entry_is_stored_once_and_reads_back_identical() {
    let j = journal("it-journal-roundtrip").await;
    let (p, o) = entry("it-journal-roundtrip");
    assert_eq!(j.record(&p, Some(&o)).await.unwrap(), Recorded::Inserted);
    // Volver a guardarla no duplica nada.
    assert_eq!(j.record(&p, Some(&o)).await.unwrap(), Recorded::Unchanged);
    assert_eq!(j.proposal(&p.id).await.unwrap(), Some(p.clone()));
    assert_eq!(
        j.outcome(&p.id, OutcomeKind::Simulated).await.unwrap(),
        Some(o)
    );
    assert_eq!(j.outcome(&p.id, OutcomeKind::Real).await.unwrap(), None);
    assert_eq!(j.proposal("no-existe").await.unwrap(), None);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn a_proposal_without_outcome_can_get_it_later() {
    let j = journal("it-journal-later").await;
    let (p, o) = entry("it-journal-later");
    assert_eq!(j.record(&p, None).await.unwrap(), Recorded::Inserted);
    assert_eq!(
        j.outcome(&p.id, OutcomeKind::Simulated).await.unwrap(),
        None
    );
    assert_eq!(j.record(&p, Some(&o)).await.unwrap(), Recorded::Inserted);
    assert_eq!(
        j.outcome(&p.id, OutcomeKind::Simulated).await.unwrap(),
        Some(o)
    );
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_journal_is_never_rewritten_silently() {
    let j = journal("it-journal-conflict").await;
    let (p, o) = entry("it-journal-conflict");
    j.record(&p, Some(&o)).await.unwrap();

    let mut other = p.clone();
    other.size = dec!(3);
    assert!(matches!(
        j.record(&other, None).await,
        Err(JournalError::Conflict { .. })
    ));
    let mut worse = o.clone();
    worse.net_pnl = dec!(-1);
    assert!(matches!(
        j.record(&p, Some(&worse)).await,
        Err(JournalError::Conflict { .. })
    ));
    // Un resultado de otra propuesta no se cuelga de esta.
    let mut stray = o.clone();
    stray.proposal_id = "otra".into();
    assert!(matches!(
        j.record(&p, Some(&stray)).await,
        Err(JournalError::Conflict { .. })
    ));
    // Lo guardado sigue intacto.
    assert_eq!(j.proposal(&p.id).await.unwrap(), Some(p.clone()));
    assert_eq!(
        j.outcome(&p.id, OutcomeKind::Simulated).await.unwrap(),
        Some(o)
    );
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_database_rejects_impossible_proposals() {
    let j = journal("it-journal-checks").await;
    let (mut p, _) = entry("it-journal-checks");
    // Stop del lado equivocado: aunque Rust lo dejara pasar, la base no.
    p.stop = dec!(101);
    assert!(matches!(j.record(&p, None).await, Err(JournalError::Db(_))));
    // Un instrumento que no existe tampoco.
    let (mut q, _) = entry("it-journal-checks");
    q.instrument_id = "NOPE".parse().unwrap();
    assert!(matches!(j.record(&q, None).await, Err(JournalError::Db(_))));
    // Y nada quedó a medias (la transacción se deshizo).
    assert_eq!(j.proposal(&p.id).await.unwrap(), None);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn a_backtest_run_is_stored_once_and_never_rewritten() {
    // La tabla es de solo agregar: un run_id propio por corrida del test.
    let run_id = format!("bt-run-{}", Utc::now().timestamp_nanos_opt().unwrap());
    let j = journal(&run_id).await;
    let run = run_sample::sample(&run_id);
    assert!(j.run(&run_id).await.unwrap().is_none());
    assert_eq!(j.record_run(&run).await.unwrap(), Recorded::Inserted);
    assert_eq!(j.run(&run_id).await.unwrap(), Some(run.clone()));

    // La misma corrida otra vez, más tarde: no guarda nada y conserva la primera.
    let mut again = run.clone();
    again.started_at = run.started_at + chrono::TimeDelta::hours(1);
    again.finished_at = run.finished_at + chrono::TimeDelta::hours(1);
    assert_eq!(j.record_run(&again).await.unwrap(), Recorded::Unchanged);
    assert_eq!(j.run(&run_id).await.unwrap(), Some(run.clone()));

    // El mismo run_id con otro resultado: error, el diario no se reescribe.
    let mut other = run.clone();
    other.metrics.trades = 99;
    assert!(matches!(
        j.record_run(&other).await,
        Err(JournalError::Conflict { .. })
    ));

    // Sin noticias también se guarda (news = NULL).
    let quiet_id = format!("{run_id}-sin-noticias");
    let mut quiet = run_sample::sample(&quiet_id);
    quiet.news = None;
    assert_eq!(j.record_run(&quiet).await.unwrap(), Recorded::Inserted);
    assert_eq!(j.run(&quiet_id).await.unwrap(), Some(quiet));

    // Una corrida inválida no llega a la base.
    let mut bad = run_sample::sample(&format!("{run_id}-mala"));
    bad.finished_at = bad.started_at - chrono::TimeDelta::seconds(1);
    assert!(matches!(
        j.record_run(&bad).await,
        Err(JournalError::InvalidRun(_))
    ));

    // La base impide modificar o borrar.
    let url = std::env::var("DATABASE_URL").expect(NEEDS);
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    for sql in [
        "UPDATE backtest_runs SET timeframe = '1h' WHERE run_id = $1",
        "DELETE FROM backtest_runs WHERE run_id = $1",
    ] {
        assert!(
            sqlx::query(sql).bind(&run_id).execute(&pool).await.is_err(),
            "{sql}"
        );
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn ml_features_are_stored_once_and_never_rewritten() {
    use cfds_journal::ml_features::{MlFeatures, SessionBand, VolTercile};
    // `ml_features` es de solo agregar: este run_id no se borra entre corridas, así que todo
    // tiene que ser idempotente (la propuesta y sus variables, siempre las mismas).
    let j = journal("it-ml-features-setup").await;
    let (p, o) = entry("it-ml-features");
    j.record(&p, Some(&o)).await.unwrap();
    let f = MlFeatures {
        proposal_id: p.id.clone(),
        as_of: p.signal_at,
        atr14_m15: dec!(4.25),
        realized_vol_tercile: VolTercile::Low,
        session_band: SessionBand::Mid,
    };
    let first = j.record_ml_features(&f).await.unwrap();
    assert!(matches!(first, Recorded::Inserted | Recorded::Unchanged));
    assert_eq!(j.record_ml_features(&f).await.unwrap(), Recorded::Unchanged);
    assert_eq!(j.ml_features(&p.id).await.unwrap(), Some(f.clone()));
    assert_eq!(j.ml_features("no-existe").await.unwrap(), None);

    let mut other = f.clone();
    other.session_band = SessionBand::Close;
    assert!(matches!(
        j.record_ml_features(&other).await,
        Err(JournalError::Conflict { .. })
    ));
    // Sin propuesta guardada, la base lo rechaza (clave foránea).
    let mut orphan = f;
    orphan.proposal_id = "it-ml-features:sin-propuesta".into();
    assert!(matches!(
        j.record_ml_features(&orphan).await,
        Err(JournalError::Db(_))
    ));
}

/// #299: la curva se guarda una vez por corrida y el detalle junta corrida, curva y operaciones.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_run_detail_joins_the_run_its_curve_and_its_trades() {
    use cfds_journal::{RunCurve, RunTrade};
    // Tablas de solo agregar: un run_id propio por corrida del test.
    let run_id = format!("bt-detail-{}", Utc::now().timestamp_nanos_opt().unwrap());
    let j = journal(&run_id).await;
    assert!(j.run_detail(&run_id).await.unwrap().is_none());

    let run = run_sample::sample(&run_id);
    j.record_run(&run).await.unwrap();
    // sin curva todavía: el detalle sale igual, con curve = None
    let detail = j.run_detail(&run_id).await.unwrap().unwrap();
    assert_eq!(detail.run, run);
    assert_eq!(detail.curve, None);
    assert!(detail.trades.is_empty());

    let day = |d| chrono::NaiveDate::from_ymd_opt(2026, 11, d).unwrap();
    let curve = RunCurve::new(
        &run_id,
        run.initial_equity,
        &[(day(16), dec!(10100)), (day(17), dec!(9999))],
        vec![],
    )
    .unwrap();
    assert_eq!(
        j.record_run_curve(&curve).await.unwrap(),
        Recorded::Inserted
    );
    assert_eq!(
        j.record_run_curve(&curve).await.unwrap(),
        Recorded::Unchanged
    );
    assert_eq!(j.run_curve(&run_id).await.unwrap(), Some(curve.clone()));
    let mut other = curve.clone();
    other.points.pop();
    assert!(matches!(
        j.record_run_curve(&other).await,
        Err(JournalError::Conflict { .. })
    ));

    let (p, o) = entry(&run_id);
    j.record(&p, Some(&o)).await.unwrap();
    let detail = j.run_detail(&run_id).await.unwrap().unwrap();
    assert_eq!(detail.curve, Some(curve));
    assert_eq!(
        detail.trades,
        vec![RunTrade {
            proposal: p,
            outcome: Some(o)
        }]
    );

    // una curva de una corrida que no existe no entra (clave foránea)
    let orphan = RunCurve::new("no-existe-299", dec!(1), &[], vec![]).unwrap();
    assert!(j.record_run_curve(&orphan).await.is_err());

    // la base impide modificar o borrar la curva
    let url = std::env::var("DATABASE_URL").expect(NEEDS);
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    for sql in [
        "UPDATE backtest_run_curves SET buy_and_hold = '[]'::jsonb WHERE run_id = $1",
        "DELETE FROM backtest_run_curves WHERE run_id = $1",
    ] {
        assert!(
            sqlx::query(sql).bind(&run_id).execute(&pool).await.is_err(),
            "{sql}"
        );
    }
}

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

//! Propuestas de cambio y votos (fase 8b, spec §1.2, #327): contar votos nunca aplica nada; en
//! PostgreSQL todo es de solo agregar y hay un voto por miembro. Datos sintéticos.

use cfds_core::{AssetClass, Currency, Instrument, InstrumentId};
use cfds_journal::learning::{ChangeKind, Inputs, LearningConfig, PENDING_VOTE, report};
use cfds_journal::pg::{JournalError, PgJournal, Recorded};
use cfds_journal::probability::{ProbabilityConfig, ProbabilityTable};
use cfds_journal::votes::{StoredChange, Vote, VoteRules, VoteStatus, tally};
use cfds_journal::{
    BreakdownItem, Direction, Exit, Outcome, OutcomeKind, Proposal, Reason, Signal, Source,
};
use cfds_storage::PgStore;
use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::Value;

const VOTES: &str = include_str!("../../../../config/votes.toml");

fn rules() -> VoteRules {
    VoteRules::from_toml(VOTES).unwrap()
}

fn at(h: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 3, 2, 9, 0, 0).unwrap() + Duration::hours(h)
}

fn change(id: &str) -> StoredChange {
    StoredChange {
        change_id: id.into(),
        report_id: "learning:r".into(),
        kind: ChangeKind::Recalibrate,
        text: "Recalibrar el rango 50%–60%".into(),
        evidence: "calibration.rows[5]".into(),
        status: PENDING_VOTE.into(),
        created_at: at(0),
    }
}

fn vote(change: &str, member: &str, yes: bool, h: i64) -> Vote {
    Vote {
        vote_id: format!("{change}:{member}"),
        change_id: change.into(),
        member_id: member.into(),
        in_favor: yes,
        voted_at: at(h),
    }
}

fn validator(name: &str) -> jsonschema::Validator {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let text = std::fs::read_to_string(path).unwrap();
    jsonschema::validator_for(&serde_json::from_str(&text).unwrap()).unwrap()
}

#[test]
fn the_rules_are_the_spec_and_bad_ones_are_rejected() {
    let r = rules();
    assert_eq!((r.members, r.majority), (6, 4));
    assert_eq!(r.window, Duration::hours(48));
    for (old, new) in [
        ("majority = 4", "majority = 3"),
        ("majority = 4", "majority = 7"),
        ("window_hours = 48", "window_hours = 0"),
    ] {
        let bad = VOTES.replacen(old, new, 1);
        assert!(VoteRules::from_toml(&bad).is_err(), "{new}");
    }
}

#[test]
fn four_of_six_within_48_hours_approves_and_nothing_else_does() {
    let c = change("c1");
    let r = rules();
    let mut v = vec![
        vote("c1", "ana", true, 1),
        vote("c1", "beto", true, 2),
        vote("c1", "caro", true, 3),
    ];
    assert_eq!(
        tally(&c, &v, &r, at(4)),
        VoteStatus::PendienteDeVoto {
            in_favor: 3,
            against: 0
        }
    );
    // El mismo miembro otra vez no suma; un voto de otra propuesta tampoco.
    v.push(vote("c1", "ana", true, 5));
    v.push(vote("otra", "dani", true, 5));
    assert!(matches!(
        tally(&c, &v, &r, at(6)),
        VoteStatus::PendienteDeVoto { in_favor: 3, .. }
    ));
    // Sin llegar a 4 en 48 h: rechazada.
    assert!(matches!(
        tally(&c, &v, &r, at(49)),
        VoteStatus::Rechazada { in_favor: 3, .. }
    ));
    // Un voto después del plazo no cuenta.
    v.push(vote("c1", "dani", true, 50));
    assert!(matches!(
        tally(&c, &v, &r, at(51)),
        VoteStatus::Rechazada { in_favor: 3, .. }
    ));
    // El cuarto a favor dentro del plazo aprueba.
    let mut ok = v.clone();
    ok.push(vote("c1", "eva", true, 47));
    assert!(matches!(
        tally(&c, &ok, &r, at(48)),
        VoteStatus::Aprobada { in_favor: 4, .. }
    ));
}

#[test]
fn three_against_rejects_early() {
    let c = change("c2");
    let v = [
        vote("c2", "ana", false, 1),
        vote("c2", "beto", false, 1),
        vote("c2", "caro", false, 1),
    ];
    assert_eq!(
        tally(&c, &v, &rules(), at(2)),
        VoteStatus::Rechazada {
            in_favor: 0,
            against: 3
        }
    );
}

#[test]
fn proposals_and_votes_follow_their_contracts() {
    let c = serde_json::to_value(change("c1")).unwrap();
    assert!(validator("change_proposal.v1.schema.json").is_valid(&c));
    let v = serde_json::to_value(vote("c1", "ana", true, 1)).unwrap();
    assert!(validator("change_vote.v1.schema.json").is_valid(&v));
    let mut not_pending = c.clone();
    not_pending["status"] = Value::from("aprobada");
    assert!(!validator("change_proposal.v1.schema.json").is_valid(&not_pending));
}

/// Una propuesta con su resultado. Con R = |100 − 99| × 1, `net` es R.
fn pair(source: Source, tag: &str, i: u32, start: DateTime<Utc>, hit: bool) -> (Proposal, Outcome) {
    let s = start + Duration::minutes(15 * i64::from(i));
    let id = format!("{tag}:US100:{i}:long");
    let net = if hit { dec!(2) } else { dec!(-1) };
    let p = Proposal {
        id: id.clone(),
        source,
        run_id: tag.into(),
        instrument_id: "US100".parse::<InstrumentId>().unwrap(),
        side: Direction::Long,
        signal_at: s,
        reference: dec!(100),
        stop: dec!(99),
        tp1: dec!(102),
        size: dec!(1),
        leverage: dec!(1),
        score: 5,
        breakdown: vec![BreakdownItem {
            component: Signal::Trend,
            points: 2,
        }],
        triggers: vec![Signal::CandlePattern],
        shown: None,
    };
    let reason = if hit { Reason::Tp1 } else { Reason::Stop };
    let o = Outcome {
        proposal_id: id,
        kind: OutcomeKind::Simulated,
        entry_at: s + Duration::minutes(15),
        entry_price: dec!(100),
        exits: vec![Exit {
            at: s + Duration::hours(1),
            price: dec!(100) + net,
            fraction: dec!(1),
            reason,
        }],
        commission: Decimal::ZERO,
        funding: Decimal::ZERO,
        gross_pnl: net,
        net_pnl: net,
        hit_tp1: hit,
        hit_stop: !hit,
        exit_reason: reason,
    };
    (p, o)
}

/// De punta a punta con la base: un backtest de entrenamiento y un período de paper sintéticos,
/// el informe con su propuesta, y los votos. Nada cambia en config.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_report_its_proposals_and_votes_are_stored_once() {
    let url = std::env::var("DATABASE_URL").expect("necesita PostgreSQL: DATABASE_URL");
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
    let j = PgJournal::new(store.pool().clone());
    // Ids propios por corrida del test (solo agregar). Fechas lejanas para no mezclarse.
    let tag = format!("t{}", Utc::now().timestamp_nanos_opt().unwrap());
    let base = Utc.with_ymd_and_hms(2090, 1, 5, 0, 0, 0).unwrap()
        + Duration::minutes(Utc::now().timestamp() % 100_000);
    let train_tag = format!("{tag}-bt");
    for i in 0..40 {
        let (p, o) = pair(Source::Backtest, &train_tag, i, base, i % 2 == 0);
        j.record(&p, Some(&o)).await.unwrap();
    }
    let from = base + Duration::days(30);
    let to = from + Duration::days(30);
    for i in 0..40 {
        let (p, o) = pair(Source::Paper, &format!("{tag}-pp"), i, from, i % 4 == 0);
        j.record(&p, Some(&o)).await.unwrap();
    }
    // Una propuesta sin resultado todavía no cuenta.
    let (p, _) = pair(Source::Paper, &format!("{tag}-pp"), 99, from, true);
    j.record(&p, None).await.unwrap();

    let train: Vec<_> = j
        .run_trades(&train_tag)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|t| Some((t.proposal, t.outcome?)))
        .collect();
    let probability =
        ProbabilityConfig::from_toml(include_str!("../../../../config/probability.toml")).unwrap();
    let cfg = LearningConfig::from_toml(include_str!("../../../../config/learning.toml")).unwrap();
    let table = ProbabilityTable::build(&train, from, &probability).unwrap();
    let pairs = j.pairs(Source::Paper, from, to).await.unwrap();
    assert_eq!(pairs.len(), 40);
    let r = report(&Inputs {
        pairs: &pairs,
        table: &table,
        probability: &probability,
        cfg: &cfg,
        from,
        to,
        created_at: to,
    });
    assert_eq!(r.change_proposals.len(), 1, "{:?}", r.change_proposals);

    assert_eq!(
        j.record_learning_report(&r).await.unwrap(),
        Recorded::Inserted
    );
    assert_eq!(
        j.record_learning_report(&r).await.unwrap(),
        Recorded::Unchanged
    );
    let stored = j.change_proposals(&r.report_id).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, PENDING_VOTE);

    let id = stored[0].change_id.clone();
    let ana = Vote {
        vote_id: format!("{tag}-ana"),
        change_id: id.clone(),
        member_id: "ana".into(),
        in_favor: true,
        voted_at: to + Duration::hours(1),
    };
    assert_eq!(j.record_vote(&ana).await.unwrap(), Recorded::Inserted);
    assert_eq!(j.record_vote(&ana).await.unwrap(), Recorded::Unchanged);
    let changed_mind = Vote {
        vote_id: format!("{tag}-ana-2"),
        in_favor: false,
        ..ana.clone()
    };
    assert!(matches!(
        j.record_vote(&changed_mind).await,
        Err(JournalError::Conflict { .. })
    ));
    let votes = j.votes(&id).await.unwrap();
    assert_eq!(votes, vec![ana]);
    assert!(matches!(
        tally(&stored[0], &votes, &rules(), to + Duration::hours(2)),
        VoteStatus::PendienteDeVoto { in_favor: 1, .. }
    ));
    // Lo que lee el gateway (#331): el último informe y el estado de sus propuestas.
    let latest = j.latest_learning_report().await.unwrap().unwrap();
    let latest_at: DateTime<Utc> = latest["created_at"].as_str().unwrap().parse().unwrap();
    assert!(
        latest_at >= r.created_at,
        "el último es el creado más tarde"
    );
    if latest["report_id"] == r.report_id.as_str() {
        assert_eq!(latest, serde_json::to_value(&r).unwrap());
    }
    let statuses = j
        .change_statuses(&r.report_id, &rules(), to + Duration::hours(2))
        .await
        .unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].change_id, id);
    assert_eq!(
        statuses[0].status,
        VoteStatus::PendienteDeVoto {
            in_favor: 1,
            against: 0
        }
    );
    let doc = serde_json::to_value(&statuses[0]).unwrap();
    assert!(
        validator("change_vote_status.v1.schema.json").is_valid(&doc),
        "{doc}"
    );
    for (sql, key) in [
        (
            "UPDATE learning_reports SET period_to = period_to WHERE report_id = $1",
            r.report_id.clone(),
        ),
        (
            "UPDATE change_proposals SET created_at = created_at WHERE change_id = $1",
            id.clone(),
        ),
        ("DELETE FROM change_votes WHERE change_id = $1", id.clone()),
    ] {
        assert!(
            sqlx::query(sql)
                .bind(&key)
                .execute(store.pool())
                .await
                .is_err(),
            "{sql}"
        );
    }
}

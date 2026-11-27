//! El estado de la Academia sale de leer los eventos en orden; la UI no decide nada.

mod common;

use cfds_academy::{
    AcademyConfig, AcademyEvent, EventBody, EventRequest, OverrideAction, Progress, QuizAnswer,
    UnlockedBy, grade_quiz, progress,
};
use chrono::{DateTime, TimeDelta, Utc};
use common::{
    assert_valid, def_validator, replay_result, validator, with_questions, with_replay_exam,
};
use serde_json::json;

fn t(min: i64) -> DateTime<Utc> {
    "2026-11-26T21:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(min)
}

fn ev(cfg: &AcademyConfig, seq: i64, body: EventBody) -> AcademyEvent {
    AcademyEvent {
        seq,
        member_id: "owner".into(),
        body,
        catalog_version: cfg.catalog.catalog_version.clone(),
        recorded_at: t(seq),
    }
}

fn attempt(cfg: &AcademyConfig, seq: i64, choice: &str) -> AcademyEvent {
    let a: QuizAnswer = serde_json::from_value(json!({
        "lesson_id": "0.1",
        "catalog_version": cfg.catalog.catalog_version,
        "answers": [{"question_id": "0.1.1", "choice": choice}, {"question_id": "0.1.2", "choice": "a"}],
    }))
    .unwrap();
    let quiz = Box::new(grade_quiz(cfg, &a, t(seq)).unwrap());
    ev(
        cfg,
        seq,
        EventBody::ExamAttempt {
            lesson_id: "0.1".into(),
            quiz,
        },
    )
}

fn over(cfg: &AcademyConfig, seq: i64, tool: &str, action: OverrideAction) -> AcademyEvent {
    ev(
        cfg,
        seq,
        EventBody::ToolOverride {
            tool_id: tool.into(),
            action,
        },
    )
}

fn tool<'a>(p: &'a Progress, id: &str) -> &'a cfds_academy::progress::ToolProgress {
    p.tools.iter().find(|t| t.tool_id == id).unwrap()
}

#[test]
fn nothing_done_is_all_locked() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let p = progress(&cfg, "owner", &[], t(0));
    // Una entrada por lección del catálogo (crece con cada nivel nuevo, #819).
    assert_eq!(p.lessons.len(), cfg.lessons().count());
    assert!(p.lessons.iter().all(|l| !l.passed && l.attempts == 0));
    assert!(p.tools.iter().all(|t| !t.unlocked && t.by.is_none()));
    assert!(!p.unlock_all);
    assert_valid(
        "academy_progress.v1.schema.json",
        &serde_json::to_value(&p).unwrap(),
    );
}

#[test]
fn passing_the_exam_unlocks_its_tools_and_every_attempt_counts() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let events = [
        attempt(&cfg, 1, "a"),
        attempt(&cfg, 2, "b"),
        attempt(&cfg, 3, "a"),
    ];
    let p = progress(&cfg, "owner", &events, t(9));
    let l = &p.lessons[0];
    assert_eq!((l.passed, l.attempts), (true, 3));
    assert_eq!(l.passed_at, Some(t(2)));
    assert_eq!(l.best_score_pct.unwrap().to_string(), "100");
    assert_eq!(tool(&p, "tool.watchlist").by, Some(UnlockedBy::Lesson));
    assert_eq!(
        tool(&p, "tool.instrument_panel").by,
        Some(UnlockedBy::Lesson)
    );
    assert!(!tool(&p, "tool.glossary").unlocked);
    let doc = serde_json::to_value(&p).unwrap();
    assert_valid("academy_progress.v1.schema.json", &doc);
    assert_eq!(serde_json::from_value::<Progress>(doc).unwrap(), p);
    // De otra persona: no cuenta.
    assert!(!progress(&cfg, "otro", &events, t(9)).lessons[0].passed);
}

#[test]
fn a_lesson_without_exam_is_passed_by_completing_it_but_an_exam_lesson_is_not() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let done = |l: &str| EventBody::LessonCompleted {
        lesson_id: l.into(),
    };
    let p = progress(
        &cfg,
        "owner",
        &[ev(&cfg, 1, done("0.3")), ev(&cfg, 2, done("0.1"))],
        t(9),
    );
    let by_id = |id: &str| p.lessons.iter().find(|l| l.lesson_id == id).unwrap().passed;
    assert!(by_id("0.3"));
    assert!(!by_id("0.1"));
}

#[test]
fn manual_unlock_and_relock_follow_the_last_event_that_reaches_the_tool() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    use OverrideAction::{Relock, Unlock};
    let events = vec![
        over(&cfg, 1, "*", Unlock),
        over(&cfg, 2, "tool.glossary", Relock),
    ];
    let p = progress(&cfg, "owner", &events, t(9));
    assert!(p.unlock_all);
    assert_eq!(tool(&p, "tool.watchlist").by, Some(UnlockedBy::Override));
    assert!(!tool(&p, "tool.glossary").unlocked);
    // Un rebloqueo de todo después lo cierra todo... salvo lo aprobado.
    let mut events = events;
    events.push(attempt(&cfg, 3, "b"));
    events.push(over(&cfg, 4, "*", Relock));
    events.push(over(&cfg, 5, "tool.cost_calculator", Unlock));
    let p = progress(&cfg, "owner", &events, t(9));
    assert!(!p.unlock_all);
    assert_eq!(tool(&p, "tool.watchlist").by, Some(UnlockedBy::Lesson));
    assert!(!tool(&p, "tool.source_selector").unlocked);
    assert_eq!(
        tool(&p, "tool.cost_calculator").by,
        Some(UnlockedBy::Override)
    );
    // El orden es el de `seq`, no el de la lista.
    let mut shuffled = events.clone();
    shuffled.reverse();
    assert_eq!(progress(&cfg, "owner", &shuffled, t(9)), p);
}

#[test]
fn events_roundtrip_and_the_contract_and_rust_reject_the_same_mixes() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    for e in [
        attempt(&cfg, 1, "b"),
        over(&cfg, 2, "*", OverrideAction::Unlock),
        ev(
            &cfg,
            3,
            EventBody::LessonCompleted {
                lesson_id: "0.3".into(),
            },
        ),
    ] {
        let doc = serde_json::to_value(&e).unwrap();
        assert_valid("academy_event.v1.schema.json", &doc);
        assert_eq!(serde_json::from_value::<AcademyEvent>(doc).unwrap(), e);
    }
    let base =
        serde_json::to_value(over(&cfg, 2, "tool.glossary", OverrideAction::Relock)).unwrap();
    let mut with_lesson = base.clone();
    with_lesson["lesson_id"] = json!("0.1");
    let mut no_action = base.clone();
    no_action["action"] = json!(null);
    let mut bad_tool = base.clone();
    bad_tool["tool_id"] = json!("tool.X");
    let mut no_seq = base;
    no_seq["seq"] = json!(0);
    for doc in [with_lesson, no_action, bad_tool, no_seq] {
        assert!(
            !validator("academy_event.v1.schema.json").is_valid(&doc),
            "{doc}"
        );
        assert!(serde_json::from_value::<AcademyEvent>(doc).is_err());
    }
}

#[test]
fn requests_are_checked_against_the_catalog() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let req = |doc: serde_json::Value| {
        let schema_ok = def_validator("academy_event.v1.schema.json", "request").is_valid(&doc);
        let parsed = serde_json::from_value::<EventRequest>(doc);
        (
            schema_ok,
            parsed.map_err(|e| e.to_string()).and_then(|r| r.body(&cfg)),
        )
    };
    let (s, b) = req(json!({"kind": "tool_override", "tool_id": "*", "action": "unlock"}));
    assert!(s);
    assert_eq!(
        b,
        Ok(EventBody::ToolOverride {
            tool_id: "*".into(),
            action: OverrideAction::Unlock
        })
    );
    assert_eq!(
        req(json!({"kind": "lesson_completed", "lesson_id": "0.3"})).1,
        Ok(EventBody::LessonCompleted {
            lesson_id: "0.3".into()
        })
    );
    // Válidos para el contrato pero no para el catálogo.
    for doc in [
        json!({"kind": "lesson_completed", "lesson_id": "0.1"}),
        json!({"kind": "lesson_completed", "lesson_id": "9.9"}),
        json!({"kind": "tool_override", "tool_id": "tool.nada", "action": "unlock"}),
    ] {
        let (s, b) = req(doc.clone());
        assert!(s && b.is_err(), "{doc}");
    }
    // Inválidos para los dos.
    for doc in [
        json!({"kind": "exam_attempt", "lesson_id": "0.1"}),
        json!({"kind": "tool_override", "tool_id": "*"}),
        json!({"kind": "lesson_completed", "lesson_id": "0.3", "action": "unlock"}),
        json!({"kind": "lesson_completed", "lesson_id": "0.3", "extra": 1}),
    ] {
        let (s, b) = req(doc.clone());
        assert!(!s && b.is_err(), "{doc}");
    }
}

fn replay_attempt(cfg: &AcademyConfig, seq: i64, passed: bool) -> AcademyEvent {
    let v = &cfg.catalog.catalog_version;
    ev(
        cfg,
        seq,
        EventBody::ReplayAttempt {
            lesson_id: "1.1".into(),
            replay: Box::new(replay_result(Some("1.1"), Some(passed), v)),
        },
    )
}

#[test]
fn a_passed_replay_exam_passes_its_lesson_and_every_attempt_counts() {
    let cfg = AcademyConfig::from_toml(&with_replay_exam()).unwrap();
    let one = progress(&cfg, "owner", &[replay_attempt(&cfg, 1, false)], t(9));
    let l = one.lessons.iter().find(|l| l.lesson_id == "1.1").unwrap();
    assert_eq!((l.passed, l.attempts), (false, 1));
    assert!(!tool(&one, "tool.source_selector").unlocked);
    let events = [
        replay_attempt(&cfg, 1, false),
        replay_attempt(&cfg, 2, true),
        replay_attempt(&cfg, 3, false),
    ];
    let p = progress(&cfg, "owner", &events, t(9));
    let l = p.lessons.iter().find(|l| l.lesson_id == "1.1").unwrap();
    // Aprobada desde el primer intento aprobado; un replay no tiene nota en %.
    assert_eq!(
        (l.passed, l.passed_at, l.attempts, l.best_score_pct),
        (true, Some(t(2)), 3, None)
    );
    let s = tool(&p, "tool.source_selector");
    assert_eq!((s.unlocked, s.by), (true, Some(UnlockedBy::Lesson)));
    assert_valid(
        "academy_progress.v1.schema.json",
        &serde_json::to_value(&p).unwrap(),
    );
}

#[test]
fn replay_attempts_roundtrip_and_mixes_are_rejected_by_the_contract_and_rust() {
    let cfg = AcademyConfig::from_toml(&with_replay_exam()).unwrap();
    let e = replay_attempt(&cfg, 4, true);
    let doc = serde_json::to_value(&e).unwrap();
    assert_valid("academy_event.v1.schema.json", &doc);
    assert_eq!(doc["kind"], "exam_attempt");
    assert_eq!(doc["quiz"], serde_json::Value::Null);
    assert_eq!(doc["replay"]["passed"], true);
    assert_eq!(
        serde_json::from_value::<AcademyEvent>(doc.clone()).unwrap(),
        e
    );
    // Un evento guardado antes de #759, sin el campo replay, sigue valiendo para los dos.
    let mut old = serde_json::to_value(attempt(&cfg, 1, "b")).unwrap();
    old.as_object_mut().unwrap().remove("replay");
    assert_valid("academy_event.v1.schema.json", &old);
    assert!(serde_json::from_value::<AcademyEvent>(old).is_ok());
    // Los dos a la vez, ninguno, o un replay en otro tipo de evento: inválidos para los dos.
    let quiz = serde_json::to_value(attempt(&cfg, 1, "b")).unwrap()["quiz"].clone();
    let mut both = doc.clone();
    both["quiz"] = quiz;
    let mut none = doc.clone();
    none["replay"] = serde_json::Value::Null;
    let mut on_override = serde_json::to_value(over(&cfg, 2, "*", OverrideAction::Unlock)).unwrap();
    on_override["replay"] = doc["replay"].clone();
    for bad in [both, none, on_override] {
        assert!(
            !validator("academy_event.v1.schema.json").is_valid(&bad),
            "{bad}"
        );
        assert!(serde_json::from_value::<AcademyEvent>(bad).is_err());
    }
    // El intento es de la lección de su sesión (Rust lo revisa; el contrato no puede).
    let mut other = doc;
    other["lesson_id"] = "0.1".into();
    assert!(serde_json::from_value::<AcademyEvent>(other).is_err());
}

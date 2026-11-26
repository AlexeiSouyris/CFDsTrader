//! Corrección de preguntas: la hace el motor, con las claves que nunca salen de él.

mod common;

use cfds_academy::{AcademyConfig, QuizAnswer, QuizError, QuizResult, grade_quiz};
use chrono::{DateTime, Utc};
use common::{REAL, assert_valid, validator, with_questions};
use serde_json::json;

fn at() -> DateTime<Utc> {
    "2026-11-26T21:00:00Z".parse().unwrap()
}

fn answer(cfg: &AcademyConfig, lesson: &str, answers: serde_json::Value) -> QuizAnswer {
    let doc = json!({
        "lesson_id": lesson,
        "catalog_version": cfg.catalog.catalog_version,
        "answers": answers,
    });
    assert_valid("quiz_answer.v1.schema.json", &doc);
    serde_json::from_value(doc).unwrap()
}

#[test]
fn all_right_passes_and_the_result_follows_the_contract() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let a = answer(
        &cfg,
        "0.1",
        json!([{"question_id": "0.1.1", "choice": "b"}, {"question_id": "0.1.2", "choice": "a"}]),
    );
    let r = grade_quiz(&cfg, &a, at()).unwrap();
    assert!(r.passed);
    assert_eq!((r.correct, r.total), (2, 2));
    let doc = serde_json::to_value(&r).unwrap();
    assert_eq!(doc["score_pct"], json!("100"));
    assert_eq!(doc["pass_pct"], json!("80"));
    assert_valid("quiz_result.v1.schema.json", &doc);
    assert_eq!(serde_json::from_value::<QuizResult>(doc).unwrap(), r);
}

#[test]
fn a_missing_or_wrong_answer_counts_as_wrong_and_the_score_rounds_down() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let a = answer(
        &cfg,
        "0.1",
        json!([{"question_id": "0.1.1", "choice": "a"}]),
    );
    let r = grade_quiz(&cfg, &a, at()).unwrap();
    assert!(!r.passed);
    assert_eq!(r.score_pct.to_string(), "0");
    assert!(r.questions.iter().all(|q| !q.correct));
    // 2 de 3 = 66,666…: hacia abajo, 66,66 (nunca se regala una décima).
    let text = with_questions().replace(
        "answer = \"a\"\n",
        "answer = \"a\"\n\n[[lessons.questions]]\nid = \"0.1.3\"\nkind = \"choice\"\nprompt = \"¿Y?\"\noptions = [{ id = \"a\", text = \"x\" }, { id = \"b\", text = \"y\" }]\nanswer = \"a\"\n",
    );
    let cfg = AcademyConfig::from_toml(&text).unwrap();
    let a = answer(
        &cfg,
        "0.1",
        json!([{"question_id": "0.1.1", "choice": "b"}, {"question_id": "0.1.2", "choice": "a"}]),
    );
    assert_eq!(
        grade_quiz(&cfg, &a, at()).unwrap().score_pct.to_string(),
        "66.66"
    );
}

#[test]
fn numbers_are_compared_with_the_tolerance_of_the_key() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    for (value, ok) in [
        ("0.25", true),
        ("0.26", true),
        ("0.24", true),
        ("0.2601", false),
        ("-0.25", false),
    ] {
        let a = answer(
            &cfg,
            "2.1",
            json!([{"question_id": "2.1.1", "value": value}]),
        );
        assert_eq!(grade_quiz(&cfg, &a, at()).unwrap().passed, ok, "{value}");
    }
}

#[test]
fn the_engine_refuses_what_it_cannot_grade() {
    let cfg = AcademyConfig::from_toml(&with_questions()).unwrap();
    let ok = json!([{"question_id": "0.1.1", "choice": "b"}]);
    let cases = [
        (
            answer(&cfg, "9.9", ok.clone()),
            QuizError::UnknownLesson("9.9".into()),
        ),
        (
            answer(&cfg, "0.3", json!([])),
            QuizError::NotAQuiz("0.3".into()),
        ),
        (
            answer(&cfg, "1.1", json!([])),
            QuizError::NotReady("1.1".into()),
        ),
        (
            answer(
                &cfg,
                "0.1",
                json!([{"question_id": "2.1.1", "value": "0.25"}]),
            ),
            QuizError::BadAnswer("2.1.1".into()),
        ),
        (
            answer(
                &cfg,
                "0.1",
                json!([{"question_id": "0.1.1", "choice": "z"}]),
            ),
            QuizError::BadAnswer("0.1.1".into()),
        ),
        (
            answer(&cfg, "0.1", json!([{"question_id": "0.1.1", "value": "1"}])),
            QuizError::BadAnswer("0.1.1".into()),
        ),
        (
            answer(
                &cfg,
                "0.1",
                json!([{"question_id": "0.1.1", "choice": "b"}, {"question_id": "0.1.1", "choice": "b"}]),
            ),
            QuizError::BadAnswer("0.1.1".into()),
        ),
    ];
    for (a, err) in cases {
        assert_eq!(grade_quiz(&cfg, &a, at()), Err(err));
    }
    let mut stale = answer(&cfg, "0.1", ok);
    stale.catalog_version = AcademyConfig::from_toml(REAL)
        .unwrap()
        .catalog
        .catalog_version;
    assert_eq!(grade_quiz(&cfg, &stale, at()), Err(QuizError::StaleCatalog));
}

#[test]
fn the_contract_and_rust_reject_the_same_answers() {
    let v = "a".repeat(64);
    for doc in [
        json!({"lesson_id": "0.1", "catalog_version": v, "answers": [{"question_id": "0.1.1", "choice": "a", "extra": 1}]}),
        json!({"lesson_id": "0.1", "catalog_version": v, "answers": [], "extra": 1}),
        json!({"lesson_id": "0.1", "catalog_version": v}),
    ] {
        assert!(
            !validator("quiz_answer.v1.schema.json").is_valid(&doc),
            "{doc}"
        );
        assert!(serde_json::from_value::<QuizAnswer>(doc).is_err());
    }
}

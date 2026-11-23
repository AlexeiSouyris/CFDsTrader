//! Rutas de las fases 8 y 8b (#331): comparación paper vs backtest e informe de aprendizaje,
//! solo lectura. Documentos de ejemplo con la forma de los contratos, no de mercado.

use std::path::PathBuf;

use cfds_gateway::learning::{
    ComparisonSource, LearningSource, is_learning_route, respond_learning,
};
use cfds_journal::votes::{ChangeStatus, StoredChange, Vote, VoteRules, tally};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn assert_valid(name: &str, doc: &Value) {
    let errors: Vec<String> = validator(name)
        .iter_errors(doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{name}: {errors:?}");
}

fn leg(entry: &str, exit: &str, pnl: &str) -> Value {
    json!({
        "entry_time": "2026-11-10T15:00:00Z", "entry_price": entry,
        "exit_time": "2026-11-10T18:00:00Z", "exit_price": exit,
        "size": "2", "commission": "0", "swap": "0", "net_pnl": pnl
    })
}

fn comparison(id: &str, instrument: &str) -> Value {
    json!({
        "comparison_id": id,
        "instrument_id": instrument,
        "from": "2026-11-02T00:00:00Z",
        "to": "2026-11-21T00:00:00Z",
        "created_at": "2026-11-23T10:00:00Z",
        "paper": { "trades": 2, "net_pnl": "36" },
        "backtest": { "trades": 2, "net_pnl": "45" },
        "difference": "-9",
        "causes": {
            "fill_price": "-1", "size": "0", "commission": "0", "swap": "0",
            "weekend_close": "0", "risk_rejection": "0", "missing_in_paper": "-20",
            "extra_in_paper": "12"
        },
        "matched": [{
            "decision_at": "2026-11-10T15:00:00Z", "side": "long", "order_id": "runner:1",
            "paper": leg("100.5", "112.5", "24"), "backtest": leg("100", "112.5", "25"),
            "difference": { "entry_price": "0.5", "exit_price": "0", "size": "0", "net_pnl": "-1" },
            "weekend_close": false
        }],
        "missing": [{
            "decision_at": "2026-11-12T10:00:00Z", "side": "short",
            "backtest_net_pnl": "20", "paper": "no_order"
        }],
        "extra": [{
            "decision_at": "2026-11-14T10:00:00Z", "side": "long", "order_id": "runner:2",
            "paper_net_pnl": "12", "backtest": "no_trade"
        }],
        "paper_rejections": { "stop_too_close": 1 }
    })
}

fn learning() -> Value {
    json!({
        "report_id": "learning:20261101T0000Z:20261121T0000Z:20261123T100000Z",
        "created_at": "2026-11-23T10:00:00Z",
        "from": "2026-11-01T00:00:00Z",
        "to": "2026-11-21T00:00:00Z",
        "sources": ["paper"],
        "proposals": 40,
        "calibration": {
            "rows": [{
                "from": "0.5", "to": "0.6", "cases": 34, "hits": 15, "mean_shown": "0.55",
                "frequency": "0.4412", "low": "0.2884", "high": "0.6063", "insufficient": false
            }, {
                "from": "0.6", "to": "1", "cases": 6, "hits": 4, "mean_shown": "0.64",
                "frequency": "0.6667", "low": "0.3", "high": "0.9032", "insufficient": true
            }],
            "without_probability": 0,
            "sample": []
        },
        "signals": [{
            "signal": "sweep",
            "with": { "cases": 12, "hits": 6, "hit_rate": "0.5", "mean_r": "0.3" },
            "without": { "cases": 28, "hits": 13, "hit_rate": "0.4643", "mean_r": "-0.1" },
            "difference_r": "0.4",
            "small_sample": true
        }],
        "change_proposals": [{
            "change_id": "learning:x:recalibrate:0",
            "kind": "recalibrate",
            "text": "Recalibrar el rango 50–60 %: se mostró 55 % y la frecuencia real fue 44 %.",
            "evidence": "calibration.rows[0]",
            "status": "pendiente_de_voto"
        }]
    })
}

struct Fake {
    comparisons: Result<Vec<Value>, String>,
}

impl ComparisonSource for Fake {
    async fn latest(&self) -> Result<Vec<Value>, String> {
        self.comparisons.clone()
    }
    async fn by_id(&self, id: &str) -> Result<Option<Value>, String> {
        Ok(self
            .comparisons
            .clone()?
            .into_iter()
            .find(|c| c["comparison_id"] == id))
    }
}

/// Sin informes guardados todavía.
struct Empty;

impl LearningSource for Empty {
    async fn latest(&self) -> Result<Option<Value>, String> {
        Ok(None)
    }
    async fn statuses(&self, _: &str, _: DateTime<Utc>) -> Result<Vec<ChangeStatus>, String> {
        Ok(vec![])
    }
}

/// Un informe guardado con sus votos. El estado lo cuenta `tally`, la misma función que usa el
/// diario en producción, con las reglas reales de `config/votes.toml`.
struct Stored {
    report: Value,
    votes: Vec<Vote>,
}

fn rules() -> VoteRules {
    VoteRules::from_toml(include_str!("../../../../config/votes.toml")).unwrap()
}

impl LearningSource for Stored {
    async fn latest(&self) -> Result<Option<Value>, String> {
        Ok(Some(self.report.clone()))
    }
    async fn statuses(
        &self,
        report_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<ChangeStatus>, String> {
        assert_eq!(self.report["report_id"], report_id);
        let created_at: DateTime<Utc> =
            self.report["created_at"].as_str().unwrap().parse().unwrap();
        Ok(self.report["change_proposals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let change: StoredChange = serde_json::from_value(json!({
                    "change_id": c["change_id"], "report_id": report_id, "kind": c["kind"],
                    "text": c["text"], "evidence": c["evidence"], "status": c["status"],
                    "created_at": created_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                }))
                .unwrap();
                ChangeStatus {
                    status: tally(&change, &self.votes, &rules(), now),
                    change_id: change.change_id,
                }
            })
            .collect())
    }
}

fn vote(member: &str, in_favor: bool, at: DateTime<Utc>) -> Vote {
    Vote {
        vote_id: format!("vote:{member}"),
        change_id: "learning:x:recalibrate:0".into(),
        member_id: member.into(),
        in_favor,
        voted_at: at,
    }
}

fn fake() -> Fake {
    Fake {
        comparisons: Ok(vec![
            comparison("paper-vs-backtest:US100.FUT:1", "US100.FUT"),
            comparison("paper-vs-backtest:US500.FUT:1", "US500.FUT"),
        ]),
    }
}

fn now() -> DateTime<Utc> {
    "2026-11-24T10:00:00Z".parse().unwrap()
}

async fn get<L: LearningSource>(c: &Fake, l: &L, method: &str, path: &str) -> (u16, Value) {
    let r = respond_learning(c, l, method, path, now()).await;
    (
        r.status,
        serde_json::from_str(&r.body).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn comparisons_latest_and_by_id_as_stored() {
    let c = fake();
    let (status, body) = get(&c, &Empty, "GET", "/api/v1/paper_comparisons").await;
    assert_eq!(status, 200);
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for r in rows {
        assert_valid("paper_comparison.v1.schema.json", r);
    }
    let (status, one) = get(
        &c,
        &Empty,
        "GET",
        "/api/v1/paper_comparisons/paper-vs-backtest:US500.FUT:1",
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(one["instrument_id"], "US500.FUT");
    // Tal cual: el gateway no recalcula las causas.
    assert_eq!(one["causes"]["missing_in_paper"], "-20");
    assert_eq!(
        get(&c, &Empty, "GET", "/api/v1/paper_comparisons/nope")
            .await
            .0,
        404
    );
    assert_eq!(
        get(&c, &Empty, "GET", "/api/v1/paper_comparisons/a/b")
            .await
            .0,
        400
    );
    assert_eq!(
        get(&c, &Empty, "GET", "/api/v1/paper_comparisons/..")
            .await
            .0,
        400
    );
}

#[tokio::test]
async fn learning_report_as_stored_or_null_until_there_is_one() {
    let c = fake();
    let (status, none) = get(&c, &Empty, "GET", "/api/v1/learning_report").await;
    assert_eq!(status, 200);
    assert_eq!(none, Value::Null);
    let report = learning();
    assert_valid("learning_report.v1.schema.json", &report);
    let stored = Stored {
        report: report.clone(),
        votes: vec![],
    };
    let (status, body) = get(&c, &stored, "GET", "/api/v1/learning_report").await;
    assert_eq!(status, 200);
    assert_eq!(body, report);
    assert_eq!(body["change_proposals"][0]["status"], "pendiente_de_voto");
}

#[tokio::test]
async fn vote_status_of_each_proposal_counted_by_the_engine() {
    let c = fake();
    // Sin informe: lista vacía.
    let (status, empty) = get(&c, &Empty, "GET", "/api/v1/learning_report/votes").await;
    assert_eq!(status, 200);
    assert_eq!(empty, json!([]));

    let created: DateTime<Utc> = "2026-11-23T10:00:00Z".parse().unwrap();
    let at = |h| created + TimeDelta::hours(h);
    // Dos a favor y uno en contra, dentro del plazo: pendiente (hacen falta 4 de 6).
    let pending = Stored {
        report: learning(),
        votes: vec![
            vote("m1", true, at(1)),
            vote("m2", true, at(2)),
            vote("m3", false, at(3)),
        ],
    };
    let (_, body) = get(&c, &pending, "GET", "/api/v1/learning_report/votes").await;
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    for r in rows {
        assert_valid("change_vote_status.v1.schema.json", r);
    }
    assert_eq!(
        rows[0],
        json!({ "change_id": "learning:x:recalibrate:0", "status": "pendiente_de_voto",
                "in_favor": 2, "against": 1 })
    );
    // Cuatro a favor (y un voto repetido de m1 que no cuenta dos veces): aprobada.
    let approved = Stored {
        report: learning(),
        votes: vec![
            vote("m1", true, at(1)),
            vote("m1", true, at(2)),
            vote("m2", true, at(2)),
            vote("m3", true, at(3)),
            vote("m4", true, at(4)),
        ],
    };
    let (_, body) = get(&c, &approved, "GET", "/api/v1/learning_report/votes").await;
    assert_eq!(body[0]["status"], "aprobada");
    assert_eq!(body[0]["in_favor"], 4);
    // Un voto fuera del plazo no cuenta.
    let late = Stored {
        report: learning(),
        votes: vec![vote("m1", true, at(49))],
    };
    let (_, body) = get(&c, &late, "GET", "/api/v1/learning_report/votes").await;
    assert_eq!(body[0]["in_favor"], 0);
}

#[tokio::test]
async fn read_only_no_vote_writes_and_no_db_errors_leak() {
    let c = fake();
    for path in [
        "/api/v1/paper_comparisons",
        "/api/v1/paper_comparisons/paper-vs-backtest:US100.FUT:1",
        "/api/v1/learning_report",
        "/api/v1/learning_report/votes",
    ] {
        assert!(is_learning_route(path));
        for method in ["POST", "PUT", "DELETE", "PATCH"] {
            assert_eq!(
                get(&c, &Empty, method, path).await.0,
                405,
                "{method} {path}"
            );
        }
    }
    // Nada para registrar votos.
    for path in [
        "/api/v1/votes",
        "/api/v1/change_votes",
        "/api/v1/learning_report/vote",
    ] {
        assert!(!is_learning_route(path), "{path}");
    }
    let broken = Fake {
        comparisons: Err("sin base: postgres://u:clave@h/db".into()),
    };
    let r = respond_learning(&broken, &Empty, "GET", "/api/v1/paper_comparisons", now()).await;
    assert_eq!(r.status, 500);
    assert!(!r.body.contains("clave"));
}

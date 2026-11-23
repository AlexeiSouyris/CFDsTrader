//! Rutas del fondo del grupo (fase 6b, #312): solo lectura, todo calculado por `cfds-fund`.
//! La aprobación de la fase: el ejemplo del glosario (Ana, Beto y Caro) sale exacto y lo de cada
//! miembro suma el valor total.

use std::path::PathBuf;

use cfds_fund::{Fund, FundConfig, Record, Request};
use cfds_gateway::fund::{FundSource, is_fund_route, respond_fund};
use rust_decimal::{Decimal, dec};
use serde_json::{Value, json};

const FUND: &str = include_str!("../../../../config/fund.toml");

fn cfg() -> FundConfig {
    FundConfig::from_toml(FUND).unwrap()
}

fn put(member: &str, amount: Decimal) -> Request {
    Request::Contribute {
        member: member.into(),
        amount,
    }
}

/// El ejemplo del glosario (`docs/GLOSARIO.md`, "Valor de la unidad"), como lo guarda el motor:
/// semana 1 Ana 100 € y Beto 50 € a 1 €; el fondo pasa a 165 €; semana 2 Caro 110 € a 1,10 €.
fn glossary_history() -> Vec<Record> {
    let c = cfg();
    let (f1, s1) = Fund::default()
        .settle(dec!(0), &[put("ana", dec!(100)), put("beto", dec!(50))], &c)
        .unwrap();
    let (_, s2) = f1.settle(dec!(165), &[put("caro", dec!(110))], &c).unwrap();
    let mut records = s1.records("2026-11-16T07:00:00Z".parse().unwrap(), &c);
    records.extend(s2.records("2026-11-23T07:00:00Z".parse().unwrap(), &c));
    records
}

struct Fake(Result<Vec<Record>, String>);

impl FundSource for Fake {
    async fn movements(&self) -> Result<Vec<Record>, String> {
        self.0.clone()
    }
}

async fn call(source: &Fake, method: &str, path: &str) -> (u16, Value) {
    let r = respond_fund(source, &cfg(), method, path).await;
    (
        r.status,
        serde_json::from_str(&r.body).unwrap_or(Value::Null),
    )
}

fn assert_valid(schema: &str, doc: &Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(schema);
    let schema_doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let v = jsonschema::validator_for(&schema_doc).unwrap();
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{schema}: {errors:?}");
}

fn dec_of(v: &Value) -> Decimal {
    v.as_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn glossary_example_comes_out_exact_through_the_route() {
    let (status, body) = call(&Fake(Ok(glossary_history())), "GET", "/api/v1/fund").await;
    assert_eq!(status, 200);
    assert_valid("fund_summary.v1.schema.json", &body);
    assert_eq!(body["currency"], "EUR");
    assert_eq!(body["basis"], "last_settlement");
    assert_eq!(body["valued_at"], "2026-11-23T07:00:00Z");
    assert_eq!(dec_of(&body["nav"]), dec!(1.1));
    assert_eq!(dec_of(&body["total_value"]), dec!(275));
    assert_eq!(dec_of(&body["total_units"]), dec!(250));

    let members = body["members"].as_array().unwrap();
    let row = |id: &str| members.iter().find(|m| m["member_id"] == id).unwrap();
    // (unidades, valor, aportes netos, P&L), calculado a mano en el glosario.
    for (id, units, value, net, pnl) in [
        ("ana", dec!(100), dec!(110), dec!(100), dec!(10)),
        ("beto", dec!(50), dec!(55), dec!(50), dec!(5)),
        ("caro", dec!(100), dec!(110), dec!(110), dec!(0)),
    ] {
        let m = row(id);
        assert_eq!(dec_of(&m["units"]), units, "{id}");
        assert_eq!(dec_of(&m["value"]), value, "{id}");
        assert_eq!(dec_of(&m["net_contributions"]), net, "{id}");
        assert_eq!(dec_of(&m["pnl"]), pnl, "{id}");
    }
    // Invariante de la fase 6b: lo de cada miembro suma el valor total, al céntimo.
    let sum: Decimal = members.iter().map(|m| dec_of(&m["value"])).sum();
    assert_eq!(sum, dec_of(&body["total_value"]));
}

#[tokio::test]
async fn members_add_up_to_the_total_with_uneven_units() {
    // Tres aportes iguales y una ganancia que no se reparte justo en céntimos.
    let c = cfg();
    let (f1, s1) = Fund::default()
        .settle(
            dec!(0),
            &[
                put("a", dec!(100)),
                put("b", dec!(100)),
                put("c", dec!(100)),
            ],
            &c,
        )
        .unwrap();
    let (_, s2) = f1
        .settle(
            dec!(310.01),
            &[
                put("d", dec!(33.33)),
                Request::Withdraw {
                    member: "a".into(),
                    amount: dec!(10),
                },
            ],
            &c,
        )
        .unwrap();
    let mut records = s1.records("2026-11-16T07:00:00Z".parse().unwrap(), &c);
    records.extend(s2.records("2026-11-23T07:00:00Z".parse().unwrap(), &c));
    let (status, body) = call(&Fake(Ok(records)), "GET", "/api/v1/fund").await;
    assert_eq!(status, 200);
    assert_valid("fund_summary.v1.schema.json", &body);
    // 310,01 + 33,33 − 10 = 333,34
    assert_eq!(dec_of(&body["total_value"]), dec!(333.34));
    let sum: Decimal = body["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| dec_of(&m["value"]))
        .sum();
    assert_eq!(sum, dec!(333.34));
}

#[tokio::test]
async fn movements_are_the_history_in_order() {
    let history = glossary_history();
    let (status, body) = call(&Fake(Ok(history.clone())), "GET", "/api/v1/fund/movements").await;
    assert_eq!(status, 200);
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 3);
    for (row, r) in rows.iter().zip(&history) {
        assert_valid("fund_movement.v1.schema.json", row);
        assert_eq!(row["id"], r.id.as_str());
    }
    assert_eq!(rows[2]["member_id"], "caro");
    assert_eq!(dec_of(&rows[2]["nav"]), dec!(1.1));
}

#[tokio::test]
async fn an_empty_fund_is_valid_and_worth_nothing() {
    let (status, body) = call(&Fake(Ok(vec![])), "GET", "/api/v1/fund").await;
    assert_eq!(status, 200);
    assert_valid("fund_summary.v1.schema.json", &body);
    assert_eq!(body["valued_at"], Value::Null);
    assert_eq!(body["total_value"], "0");
    assert_eq!(body["nav"], "1");
    assert_eq!(body["members"], json!([]));
    let (_, moves) = call(&Fake(Ok(vec![])), "GET", "/api/v1/fund/movements").await;
    assert_eq!(moves, json!([]));
}

#[tokio::test]
async fn only_get_and_no_half_fund_on_errors() {
    let ok = Fake(Ok(glossary_history()));
    for path in ["/api/v1/fund", "/api/v1/fund/movements"] {
        assert!(is_fund_route(path));
        for method in ["POST", "PUT", "DELETE"] {
            assert_eq!(call(&ok, method, path).await.0, 405, "{method} {path}");
        }
    }
    assert!(!is_fund_route("/api/v1/fund/contributions"));
    assert_eq!(call(&ok, "GET", "/api/v1/funds").await.0, 404);
    // La base falla: 500 sin el detalle.
    let down = Fake(Err("conexión rechazada".into()));
    let r = respond_fund(&down, &cfg(), "GET", "/api/v1/fund").await;
    assert_eq!(r.status, 500);
    assert!(!r.body.contains("conexión"));
    // Un historial que vende más de lo que hay: 500, nunca un resumen a medias.
    let mut bad = glossary_history();
    bad[0].kind = cfds_fund::MoveKind::Withdrawal;
    assert_eq!(call(&Fake(Ok(bad)), "GET", "/api/v1/fund").await.0, 500);
}

//! Rutas del broker simulado del gateway (#278): contratos, idempotencia, rechazo por riesgo,
//! anti-CSRF, tamaño y hora de la orden. El broker es el `PaperBroker` real, en memoria y con
//! precios sintéticos; la sesión de solo lectura con el registro se prueba aparte (`#[ignore]`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{Broker, BrokerConfig, FundingSetup, MarketSetup, PaperBroker};
use cfds_core::{
    Candle, InstrumentId, Order, OrderReport, OrderSide, PaperAccount, PaperMode, Position,
    TicketVerdict, Timeframe,
};
use cfds_gateway::Reply;
use cfds_gateway::paper::{HttpIn, PaperConfig, PaperService, respond_paper};
use cfds_journal::BacktestRun;
use cfds_risk::{InstrumentLimits, RiskConfig};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::{Value, json};
use tokio::sync::Mutex;

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("../../../../config/broker.toml");
const TOKEN: &str = "token-local-de-prueba-123";

fn at(i: i64) -> DateTime<Utc> {
    "2026-11-18T09:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(15 * i)
}

/// El broker simulado real, en memoria (sin registro en la base).
struct MemPaper(Mutex<PaperBroker>);

impl PaperService for MemPaper {
    async fn preview(&self, order: Order, now: DateTime<Utc>) -> Result<TicketVerdict, String> {
        Ok(self.0.lock().await.preview(&order, now))
    }
    async fn submit(&self, order: Order) -> Result<OrderReport, String> {
        Ok(self.0.lock().await.submit(&order))
    }
    async fn positions(&self, now: DateTime<Utc>) -> Result<Vec<Position>, String> {
        Ok(self.0.lock().await.positions(now))
    }
    async fn account(&self, now: DateTime<Utc>) -> Result<PaperAccount, String> {
        Ok(self.0.lock().await.account(now))
    }
    async fn runs(&self, _limit: u32) -> Result<Vec<BacktestRun>, String> {
        Ok(Vec::new())
    }
}

fn paper() -> MemPaper {
    let candles: Vec<Candle> = (0..40)
        .map(|i| Candle {
            instrument_id: "US100".parse().unwrap(),
            timeframe: Timeframe::M15,
            start: at(i),
            open: dec!(100),
            high: dec!(101),
            low: dec!(99),
            close: dec!(100),
            volume: None,
        })
        .collect();
    let market = MarketSetup {
        limits: InstrumentLimits {
            max_leverage: dec!(10),
            margin_pct: dec!(5),
            min_deal_size: Some(dec!(0.01)),
        },
        costs: InstrumentCosts {
            spread: dec!(1),
            tick: dec!(0.1),
            commission: Commission::None,
            funding_admin_fee_pct: dec!(2.5),
            funding_day_count: dec!(365),
        },
        account_to_instrument: Decimal::ONE,
        candles,
        funding: FundingSetup {
            cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            tz: chrono_tz::Europe::Madrid,
            rates: FundingRates::Constant(dec!(2)),
        },
    };
    let cfg = BrokerConfig::from_toml(
        BROKER,
        RiskConfig::from_toml(RISK).unwrap(),
        CostConfig {
            slippage_spread_fraction: dec!(0.1),
            slippage_ticks: dec!(1),
        },
        ExitConfig::from_toml(BACKTEST).unwrap(),
        chrono_tz::Europe::Madrid,
    )
    .unwrap();
    let markets: BTreeMap<InstrumentId, MarketSetup> =
        [("US100".parse().unwrap(), market)].into_iter().collect();
    MemPaper(Mutex::new(PaperBroker::new(cfg, markets)))
}

fn cfg() -> PaperConfig {
    PaperConfig::from_toml(GATEWAY).unwrap()
}

fn order(id: &str, stop: Decimal) -> Value {
    serde_json::to_value(Order {
        order_id: id.parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: "US100".parse().unwrap(),
        side: OrderSide::Long,
        stop,
        tp1: None,
        proposal_id: None,
        created_at: at(20),
    })
    .unwrap()
}

async fn call(
    p: Option<&MemPaper>,
    method: &str,
    path: &str,
    token: Option<&str>,
    content_type: Option<&str>,
    body: &[u8],
    now: DateTime<Utc>,
) -> Reply {
    respond_paper(
        p,
        &cfg(),
        Some(TOKEN),
        HttpIn {
            method,
            path,
            token,
            content_type,
            body,
        },
        now,
    )
    .await
}

async fn post(p: &MemPaper, path: &str, body: &Value) -> Reply {
    let bytes = serde_json::to_vec(body).unwrap();
    call(
        Some(p),
        "POST",
        path,
        Some(TOKEN),
        Some("application/json"),
        &bytes,
        at(20),
    )
    .await
}

async fn get(p: &MemPaper, path: &str) -> Reply {
    call(Some(p), "GET", path, None, None, b"", at(29)).await
}

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    jsonschema::validator_for(&schema).unwrap()
}

fn assert_valid(schema: &str, body: &str) {
    let doc: Value = serde_json::from_str(body).unwrap();
    let v = validator(schema);
    let docs = doc.as_array().cloned().unwrap_or_else(|| vec![doc]);
    for d in docs {
        let errors: Vec<String> = v.iter_errors(&d).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{schema}: {errors:?}");
    }
}

#[test]
fn the_repo_config_has_safe_limits() {
    let c = cfg();
    assert_eq!(c.token_env, "CFDS_GATEWAY_TOKEN");
    assert!(c.max_body_bytes <= 65_536);
    assert!(c.max_order_age <= TimeDelta::minutes(5));
}

#[tokio::test]
async fn preview_answers_without_effects_and_submit_is_idempotent() {
    let p = paper();
    let v = post(&p, "/api/v1/paper/preview", &order("o1", dec!(98))).await;
    assert_eq!(v.status, 200, "{}", v.body);
    assert_valid("ticket_verdict.v1.schema.json", &v.body);
    assert_eq!(get(&p, "/api/v1/paper/positions").await.body, "[]");

    let first = post(&p, "/api/v1/paper/orders", &order("o1", dec!(98))).await;
    assert_eq!(first.status, 200, "{}", first.body);
    assert_valid("order_report.v1.schema.json", &first.body);
    let again = post(&p, "/api/v1/paper/orders", &order("o1", dec!(98))).await;
    assert_eq!(
        first.body, again.body,
        "el mismo order_id devuelve el mismo informe"
    );

    let positions = get(&p, "/api/v1/paper/positions").await;
    assert_valid("position.v1.schema.json", &positions.body);
    assert_eq!(
        serde_json::from_str::<Value>(&positions.body)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let account = get(&p, "/api/v1/paper/account").await;
    assert_valid("paper_account.v1.schema.json", &account.body);
    assert_eq!(get(&p, "/api/v1/backtest_runs").await.body, "[]");
}

#[tokio::test]
async fn the_risk_module_rejects_through_the_route() {
    let p = paper();
    let r = post(&p, "/api/v1/paper/orders", &order("close", dec!(99.5))).await;
    assert_eq!(r.status, 200);
    let doc: Value = serde_json::from_str(&r.body).unwrap();
    assert_eq!(doc["status"], "rejected");
    assert_eq!(doc["rejection"], "stop_too_close");
    let v = post(&p, "/api/v1/paper/preview", &order("far", dec!(90))).await;
    let doc: Value = serde_json::from_str(&v.body).unwrap();
    assert_eq!(doc["accepted"], false);
    assert_eq!(doc["rejection"], "stop_too_far");
}

#[tokio::test]
async fn writes_need_the_local_token_json_and_a_small_body() {
    let p = paper();
    let body = serde_json::to_vec(&order("o1", dec!(98))).unwrap();
    let path = "/api/v1/paper/orders";
    let json_ct = Some("application/json");
    // Sin cabecera, con otra o con el token de otro largo: 403 (anti-CSRF).
    for token in [None, Some("otro"), Some("token-local-de-prueba-124")] {
        let r = call(Some(&p), "POST", path, token, json_ct, &body, at(20)).await;
        assert_eq!(r.status, 403, "{token:?}");
    }
    // Un formulario de otra web llega como text/plain o form: 415.
    let r = call(
        Some(&p),
        "POST",
        path,
        Some(TOKEN),
        Some("text/plain"),
        &body,
        at(20),
    )
    .await;
    assert_eq!(r.status, 415);
    // Body demasiado grande: 413.
    let big = vec![b' '; cfg().max_body_bytes + 1];
    let r = call(Some(&p), "POST", path, Some(TOKEN), json_ct, &big, at(20)).await;
    assert_eq!(r.status, 413);
    // Sin token configurado en el gateway, la escritura está apagada aunque se mande uno.
    let off = respond_paper(
        Some(&p),
        &cfg(),
        None,
        HttpIn {
            method: "POST",
            path,
            token: Some(TOKEN),
            content_type: json_ct,
            body: &body,
        },
        at(20),
    )
    .await;
    assert_eq!(off.status, 403);
    // Nada de eso abrió una posición.
    assert_eq!(get(&p, "/api/v1/paper/positions").await.body, "[]");
}

#[tokio::test]
async fn bad_orders_and_routes_are_rejected() {
    let p = paper();
    let path = "/api/v1/paper/orders";
    // Modo real, campo inventado o sin stop: no es un order.v1.
    let mut live = order("x", dec!(98));
    live["mode"] = json!("live");
    let mut sized = order("y", dec!(98));
    sized["size"] = json!("10");
    let mut no_stop = order("z", dec!(98));
    no_stop.as_object_mut().unwrap().remove("stop");
    for bad in [live, sized, no_stop] {
        assert_eq!(post(&p, path, &bad).await.status, 400, "{bad}");
    }
    // created_at viejo o en el futuro: nadie elige a qué precio entra.
    let body = serde_json::to_vec(&order("t", dec!(98))).unwrap();
    for now in [
        at(20) + TimeDelta::minutes(5),
        at(20) - TimeDelta::seconds(1),
    ] {
        let r = call(
            Some(&p),
            "POST",
            path,
            Some(TOKEN),
            Some("application/json"),
            &body,
            now,
        )
        .await;
        assert_eq!(r.status, 400);
    }
    // Métodos y rutas.
    assert_eq!(
        call(Some(&p), "GET", path, None, None, b"", at(20))
            .await
            .status,
        405
    );
    assert_eq!(
        call(
            Some(&p),
            "POST",
            "/api/v1/paper/account",
            Some(TOKEN),
            None,
            b"",
            at(20)
        )
        .await
        .status,
        405
    );
    assert_eq!(
        call(
            Some(&p),
            "GET",
            "/api/v1/paper/otra",
            None,
            None,
            b"",
            at(20)
        )
        .await
        .status,
        404
    );
    // Sin broker: 503.
    assert_eq!(
        call(
            None,
            "GET",
            "/api/v1/paper/account",
            None,
            None,
            b"",
            at(20)
        )
        .await
        .status,
        503
    );
}

/// La sesión del gateway es de solo lectura: el registro del broker se escribe solo dentro de la
/// transacción que `insert_paper_order` declara de escritura; cualquier otra escritura falla.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_read_only_session_writes_only_the_broker_journal() {
    let url = std::env::var("DATABASE_URL").expect("falta DATABASE_URL (necesita PostgreSQL)");
    cfds_storage::PgStore::connect(&url)
        .await
        .unwrap()
        .migrate()
        .await
        .unwrap();
    let ro = cfds_storage::PgStore::connect(&cfds_gateway::read_only_url(&url))
        .await
        .unwrap();
    let nanos = Utc::now().timestamp_nanos_opt().unwrap();
    let id = format!("gw-ro-{nanos}");
    let order: Order = serde_json::from_value(order(&id, dec!(98))).unwrap();
    let report = paper().0.lock().await.submit(&order);
    assert!(ro.insert_paper_order(&order, &report).await.unwrap());
    let direct = sqlx::query(
        "INSERT INTO paper_orders (order_id, instrument_id, created_at, document)
         VALUES ('gw-ro-direct', 'US100', now(), '{\"order_id\": \"gw-ro-direct\", \"mode\": \"paper\"}')",
    )
    .execute(ro.pool())
    .await;
    assert!(
        direct.is_err(),
        "fuera de esa transacción, la sesión no escribe"
    );
}

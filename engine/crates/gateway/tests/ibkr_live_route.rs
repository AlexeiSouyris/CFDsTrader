//! `GET /api/v1/ibkr/live` (#558): la vela en formación por SSE, solo visual. Sin red ni IB
//! Gateway: el hub recibe lo que mandaría `cfds_ibkr::live_feed` y se leen los eventos SSE.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cfds_core::InstrumentId;
use cfds_gateway::ibkr::IbkrViewConfig;
use cfds_gateway::ibkr_live::{LiveHub, LiveReply, SseBody, is_ibkr_live_route, respond_ibkr_live};
use cfds_ibkr::live::{BarUpdate, LiveBar, LiveDataType, LiveTimeframe};
use cfds_ibkr::live_feed::{FeedStatus, LiveSink, QuoteInfo};
use chrono::{DateTime, TimeDelta, Utc};
use http_body_util::BodyExt;
use rust_decimal::{Decimal, dec};
use serde_json::Value;

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const TERMINAL: &str = include_str!("../../../../config/terminal.toml");
const IBKR: &str = include_str!("../../../../config/ibkr.toml");

fn contracts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts")
}

fn assert_valid(doc: &Value) {
    let text = std::fs::read_to_string(contracts().join("ibkr_live_bar.v1.schema.json")).unwrap();
    let v = jsonschema::validator_for(&serde_json::from_str(&text).unwrap()).unwrap();
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(
        errors.is_empty(),
        "no valida contra ibkr_live_bar.v1: {errors:?}\n{doc}"
    );
}

fn t(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn mnq() -> InstrumentId {
    "MNQ".parse().unwrap()
}

fn hub(status: FeedStatus) -> Arc<LiveHub> {
    Arc::new(LiveHub::new(
        "ibkr",
        vec![mnq(), "QQQ".parse().unwrap()],
        LiveTimeframe::ALL.to_vec(),
        status,
        None,
    ))
}

fn bar(start: &str, close: Decimal, partial: bool) -> LiveBar {
    LiveBar {
        contract: "MNQZ6".into(),
        timeframe: LiveTimeframe::M1,
        start: t(start),
        open: dec!(26000),
        high: dec!(26010.25),
        low: dec!(25990.5),
        close,
        last_at: t(start) + TimeDelta::seconds(30),
        ticks: 7,
        partial,
    }
}

fn delayed() -> QuoteInfo {
    QuoteInfo {
        contract: Some("MNQZ6".into()),
        data_type: Some(LiveDataType::Delayed),
        delay: Some(TimeDelta::seconds(600)),
        rejected: None,
    }
}

fn open(h: &Arc<LiveHub>, query: &str) -> SseBody {
    match respond_ibkr_live(h, "GET", Some(query), 4, Duration::from_secs(15)) {
        LiveReply::Stream(b) => b,
        LiveReply::Error { status, body } => panic!("{status}: {body}"),
    }
}

/// El próximo evento `data:` (saltea `retry:` y los comentarios), como JSON.
async fn next(body: &mut SseBody) -> Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(60), body.frame())
            .await
            .expect("llega un evento")
            .expect("el SSE sigue abierto")
            .unwrap();
        let bytes = frame.into_data().unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        if let Some(json) = text.strip_prefix("data: ") {
            let v: Value = serde_json::from_str(json.trim_end()).unwrap();
            assert_valid(&v);
            return v;
        }
    }
}

#[test]
fn route_and_config() {
    assert!(is_ibkr_live_route("/api/v1/ibkr/live"));
    assert!(!is_ibkr_live_route("/api/v1/ibkr/bars"));
    let cfg = IbkrViewConfig::from_toml(GATEWAY, TERMINAL, IBKR).unwrap();
    // El dueño la prendió en config/gateway.toml (#566); el test solo verifica que la clave se lee.
    let _ = cfg.live;
    assert!(cfg.live_max_clients >= 4);
}

#[tokio::test(start_paused = true)]
async fn bad_requests_are_rejected() {
    let h = hub(FeedStatus::Disabled);
    let status =
        |q: &str, m: &str| match respond_ibkr_live(&h, m, Some(q), 4, Duration::from_secs(15)) {
            LiveReply::Error { status, .. } => status,
            LiveReply::Stream(_) => 200,
        };
    assert_eq!(status("instrument=MNQ&timeframe=1m", "POST"), 405);
    assert_eq!(status("timeframe=1m", "GET"), 400);
    assert_eq!(status("instrument=MNQ&timeframe=1d", "GET"), 400);
    assert_eq!(status("instrument=MES&timeframe=1m", "GET"), 404);
}

#[tokio::test(start_paused = true)]
async fn disabled_says_so() {
    let h = hub(FeedStatus::Disabled);
    let mut b = open(&h, "instrument=MNQ&timeframe=1m");
    let m = next(&mut b).await;
    assert_eq!(m["status"], "disabled");
    assert_eq!(m["bar"], Value::Null);
    assert_eq!(m["forming"], false);
}

#[tokio::test(start_paused = true)]
async fn forming_then_closed_then_next_at_most_one_per_second() {
    let h = hub(FeedStatus::Connecting);
    let mut b = open(&h, "instrument=MNQ&timeframe=1m");
    assert_eq!(next(&mut b).await["status"], "connecting");
    let id = mnq();
    h.status(FeedStatus::Connected, Utc::now());
    h.quote(&id, &delayed(), Utc::now());
    let m = next(&mut b).await;
    assert_eq!(m["status"], "waiting");
    assert_eq!(m["data_type"], "delayed");
    assert_eq!(m["delay_seconds"], 600);

    let forming = bar("2026-11-25T03:09:00Z", dec!(26001), true);
    h.bar(
        &id,
        &delayed(),
        &BarUpdate {
            bar: forming.clone(),
            forming: true,
        },
    );
    let started = tokio::time::Instant::now();
    let m = next(&mut b).await;
    assert_eq!(m["status"], "streaming");
    assert_eq!(m["forming"], true);
    assert_eq!(m["bar"]["close"], "26001");
    assert_eq!(m["bar"]["partial"], true);
    assert_eq!(m["bar"]["end"], "2026-11-25T03:10:00Z");
    assert_eq!(m["contract"], "MNQZ6");

    // Cierra y abre la siguiente al mismo tiempo: primero la cerrada, un segundo después la nueva.
    let closed = bar("2026-11-25T03:09:00Z", dec!(26005.5), true);
    let following = bar("2026-11-25T03:10:00Z", dec!(26006), false);
    h.bar(
        &id,
        &delayed(),
        &BarUpdate {
            bar: closed,
            forming: false,
        },
    );
    h.bar(
        &id,
        &delayed(),
        &BarUpdate {
            bar: following,
            forming: true,
        },
    );
    let m = next(&mut b).await;
    assert!(started.elapsed() >= Duration::from_secs(1));
    assert_eq!(m["forming"], false);
    assert_eq!(m["bar"]["start"], "2026-11-25T03:09:00Z");
    assert_eq!(m["bar"]["close"], "26005.5");
    let t1 = tokio::time::Instant::now();
    let m = next(&mut b).await;
    assert!(
        t1.elapsed() >= Duration::from_secs(1),
        "tope de un mensaje por segundo"
    );
    assert_eq!(m["forming"], true);
    assert_eq!(m["bar"]["start"], "2026-11-25T03:10:00Z");
    assert_eq!(m["bar"]["partial"], false);

    // Muchas actualizaciones seguidas: llega la última, no todas.
    for i in 0..50 {
        let mut x = bar(
            "2026-11-25T03:10:00Z",
            dec!(26006) + Decimal::from(i),
            false,
        );
        x.ticks = 8 + i;
        h.bar(
            &id,
            &delayed(),
            &BarUpdate {
                bar: x,
                forming: true,
            },
        );
    }
    let m = next(&mut b).await;
    assert_eq!(m["bar"]["close"], "26055");

    // Se corta IB Gateway: la vela en curso se descarta y se informa el motivo.
    h.status(
        FeedStatus::Disconnected {
            reason: "IB Gateway no responde".into(),
        },
        Utc::now(),
    );
    let m = next(&mut b).await;
    assert_eq!(m["status"], "disconnected");
    assert_eq!(m["reason"], "IB Gateway no responde");
    assert_eq!(m["data_type"], Value::Null);
}

#[tokio::test(start_paused = true)]
async fn too_many_clients() {
    let h = hub(FeedStatus::Disabled);
    let mut open_ones = Vec::new();
    for _ in 0..4 {
        open_ones.push(open(&h, "instrument=QQQ&timeframe=5m"));
    }
    assert!(matches!(
        respond_ibkr_live(
            &h,
            "GET",
            Some("instrument=QQQ&timeframe=5m"),
            4,
            Duration::from_secs(15)
        ),
        LiveReply::Error { status: 503, .. }
    ));
    // Al cerrar una, entra otra.
    drop(open_ones.pop());
    tokio::time::sleep(Duration::from_secs(20)).await;
    assert!(matches!(
        respond_ibkr_live(
            &h,
            "GET",
            Some("instrument=QQQ&timeframe=5m"),
            4,
            Duration::from_secs(15)
        ),
        LiveReply::Stream(_)
    ));
}

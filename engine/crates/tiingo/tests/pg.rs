//! Cliente, topes y registro contra PostgreSQL **real**, con un transporte falso (sin red ni
//! Tiingo). Respuestas SINTÉTICAS, armadas a mano: nunca precios reales de Tiingo (ADR 0009).
//!
//! Necesitan `DATABASE_URL` con las migraciones (las aplica `TiingoStore::connect`). Marcados
//! `#[ignore]`; el CI los corre en el job de integración con `-- --ignored`.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use cfds_core::InstrumentId;
use cfds_tiingo::{
    ApiKey, Client, DayVerdict, Fetched, HttpTransport, Method, TiingoConfig, TiingoError,
    TiingoStore, TransportError, Verdict,
};
use chrono::{NaiveDate, TimeDelta, Utc};

const REAL: &str = include_str!("../../../../config/tiingo.toml");
const BODY: &str = r#"[{"date":"2026-01-05T00:00:00.000Z","close":100,"divCash":0,"splitFactor":1},
                       {"date":"2026-01-06T00:00:00.000Z","close":101,"divCash":0,"splitFactor":1}]"#;

fn url() -> String {
    std::env::var("DATABASE_URL").expect("falta DATABASE_URL")
}

/// Símbolo único por corrida: las tablas son de solo agregar y no se limpian.
fn symbol() -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        % 1_000_000_000_000;
    format!("T{n}")
}

/// Config con un plan enorme, para que los topes de la prueba no dependan del uso de otras corridas.
fn config(per_hour: u64) -> TiingoConfig {
    let text = REAL
        .replace("requests_per_hour = 50", "requests_per_hour = 1000000000")
        .replace("requests_per_day = 1000", "requests_per_day = 1000000000")
        .replace("symbols_per_month = 500", "symbols_per_month = 1000000000")
        .replace(
            "bytes_per_month = 1000000000",
            "bytes_per_month = 100000000000000",
        )
        .replace(
            "requests_per_hour = 40",
            &format!("requests_per_hour = {per_hour}"),
        )
        .replace("requests_per_day = 800", "requests_per_day = 999999999")
        .replace(
            "symbols_per_31_days = 20",
            "symbols_per_31_days = 999999999",
        )
        .replace(
            "bytes_per_31_days = 800000000",
            "bytes_per_31_days = 99999999999999",
        );
    TiingoConfig::from_toml(&text).unwrap()
}

enum Answer {
    Ok(u16, &'static str),
    Network,
    TooLarge,
}

struct Fake {
    answer: Answer,
    calls: Mutex<Vec<String>>,
}

impl Fake {
    fn new(answer: Answer) -> Self {
        Self {
            answer,
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl HttpTransport for Fake {
    async fn get(&self, url: &str, _: &ApiKey, max: u64) -> Result<Fetched, TransportError> {
        self.calls.lock().unwrap().push(url.to_owned());
        match self.answer {
            Answer::Ok(status, body) => Ok(Fetched {
                status,
                body: body.as_bytes().to_vec(),
            }),
            Answer::Network => Err(TransportError::Network("sin conexión".into())),
            Answer::TooLarge => Err(TransportError::TooLarge { bytes: max + 1 }),
        }
    }
}

async fn logged(store: &TiingoStore, sym: &str) -> Vec<(Option<i32>, i64)> {
    sqlx::query_as("SELECT http_status, bytes FROM tiingo_usage_log WHERE symbol = $1 ORDER BY id")
        .bind(sym)
        .fetch_all(store.pool())
        .await
        .unwrap()
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, d).unwrap()
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn a_good_answer_is_parsed_and_only_its_usage_is_logged() {
    let store = TiingoStore::connect(&url()).await.unwrap();
    let sym = symbol();
    let client = Client::new(
        config(1_000_000),
        ApiKey::new("x".into()).unwrap(),
        Fake::new(Answer::Ok(200, BODY)),
    );
    let rows = client
        .daily_prices(&store, &sym, day(5), day(6), Utc::now())
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(logged(&store, &sym).await, [(Some(200), BODY.len() as i64)]);
    let url = client_calls(&client);
    assert!(url.ends_with(&format!(
        "/tiingo/daily/{sym}/prices?startDate=2026-01-05&endDate=2026-01-06"
    )));
    assert!(!url.contains("token"), "el token nunca va en la URL");
}

fn client_calls(c: &Client<Fake>) -> String {
    c.transport().calls.lock().unwrap().last().cloned().unwrap()
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn failures_are_kill_switch_and_still_logged() {
    let store = TiingoStore::connect(&url()).await.unwrap();
    let key = ApiKey::new("x".into()).unwrap();
    type Case = (Answer, fn(&TiingoError) -> bool, (Option<i32>, i64));
    let cases: [Case; 5] = [
        (
            Answer::Ok(401, "{}"),
            |e| matches!(e, TiingoError::Http { status: 401, .. }),
            (Some(401), 2),
        ),
        (
            Answer::Ok(429, ""),
            |e| e.to_string().contains("límite de pedidos"),
            (Some(429), 0),
        ),
        (
            Answer::Ok(200, "{\"detail\":\"x\"}"),
            |e| matches!(e, TiingoError::Shape { .. }),
            (Some(200), 14),
        ),
        (
            Answer::Network,
            |e| matches!(e, TiingoError::Network(_)),
            (None, 0),
        ),
        (
            Answer::TooLarge,
            |e| matches!(e, TiingoError::TooLarge { .. }),
            (None, 262_145),
        ),
    ];
    for (answer, is, row) in cases {
        let sym = symbol();
        let client = Client::new(config(1_000_000), key.clone(), Fake::new(answer));
        let e = client
            .daily_prices(&store, &sym, day(5), day(6), Utc::now())
            .await
            .unwrap_err();
        assert!(is(&e), "{e}");
        assert!(e.to_string().starts_with("kill switch"), "{e}");
        assert_eq!(logged(&store, &sym).await, [row]);
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_hourly_cap_counts_a_rolling_window_and_stops_before_asking() {
    let store = TiingoStore::connect(&url()).await.unwrap();
    // Una hora futura propia de esta corrida (separada de otras por más de 2 h): así la ventana
    // de 60 minutos no ve los pedidos de los otros tests, que corren en paralelo.
    let slot = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        % 100_000;
    let now =
        Utc::now() + TimeDelta::days(3650) + TimeDelta::hours(2 * i64::try_from(slot).unwrap());
    let used = store.usage(now).await.unwrap().requests_last_hour;
    let client = Client::new(
        config(used + 1),
        ApiKey::new("x".into()).unwrap(),
        Fake::new(Answer::Ok(200, BODY)),
    );
    let sym = symbol();
    client
        .daily_prices(&store, &sym, day(5), day(6), now)
        .await
        .unwrap();
    let e = client
        .daily_prices(&store, &sym, day(5), day(6), now)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("60 minutos"), "{e}");
    assert_eq!(
        client.transport().calls.lock().unwrap().len(),
        1,
        "el segundo no se pidió"
    );
    // Una hora y un minuto después, ese pedido ya no cuenta en la ventana de 60 minutos.
    let later = store.usage(now + TimeDelta::minutes(61)).await.unwrap();
    assert!(later.requests_last_hour < used + 1);
    assert!(later.symbols_last_31_days.contains(&sym));
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn only_verdicts_are_stored_and_the_tables_are_append_only() {
    let store = TiingoStore::connect(&url()).await.unwrap();
    let id: InstrumentId = "US500".parse().unwrap();
    sqlx::query("INSERT INTO instruments (id, name, asset_class, currency, exchange_timezone) VALUES ('US500', 'US 500', 'index', 'USD', 'America/New_York') ON CONFLICT (id) DO NOTHING")
        .execute(store.pool())
        .await
        .unwrap();
    let at = Utc::now();
    let verdicts = [
        DayVerdict {
            day: day(6),
            method: Method::M15Close,
            verdict: Verdict::Match,
        },
        DayVerdict {
            day: day(7),
            method: Method::None,
            verdict: Verdict::NoData,
        },
    ];
    store
        .record_verdicts(&id, "SPY", at, &verdicts)
        .await
        .unwrap();
    let rows: Vec<(NaiveDate, String, String)> = sqlx::query_as(
        "SELECT day, method, verdict FROM tiingo_verifications WHERE verified_at = $1 ORDER BY day",
    )
    .bind(at)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        rows,
        [
            (day(6), "m15_close".into(), "match".into()),
            (day(7), "none".into(), "no_data".into())
        ]
    );
    // Los triggers se prueban sobre filas propias: la tabla puede estar vacía o tener filas de
    // otros tests que corren en paralelo (#229).
    let (verification,): (i64,) =
        sqlx::query_as("SELECT min(id) FROM tiingo_verifications WHERE verified_at = $1")
            .bind(at)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let (usage,): (i64,) = sqlx::query_as(
        "INSERT INTO tiingo_usage_log (requested_at, endpoint, symbol, http_status, bytes)
         VALUES ($1, 'eod', 'SPY', 200, 1) RETURNING id",
    )
    .bind(at)
    .fetch_one(store.pool())
    .await
    .unwrap();
    for (sql, id) in [
        (
            "DELETE FROM tiingo_verifications WHERE id = $1",
            verification,
        ),
        ("DELETE FROM tiingo_usage_log WHERE id = $1", usage),
        ("UPDATE tiingo_usage_log SET bytes = 0 WHERE id = $1", usage),
    ] {
        let e = sqlx::query(sql)
            .bind(id)
            .execute(store.pool())
            .await
            .unwrap_err();
        assert!(e.to_string().contains("solo agregar"), "{e}");
    }
    // Un veredicto sin dato con método, o al revés, no entra.
    let bad = [DayVerdict {
        day: day(8),
        method: Method::M15Close,
        verdict: Verdict::NoData,
    }];
    assert!(store.record_verdicts(&id, "SPY", at, &bad).await.is_err());
}

//! `GET /api/v1/ibkr/bars` y `GET /api/v1/ibkr/watchlist` (#551): velas cerradas de IBKR y el
//! resumen de la watchlist, solo lectura. Datos sintéticos; la lectura de PostgreSQL real está al
//! final (`#[ignore]`, el CI la corre con `-- --ignored`).

use std::path::PathBuf;

use cfds_gateway::ibkr::{IbkrBarSource, IbkrViewConfig, is_ibkr_route, quote, respond_ibkr};
use cfds_storage::{IbkrBarRow, IbkrSeries};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::Value;

const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
const TERMINAL: &str = include_str!("../../../../config/terminal.toml");
const IBKR: &str = include_str!("../../../../config/ibkr.toml");

fn t(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn cfg() -> IbkrViewConfig {
    IbkrViewConfig::from_toml(GATEWAY, TERMINAL, IBKR).unwrap()
}

fn contracts() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../contracts")
}

/// Resuelve los `$ref` entre contratos con los archivos del repo, sin red.
struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri.as_str().rsplit('/').next().ok_or("uri sin archivo")?;
        let text = std::fs::read_to_string(contracts().join(name))?;
        Ok(serde_json::from_str(&text)?)
    }
}

fn assert_valid(name: &str, doc: &Value) {
    let text = std::fs::read_to_string(contracts().join(name)).unwrap();
    let v = jsonschema::options()
        .with_retriever(Local)
        .build(&serde_json::from_str(&text).unwrap())
        .unwrap();
    let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
    assert!(
        errors.is_empty(),
        "no valida contra {name}: {errors:?}\n{doc}"
    );
}

#[allow(clippy::too_many_arguments)]
fn bar(
    id: &str,
    kind: &str,
    contract: &str,
    size: &str,
    start: &str,
    close: Decimal,
    volume: Option<Decimal>,
) -> IbkrBarRow {
    IbkrBarRow {
        source: "ibkr".into(),
        instrument_id: id.into(),
        kind: kind.into(),
        contract: contract.into(),
        bar_size: size.into(),
        what_to_show: "TRADES".into(),
        start: t(start),
        open: close,
        high: close + dec!(1),
        low: close - dec!(1),
        close,
        volume,
    }
}

fn len(size: &str) -> TimeDelta {
    cfds_gateway::ibkr::bar_length(size).unwrap()
}

/// Lo mismo que hace PostgreSQL, en memoria.
struct Fake(Vec<IbkrBarRow>);

fn same(r: &IbkrBarRow, s: &IbkrSeries<'_>) -> bool {
    r.source == s.source
        && r.instrument_id == s.instrument_id
        && r.contract == s.contract
        && r.bar_size == s.bar_size
        && r.what_to_show == s.what_to_show
}

impl IbkrBarSource for Fake {
    async fn latest_closed(
        &self,
        source: &str,
        instrument: &str,
        bar_size: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, String> {
        let rank = |s: &str| ["1m", "15m", "1h"].iter().position(|x| *x == s);
        Ok(self
            .0
            .iter()
            .filter(|r| r.source == source && r.instrument_id == instrument)
            .filter(|r| bar_size.is_none_or(|b| r.bar_size == b))
            .filter(|r| r.start + len(&r.bar_size) <= at)
            .max_by(|a, b| {
                (a.start + len(&a.bar_size))
                    .cmp(&(b.start + len(&b.bar_size)))
                    .then(rank(&b.bar_size).cmp(&rank(&a.bar_size)))
            })
            .cloned())
    }
    async fn closed_by(
        &self,
        s: &IbkrSeries<'_>,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<IbkrBarRow>, String> {
        let mut v: Vec<IbkrBarRow> = self
            .0
            .iter()
            .filter(|r| same(r, s) && r.start + len(&r.bar_size) <= at)
            .cloned()
            .collect();
        v.sort_by_key(|r| r.start);
        let skip = v.len().saturating_sub(limit as usize);
        Ok(v.split_off(skip))
    }
    async fn starting_before(
        &self,
        s: &IbkrSeries<'_>,
        before: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, String> {
        Ok(self
            .0
            .iter()
            .filter(|r| same(r, s) && r.start < before)
            .max_by_key(|r| r.start)
            .cloned())
    }
}

struct Broken;

impl IbkrBarSource for Broken {
    async fn latest_closed(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, String> {
        Err("sin base: password=secreto".into())
    }
    async fn closed_by(
        &self,
        _: &IbkrSeries<'_>,
        _: DateTime<Utc>,
        _: u32,
    ) -> Result<Vec<IbkrBarRow>, String> {
        Err("sin base".into())
    }
    async fn starting_before(
        &self,
        _: &IbkrSeries<'_>,
        _: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, String> {
        Err("sin base".into())
    }
}

/// MNQ (futuro, +2 h): la sesión del 25 abre el 24 a las 22:00 UTC. Velas de 1m del 24 a la
/// tarde (sesión del 24) y del 25; una de 15m en curso; otro vencimiento cargado antes.
fn fake() -> Fake {
    Fake(vec![
        bar(
            "MNQ",
            "future",
            "MNQU6",
            "1m",
            "2026-11-24T21:59:00Z",
            dec!(19000),
            None,
        ),
        bar(
            "MNQ",
            "future",
            "MNQZ6",
            "1m",
            "2026-11-24T21:58:00Z",
            dec!(20000),
            Some(dec!(3)),
        ),
        bar(
            "MNQ",
            "future",
            "MNQZ6",
            "1m",
            "2026-11-24T21:59:00Z",
            dec!(20010.25),
            Some(dec!(4)),
        ),
        bar(
            "MNQ",
            "future",
            "MNQZ6",
            "1m",
            "2026-11-25T11:58:00Z",
            dec!(20100),
            Some(dec!(5)),
        ),
        bar(
            "MNQ",
            "future",
            "MNQZ6",
            "1m",
            "2026-11-25T11:59:00Z",
            dec!(20110.5),
            Some(dec!(6)),
        ),
        // A las 12:15 cerró hace 14 minutos (12:01) y es la última de la serie: puede tener datos a
        // medias por el atraso de IBKR (15 min en config/ibkr.toml), no se sirve (#575).
        bar(
            "MNQ",
            "future",
            "MNQZ6",
            "1m",
            "2026-11-25T12:00:00Z",
            dec!(20999),
            Some(dec!(1)),
        ),
        // 15m que cierra 12:00, a la vez que la de 1m de 11:59: gana la de 1m.
        bar(
            "MNQ",
            "future",
            "MNQZ6",
            "15m",
            "2026-11-25T11:45:00Z",
            dec!(20111),
            Some(dec!(40)),
        ),
        // QQQ (acción, -5 h): sesión del 24 hasta 05:00 UTC del 25.
        bar(
            "QQQ",
            "stock",
            "QQQ",
            "1h",
            "2026-11-24T20:00:00Z",
            dec!(500),
            Some(dec!(10)),
        ),
        bar(
            "QQQ",
            "stock",
            "QQQ",
            "1h",
            "2026-11-25T00:00:00Z",
            dec!(501),
            Some(dec!(10)),
        ),
        bar(
            "QQQ",
            "stock",
            "QQQ",
            "1h",
            "2026-11-25T09:00:00Z",
            dec!(495),
            Some(dec!(10)),
        ),
    ])
}

fn now() -> DateTime<Utc> {
    t("2026-11-25T12:15:00Z")
}

async fn get<S: IbkrBarSource>(s: &S, method: &str, path: &str, q: Option<&str>) -> (u16, Value) {
    let r = respond_ibkr(s, &cfg(), method, path, q, now()).await;
    (
        r.status,
        serde_json::from_str(&r.body).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn bars_are_closed_from_one_contract_and_follow_the_contract() {
    let f = fake();
    let (status, body) = get(
        &f,
        "GET",
        "/api/v1/ibkr/bars",
        Some("instrument=MNQ&timeframe=1m"),
    )
    .await;
    assert_eq!(status, 200);
    assert_valid("gateway_ibkr_bars.v1.schema.json", &body);
    assert_eq!(body["series"]["contract"], "MNQZ6");
    assert_eq!(body["as_of"], "2026-11-25T12:15:00Z");
    let starts: Vec<&str> = body["bars"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["start"].as_str().unwrap())
        .collect();
    // Sin la vela a medias ni la del otro vencimiento, en orden.
    assert_eq!(
        starts,
        [
            "2026-11-24T21:58:00Z",
            "2026-11-24T21:59:00Z",
            "2026-11-25T11:58:00Z",
            "2026-11-25T11:59:00Z"
        ]
    );
    assert_eq!(body["bars"][1]["close"], "20010.25");
    assert_eq!(body["bars"][1]["volume"], "4");

    // limit: las últimas n; before: lo conocido en ese momento (paginación hacia atrás).
    let (_, two) = get(
        &f,
        "GET",
        "/api/v1/ibkr/bars",
        Some("instrument=MNQ&timeframe=1m&limit=2"),
    )
    .await;
    assert_eq!(two["bars"].as_array().unwrap().len(), 2);
    assert_eq!(two["bars"][0]["start"], "2026-11-25T11:58:00Z");
    let (_, older) = get(
        &f,
        "GET",
        "/api/v1/ibkr/bars",
        Some("instrument=MNQ&timeframe=1m&before=2026-11-25T11%3A58%3A00Z"),
    )
    .await;
    assert_valid("gateway_ibkr_bars.v1.schema.json", &older);
    assert_eq!(older["as_of"], "2026-11-25T11:58:00Z");
    assert_eq!(older["bars"].as_array().unwrap().len(), 2);
    assert_eq!(older["bars"][1]["start"], "2026-11-24T21:59:00Z");

    // Un before futuro cuenta como ahora.
    let (_, future) = get(
        &f,
        "GET",
        "/api/v1/ibkr/bars",
        Some("instrument=MNQ&timeframe=1m&before=2030-01-01T00:00:00Z"),
    )
    .await;
    assert_eq!(future["as_of"], "2026-11-25T12:15:00Z");

    // Sin datos: series null y sin velas, válido.
    let (status, empty) = get(
        &f,
        "GET",
        "/api/v1/ibkr/bars",
        Some("instrument=MES&timeframe=1h"),
    )
    .await;
    assert_eq!(status, 200);
    assert_valid("gateway_ibkr_bars.v1.schema.json", &empty);
    assert_eq!(empty["series"], Value::Null);
    assert_eq!(empty["bars"], serde_json::json!([]));
}

#[tokio::test]
async fn watchlist_last_close_delay_and_previous_session() {
    let f = fake();
    let (status, body) = get(&f, "GET", "/api/v1/ibkr/watchlist", None).await;
    assert_eq!(status, 200);
    assert_valid("gateway_ibkr_watchlist.v1.schema.json", &body);
    let ids: Vec<&str> = body["instruments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["instrument_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["MNQ", "MES", "QQQ"], "orden de [ibkr] instruments");

    let mnq = &body["instruments"][0]["quote"];
    assert_eq!(mnq["timeframe"], "1m");
    assert_eq!(mnq["contract"], "MNQZ6");
    assert_eq!(mnq["last"], "20110.5");
    assert_eq!(mnq["bar_start"], "2026-11-25T11:59:00Z");
    assert_eq!(mnq["bar_end"], "2026-11-25T12:00:00Z");
    assert_eq!(mnq["delay_seconds"], 900);
    assert_eq!(mnq["session"], "2026-11-25");
    // La referencia es del mismo contrato (MNQU6 tiene otra vela a las 21:59).
    assert_eq!(mnq["previous_session"], "2026-11-24");
    assert_eq!(mnq["previous_close"], "20010.25");
    assert_eq!(mnq["change"], "100.25");
    assert_eq!(mnq["change_pct"], "0.50");

    assert_eq!(body["instruments"][1]["quote"], Value::Null);

    // QQQ: la vela de 00:00 UTC del 25 es de la sesión del 24 (-5 h).
    let qqq = &body["instruments"][2]["quote"];
    assert_eq!(qqq["session"], "2026-11-25");
    assert_eq!(qqq["previous_session"], "2026-11-24");
    assert_eq!(qqq["previous_close"], "501");
    assert_eq!(qqq["change"], "-6");
    assert_eq!(qqq["change_pct"], "-1.20");
    assert_eq!(qqq["delay_seconds"], 8100);
}

/// QA del 25-11-2026 (#575): a las 04:04 la ruta servía como cerrada la vela de 15m de las 03:45
/// armada con solo 5 velas de 1m (03:45 a 03:49; los datos de IBKR llegan con ~10 min de atraso), y
/// la watchlist tomaba su cierre. Ahora, con el criterio de la carga (`complete_prefix_by`), la
/// última vela de una serie se sirve solo si cerró hace al menos `[load].data_delay_secs`.
#[tokio::test]
async fn regression_575_a_bar_with_partial_data_is_not_served_as_closed() {
    let mut rows = Vec::new();
    for m in 30..50 {
        rows.push(bar(
            "MNQ",
            "future",
            "MNQZ6",
            "1m",
            &format!("2026-11-25T03:{m:02}:00Z"),
            Decimal::from(26000 + m),
            Some(dec!(1)),
        ));
    }
    rows.push(bar(
        "MNQ",
        "future",
        "MNQZ6",
        "15m",
        "2026-11-25T03:30:00Z",
        dec!(26044),
        Some(dec!(15)),
    ));
    // A medias: el cierre es el de la 1m de las 03:49.
    rows.push(bar(
        "MNQ",
        "future",
        "MNQZ6",
        "15m",
        "2026-11-25T03:45:00Z",
        dec!(26049),
        Some(dec!(5)),
    ));
    let f = Fake(rows);
    let ask = |q: &'static str, at: &'static str| {
        let f = &f;
        async move {
            let r = respond_ibkr(f, &cfg(), "GET", "/api/v1/ibkr/bars", Some(q), t(at)).await;
            let body: Value = serde_json::from_str(&r.body).unwrap();
            assert_valid("gateway_ibkr_bars.v1.schema.json", &body);
            body["bars"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| b["start"].as_str().unwrap().to_owned())
                .collect::<Vec<String>>()
        }
    };
    let q15 = "instrument=MNQ&timeframe=15m";
    assert_eq!(
        ask(q15, "2026-11-25T04:04:00Z").await,
        ["2026-11-25T03:30:00Z"]
    );
    // `limit` cuenta solo las que se sirven.
    assert_eq!(
        ask(
            "instrument=MNQ&timeframe=15m&limit=1",
            "2026-11-25T04:04:00Z"
        )
        .await,
        ["2026-11-25T03:30:00Z"]
    );
    // Cerró hace 15 minutos: ya se sirve.
    assert_eq!(
        ask(q15, "2026-11-25T04:15:00Z").await,
        ["2026-11-25T03:30:00Z", "2026-11-25T03:45:00Z"]
    );
    // En 1m: la última (03:49) espera; las anteriores tienen una vela después.
    let m1 = ask(
        "instrument=MNQ&timeframe=1m&limit=2",
        "2026-11-25T04:04:00Z",
    )
    .await;
    assert_eq!(m1, ["2026-11-25T03:47:00Z", "2026-11-25T03:48:00Z"]);

    // La watchlist: la última vela completa (1m de las 03:48), no el cierre de la de 15m a medias.
    let r = respond_ibkr(
        &f,
        &cfg(),
        "GET",
        "/api/v1/ibkr/watchlist",
        None,
        t("2026-11-25T04:04:00Z"),
    )
    .await;
    let body: Value = serde_json::from_str(&r.body).unwrap();
    assert_valid("gateway_ibkr_watchlist.v1.schema.json", &body);
    let mnq = &body["instruments"][0]["quote"];
    assert_eq!(
        (&mnq["timeframe"], &mnq["bar_start"], &mnq["last"]),
        (
            &Value::from("1m"),
            &Value::from("2026-11-25T03:48:00Z"),
            &Value::from("26048")
        )
    );
}

#[test]
fn quote_without_reference_or_with_one_of_the_same_session() {
    let c = cfg();
    let last = bar(
        "MNQ",
        "future",
        "MNQZ6",
        "1m",
        "2026-11-25T11:59:00Z",
        dec!(10),
        None,
    );
    let q = quote(&last, None, &c.session, now()).unwrap();
    assert_eq!(
        (q.previous_close, q.change, q.change_pct),
        (None, None, None)
    );
    // Una vela de la misma sesión no es referencia.
    let same_day = bar(
        "MNQ",
        "future",
        "MNQZ6",
        "1m",
        "2026-11-24T22:00:00Z",
        dec!(9),
        None,
    );
    let q = quote(&last, Some(&same_day), &c.session, now()).unwrap();
    assert_eq!(q.previous_close, None);
    // Ni una de otro contrato.
    let other = bar(
        "MNQ",
        "future",
        "MNQU6",
        "1m",
        "2026-11-24T21:00:00Z",
        dec!(9),
        None,
    );
    assert_eq!(
        quote(&last, Some(&other), &c.session, now())
            .unwrap()
            .change,
        None
    );
    // Una vela que todavía no cerró (reloj atrasado) no da atraso negativo.
    let q = quote(&last, None, &c.session, t("2026-11-25T11:59:30Z")).unwrap();
    assert_eq!(q.delay_seconds, 0);
}

#[tokio::test]
async fn only_get_on_known_routes_with_valid_params() {
    let f = fake();
    let path = "/api/v1/ibkr/bars";
    for q in [
        None,
        Some("timeframe=1m"),
        Some("instrument=MNQ"),
        Some("instrument=MNQ&timeframe=1d"),
        Some("instrument=MNQ&timeframe=1m&limit=0"),
        Some("instrument=MNQ&timeframe=1m&limit=5001"),
        Some("instrument=MNQ&timeframe=1m&limit=x"),
        Some("instrument=MNQ&timeframe=1m&before=2026-11-25T12:00:00"),
        Some("instrument=mnq;drop&timeframe=1m"),
    ] {
        assert_eq!(get(&f, "GET", path, q).await.0, 400, "{q:?}");
    }
    assert_eq!(
        get(&f, "GET", path, Some("instrument=EURUSD&timeframe=1m"))
            .await
            .0,
        404
    );
    for method in ["POST", "PUT", "DELETE"] {
        assert_eq!(
            get(&f, method, path, Some("instrument=MNQ&timeframe=1m"))
                .await
                .0,
            405
        );
        assert_eq!(get(&f, method, "/api/v1/ibkr/watchlist", None).await.0, 405);
    }
    assert!(!is_ibkr_route("/api/v1/ibkr"));
    assert!(!is_ibkr_route("/api/v1/ibkr/bars/MNQ"));
    // Los errores de la base no llegan al cliente.
    for (p, q) in [
        (path, Some("instrument=MNQ&timeframe=1m")),
        ("/api/v1/ibkr/watchlist", None),
    ] {
        let r = respond_ibkr(&Broken, &cfg(), "GET", p, q, now()).await;
        assert_eq!(r.status, 500);
        assert!(!r.body.contains("secreto"));
    }
}

/// Las lecturas de PostgreSQL: solo velas cerradas, una sola serie, la referencia de la sesión
/// anterior y documentos válidos de los dos contratos. La tabla es de solo agregar: cada corrida
/// usa un instrumento propio.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_store_reads_closed_bars_and_the_previous_session() {
    use cfds_storage::PgStore;

    let url = cfds_storage::test_db::test_database_url();
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    let id = cfds_storage::test_db::unique_id("ITGI");
    let mut rows = fake().0;
    rows.retain(|r| r.instrument_id == "MNQ");
    for r in &mut rows {
        r.instrument_id.clone_from(&id);
    }
    store.insert_ibkr_bars(&rows).await.unwrap();

    let at = t("2026-11-25T12:00:30Z");
    let last = store
        .latest_closed_ibkr_bar("ibkr", &id, None, at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((last.bar_size.as_str(), last.close), ("1m", dec!(20110.5)));
    let last_15m = store
        .latest_closed_ibkr_bar("ibkr", &id, Some("15m"), at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(last_15m.close, dec!(20111));
    // A las 11:59:59 la de 15m de 11:45 todavía no cerró.
    assert!(
        store
            .latest_closed_ibkr_bar("ibkr", &id, Some("15m"), t("2026-11-25T11:59:59Z"))
            .await
            .unwrap()
            .is_none()
    );
    let series = IbkrSeries {
        source: "ibkr",
        instrument_id: &id,
        contract: "MNQZ6",
        bar_size: "1m",
        what_to_show: "TRADES",
    };
    let bars = store.ibkr_bars_closed_by(&series, at, 3).await.unwrap();
    let closes: Vec<Decimal> = bars.iter().map(|b| b.close).collect();
    assert_eq!(closes, [dec!(20010.25), dec!(20100), dec!(20110.5)]);
    assert_eq!(bars[0].volume, Some(dec!(4)));
    let prev = store
        .last_ibkr_bar_starting_before(&series, t("2026-11-24T22:00:00Z"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prev.close, dec!(20010.25));

    // Lo mismo de punta a punta por las rutas, contra PostgreSQL.
    let mut c = cfg();
    c.instruments = vec![id.parse().unwrap()];
    let r = respond_ibkr(
        &store,
        &c,
        "GET",
        "/api/v1/ibkr/bars",
        Some(&format!("instrument={id}&timeframe=1m&limit=2")),
        now(),
    )
    .await;
    assert_eq!(r.status, 200);
    let body: Value = serde_json::from_str(&r.body).unwrap();
    assert_valid("gateway_ibkr_bars.v1.schema.json", &body);
    assert_eq!(body["bars"][1]["close"], "20110.5");
    let r = respond_ibkr(&store, &c, "GET", "/api/v1/ibkr/watchlist", None, now()).await;
    let body: Value = serde_json::from_str(&r.body).unwrap();
    assert_valid("gateway_ibkr_watchlist.v1.schema.json", &body);
    assert_eq!(body["instruments"][0]["quote"]["change"], "100.25");
}

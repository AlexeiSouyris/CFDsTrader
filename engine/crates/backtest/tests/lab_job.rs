//! Laboratorio (#774): `lab_job_request.v1` y `lab_job_event.v1` contra sus schemas, el hash del
//! pedido (normalizado), el id del trabajo y el chequeo de las variantes. La curva en vivo de cada
//! variante (`curve`, #854): reducción de puntos y contrato.

use std::path::PathBuf;

use cfds_backtest::lab::{
    JobStage, JobState, LabConfig, LabJobEvent, LabJobRequest, StrategyCatalog, StrategySpec,
    TrialCurve, ValidSpec, catalog, check_variants, downsample, job_id, request_sha256,
    validate_spec,
};
use cfds_market_data::{DataSplit, Period};
use cfds_risk::RiskConfig;
use rust_decimal::dec;
use serde_json::{Value, json};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
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
        Ok(serde_json::from_str(&std::fs::read_to_string(
            contracts().join(name),
        )?)?)
    }
}

fn validator(name: &str) -> jsonschema::Validator {
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(contracts().join(name)).unwrap()).unwrap();
    jsonschema::options()
        .with_retriever(Local)
        .build(&doc)
        .expect("schema inválido")
}

fn engine_catalog() -> StrategyCatalog {
    let lab = LabConfig::from_toml(cfg!("lab.toml")).unwrap();
    let risk = RiskConfig::from_toml(cfg!("risk.toml")).unwrap();
    catalog(&lab, &risk, ["US500".parse().unwrap()]).unwrap()
}

fn split() -> DataSplit {
    DataSplit {
        windows: Vec::new(),
        out_of_sample: Period {
            start: "2025-11-01T00:00:00Z".parse().unwrap(),
            end: "2026-11-01T00:00:00Z".parse().unwrap(),
        },
    }
}

fn spec(fast: u32, k: &str) -> Value {
    json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": "1h" },
        "direction": "both",
        "entry": [
            { "type": "ma_cross", "params": { "average": "ema", "fast": fast, "slow": 50 } }
        ],
        "filters": [],
        "stop": { "type": "atr", "params": { "period": 14, "k": k } },
        "exit": { "type": "time_target", "params": { "max_candles": 6, "target_r": "2" } },
        "risk_pct": "0.5",
        "cost_multiplier": "1",
        "period": { "from": "2015-01-01T00:00:00Z", "to": "2023-01-01T00:00:00Z" },
        "mode": "exploratory"
    })
}

fn valid(doc: &Value) -> ValidSpec {
    let s: StrategySpec = serde_json::from_value(doc.clone()).unwrap();
    validate_spec(&s, &engine_catalog(), &split()).unwrap()
}

fn event(state: &str, stage: Value, reports: Value, error: Value, percent: u8) -> Value {
    json!({
        "job_id": "a".repeat(64),
        "seq": 7,
        "state": state,
        "stage": stage,
        "percent": percent,
        "variants_done": 1,
        "variants_total": 2,
        "report_ids": reports,
        "error": error,
        "recorded_at": "2026-11-27T01:02:03.456Z"
    })
}

#[test]
fn a_request_roundtrips_and_the_schema_checks_its_shape() {
    let v = validator("lab_job_request.v1.schema.json");
    let doc = json!({ "specs": [spec(20, "1.5"), spec(30, "1.5")], "initial_equity": "100000" });
    let errors: Vec<String> = v.iter_errors(&doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let req: LabJobRequest = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(serde_json::to_value(&req).unwrap(), doc);
    for bad in [
        json!({ "specs": [], "initial_equity": "100000" }),
        json!({ "specs": [spec(20, "1.5")], "initial_equity": "0" }),
        json!({ "specs": [spec(20, "1.5")], "initial_equity": 100000 }),
        json!({ "specs": [spec(20, "1.5")], "initial_equity": "1", "extra": 1 }),
        json!({ "specs": [{ "mode": "exploratory" }], "initial_equity": "1" }),
    ] {
        assert!(!v.is_valid(&bad), "el schema debería rechazar {bad}");
    }
    assert!(serde_json::from_value::<LabJobRequest>(json!({ "specs": [] })).is_err());
}

#[test]
fn events_roundtrip_and_the_schema_ties_fields_to_the_state() {
    let v = validator("lab_job_event.v1.schema.json");
    let h = json!(["b".repeat(64), "c".repeat(64)]);
    let good = [
        event("queued", json!(null), json!([]), json!(null), 0),
        event(
            "running",
            json!("running_variants"),
            json!([]),
            json!(null),
            40,
        ),
        event("done", json!(null), h.clone(), json!(null), 100),
        event(
            "failed",
            json!(null),
            json!([]),
            json!("sin velas de futuros"),
            10,
        ),
        event("cancelled", json!(null), json!([]), json!(null), 0),
    ];
    for doc in &good {
        let errors: Vec<String> = v.iter_errors(doc).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{doc}: {errors:?}");
        let e: LabJobEvent = serde_json::from_value(doc.clone()).unwrap();
        assert_eq!(&serde_json::to_value(&e).unwrap(), doc);
    }
    let bad = [
        event("running", json!(null), json!([]), json!(null), 40),
        event("queued", json!("started"), json!([]), json!(null), 0),
        event("done", json!(null), json!([]), json!(null), 100),
        event("done", json!(null), h.clone(), json!(null), 90),
        event("queued", json!(null), h, json!(null), 0),
        event("failed", json!(null), json!([]), json!(null), 0),
        event("cancelled", json!(null), json!([]), json!("x"), 0),
        event("running", json!("thinking"), json!([]), json!(null), 0),
        event("running", json!("started"), json!([]), json!(null), 101),
    ];
    for doc in &bad {
        assert!(!v.is_valid(doc), "el schema debería rechazar {doc}");
    }
    let e: LabJobEvent = serde_json::from_value(good[1].clone()).unwrap();
    assert_eq!(e.state, JobState::Running);
    assert_eq!(e.stage, Some(JobStage::RunningVariants));
    assert!(!e.state.is_end() && JobState::Cancelled.is_end());
    for s in [JobStage::Started, JobStage::Saving] {
        assert_eq!(JobStage::parse(s.as_str()), Some(s));
    }
}

#[test]
fn the_request_hash_ignores_decimal_spelling_and_the_attempt_changes_the_job() {
    let a = [valid(&spec(20, "1.5")), valid(&spec(30, "1.5"))];
    let b = [valid(&spec(20, "1.50")), valid(&spec(30, "1.500"))];
    let ha = request_sha256(&a, dec!(100000));
    assert_eq!(ha, request_sha256(&b, dec!(100000.00)));
    assert_ne!(ha, request_sha256(&a, dec!(50000)));
    // El orden de las variantes es el de los informes: cuenta.
    assert_ne!(
        ha,
        request_sha256(&[a[1].clone(), a[0].clone()], dec!(100000))
    );
    assert_eq!(ha.len(), 64);
    assert_ne!(job_id(&ha, 1), job_id(&ha, 2));
    assert_eq!(job_id(&ha, 1), job_id(&ha, 1));
    let req = LabJobRequest::normalized(&b, dec!(100000.00));
    assert_eq!(req.initial_equity.to_string(), "100000");
    assert_eq!(req.specs[0], a[0].spec);
}

#[test]
fn variants_are_one_family_one_period_and_capped() {
    let cat = engine_catalog();
    let a = valid(&spec(20, "1.5"));
    let b = valid(&spec(30, "2"));
    assert!(check_variants(&[a.clone(), b.clone()], &cat).is_ok());
    assert!(check_variants(&[], &cat).is_err());
    let mut other_period = spec(30, "2");
    other_period["period"]["from"] = json!("2016-01-01T00:00:00Z");
    let err = check_variants(&[a.clone(), valid(&other_period)], &cat).unwrap_err();
    assert!(err.contains("misma familia y período"), "{err}");
    let mut other_family = spec(30, "2");
    other_family["filters"] = json!([
        { "type": "entry_hours_utc", "params": { "from": "13:30", "to": "20:00" } }
    ]);
    assert!(check_variants(&[a.clone(), valid(&other_family)], &cat).is_err());
    let too_many = vec![a; cat.limits.max_variants_per_run + 1];
    assert!(
        check_variants(&too_many, &cat)
            .unwrap_err()
            .contains("como mucho")
    );
}

#[test]
fn downsample_keeps_the_ends_and_spreads_the_rest() {
    let xs: Vec<u32> = (0..1000).collect();
    let d = downsample(&xs, 200);
    assert_eq!(d.len(), 200);
    assert_eq!((d[0], d[199]), (0, 999));
    assert!(d.windows(2).all(|w| w[0] < w[1]), "en orden y sin repetir");
    // pasos parejos: ninguno se aleja más de uno del ideal (999 / 199 ≈ 5,02)
    assert!(d.windows(2).all(|w| (5..=6).contains(&(w[1] - w[0]))));
    // casos borde: corto, justo, vacío, tope 0, 1 y 2
    assert_eq!(downsample(&xs[..5], 200), xs[..5].to_vec());
    assert_eq!(downsample(&xs[..200], 200), xs[..200].to_vec());
    assert!(downsample::<u32>(&[], 200).is_empty());
    assert!(downsample(&xs, 0).is_empty());
    assert_eq!(downsample(&xs, 1), vec![999]);
    assert_eq!(downsample(&xs, 2), vec![0, 999]);
    assert_eq!(downsample(&xs[..201], 200).len(), 200);
}

#[test]
fn the_live_curve_is_the_report_curve_from_one_and_fits_the_contract() {
    let v = validator("lab_job_event.v1.schema.json");
    let c = TrialCurve::from_monthly(3, &[0.1, -0.5, 0.2], 200);
    assert_eq!(c.variant, 3);
    assert_eq!(c.months, 3);
    // 1 → 1,1 → 0,55 → 0,66: capital compuesto, como months[].equity del informe
    assert_eq!(c.equity, ["1", "1.1", "0.55", "0.66"]);
    // sin meses: solo el punto de partida
    assert_eq!(TrialCurve::from_monthly(0, &[], 200).equity, ["1"]);
    // un mes sin valor corta la curva ahí (nunca se inventa)
    assert_eq!(
        TrialCurve::from_monthly(0, &[0.1, f64::NAN, 0.1], 200).equity,
        ["1", "1.1"]
    );
    // 20 años de meses con el tope de config → 200 puntos, último = capital final
    let long: Vec<f64> = vec![0.001; 240];
    let big = TrialCurve::from_monthly(1, &long, 200);
    assert_eq!((big.months, big.equity.len()), (240, 200));
    let full = TrialCurve::from_monthly(1, &long, 1000);
    assert_eq!(big.equity.last(), full.equity.last());

    let with = |curve: Value, state: &str, stage: Value| {
        let mut doc = event(state, stage, json!([]), json!(null), 40);
        doc["curve"] = curve;
        doc
    };
    let running = json!("running_variants");
    let doc = with(
        serde_json::to_value(&c).unwrap(),
        "running",
        running.clone(),
    );
    let errors: Vec<String> = v.iter_errors(&doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let e: LabJobEvent = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(e.curve.as_ref(), Some(&c));
    assert_eq!(serde_json::to_value(&e).unwrap(), doc);
    // sin curva no se escribe el campo (los eventos de antes siguen iguales)
    let plain: LabJobEvent = serde_json::from_value(event(
        "running",
        running.clone(),
        json!([]),
        json!(null),
        40,
    ))
    .unwrap();
    assert!(serde_json::to_value(&plain).unwrap().get("curve").is_none());

    let points = |n: usize| json!({ "variant": 0, "months": n, "equity": vec!["1"; n] });
    for bad in [
        with(points(201), "running", running.clone()),
        with(points(0), "running", running.clone()),
        with(
            json!({ "variant": 0, "months": 1, "equity": ["uno"] }),
            "running",
            running.clone(),
        ),
        with(
            json!({ "variant": 0, "months": 1, "equity": ["1"], "x": 1 }),
            "running",
            running,
        ),
        with(points(2), "running", json!("building_reports")),
        with(points(2), "queued", json!(null)),
    ] {
        assert!(!v.is_valid(&bad), "el schema debería rechazar {bad}");
    }
}

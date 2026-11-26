//! Laboratorio (#761): `lab_report` arma el `lab_report.v1` completo de una corrida sintética
//! (spec → planes → `run_plans` → informe), que valida contra el schema, con y sin operaciones.
//! Calendario real de XNYS y config del repositorio; nunca la base real.

use std::path::PathBuf;

use cfds_backtest::lab::{
    LabConfig, LabSeries, ReportInputs, StrategySpec, ValidSpec, catalog, lab_report,
    plans_from_spec, run_setup, validate_spec,
};
use cfds_backtest::research::luck::{self, LuckConfig, LuckSeries};
use cfds_backtest::research::{ResearchConfig, StudyRun, buy_and_hold_returns, run_plans};
use cfds_backtest::{BacktestConfig, Setup, apply_funding_scenario, setup_from_catalog};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Catalog, DataSplit, ExchangeCalendar, Period, Session, aggregate_h1};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};
use serde_json::{Value, json};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn us500() -> InstrumentId {
    "US500".parse().unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn session() -> Session {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    let cal = ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap();
    cal.session_for(&us500()).unwrap().clone()
}

fn setup() -> Setup {
    let mut catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    apply_funding_scenario(&mut catalog, &us500(), "TEST", dec!(4)).unwrap();
    setup_from_catalog(
        &catalog,
        &us500(),
        Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
            trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
        },
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        dec!(100000),
    )
    .unwrap()
}

fn lab_cfg() -> LabConfig {
    LabConfig::from_toml(cfg!("lab.toml")).unwrap()
}

/// Velas de 15m de la sesión de contado de cada día hábil de `[from, to]`, con un camino al azar
/// determinista alrededor de 4800.
fn session_candles(session: &Session, from: NaiveDate, to: NaiveDate) -> Vec<Candle> {
    let (mut out, mut d, mut state, mut price) = (Vec::new(), from, 7_u64, dec!(4800));
    while d <= to {
        if let Some(Some((open, close))) = session.hours(d) {
            let mut s = open;
            while s + TimeDelta::minutes(15) <= close {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let step = Decimal::from((state >> 33) % 13) - dec!(6);
                let (o, c) = (price, price + step);
                out.push(Candle {
                    instrument_id: us500(),
                    timeframe: Timeframe::M15,
                    start: s,
                    open: o,
                    high: o.max(c) + dec!(1),
                    low: o.min(c) - dec!(1),
                    close: c,
                    volume: None,
                });
                price = c;
                s += TimeDelta::minutes(15);
            }
        }
        d = d.succ_opt().unwrap();
    }
    out
}

fn spec(fast: i64) -> ValidSpec {
    let doc = json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": "15m" },
        "direction": "both",
        "entry": [{ "type": "ma_cross", "params": { "average": "ema", "fast": fast, "slow": 12 } }],
        "filters": [],
        "stop": { "type": "atr", "params": { "period": 14, "k": "1.5" } },
        "exit": { "type": "time_target", "params": { "max_candles": 6, "target_r": "1.5" } },
        "risk_pct": "0.5",
        "cost_multiplier": "1",
        "period": { "from": "2024-01-01T00:00:00Z", "to": "2024-05-01T00:00:00Z" },
        "mode": "exploratory"
    });
    let c = catalog(
        &lab_cfg(),
        &RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        [us500()],
    )
    .unwrap();
    let split = DataSplit {
        windows: Vec::new(),
        out_of_sample: Period {
            start: at("2025-11-01T00:00:00Z"),
            end: at("2026-11-01T00:00:00Z"),
        },
    };
    let s: StrategySpec = serde_json::from_value(doc).unwrap();
    validate_spec(&s, &c, &split).unwrap()
}

const MONTHS: [(i32, u32); 4] = [(2024, 1), (2024, 2), (2024, 3), (2024, 4)];

struct Fixture {
    session: Session,
    m15: Vec<Candle>,
    entries: Vec<bool>,
    setup: Setup,
    research: ResearchConfig,
}

fn fixture() -> Fixture {
    let session = session();
    let from = NaiveDate::from_ymd_opt(2024, 1, 2).unwrap();
    let to = NaiveDate::from_ymd_opt(2024, 4, 30).unwrap();
    let m15 = session_candles(&session, from, to);
    let entries = vec![true; m15.len()];
    Fixture {
        session,
        m15,
        entries,
        setup: setup(),
        research: ResearchConfig::from_toml(cfg!("research.toml")).unwrap(),
    }
}

fn run(fx: &Fixture, spec: &ValidSpec) -> (StudyRun, Setup) {
    let setup = run_setup(&fx.setup, spec);
    let h1 = aggregate_h1(&fx.m15).unwrap();
    let series = LabSeries {
        m15: &fx.m15,
        h1: &h1,
        session: &fx.session,
        entries: &fx.entries,
    };
    let plans = plans_from_spec(spec, &series);
    let (from, to) = (spec.spec.period.from, spec.spec.period.to);
    (
        run_plans(&plans, &fx.m15, &[], &setup, from, to).unwrap(),
        setup,
    )
}

fn monthly(fx: &Fixture, spec: &ValidSpec) -> Vec<f64> {
    let (r, s) = run(fx, spec);
    let closed: Vec<_> = r
        .trades
        .iter()
        .map(|t| {
            let ret = cfds_backtest::research::trade_return(t, Decimal::ONE, &s).unwrap();
            (t.exit_time, ret)
        })
        .collect();
    cfds_backtest::research::monthly_returns(&closed, &MONTHS)
        .into_iter()
        .map(|d| d.to_string().parse().unwrap())
        .collect()
}

fn report(
    fx: &Fixture,
    spec: &ValidSpec,
    run: &StudyRun,
    setup: &Setup,
    variants: &[Vec<f64>],
) -> Value {
    let lab = lab_cfg();
    let (from, to) = (spec.spec.period.from, spec.spec.period.to);
    let hold = buy_and_hold_returns(&fx.m15, &fx.session, &[], setup, from, to).unwrap();
    let luck = luck::luck_test(
        &LuckSeries {
            m15: &fx.m15,
            session: &fx.session,
            entries: &fx.entries,
            force_close: &[],
        },
        |j| Some((j + 5).min(fx.m15.len() - 1)),
        &LuckConfig {
            simulations: 50,
            seed: fx.research.montecarlo.seed,
            target_r: spec.exit.target_r,
        },
        &run.trades,
        setup,
        dec!(0.005),
    );
    let windows = [
        Period {
            start: at("2024-01-01T00:00:00Z"),
            end: at("2024-03-01T00:00:00Z"),
        },
        Period {
            start: at("2024-03-01T00:00:00Z"),
            end: at("2024-05-01T00:00:00Z"),
        },
    ];
    lab_report(&ReportInputs {
        spec,
        run,
        setup,
        instrument_id: "US500.FUT",
        months: &MONTHS,
        windows: &windows,
        buy_and_hold: Some(&hold),
        luck: Some(&luck),
        variants,
        project_trials: 361,
        family_variants: 4,
        montecarlo: &fx.research.montecarlo,
        pbo_partitions: 2,
        cost_stress: fx.research.rule.cost_stress,
        edge_cost_factor: fx.research.alpha_budget.edge_cost_factor,
        cfg: &lab.report,
        holdout_start: at("2025-11-01T00:00:00Z"),
        config_sha256: &"c".repeat(64),
        code_version: Some("b12b0f8"),
        last_candle: fx.m15.last().unwrap().start,
        computed_at: at("2026-11-26T23:00:00Z"),
    })
    .unwrap()
}

fn assert_schema(doc: &Value) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/lab_report.v1.schema.json");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    let errors: Vec<String> = v
        .iter_errors(doc)
        .map(|e| format!("{} en {}", e, e.instance_path()))
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn a_full_report_of_a_synthetic_run_matches_the_contract() {
    let fx = fixture();
    let s = spec(5);
    let (r, setup) = run(&fx, &s);
    assert!(r.trades.len() > 30, "operaciones: {}", r.trades.len());
    // El riesgo de la spec (0,5 %) manda sobre el de config (1 %).
    assert_eq!(setup.risk.risk_per_trade_pct, dec!(0.5));
    let variants: Vec<Vec<f64>> = (3..7).map(|fast| monthly(&fx, &spec(fast))).collect();
    let doc = report(&fx, &s, &r, &setup, &variants);
    assert_schema(&doc);
    assert_eq!(doc["votes"], json!(false));
    assert_eq!(doc["label"], json!("exploratorio, no vota"));
    assert_eq!(doc["trades"]["count"], json!(r.trades.len()));
    assert_eq!(doc["sharpe"]["trials"], json!(365));
    assert_eq!(doc["months"].as_array().unwrap().len(), 4);
    assert_eq!(doc["walk_forward"].as_array().unwrap().len(), 2);
    assert_eq!(doc["cost_stress"]["stress"], json!("1.5"));
    assert_eq!(doc["pbo"]["variants"], json!(4));
    assert!(doc["monte_carlo"]["risk_of_ruin"].is_string());
    assert!(doc["luck"]["percentile_mean_r"].is_string());
    assert!(doc["buy_and_hold"]["total_return"].is_string());
    assert!(doc["excursions"]["mae_r"]["p50"].is_string());
    // Los costos × 1,5 nunca mejoran el R neto medio.
    let r_net: f64 = doc["expectancy"]["mean_r_net"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let r_stress: f64 = doc["cost_stress"]["mean_r_net"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(r_stress < r_net);
    // Las operaciones de los meses suman las de la corrida.
    let per_month: u64 = doc["months"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["trades"].as_u64().unwrap())
        .sum();
    assert_eq!(per_month, r.trades.len() as u64);
    assert!(r.trades.iter().all(|t| t.exit_time.year() == 2024));
    // La lista de operaciones (#804): una por operación, en orden de cierre, con el R neto de
    // la esperanza.
    let list = doc["trade_list"].as_array().unwrap();
    assert_eq!(list.len(), r.trades.len());
    let closed: Vec<&str> = list
        .iter()
        .map(|t| t["closed_at"].as_str().unwrap())
        .collect();
    assert!(closed.windows(2).all(|w| w[0] <= w[1]));
    let rs: Vec<f64> = list
        .iter()
        .map(|t| t["r_net"].as_str().unwrap().parse().unwrap())
        .collect();
    let mean = rs.iter().sum::<f64>() / rs.len() as f64;
    assert!((mean - r_net).abs() < 1e-5, "{mean} vs {r_net}");
}

#[test]
fn an_old_report_without_the_trade_list_is_still_valid() {
    let fx = fixture();
    let s = spec(5);
    let (r, setup) = run(&fx, &s);
    let mut doc = report(&fx, &s, &r, &setup, &[]);
    doc.as_object_mut().unwrap().remove("trade_list");
    assert_schema(&doc);
}

#[test]
fn the_same_run_gives_the_same_report_id() {
    let fx = fixture();
    let s = spec(5);
    let (r, setup) = run(&fx, &s);
    let a = report(&fx, &s, &r, &setup, &[]);
    let b = report(&fx, &s, &r, &setup, &[]);
    assert_eq!(a, b);
    assert_eq!(a["pbo"], Value::Null);
    assert!(
        a["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("PBO"))
    );
    let other = report(&fx, &spec(4), &r, &setup, &[]);
    assert_ne!(a["report_id"], other["report_id"]);
}

#[test]
fn a_run_without_trades_reports_nulls() {
    let fx = fixture();
    let s = spec(5);
    let setup = run_setup(&fx.setup, &s);
    let doc = report(&fx, &s, &StudyRun::default(), &setup, &[]);
    assert_schema(&doc);
    assert_eq!(doc["trades"]["count"], json!(0));
    assert_eq!(doc["trade_list"], json!([]));
    for key in ["expectancy", "monte_carlo", "luck", "pbo"] {
        assert_eq!(doc[key], Value::Null, "{key}");
    }
    assert_eq!(doc["sharpe"]["sharpe"], Value::Null);
    assert_eq!(doc["excursions"]["mae_r"], Value::Null);
    assert!(
        doc["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("pocas operaciones"))
    );
}

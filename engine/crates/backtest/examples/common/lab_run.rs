//! La corrida del laboratorio (#730, #775), compartida por el ejemplo `laboratorio` y el
//! `lab_worker`: la config, la separación de datos, la carga de las velas **solo hasta el inicio
//! del fuera de muestra** (como las rondas, nunca IG), una variante y su informe. Solo
//! orquestación: señales, ejecución, estadística e informe viven en `cfds_backtest::lab` y
//! `research`. La misma spec con los mismos datos da el mismo informe.
#![allow(dead_code, reason = "cada ejemplo usa una parte")]

use cfds_backtest::Setup;
use cfds_backtest::lab::{
    LabConfig, LabSeries, ReportInputs, StrategyCatalog, ValidSpec, catalog, lab_report,
    planned_exit_for, plans_from_spec, run_setup,
};
use cfds_backtest::research::luck::{LuckConfig, LuckSeries, luck_test};
use cfds_backtest::research::{
    ResearchConfig, StudyRun, buy_and_hold_returns, monthly_returns, run_plans, trade_return,
};
use cfds_core::{InstrumentId, Timeframe};
use cfds_market_data::{CandleSources, DataSplit, Period, SplitConfig, plan};
use cfds_risk::RiskConfig;
use cfds_storage::{CandleFeed, PgStore};
use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use super::research_round::{self, Loaded};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../../config/", $f))
    };
}

type Error = Box<dyn std::error::Error>;

/// Lo que lee el laboratorio, para el hash de la config del informe.
pub const CONFIG_FILES: [(&str, &str); 7] = [
    ("backtest.toml", cfg!("backtest.toml")),
    ("candle_sources.toml", cfg!("candle_sources.toml")),
    ("data_split.toml", cfg!("data_split.toml")),
    ("instruments.toml", cfg!("instruments.toml")),
    ("lab.toml", cfg!("lab.toml")),
    ("research.toml", cfg!("research.toml")),
    ("risk.toml", cfg!("risk.toml")),
];

/// La config del laboratorio, validada al arrancar, y lo que identifica al código.
pub struct LabEnv {
    pub lab: LabConfig,
    pub risk: RiskConfig,
    pub research: ResearchConfig,
    pub split_cfg: SplitConfig,
    pub sources: CandleSources,
    pub catalog: StrategyCatalog,
    pub config_sha256: String,
    pub code_version: Option<String>,
}

impl LabEnv {
    pub fn load() -> Result<Self, Error> {
        let lab = LabConfig::from_toml(cfg!("lab.toml"))?;
        let risk = RiskConfig::from_toml(cfg!("risk.toml"))?;
        let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
        let catalog = catalog(&lab, &risk, sources.roll.keys().cloned())?;
        Ok(Self {
            catalog,
            lab,
            risk,
            research: ResearchConfig::from_toml(cfg!("research.toml"))?,
            split_cfg: SplitConfig::from_toml(cfg!("data_split.toml"))?,
            sources,
            config_sha256: cfds_journal::config_sha256(&CONFIG_FILES),
            code_version: code_version(),
        })
    }

    /// N del proyecto: el último `dsr_trials` de `config/research.toml` (nunca baja, así que es
    /// el máximo; lo verifica `tools/research/check_dsr_trials.py`).
    pub fn project_trials(&self) -> u32 {
        let r = &self.research;
        [
            &r.rule,
            &r.rule_round2,
            &r.rule_round3,
            &r.rule_nasdaq,
            &r.rule_program4,
            &r.rule_program5,
        ]
        .iter()
        .map(|x| x.dsr_trials)
        .max()
        .unwrap_or(0)
    }

    /// La separación de datos vigente para `id`, desde `from`: sale solo de la hora de la última
    /// vela de futuros guardada (nada del fuera de muestra se lee).
    pub async fn split_for(
        &self,
        store: &PgStore,
        id: &InstrumentId,
        from: DateTime<Utc>,
    ) -> Result<DataSplit, Error> {
        let feed = CandleFeed::Futures(&self.sources.futures_source);
        let last = store
            .last_candle_start_from(feed, id, Timeframe::M15, Utc::now())
            .await?
            .ok_or("sin velas de futuros")?;
        Ok(plan(from, last, &self.split_cfg)?)
    }
}

fn code_version() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    let v = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (out.status.success() && v.len() >= 7).then_some(v)
}

/// Meses UTC que empiezan en `[from, to)`.
fn months(from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<(i32, u32)> {
    let mut out = Vec::new();
    let mut d = NaiveDate::from_ymd_opt(from.year(), from.month(), 1);
    while let Some(m) = d.filter(|m| m.and_hms_opt(0, 0, 0).is_some_and(|t| t.and_utc() < to)) {
        out.push((m.year(), m.month()));
        d = m.checked_add_months(Months::new(1));
    }
    out
}

/// Las velas de una corrida, cargadas, con los meses y las ventanas del período de las specs.
pub struct LabData {
    /// El CFD de las specs (las velas se guardan bajo el derivado, `loaded.p.out`).
    pub id: InstrumentId,
    pub loaded: Loaded,
    pub split: DataSplit,
    pub period: Period,
    pub months: Vec<(i32, u32)>,
    pub windows: Vec<Period>,
}

/// Carga las velas del instrumento de las specs (variantes de una familia y un período, ya
/// validadas) desde `from` y solo hasta el inicio del fuera de muestra. `study_args` son los
/// flags de estudio (`--escenario-financiacion`, `--supuestos`).
pub async fn load(
    env: &LabEnv,
    specs: &[ValidSpec],
    study_args: &[String],
    from: DateTime<Utc>,
    equity: Decimal,
    split: DataSplit,
    store: &PgStore,
) -> Result<LabData, Error> {
    let first = specs.first().ok_or("sin specs")?;
    let id = &first.spec.market.instrument_id;
    let loaded = research_round::load(
        id,
        study_args,
        from,
        equity,
        &env.sources,
        store,
        &env.split_cfg,
    )
    .await?;
    let p = first.spec.period;
    let period = Period {
        start: p.from,
        end: p.to,
    };
    let windows = split
        .windows
        .iter()
        .map(|w| Period {
            start: w.validation.start.max(period.start),
            end: w.validation.end.min(period.end),
        })
        .filter(|w| w.start < w.end)
        .collect();
    Ok(LabData {
        id: id.clone(),
        loaded,
        months: months(period.start, period.end),
        split,
        period,
        windows,
    })
}

fn series(data: &LabData) -> Result<LabSeries<'_>, String> {
    let p = &data.loaded.p;
    let session = p
        .calendar
        .session_for(&data.id)
        .ok_or("instrumento sin bolsa")?;
    Ok(LabSeries {
        m15: &p.m15,
        h1: &p.h1,
        session,
        entries: &p.entries,
    })
}

/// Una variante corrida: su setup (riesgo de la spec, nunca más que config), el recorrido y sus
/// retornos mensuales (para el PBO).
pub struct VariantRun {
    pub setup: Setup,
    pub run: StudyRun,
    pub monthly: Vec<f64>,
}

/// Corre una variante: `plans_from_spec` → `run_setup` → `run_plans`, con los costos de la spec.
pub fn run_variant(data: &LabData, v: &ValidSpec) -> Result<VariantRun, String> {
    let p = &data.loaded.p;
    let setup = run_setup(&p.setup, v);
    let plans = plans_from_spec(v, &series(data)?);
    let (from, to) = (data.period.start, data.period.end);
    let run = run_plans(&plans, &p.m15, &p.force_close, &setup, from, to)
        .map_err(|e| format!("{e:?}"))?;
    let m = v.spec.cost_multiplier;
    let closed = run
        .trades
        .iter()
        .map(|t| Ok((t.exit_time, trade_return(t, m, &setup)?)))
        .collect::<Result<Vec<_>, cfds_backtest::RunError>>()
        .map_err(|e| format!("{e:?}"))?;
    let monthly = monthly_returns(&closed, &data.months)
        .iter()
        .map(|d| research_round::f(*d))
        .collect();
    Ok(VariantRun {
        setup,
        run,
        monthly,
    })
}

/// El informe (`lab_report.v1`) de una variante: test de suerte con la misma regla de salida,
/// comprar y mantener y el N del Sharpe deflactado = N del proyecto + `family_variants`.
pub fn report(
    env: &LabEnv,
    data: &LabData,
    v: &ValidSpec,
    vr: &VariantRun,
    variants: &[Vec<f64>],
    family_variants: u32,
    now: DateTime<Utc>,
) -> Result<Value, String> {
    let p = &data.loaded.p;
    let s = series(data)?;
    let (from, to) = (data.period.start, data.period.end);
    let rolls = p
        .setup
        .futures_basis
        .as_ref()
        .map(|b| b.rolls.clone())
        .unwrap_or_default();
    let hold = buy_and_hold_returns(&p.m15, s.session, &rolls, &vr.setup, from, to)
        .map_err(|e| println!("⚠️  comprar y mantener: {e:?}"))
        .ok();
    let luck = luck_test(
        &LuckSeries {
            m15: &p.m15,
            session: s.session,
            entries: &p.entries,
            force_close: &p.force_close,
        },
        |j| j.checked_sub(1).and_then(|d| planned_exit_for(v, &s, d)),
        &LuckConfig {
            simulations: env.lab.report.luck_simulations,
            seed: env.research.montecarlo.seed,
            target_r: v.exit.target_r,
        },
        &vr.run.trades,
        &vr.setup,
        v.spec.risk_pct / Decimal::ONE_HUNDRED,
    );
    let r = &env.research;
    lab_report(&ReportInputs {
        spec: v,
        run: &vr.run,
        setup: &vr.setup,
        instrument_id: p.out.as_str(),
        months: &data.months,
        windows: &data.windows,
        buy_and_hold: hold.as_deref(),
        luck: Some(&luck),
        variants,
        project_trials: env.project_trials(),
        family_variants,
        montecarlo: &r.montecarlo,
        pbo_partitions: r.pbo_partitions,
        cost_stress: r.rule.cost_stress,
        edge_cost_factor: r.alpha_budget.edge_cost_factor,
        cfg: &env.lab.report,
        holdout_start: data.split.out_of_sample.start,
        config_sha256: &env.config_sha256,
        code_version: env.code_version.as_deref(),
        last_candle: p.m15.last().ok_or("sin velas")?.start,
        computed_at: now,
    })
    .map_err(|e| format!("{e:?}"))
}

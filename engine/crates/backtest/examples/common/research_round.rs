//! Lo común de las rondas de investigación (`ronda1`, `ronda2`): cargar cada índice sin el
//! fuera de muestra, correr los planes de una familia en las ventanas de validación, describir
//! los resultados y aplicar la regla pre-registrada de #466 §4.3 (criterios 1 a 4). Solo
//! presentación y orquestación: las señales, la ejecución y la estadística viven en
//! `cfds_backtest::research`.
#![allow(
    clippy::float_arithmetic,
    reason = "solo estadísticos y porcentajes para mostrar; el dinero se calcula en Decimal"
)]
#![allow(dead_code, reason = "cada ronda usa una parte")]

use std::collections::BTreeMap;

use cfds_backtest::research::{
    Plan, RuleConfig, StudyExit, StudyTrade, monthly_returns, run_plans, stats, trade_return,
};
use cfds_core::{InstrumentId, Timeframe};
use cfds_market_data::{CandleSource, CandleSources, DataSplit, SplitConfig, plan};
use cfds_storage::{CandleFeed, PgStore};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::prepare::{self, Prepared};
use super::study;

/// Lo de una familia en un índice.
pub struct FamilyResult {
    pub trades: Vec<StudyTrade>,
    pub rejected: BTreeMap<String, usize>,
    /// Planes con objetivo no operados porque la entrada abrió del lado del stop.
    pub through_stop: usize,
    /// Planes con tope del objetivo no operados porque la entrada abrió en el tope (H4.3).
    pub through_cap: usize,
    pub monthly: Vec<f64>,
    pub monthly_stress: Vec<f64>,
    pub min_deal: Decimal,
}

/// Un índice cargado: la preparación, la separación y los meses de validación.
pub struct Loaded {
    pub p: Prepared,
    pub split: DataSplit,
    pub months: Vec<(i32, u32)>,
}

pub fn f(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(f64::NAN)
}

pub fn mean(xs: &[f64]) -> f64 {
    stats::mean(xs).unwrap_or(f64::NAN)
}

/// Carga `id` desde `from` **solo hasta el inicio del fuera de muestra** (para saber dónde
/// empieza se lee solo la hora de la última vela guardada) y verifica que no entró ninguna vela
/// del fuera de muestra.
pub async fn load(
    id: &InstrumentId,
    args: &[String],
    from: DateTime<Utc>,
    equity: Decimal,
    sources: &CandleSources,
    store: &PgStore,
    split_cfg: &SplitConfig,
) -> Result<Loaded, Box<dyn std::error::Error>> {
    let (study, _) = study::parse(args.to_vec())?;
    if study.as_ref().is_none_or(|s| s.funding.is_none()) {
        return Err("falta --escenario-financiacion (el tipo de EE. UU. está pendiente)".into());
    }
    let last = store
        .last_candle_start_from(
            CandleFeed::Futures(&sources.futures_source),
            id,
            Timeframe::M15,
            Utc::now(),
        )
        .await?
        .ok_or("sin velas de futuros")?;
    let oos_start = plan(from, last, split_cfg)?.out_of_sample.start;
    println!(
        "══ {id}: última vela guardada {last}; fuera de muestra desde {oos_start} (NO se carga)"
    );
    let p = prepare::prepare_with(
        id,
        CandleSource::Futures,
        (from, oos_start),
        equity,
        None,
        true,
        study,
    )
    .await?;
    let (first, last_loaded) = match (p.m15.first(), p.m15.last()) {
        (Some(a), Some(b)) => (a.start, b.start),
        _ => return Err("sin velas".into()),
    };
    if last_loaded >= oos_start {
        return Err("se cargó una vela del fuera de muestra".into());
    }
    let split = plan(first, last, split_cfg)?;
    if split.out_of_sample.start != oos_start {
        return Err("el fuera de muestra no coincide".into());
    }
    let mut months = Vec::new();
    let mut d = split.windows[0].validation.start.date_naive();
    let end = split.out_of_sample.start.date_naive();
    while d < end {
        months.push((d.year(), d.month()));
        d = d
            .checked_add_months(chrono::Months::new(1))
            .ok_or("fecha")?;
    }
    println!(
        "{id}: {} ventanas de validación, {} → {} ({} meses)",
        split.windows.len(),
        split.windows[0].validation.start.date_naive(),
        end,
        months.len()
    );
    Ok(Loaded { p, split, months })
}

/// Ejecuta los planes de una familia en cada ventana de validación (cuenta nueva por ventana)
/// y arma sus retornos mensuales, con y sin el estrés de costos.
pub fn run_family(
    plans: &[Plan],
    l: &Loaded,
    stress: Decimal,
) -> Result<FamilyResult, Box<dyn std::error::Error>> {
    let p = &l.p;
    let mut trades = Vec::new();
    let mut rejected: BTreeMap<String, usize> = BTreeMap::new();
    let mut through_stop = 0;
    let mut through_cap = 0;
    for w in &l.split.windows {
        let run = run_plans(
            plans,
            &p.m15,
            &p.force_close,
            &p.setup,
            w.validation.start,
            w.validation.end,
        )
        .map_err(|e| format!("{e:?}"))?;
        for r in run.rejected {
            *rejected.entry(format!("{:?}", r.reason)).or_default() += 1;
        }
        through_stop += run.through_stop.len();
        through_cap += run.through_cap.len();
        trades.extend(run.trades);
    }
    let returns = |s: Decimal| -> Result<Vec<f64>, String> {
        let closed = trades
            .iter()
            .map(|t| Ok((t.exit_time, trade_return(t, s, &p.setup)?)))
            .collect::<Result<Vec<_>, cfds_backtest::RunError>>()
            .map_err(|e| format!("{e:?}"))?;
        Ok(monthly_returns(&closed, &l.months)
            .into_iter()
            .map(f)
            .collect())
    };
    let monthly = returns(Decimal::ONE)?;
    let monthly_stress = returns(stress)?;
    Ok(FamilyResult {
        trades,
        rejected,
        through_stop,
        through_cap,
        monthly,
        monthly_stress,
        min_deal: p
            .setup
            .limits
            .min_deal_size
            .ok_or("tamaño mínimo pendiente")?,
    })
}

/// Promedio simple por mes de los retornos de los índices.
pub fn portfolio(per: &[(InstrumentId, FamilyResult)], n: usize, stressed: bool) -> Vec<f64> {
    (0..n)
        .map(|k| {
            mean(
                &per.iter()
                    .map(|(_, r)| {
                        if stressed {
                            r.monthly_stress[k]
                        } else {
                            r.monthly[k]
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

pub fn describe(id: &InstrumentId, r: &FamilyResult, equity: Decimal) {
    let t = &r.trades;
    let r_of = |x: Decimal, risk: Decimal| f(x / risk);
    let mid_r: Vec<f64> = t.iter().map(|x| r_of(x.mid_pnl, x.risk)).collect();
    let net_r: Vec<f64> = t.iter().map(|x| r_of(x.net_pnl, x.risk)).collect();
    let cost_r: Vec<f64> = t.iter().map(|x| r_of(x.total_costs(), x.risk)).collect();
    let longs = t
        .iter()
        .filter(|x| x.side == cfds_strategy::Side::Long)
        .count();
    let mut exits: BTreeMap<String, usize> = BTreeMap::new();
    for x in t {
        let k = match x.exit {
            StudyExit::Stop | StudyExit::StopGap => "stop",
            StudyExit::ForceClose => "cierre forzado",
            StudyExit::Planned => "salida de la definición",
            StudyExit::Target => "objetivo",
        };
        *exits.entry(k.to_owned()).or_default() += 1;
    }
    // Con 10 000 €: el tamaño escala con el capital al decidir.
    let small = Decimal::from(10_000);
    let too_small = t
        .iter()
        .filter(|x| x.size * small / x.equity_at_entry < r.min_deal)
        .count();
    let wins = net_r.iter().filter(|x| **x > 0.0).count();
    let mut risk_points: Vec<Decimal> = t.iter().map(|x| (x.reference - x.stop).abs()).collect();
    risk_points.sort();
    println!(
        "  {id}: {} operaciones ({} largos, {} cortos); salidas {:?}; rechazos del riesgo {:?}",
        t.len(),
        longs,
        t.len() - longs,
        exits,
        r.rejected
    );
    println!(
        "    R medio a precio medio {:+.4}, costo medio {:.4} R, R neto medio {:+.4} \
         (aciertos {:.1} %); R mediano {} puntos; con {small} € no se habrían abierto {} \
         ({:.1} %); capital {equity}",
        mean(&mid_r),
        mean(&cost_r),
        mean(&net_r),
        100.0 * wins as f64 / t.len().max(1) as f64,
        risk_points
            .get(risk_points.len() / 2)
            .map_or("-".to_owned(), |x| x.round_dp(1).to_string()),
        too_small,
        100.0 * too_small as f64 / t.len().max(1) as f64
    );
    println!(
        "    retorno mensual medio {:+.4} % (costos × 1,5: {:+.4} %)",
        100.0 * mean(&r.monthly),
        100.0 * mean(&r.monthly_stress)
    );
}

/// Criterios 1 a 4 de #466 §4.3 sobre la cartera, más los criterios `extra` (nombre, pasa) de
/// la familia. Imprime cada uno y, si no es descriptivo, el veredicto. Devuelve si pasa todos.
pub fn verdict(
    rule: &RuleConfig,
    months: &[(i32, u32)],
    portfolio: &[f64],
    stress: &[f64],
    per: &[(InstrumentId, FamilyResult)],
    extra: &[(String, bool)],
    descriptive: bool,
) -> bool {
    let alpha = f(rule.alpha);
    let t = stats::t_test_greater(portfolio);
    let dsr = stats::deflated_sharpe(portfolio, rule.dsr_trials);
    let half = NaiveDate::from_ymd_opt(
        rule.second_half_from.year(),
        rule.second_half_from.month(),
        1,
    )
    .map(|d| (d.year(), d.month()));
    let (first, second): (Vec<f64>, Vec<f64>) = {
        let mut a = Vec::new();
        let mut b = Vec::new();
        for (k, m) in months.iter().enumerate() {
            if Some(*m) < half {
                a.push(portfolio[k])
            } else {
                b.push(portfolio[k])
            }
        }
        (a, b)
    };
    let positive_indices = per.iter().filter(|(_, r)| mean(&r.monthly) > 0.0).count();
    println!(
        "  cartera ({} índices, {} meses): retorno mensual medio {:+.4} %, desvío {:.4} %",
        per.len(),
        portfolio.len(),
        100.0 * mean(portfolio),
        100.0 * stats::std_dev(portfolio).unwrap_or(f64::NAN)
    );
    let c1 = t.is_some_and(|t| t.p_value < alpha);
    if let Some(t) = t {
        println!(
            "  1. prueba t unilateral: t = {:.3}, p = {:.5} (nivel {alpha}) → {}",
            t.t,
            t.p_value,
            if c1 { "PASA" } else { "no pasa" }
        );
    }
    let c2 = dsr.is_some_and(|d| d.dsr >= f(rule.dsr_min));
    if let Some(d) = dsr {
        println!(
            "  2. Sharpe deflactado (N = {}): Sharpe mensual {:.4} (anual ≈ {:.2}), SR0 {:.4}, \
             asimetría {:.3}, curtosis {:.3}, DSR {:.4} (mínimo {}) → {}",
            rule.dsr_trials,
            d.sharpe,
            d.sharpe * 12f64.sqrt(),
            d.sharpe0,
            d.skew,
            d.kurtosis,
            d.dsr,
            rule.dsr_min,
            if c2 { "PASA" } else { "no pasa" }
        );
        for n in [100, 1000] {
            if let Some(x) = stats::deflated_sharpe(portfolio, n) {
                println!(
                    "     (referencia, N = {n}: SR0 {:.4}, DSR {:.4})",
                    x.sharpe0, x.dsr
                );
            }
        }
    }
    let c3 = mean(&first) > 0.0 && mean(&second) > 0.0 && positive_indices * 3 >= per.len() * 2;
    println!(
        "  3. consistencia: primera mitad {:+.4} %, segunda {:+.4} %, índices con media > 0: {} de {} → {}",
        100.0 * mean(&first),
        100.0 * mean(&second),
        positive_indices,
        per.len(),
        if c3 { "PASA" } else { "no pasa" }
    );
    let c4 = mean(stress) > 0.0;
    println!(
        "  4. costos × {}: retorno mensual medio {:+.4} % → {}",
        rule.cost_stress,
        100.0 * mean(stress),
        if c4 { "PASA" } else { "no pasa" }
    );
    for (k, (name, ok)) in extra.iter().enumerate() {
        println!(
            "  {}. {name} → {}",
            5 + k,
            if *ok { "PASA" } else { "no pasa" }
        );
    }
    let all = c1 && c2 && c3 && c4 && extra.iter().all(|(_, ok)| *ok);
    if descriptive {
        println!("  (descriptivo: sin veredicto)");
    } else {
        println!(
            "  VEREDICTO con este escenario: {}",
            if all {
                if extra.is_empty() {
                    "PASA los cuatro criterios".to_owned()
                } else {
                    format!("PASA los {} criterios", 4 + extra.len())
                }
            } else {
                "NO PASA".to_owned()
            }
        );
    }
    all
}

/// Retornos mensuales por familia e índice, con y sin estrés.
pub fn returns_csv(
    families: &[&str],
    results: &[Vec<(InstrumentId, FamilyResult)>],
    months: &[(i32, u32)],
) -> String {
    use std::fmt::Write as _;
    let mut csv = String::from("familia,instrumento,mes,retorno,retorno_costos_estresados\n");
    for (fi, per) in results.iter().enumerate() {
        for (id, r) in per {
            for (k, (y, m)) in months.iter().enumerate() {
                let _ = writeln!(
                    csv,
                    "{},{id},{y}-{m:02},{:.8},{:.8}",
                    families[fi], r.monthly[k], r.monthly_stress[k]
                );
            }
        }
    }
    csv
}

/// Registro de operaciones por familia e índice.
pub fn trades_csv(families: &[&str], results: &[Vec<(InstrumentId, FamilyResult)>]) -> String {
    use std::fmt::Write as _;
    let mut t = String::from(
        "familia,instrumento,evento,lado,decision,entrada,referencia,stop,entrada_medio,\
         salida,salida_medio,motivo,tamano,riesgo,pl_medio,costos,financiacion,pl_neto,capital\n",
    );
    for (fi, per) in results.iter().enumerate() {
        for (id, r) in per {
            for x in &r.trades {
                let _ = writeln!(
                    t,
                    "{},{id},{},{:?},{},{},{},{},{},{},{},{:?},{},{},{},{},{},{},{}",
                    families[fi],
                    x.event,
                    x.side,
                    x.signal_at,
                    x.entry_time,
                    x.reference,
                    x.stop,
                    x.entry_mid,
                    x.exit_time,
                    x.exit_mid,
                    x.exit,
                    x.size.round_dp(4),
                    x.risk.round_dp(2),
                    x.mid_pnl.round_dp(2),
                    x.total_costs().round_dp(2),
                    x.funding.round_dp(2),
                    x.net_pnl.round_dp(2),
                    x.equity_at_entry.round_dp(2)
                );
            }
        }
    }
    t
}

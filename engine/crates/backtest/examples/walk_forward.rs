//! Evaluación de la Fase 5 (ROADMAP, #298) sobre velas reales guardadas en PostgreSQL.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-backtest --example walk_forward -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> <capital inicial> <tamaño comprar y mantener> \
//!     [<report_id>] [--fuente ig|futuros] [--sin-noticias] [--sin-sensibilidad] \
//!     [--escenario-financiacion ESTR | --escenario-financiacion <ETIQUETA>=<archivo.csv>] \
//!     [--supuestos <archivo.toml>]
//! ```
//! Prepara los datos igual que `backtest_run` (mismo módulo `common/prepare.rs`) y:
//! - separa el período con `config/data_split.toml` (ventanas walk-forward y fuera de muestra);
//! - recorre el entrenamiento y la prueba de cada ventana con los parámetros de config, cada tramo
//!   desde el capital inicial y con comprar y mantener (costos y financiación incluidos);
//! - mide la sensibilidad de los parámetros de `config/evaluation.toml` SOLO con el entrenamiento
//!   de cada ventana, sin elegir ningún valor (`--sin-sensibilidad` la saltea);
//! - evalúa el fuera de muestra una sola vez, con los parámetros de config, y lo muestra aparte.
//!
//! `--escenario-financiacion` (estudio de sensibilidad, #405): reemplaza EN MEMORIA el tipo de
//! referencia de la financiación por uno rotulado, sin cambiar `config/`. `ESTR` usa la serie
//! diaria guardada en `reference_rates`; `SOFR=<archivo.csv>` lee `fecha,tasa` de la fuente
//! oficial (NY Fed), y cada valor cuenta desde las 13:00 UTC de dos días hábiles después (la NY
//! Fed lo publica hacia las 08:00 ET del día hábil siguiente: nunca antes de estar publicado). En
//! ese modo no se guarda ningún reporte y **no se evalúa el fuera de muestra**. Cualquier
//! `<ETIQUETA>=<archivo.csv>` funciona igual (#421, por ejemplo series empalmadas EONIA→€STR).
//! `--supuestos` (#421) completa en memoria costos `pending` del catálogo con supuestos rotulados
//! (ver `common/study.rs`); mismo modo de estudio.
//!
//! Con `report_id`, guarda el reporte en `walk_forward_reports` (`walk_forward_report.v1`), con
//! futuros bajo el instrumento derivado. No cambia ningún parámetro de la estrategia.

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;
#[path = "common/study.rs"]
mod study;

use cfds_backtest::{
    ConfigTexts, EvaluationConfig, PeriodResult, aggregate, evaluate_holdout, sensitivity,
    stability, walk_forward,
};
use cfds_journal::{CandleSourceName, ReportHeader, WalkForwardReport};
use cfds_market_data::{CandleSource, SplitConfig, plan};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// Archivos de `config/` que lee la evaluación, para `config_sha256`.
const CONFIG_FILES: [(&str, &str); 17] = [
    ("backtest.toml", cfg!("backtest.toml")),
    ("candle_sources.toml", cfg!("candle_sources.toml")),
    ("data_split.toml", cfg!("data_split.toml")),
    ("data_validation.toml", cfg!("data_validation.toml")),
    ("evaluation.toml", cfg!("evaluation.toml")),
    ("indicators.toml", cfg!("indicators.toml")),
    ("instruments.toml", cfg!("instruments.toml")),
    ("liquidity.toml", cfg!("liquidity.toml")),
    ("news_filter.toml", cfg!("news_filter.toml")),
    ("news_interpreter.toml", cfg!("news_interpreter.toml")),
    ("patterns.toml", cfg!("patterns.toml")),
    ("probability.toml", cfg!("probability.toml")),
    ("risk.toml", cfg!("risk.toml")),
    ("sessions.toml", cfg!("sessions.toml")),
    ("spread_filter.toml", cfg!("spread_filter.toml")),
    ("strategy.toml", cfg!("strategy.toml")),
    ("trading_window.toml", cfg!("trading_window.toml")),
];

fn pct(d: Option<Decimal>) -> String {
    d.map_or("—".into(), |d| {
        format!("{:+.2} %", d * Decimal::ONE_HUNDRED)
    })
}

fn num(d: Option<Decimal>) -> String {
    d.map_or("—".into(), |d| format!("{:.2}", d))
}

fn line(name: &str, p: &PeriodResult) -> String {
    let (from, to) = (p.period.start.date_naive(), p.period.end.date_naive());
    match &p.metrics {
        None => format!("  {name} {from} → {to}: sin velas"),
        Some(m) => format!(
            "  {name} {from} → {to}: {} velas, {} operaciones, retorno {}, drawdown {}, Sharpe {}, \
             PF {} | comprar y mantener {}",
            p.candles,
            m.trades,
            pct(Some(m.total_return)),
            pct(Some(m.max_drawdown)),
            num(m.sharpe),
            num(m.profit_factor),
            pct(p.buy_and_hold_return)
        ),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let without_news = all.iter().any(|a| a == "--sin-noticias");
    let without_sensitivity = all.iter().any(|a| a == "--sin-sensibilidad");
    let (study, all) = study::parse(all)?;
    let (source, all) = match all.iter().position(|a| a == "--fuente") {
        None => (CandleSource::Ig, all),
        Some(i) => {
            let s = all
                .get(i + 1)
                .and_then(|s| CandleSource::parse(s))
                .ok_or("--fuente ig|futuros")?;
            (s, [&all[..i], all.get(i + 2..).unwrap_or(&[])].concat())
        }
    };
    let args: Vec<&str> = all
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with("--"))
        .collect();
    let (instrument, from, to, equity, size, report_id) = match args.as_slice() {
        [i, f, t, e, s] => (i, f, t, e, s, None),
        [i, f, t, e, s, r] => (i, f, t, e, s, Some(*r)),
        _ => {
            return Err(
                "uso: <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <tamaño B&H> \
                        [<report_id>] [--fuente ig|futuros] [--sin-noticias] [--sin-sensibilidad]"
                    .into(),
            );
        }
    };
    let id: cfds_core::InstrumentId = instrument.parse()?;
    let (from, to) = (from.parse()?, to.parse()?);
    let (equity, size): (Decimal, Decimal) = (equity.parse()?, size.parse()?);
    let studying = study.is_some();
    if studying && report_id.is_some() {
        return Err(
            "con --escenario-financiacion o --supuestos no se guarda ningún reporte (no es oficial)".into(),
        );
    }
    let p =
        prepare::prepare_with(&id, source, (from, to), equity, None, without_news, study).await?;
    println!("velas: {} ({})", p.sources.label(source), p.out);

    let (first, last) = match (p.m15.first(), p.m15.last()) {
        (Some(a), Some(b)) => (a.start, b.start),
        _ => return Err("sin velas en el período".into()),
    };
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let split = plan(first, last, &split_cfg).map_err(|e| {
        format!(
            "{e}. Hay velas del {} al {}: con config/data_split.toml hacen falta {} meses de \
             ventana más {} de fuera de muestra. No se achica nada acá: es config del dueño.",
            first.date_naive(),
            last.date_naive(),
            split_cfg.train_months + split_cfg.validation_months,
            split_cfg.out_of_sample_months
        )
    })?;
    let cfg = EvaluationConfig::from_toml(cfg!("evaluation.toml")).map_err(|e| format!("{e:?}"))?;
    let inputs = cfds_backtest::Inputs {
        m15: &p.m15,
        h1: &p.h1,
        entries: &p.entries,
        force_close: &p.force_close,
        data_errors: &p.data_errors,
        news: &p.news,
        published_from: p.published_from,
    };

    let folds =
        walk_forward(&inputs, &split, &p.setup, &cfg, size).map_err(|e| format!("{e:?}"))?;
    println!(
        "walk-forward: {} ventanas ({} meses de entrenamiento, {} de prueba, paso {}), parámetros \
         de config",
        folds.len(),
        split_cfg.train_months,
        split_cfg.validation_months,
        split_cfg.step_months
    );
    for f in &folds {
        println!("ventana {}", f.index);
        println!("{}", line("entrenamiento", &f.train));
        println!("{}", line("prueba       ", &f.test));
    }
    let agg = aggregate(folds.iter().map(|f| &f.test));
    println!(
        "pruebas: {} de {} con datos, {} operaciones, retorno medio {} (mediana {}), {} positivas, \
         peor drawdown {}, Sharpe medio {} ({} definidos), superan a comprar y mantener {} de {}",
        agg.with_data,
        agg.periods,
        agg.trades,
        pct(agg.mean_return),
        pct(agg.median_return),
        agg.positive,
        pct(agg.worst_drawdown),
        num(agg.mean_sharpe),
        agg.sharpe_defined,
        agg.beat_buy_and_hold,
        agg.with_buy_and_hold
    );

    let texts = ConfigTexts {
        strategy: cfg!("strategy.toml"),
        backtest: cfg!("backtest.toml"),
        risk: cfg!("risk.toml"),
    };
    let points = if without_sensitivity {
        println!("sensibilidad: salteada (--sin-sensibilidad)");
        Vec::new()
    } else {
        sensitivity(&inputs, &split, &p.setup, &texts, &cfg).map_err(|e| format!("{e:?}"))?
    };
    let stab = if points.is_empty() {
        Vec::new()
    } else {
        stability(&points, &cfg)
    };
    if !stab.is_empty() {
        println!("sensibilidad (solo entrenamiento; NO elige ningún valor):");
    }
    for s in &stab {
        let values: Vec<String> = s
            .by_value
            .iter()
            .map(|(v, r, sh, n)| format!("{v}: {} / Sharpe {} / {n} op.", pct(*r), num(*sh)))
            .collect();
        println!(
            "  {} {}: {} | diferencia {} | {}",
            s.file.name(),
            s.key,
            values.join(" · "),
            pct(s.return_spread),
            if s.same_sign {
                "mismo signo"
            } else {
                "CAMBIA DE SIGNO"
            }
        );
    }

    if studying {
        println!(
            "fuera de muestra: NO se evalúa en un estudio (#405, #421); queda intacto \
             para la Fase 5"
        );
        return Ok(());
    }
    let holdout =
        evaluate_holdout(&inputs, &split, &p.setup, &cfg, size).map_err(|e| format!("{e:?}"))?;
    println!("══ FUERA DE MUESTRA (aparte, sin retocar, parámetros de config) ══");
    println!("{}", line("fuera de muestra", &holdout));

    if let Some(report_id) = report_id {
        if source == CandleSource::Futures {
            let spec = p
                .catalog
                .instruments
                .get(&id)
                .ok_or("instrumento sin catálogo")?;
            p.store
                .upsert_instrument(&p.sources.derived_instrument(source, &spec.instrument)?)
                .await?;
        }
        let report = WalkForwardReport::new(
            ReportHeader {
                report_id: report_id.to_owned(),
                instrument_id: p.out.clone(),
                candle_source: match source {
                    CandleSource::Ig => CandleSourceName::Ig,
                    CandleSource::Futures => CandleSourceName::Futures,
                    // `prepare` ya rechaza las barras de IBKR (#630).
                    CandleSource::Ibkr => {
                        return Err("el backtest no usa las barras de IBKR".into());
                    }
                },
                config_sha256: cfds_journal::config_sha256(&CONFIG_FILES),
                split: split_cfg,
                warmup_days: u32::try_from(cfg.warmup.num_days())?,
                initial_equity: equity,
                buy_and_hold_size: size,
                computed_at: chrono::Utc::now(),
            },
            &folds,
            &agg,
            &points,
            &stab,
            &holdout,
        );
        let journal = cfds_journal::pg::PgJournal::new(p.store.pool().clone());
        let recorded = journal.record_walk_forward(&report).await?;
        println!("reporte {report_id} en walk_forward_reports ({recorded:?})");
    }
    Ok(())
}

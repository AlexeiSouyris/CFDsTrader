//! Datos del modelo de ML v1 **sin P/L** (#399, opción C del dueño): cada propuesta de la
//! estrategia sobre los futuros, con su etiqueta (`ml_label.v1`) y sus variables
//! (`ml_features.v1`), bajo un `run_id` (`ml_label_run.v1`).
//!
//! ```text
//! # Solo lectura: cuenta y muestra las filas por ventana walk-forward, sin escribir nada.
//! DATABASE_URL=... cargo run --release -p cfds-backtest --example ml_dataset -- \
//!     <run_id> [INSTRUMENTO] [desde UTC] [hasta UTC]
//! # Guarda (idempotente): hace falta `hasta` explícito, para que el período quede fijo.
//! DATABASE_URL=... cargo run --release -p cfds-backtest --example ml_dataset -- \
//!     <run_id> US500 2011-01-01T00:00:00Z 2026-11-23T00:00:00Z --escribir
//! ```
//!
//! - Las propuestas son las de `cfds_backtest::proposals` en el modo del backtest (una posición
//!   a la vez), con la serie de `common/futures_series.rs`: las mismas que cuenta
//!   `propuestas_por_mes` (#390). Se guardan bajo el instrumento derivado (`US500.FUT`) con
//!   `source = backtest`.
//! - La etiqueta: `y = 1` si toca TP1 antes que el stop; stop, tiempo y cierre forzado = 0. A
//!   precio medio y sin costos (`method = mid_no_costs`). Sin resolver al final de los datos: no
//!   hay etiqueta.
//! - Las variables: las mismas de `backtest_run` (`cfds_journal::ml_features`). Los tercios de la
//!   volatilidad son los guardados (`daily_vol_terciles` del derivado, de `futures_daily` y
//!   `daily_indicators -- --fuente futuros`). Si falta alguna variable, no hay fila (spec §3).
//! - Idempotente: guardar otra vez lo mismo no cambia nada. Un `run_id` ya guardado con otro
//!   período o con otra config es un error: no se reusa.
//! - No guarda nada en `backtest_runs` ni en `outcomes`: no hay P/L.

#[path = "common/futures_series.rs"]
mod futures_series;
#[path = "common/news_filter.rs"]
mod news_filter;

use cfds_backtest::{Overlap, Proposal, Tally, proposals};
use cfds_core::{DailyVolTercile, InstrumentId};
use cfds_journal::ml_features::atr14_m15;
use cfds_journal::ml_label::{LabelCandles, LabelMethod, MlLabelRun, features, label, proposal};
use cfds_journal::pg::{PgJournal, Recorded};
use cfds_market_data::{SplitConfig, plan};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};
use futures_series::{CONFIG_FILES, FuturesSeries};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// Lo que se guarda (o se guardaría) de cada propuesta.
struct Row {
    proposal: Proposal,
    labeled: bool,
    with_features: bool,
}

async fn terciles(
    store: &PgStore,
    out: &InstrumentId,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
) -> Result<Vec<DailyVolTercile>, Box<dyn std::error::Error>> {
    let daily_cfg = cfds_indicators::DailyConfig::from_toml(cfg!("indicators.toml"))?;
    // El tercio vigente en `from` puede ser de unos días antes (fines de semana, feriados).
    Ok(store
        .daily_vol_terciles(
            out,
            u32::try_from(daily_cfg.realized_vol_window)?,
            daily_cfg.vol_tercile_years,
            from.date_naive() - chrono::Days::new(30),
            to.date_naive() + chrono::Days::new(1),
        )
        .await?)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let write = match args.iter().position(|a| a == "--escribir") {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    let usage = "uso: <run_id> [INSTRUMENTO] [desde UTC] [hasta UTC] [--escribir]";
    let run_id = args.first().ok_or(usage)?.clone();
    let id: InstrumentId = args.get(1).map_or("US500", String::as_str).parse()?;
    let from: DateTime<Utc> = match args.get(2) {
        Some(s) => s.parse()?,
        None => "2000-01-01T00:00:00Z".parse()?,
    };
    let to: DateTime<Utc> = match args.get(3) {
        Some(s) => s.parse()?,
        None if write => {
            return Err(format!("con --escribir, `hasta` es obligatorio. {usage}").into());
        }
        None => Utc::now(),
    };

    // Sin --escribir, la sesión no puede escribir.
    let store = if write {
        PgStore::connect(&std::env::var("DATABASE_URL")?).await?
    } else {
        futures_series::read_only_store().await?
    };
    let FuturesSeries {
        rules,
        out,
        session,
        m15,
        h1,
        entries,
        force_close,
        data_errors,
        ..
    } = futures_series::prepare(&store, &id, (from, to)).await?;
    let (Some(first), Some(last)) = (m15.first(), m15.last()) else {
        return Err("sin velas".into());
    };
    let run = MlLabelRun {
        run_id: run_id.clone(),
        instrument_id: out.clone(),
        candle_source: LabelCandles::Futures,
        from: first.start,
        to: last.end(),
        method: LabelMethod::MidNoCosts,
        config_sha256: cfds_journal::config_sha256(&CONFIG_FILES),
    };
    let counted = proposals(
        &m15,
        &h1,
        &entries,
        &force_close,
        &data_errors,
        &rules,
        Overlap::OneAtATime,
    )
    .map_err(|e| format!("{e:?}"))?
    .proposals;
    let daily = terciles(&store, &out, (run.from, run.to)).await?;
    if daily.is_empty() {
        eprintln!(
            "⚠️  sin tercios de volatilidad de {out}: ninguna propuesta tiene variables de ML \
             (cargarlos con `futures_daily` y `daily_indicators -- --fuente futuros` de cfds-ingest)"
        );
    } else if let Some(d) = daily.first() {
        eprintln!("tercios de volatilidad de {out} desde el {}", d.day);
    }
    let atr = atr14_m15(&m15);

    let journal = PgJournal::new(store.pool().clone());
    if write {
        journal.record_ml_label_run(&run).await?;
    }
    let (mut rows, mut new) = (Vec::new(), [0usize; 3]);
    for p in &counted {
        let prop = proposal(&run_id, &out, p);
        let lab = label(&run_id, &out, p);
        let feat = features(&run_id, &out, p, &atr, &daily, &session);
        if write {
            let inserted = [
                journal.record(&prop, None).await? == Recorded::Inserted,
                match &lab {
                    Some(l) => journal.record_ml_label(l).await? == Recorded::Inserted,
                    None => false,
                },
                match &feat {
                    Some(f) => journal.record_ml_features(f).await? == Recorded::Inserted,
                    None => false,
                },
            ];
            for (n, i) in new.iter_mut().zip(inserted) {
                *n += usize::from(i);
            }
        }
        rows.push(Row {
            proposal: p.clone(),
            labeled: lab.is_some(),
            with_features: feat.is_some(),
        });
    }

    println!("# Datos de ML sin P/L: {run_id} ({out})\n");
    println!(
        "- Período de los datos: {} → {} ({} velas de 15m). config_sha256 {}.",
        run.from,
        run.to,
        m15.len(),
        &run.config_sha256[..12]
    );
    let labeled = rows.iter().filter(|r| r.labeled).count();
    let complete = rows.iter().filter(|r| r.labeled && r.with_features).count();
    println!(
        "- Propuestas: {}; con etiqueta: {labeled}; con etiqueta y variables: {complete}.",
        rows.len()
    );
    if write {
        println!(
            "- Guardado: {} propuestas, {} etiquetas y {} variables nuevas (el resto ya estaba).",
            new[0], new[1], new[2]
        );
    } else {
        println!("- Solo lectura: no se guardó nada (`--escribir` para guardar).");
    }

    // Filas por ventana walk-forward (las que puede usar el entrenamiento, con la purga).
    let split = plan(
        run.from,
        last.start,
        &SplitConfig::from_toml(cfg!("data_split.toml"))?,
    )
    .ok();
    let Some(split) = split else {
        println!("\nCon menos datos que una ventana, no hay walk-forward.");
        return Ok(());
    };
    println!(
        "\n### Filas por ventana de entrenamiento (con etiqueta y variables; purga: resultado \
         conocido antes del fin)\n"
    );
    println!("| Entrenamiento | Propuestas | Con etiqueta y variables | y = 1 | y = 0 |");
    println!("|---|---|---|---|---|");
    for w in &split.windows {
        let inside: Vec<&Row> = rows
            .iter()
            .filter(|r| {
                let start = m15[r.proposal.index].start;
                start >= w.train.start && r.proposal.resolved_at <= w.train.end
            })
            .collect();
        let mut t = Tally::default();
        for r in inside.iter().filter(|r| r.labeled && r.with_features) {
            t.add(r.proposal.label);
        }
        println!(
            "| {} a {} | {} | {} | {} | {} |",
            w.train.start.date_naive(),
            w.train.end.date_naive(),
            inside.len(),
            t.y1 + t.y0,
            t.y1,
            t.y0
        );
    }
    let oos = rows
        .iter()
        .filter(|r| r.proposal.resolved_at > split.out_of_sample.start)
        .count();
    println!(
        "\nFuera de muestra (desde {}): {oos} propuestas, que el entrenamiento no lee.",
        split.out_of_sample.start
    );
    Ok(())
}

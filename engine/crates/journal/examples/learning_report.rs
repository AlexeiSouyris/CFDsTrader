//! Informe de aprendizaje continuo (fase 8b, spec §15b, #327) con el diario guardado.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-journal --example learning_report -- \
//!     <desde UTC> <hasta UTC> --entrenamiento <run_id> [--corridas <run_id>,...] [--paper] \
//!     [--corte <UTC>] [--guardar]
//! ```
//! - **Probabilidad mostrada (§13):** la tabla se arma con la corrida de backtest de
//!   `--entrenamiento`, solo con resultados terminados antes de `--corte` (por omisión, `desde`:
//!   nada del período evaluado entra a la tabla).
//! - **Período evaluado:** las propuestas con resultado simulado decididas en `[desde, hasta)`,
//!   de las corridas de `--corridas` y/o de paper (`--paper`).
//! - Imprime el informe (`learning_report.v1`). Con `--guardar` lo guarda con sus propuestas de
//!   cambio, todas "pendiente de voto" (migración 0034). **Nunca aplica un cambio**: eso es
//!   a mano en `config/`, con el voto registrado (spec §1.2).

use cfds_journal::learning::{Inputs, LearningConfig, report};
use cfds_journal::pg::PgJournal;
use cfds_journal::probability::{ProbabilityConfig, ProbabilityTable};
use cfds_journal::{Outcome, Proposal, Source};
use cfds_storage::PgStore;
use chrono::{DateTime, Utc};

fn value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).map(String::as_str)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "uso: <desde UTC> <hasta UTC> --entrenamiento <run_id> [--corridas a,b] \
                 [--paper] [--corte <UTC>] [--guardar]";
    let (Some(from), Some(to)) = (args.first(), args.get(1)) else {
        return Err(usage.into());
    };
    let (from, to): (DateTime<Utc>, DateTime<Utc>) = (from.parse()?, to.parse()?);
    if to <= from {
        return Err("<hasta> tiene que ser posterior a <desde>".into());
    }
    let training = value(&args, "--entrenamiento").ok_or(usage)?;
    let cutoff: DateTime<Utc> = match value(&args, "--corte") {
        Some(c) => c.parse()?,
        None => from,
    };
    if cutoff > from {
        return Err("--corte no puede ser posterior a <desde> (look-ahead)".into());
    }
    let runs: Vec<&str> = value(&args, "--corridas")
        .map(|r| r.split(',').filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let paper = args.iter().any(|a| a == "--paper");
    if runs.is_empty() && !paper {
        return Err("hace falta --corridas o --paper".into());
    }

    let probability =
        ProbabilityConfig::from_toml(include_str!("../../../../config/probability.toml"))?;
    let cfg = LearningConfig::from_toml(include_str!("../../../../config/learning.toml"))?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let journal = PgJournal::new(store.pool().clone());

    let pairs_of = |trades: Vec<cfds_journal::RunTrade>| -> Vec<(Proposal, Outcome)> {
        trades
            .into_iter()
            .filter_map(|t| Some((t.proposal, t.outcome?)))
            .collect()
    };
    let train = pairs_of(journal.run_trades(training).await?);
    let table = ProbabilityTable::build(&train, cutoff, &probability)?;
    println!(
        "probabilidad §13: {} propuestas de {training}, con resultados hasta {cutoff}",
        train.len()
    );

    let mut pairs: Vec<(Proposal, Outcome)> = Vec::new();
    for run in &runs {
        pairs.extend(
            pairs_of(journal.run_trades(run).await?)
                .into_iter()
                .filter(|(p, _)| p.signal_at >= from && p.signal_at < to),
        );
    }
    if paper {
        pairs.extend(journal.pairs(Source::Paper, from, to).await?);
    }
    if pairs.is_empty() {
        println!("no hay propuestas con resultado en el período: el informe queda vacío");
    }
    let r = report(&Inputs {
        pairs: &pairs,
        table: &table,
        probability: &probability,
        cfg: &cfg,
        from,
        to,
        created_at: Utc::now(),
    });
    println!(
        "{} propuestas evaluadas; {} sin probabilidad mostrada; {} propuestas de cambio \
         (pendientes de voto)",
        r.proposals,
        r.calibration.without_probability,
        r.change_proposals.len()
    );
    for c in &r.change_proposals {
        println!("  - [{}] {}", c.evidence, c.text);
    }
    if args.iter().any(|a| a == "--guardar") {
        println!(
            "guardado {}: {:?}",
            r.report_id,
            journal.record_learning_report(&r).await?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&r)?);
    }
    Ok(())
}

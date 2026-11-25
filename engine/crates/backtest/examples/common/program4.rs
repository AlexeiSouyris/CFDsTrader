//! Lo común de los ejemplos del programa 4 (#561): ATR y estructura por tramo de contrato, días
//! excluidos (roll y datos fuertes) y señales de H4.1 y H4.2. Solo orquestación: las reglas viven
//! en `cfds_backtest::research::program4`.
#![allow(dead_code, reason = "cada ejemplo usa una parte")]

use std::collections::BTreeSet;

use cfds_backtest::research::program4::{
    BodyThreshold, Structure, impulse_plans, release_days, roll_days, segment_atr, structure,
};
use cfds_backtest::research::{Plan, Program4Config};
use cfds_market_data::Session;
use cfds_storage::PgStore;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;

use super::prepare::Prepared;

/// Lo que necesitan las señales, alineado con `p.m15`.
pub struct Inputs {
    pub rolls: Vec<DateTime<Utc>>,
    pub atr: Vec<Option<Decimal>>,
    pub states: Vec<Structure>,
    pub roll_days: BTreeSet<NaiveDate>,
    pub release_days: BTreeSet<NaiveDate>,
    /// Roll ∪ datos fuertes: días sin señales.
    pub excluded: BTreeSet<NaiveDate>,
}

/// Arma las entradas de las señales. Los comunicados fuertes salen de `official_releases`
/// (solo los programados y publicados hasta `published_until`).
pub async fn inputs(
    p: &Prepared,
    session: &Session,
    cfg: &Program4Config,
    store: &PgStore,
    published_until: DateTime<Utc>,
) -> Result<Inputs, Box<dyn std::error::Error>> {
    let rolls = p
        .setup
        .futures_basis
        .as_ref()
        .map(|b| b.rolls.clone())
        .ok_or("sin vencimientos: el programa 4 corre sobre futuros")?;
    let atr = segment_atr(&p.m15, &rolls, cfg.atr_period);
    let (_, states) = structure(&p.m15, &rolls, cfg.min_valid_in_zone);
    let roll = roll_days(session, &rolls);
    let mut scheduled = Vec::new();
    for (source, series) in &cfg.strong_releases {
        let rows = store
            .official_releases(source, series, published_until)
            .await?;
        if rows.is_empty() {
            return Err(format!("official_releases sin filas de {source}/{series}").into());
        }
        scheduled.extend(
            rows.iter()
                .filter(|r| r.status == "scheduled")
                .filter_map(|r| r.scheduled_at),
        );
    }
    let release = release_days(session, &scheduled);
    let excluded = roll.union(&release).copied().collect();
    println!(
        "días sin señales: {} de roll y {} con dato fuerte ({} comunicados programados de {:?})",
        roll.len(),
        release.len(),
        scheduled.len(),
        cfg.strong_releases
    );
    Ok(Inputs {
        rolls,
        atr,
        states,
        roll_days: roll,
        release_days: release,
        excluded,
    })
}

/// Señales de H4.1 (cuerpo ≥ `body_points`) y H4.2 (cuerpo ≥ k × ATR).
pub fn plans(
    p: &Prepared,
    session: &Session,
    i: &Inputs,
    cfg: &Program4Config,
) -> Result<[Vec<Plan>; 2], Box<dyn std::error::Error>> {
    let k = cfg
        .body_atr_k
        .ok_or("program4.body_atr_k sin fijar: correr antes programa_4_k (§5.1)")?;
    let run = |th| {
        impulse_plans(
            &p.m15,
            session,
            &p.entries,
            &i.atr,
            &i.states,
            &i.excluded,
            th,
            cfg,
        )
    };
    Ok([
        run(BodyThreshold::Points(cfg.body_points)),
        run(BodyThreshold::Atr(k)),
    ])
}

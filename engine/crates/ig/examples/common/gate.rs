//! Antes de conectarse a IG (#143): puntos estimados, `--dry-run`, confirmación de cargas
//! grandes, tope en las máquinas que no guardan los datos y presupuesto semanal del uso (#167).

use std::io::{BufRead, IsTerminal, Write};

use cfds_ig::budget;
use cfds_ig::{
    DATA_MACHINE_ENV, IgConfig, LoadDecision, LoadFlags, decide, is_data_machine, usage_of,
};
use cfds_storage::PgStore;

use crate::budget as week;

/// `Ok(true)` si se carga, `Ok(false)` si fue `--dry-run`, `Err` si no se debe cargar.
/// `command` es el nombre con que la carga se guarda en `ig_allowance_log`.
pub async fn check(
    estimated: u64,
    flags: LoadFlags,
    cfg: &IgConfig,
    store: &PgStore,
    command: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let data_machine = is_data_machine(std::env::var(DATA_MACHINE_ENV).ok().as_deref());
    let usage = usage_of(command, data_machine);
    let spend = week::spend(store, data_machine).await?;
    println!("{}", week::describe(usage, &spend, &cfg.budget));
    let budget = budget::check(usage, estimated, &spend, &cfg.budget);
    println!(
        "estimado: ~{estimated} puntos del cupo semanal (cota, con el margen de cada pedido){}",
        if data_machine {
            ""
        } else {
            " — esta máquina no guarda los datos"
        }
    );
    if let (true, Err(why)) = (flags.dry_run, &budget) {
        println!("sin --dry-run se negaría: {why}");
    }
    match decide(estimated, flags, data_machine, &cfg.loads, budget) {
        LoadDecision::Go => Ok(true),
        LoadDecision::DryRun => {
            println!("--dry-run: no se pide nada a IG");
            Ok(false)
        }
        LoadDecision::Refuse(why) => Err(why.into()),
        LoadDecision::Confirm => {
            if !std::io::stdin().is_terminal() {
                return Err(format!(
                    "~{estimated} puntos supera loads.confirm_above_points ({}) y no hay a quién \
                     preguntar: agregar --yes para confirmar",
                    cfg.loads.confirm_above_points
                )
                .into());
            }
            print!(
                "~{estimated} puntos supera {} (loads.confirm_above_points). ¿Seguir? [s/N] ",
                cfg.loads.confirm_above_points
            );
            std::io::stdout().flush()?;
            let mut answer = String::new();
            std::io::stdin().lock().read_line(&mut answer)?;
            if matches!(answer.trim().to_lowercase().as_str(), "s" | "si" | "sí") {
                Ok(true)
            } else {
                Err("cancelado: no se pidió nada".into())
            }
        }
    }
}

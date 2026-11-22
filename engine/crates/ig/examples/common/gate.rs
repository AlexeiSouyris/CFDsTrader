//! Antes de conectarse a IG (#143): puntos estimados, `--dry-run`, confirmación de cargas
//! grandes y tope en las máquinas que no guardan los datos.

use std::io::{BufRead, IsTerminal, Write};

use cfds_ig::{DATA_MACHINE_ENV, IgConfig, LoadDecision, LoadFlags, decide, is_data_machine};

/// `Ok(true)` si se carga, `Ok(false)` si fue `--dry-run`, `Err` si no se debe cargar.
pub fn check(
    estimated: u64,
    flags: LoadFlags,
    cfg: &IgConfig,
) -> Result<bool, Box<dyn std::error::Error>> {
    let data_machine = is_data_machine(std::env::var(DATA_MACHINE_ENV).ok().as_deref());
    println!(
        "estimado: ~{estimated} puntos del cupo semanal (cota, con el margen de cada pedido){}",
        if data_machine {
            ""
        } else {
            " — esta máquina no guarda los datos"
        }
    );
    match decide(estimated, flags, data_machine, &cfg.loads) {
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

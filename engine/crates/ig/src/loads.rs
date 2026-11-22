//! Reglas antes de una carga histórica de IG (#143, propuestas 4 y 8 del #132).
//!
//! - `--dry-run`: muestra el plan y los puntos estimados, sin conectarse.
//! - Arriba de `confirm_above_points` se pide confirmación, salvo `--yes`.
//! - **Tope por defecto:** una máquina que no está marcada con `CFDS_DATA_MACHINE=1` (la que
//!   guarda los datos, el Mac mini) no puede hacer cargas de más de
//!   `max_points_off_data_machine`. Es una marca a favor y no una detección de "la nube": una
//!   máquina nueva queda limitada sin que nadie tenga que acordarse.

use chrono::{DateTime, TimeDelta, Utc};

use crate::config::LoadLimits;
use crate::model;

/// Variable de entorno que marca la máquina que guarda los datos (solo en su `.env`).
pub const DATA_MACHINE_ENV: &str = "CFDS_DATA_MACHINE";

/// Opciones de la línea de comandos de una carga.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadFlags {
    pub dry_run: bool,
    pub yes: bool,
}

/// Separa `--dry-run` y `--yes` de los argumentos posicionales. Otra opción es un error.
pub fn parse_flags(args: Vec<String>) -> Result<(Vec<String>, LoadFlags), String> {
    let mut flags = LoadFlags::default();
    let mut positional = Vec::new();
    for a in args {
        match a.as_str() {
            "--dry-run" => flags.dry_run = true,
            "--yes" => flags.yes = true,
            other if other.starts_with("--") => {
                return Err(format!(
                    "opción desconocida: {other} (se aceptan --dry-run y --yes)"
                ));
            }
            _ => positional.push(a),
        }
    }
    Ok((positional, flags))
}

/// `true` solo con `1` o `true` (sin distinguir mayúsculas). Ausente o cualquier otro valor: no.
pub fn is_data_machine(value: Option<&str>) -> bool {
    value.is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// Puntos que puede costar pedir `ranges` con velas de largo `step`, contando el margen
/// horario de cada pedido (`history.local_time_margin_hours`). Es una cota: IG puede devolver
/// menos (por ejemplo, sin velas el fin de semana).
pub fn estimate_points(
    ranges: &[(DateTime<Utc>, DateTime<Utc>)],
    step: TimeDelta,
    margin_hours: u32,
) -> u64 {
    let margin = TimeDelta::hours(i64::from(margin_hours));
    ranges
        .iter()
        .map(|&(from, to)| model::estimated_points_step(from, to, step, margin))
        .sum()
}

/// Qué hacer con una carga.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadDecision {
    /// Se carga.
    Go,
    /// Solo mostrar el plan: no se conecta.
    DryRun,
    /// Preguntar antes (o negarse si no hay a quién).
    Confirm,
    /// No se carga, con el motivo.
    Refuse(String),
}

/// Decide según los puntos estimados, las opciones, la máquina y el presupuesto semanal del
/// uso (`budget`, el resultado de [`crate::budget::check`], #167). Pasar el presupuesto se
/// niega aunque haya `--yes`.
pub fn decide(
    estimated: u64,
    flags: LoadFlags,
    data_machine: bool,
    limits: &LoadLimits,
    budget: Result<(), String>,
) -> LoadDecision {
    if flags.dry_run {
        return LoadDecision::DryRun;
    }
    if let Err(why) = budget {
        return LoadDecision::Refuse(why);
    }
    if !data_machine && estimated > limits.max_points_off_data_machine {
        return LoadDecision::Refuse(format!(
            "~{estimated} puntos y esta máquina no es la que guarda los datos: el tope es {} \
             (config/ig.toml, loads.max_points_off_data_machine). Las cargas grandes se hacen en \
             el Mac mini (docs/guides/trabajo-en-paralelo.md). Si de verdad hace falta acá, \
             correr ese comando con {DATA_MACHINE_ENV}=1",
            limits.max_points_off_data_machine
        ));
    }
    if estimated > limits.confirm_above_points && !flags.yes {
        return LoadDecision::Confirm;
    }
    LoadDecision::Go
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: LoadLimits = LoadLimits {
        confirm_above_points: 500,
        max_points_off_data_machine: 100,
    };

    fn args(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn flags_can_go_anywhere_and_unknown_ones_are_errors() {
        let (pos, f) = parse_flags(args(&["--yes", "DE40", "2026-01-01", "--dry-run"])).unwrap();
        assert_eq!(pos, args(&["DE40", "2026-01-01"]));
        assert_eq!(
            f,
            LoadFlags {
                dry_run: true,
                yes: true
            }
        );
        assert!(parse_flags(args(&["--force"])).is_err());
    }

    #[test]
    fn only_an_explicit_mark_makes_the_data_machine() {
        assert!(is_data_machine(Some("1")));
        assert!(is_data_machine(Some("TRUE")));
        for v in [None, Some(""), Some("0"), Some("no"), Some("yes")] {
            assert!(!is_data_machine(v), "{v:?}");
        }
    }

    #[test]
    fn dry_run_never_loads_even_when_small() {
        let f = LoadFlags {
            dry_run: true,
            yes: true,
        };
        assert_eq!(decide(1, f, true, &LIMITS, Ok(())), LoadDecision::DryRun);
        assert_eq!(
            decide(9_999, f, false, &LIMITS, Ok(())),
            LoadDecision::DryRun
        );
    }

    #[test]
    fn off_the_data_machine_only_small_loads_and_yes_does_not_help() {
        let yes = LoadFlags {
            yes: true,
            ..Default::default()
        };
        assert_eq!(decide(100, yes, false, &LIMITS, Ok(())), LoadDecision::Go);
        assert!(matches!(
            decide(101, yes, false, &LIMITS, Ok(())),
            LoadDecision::Refuse(m) if m.contains("CFDS_DATA_MACHINE=1")
        ));
    }

    #[test]
    fn on_the_data_machine_big_loads_ask_unless_yes() {
        let none = LoadFlags::default();
        assert_eq!(decide(500, none, true, &LIMITS, Ok(())), LoadDecision::Go);
        assert_eq!(
            decide(501, none, true, &LIMITS, Ok(())),
            LoadDecision::Confirm
        );
        let yes = LoadFlags {
            yes: true,
            ..Default::default()
        };
        assert_eq!(decide(5_000, yes, true, &LIMITS, Ok(())), LoadDecision::Go);
    }

    #[test]
    fn over_budget_is_refused_even_with_yes_but_dry_run_still_shows_the_plan() {
        let yes = LoadFlags {
            yes: true,
            ..Default::default()
        };
        let over = || Err("presupuesto".to_owned());
        assert_eq!(
            decide(10, yes, true, &LIMITS, over()),
            LoadDecision::Refuse("presupuesto".to_owned())
        );
        let dry = LoadFlags {
            dry_run: true,
            ..Default::default()
        };
        assert_eq!(decide(10, dry, true, &LIMITS, over()), LoadDecision::DryRun);
    }

    #[test]
    fn estimate_adds_the_margin_of_every_request() {
        let t = |s: &str| -> DateTime<Utc> { s.parse().unwrap() };
        // 2 velas de 1h + 1 h de margen de cada lado, contando los extremos = 5.
        let one = [(t("2026-11-20T15:00:00Z"), t("2026-11-20T17:00:00Z"))];
        assert_eq!(estimate_points(&one, TimeDelta::hours(1), 1), 5);
        let two = [
            one[0],
            (t("2026-11-23T15:00:00Z"), t("2026-11-23T16:00:00Z")),
        ];
        assert_eq!(estimate_points(&two, TimeDelta::hours(1), 1), 5 + 4);
        assert_eq!(estimate_points(&[], TimeDelta::hours(1), 1), 0);
    }
}

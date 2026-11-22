//! Presupuesto semanal del cupo de IG por uso (#167, propuesta 6 del #132).
//!
//! `config/ig.toml` (`[budget]`) reparte el cupo semanal entre usos. Cada comando sabe a qué
//! uso pertenece ([`usage_of`]) y se frena **antes** de pasar su parte ([`check`]).
//!
//! Lo gastado en la semana **no** se lleva aparte: sale de `ig_allowance_log` (#140), con las
//! filas de la ventana vigente del cupo ([`week_spend`]):
//! - La ventana vigente son las observaciones cuya renovación anunciada (`resets_at`) todavía no
//!   llegó. Si la última ya pasó, el cupo se renovó y no se gastó nada en esta ventana.
//! - En la máquina que guarda los datos, cada fila cuenta para el uso de su comando, y lo que
//!   bajó el cupo de la cuenta sin que lo pidiera esta máquina (total − restante − lo propio)
//!   cuenta como uso fuera de esa máquina.
//! - En otra máquina, todo lo que registró ella misma es uso fuera de la máquina de los datos.
//!   Esa máquina no ve el registro del Mac mini (su base es otra): su parte se controla con lo
//!   que ella misma registró, más el tope por carga de #143.

use chrono::{DateTime, Utc};

use crate::allowance::AllowanceObservation;
use crate::config::UsageBudget;

/// A qué parte del presupuesto se carga un pedido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    /// `ig_live` en la máquina que guarda los datos.
    Live,
    /// Cargas de historia (`ig_backfill`, `ig_backfill_daily`) en la máquina de los datos.
    History,
    /// Cualquier pedido desde una máquina sin `CFDS_DATA_MACHINE=1`.
    OffDataMachine,
}

impl Usage {
    /// Nombre del campo en `[budget]`, para los mensajes.
    pub fn field(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::History => "history",
            Self::OffDataMachine => "off_data_machine",
        }
    }
}

/// Uso de un comando según la máquina. Un comando desconocido de la máquina de los datos se
/// cuenta como historia (lo más parecido: pedidos puntuales de velas).
pub fn usage_of(command: &str, data_machine: bool) -> Usage {
    if !data_machine {
        return Usage::OffDataMachine;
    }
    match command {
        "ig_live" => Usage::Live,
        _ => Usage::History,
    }
}

impl UsageBudget {
    /// La parte de un uso.
    pub fn share(&self, usage: Usage) -> u64 {
        match usage {
            Usage::Live => self.live,
            Usage::History => self.history,
            Usage::OffDataMachine => self.off_data_machine,
        }
    }
}

/// Una fila de `ig_allowance_log`: la observación y el comando que la registró.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggedObservation {
    pub command: String,
    pub observation: AllowanceObservation,
}

/// Lo gastado en la ventana vigente del cupo, por uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WeekSpend {
    pub live: u64,
    pub history: u64,
    pub off_data_machine: u64,
    /// Cupo restante de la cuenta según la última observación de la ventana, menos lo que se
    /// sumó después con [`WeekSpend::add`]. `None` si no hay observaciones en la ventana.
    pub remaining: Option<u64>,
    /// Renovación anunciada por la última observación de la ventana.
    pub resets_at: Option<DateTime<Utc>>,
}

impl WeekSpend {
    pub fn spent(&self, usage: Usage) -> u64 {
        match usage {
            Usage::Live => self.live,
            Usage::History => self.history,
            Usage::OffDataMachine => self.off_data_machine,
        }
    }

    /// Suma puntos ya pedidos que todavía no están en el registro (por ejemplo, entre dos
    /// lecturas del registro en `ig_live`, o sin base).
    pub fn add(&mut self, usage: Usage, points: u64) {
        let slot = match usage {
            Usage::Live => &mut self.live,
            Usage::History => &mut self.history,
            Usage::OffDataMachine => &mut self.off_data_machine,
        };
        *slot = slot.saturating_add(points);
        self.remaining = self.remaining.map(|r| r.saturating_sub(points));
    }
}

/// Lo gastado por uso en la ventana vigente del cupo, a partir del registro (`log`, en el
/// orden en que se guardó).
pub fn week_spend(log: &[LoggedObservation], now: DateTime<Utc>, data_machine: bool) -> WeekSpend {
    let window: Vec<_> = log
        .iter()
        .filter(|l| l.observation.resets_at > now)
        .collect();
    let Some(last) = window.iter().max_by_key(|l| l.observation.at) else {
        return WeekSpend::default();
    };
    let mut spend = WeekSpend {
        remaining: Some(last.observation.remaining),
        resets_at: Some(last.observation.resets_at),
        ..WeekSpend::default()
    };
    let mut own = 0u64;
    for l in &window {
        let points = l.observation.points_returned;
        own = own.saturating_add(points);
        let slot = match usage_of(&l.command, data_machine) {
            Usage::Live => &mut spend.live,
            Usage::History => &mut spend.history,
            Usage::OffDataMachine => &mut spend.off_data_machine,
        };
        *slot = slot.saturating_add(points);
    }
    if data_machine {
        // Lo que bajó la cuenta en la ventana y no pidió esta máquina: la nube u otra computadora.
        let account = last
            .observation
            .total
            .saturating_sub(last.observation.remaining);
        spend.off_data_machine = spend
            .off_data_machine
            .saturating_add(account.saturating_sub(own));
    }
    spend
}

/// `Ok` si `estimated` puntos más entran en la parte de `usage` y no tocan la reserva; si no,
/// el motivo. Se llama **antes** de pedir.
pub fn check(
    usage: Usage,
    estimated: u64,
    spend: &WeekSpend,
    budget: &UsageBudget,
) -> Result<(), String> {
    let (spent, share) = (spend.spent(usage), budget.share(usage));
    let renewal = spend.resets_at.map_or_else(String::new, |r| {
        format!("; el cupo se renueva el {}", r.format("%Y-%m-%d %H:%M UTC"))
    });
    if spent.saturating_add(estimated) > share {
        return Err(format!(
            "presupuesto semanal de IG: el uso \"{}\" ya gastó {spent} de {share} puntos y esto \
             pediría ~{estimated} más (config/ig.toml, budget.{}){renewal}",
            usage.field(),
            usage.field()
        ));
    }
    if let Some(remaining) = spend.remaining
        && remaining.saturating_sub(estimated) < budget.reserve
    {
        return Err(format!(
            "presupuesto semanal de IG: a la cuenta le quedan {remaining} puntos y pedir ~{estimated} \
             tocaría la reserva de {} (config/ig.toml, budget.reserve){renewal}",
            budget.reserve
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGET: UsageBudget = UsageBudget {
        live: 3_400,
        history: 5_000,
        off_data_machine: 1_000,
        reserve: 600,
    };

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().expect("fecha de test")
    }

    fn row(
        command: &str,
        at: &str,
        points: u64,
        remaining: u64,
        resets: &str,
    ) -> LoggedObservation {
        LoggedObservation {
            command: command.to_owned(),
            observation: AllowanceObservation {
                at: t(at),
                points_returned: points,
                remaining,
                total: 10_000,
                resets_at: t(resets),
            },
        }
    }

    const RESET: &str = "2026-11-28T12:00:00Z";
    const NOW: &str = "2026-11-23T12:00:00Z";

    #[test]
    fn each_command_belongs_to_one_usage_and_the_machine_matters() {
        assert_eq!(usage_of("ig_live", true), Usage::Live);
        assert_eq!(usage_of("ig_backfill", true), Usage::History);
        assert_eq!(usage_of("ig_backfill_daily", true), Usage::History);
        for c in ["ig_live", "ig_backfill", "ig_backfill_daily"] {
            assert_eq!(usage_of(c, false), Usage::OffDataMachine, "{c}");
        }
    }

    #[test]
    fn spend_comes_from_the_log_of_the_current_window() {
        let log = [
            // Ventana anterior: ya se renovó, no cuenta.
            row(
                "ig_backfill",
                "2026-11-20T10:00:00Z",
                4_000,
                6_000,
                "2026-11-21T12:00:00Z",
            ),
            row(
                "ig_backfill_daily",
                "2026-11-22T10:00:00Z",
                300,
                9_700,
                RESET,
            ),
            row("ig_live", "2026-11-22T11:00:00Z", 10, 9_690, RESET),
            // Entre estas dos, alguien más gastó 200.
            row("ig_live", "2026-11-23T11:00:00Z", 5, 9_485, RESET),
        ];
        let s = week_spend(&log, t(NOW), true);
        assert_eq!(s.history, 300);
        assert_eq!(s.live, 15);
        assert_eq!(s.off_data_machine, 200, "lo gastado por otro lado");
        assert_eq!(s.remaining, Some(9_485));
        assert_eq!(s.resets_at, Some(t(RESET)));
    }

    #[test]
    fn off_the_data_machine_everything_it_logged_is_its_own_usage() {
        let log = [
            row("ig_backfill", "2026-11-22T10:00:00Z", 50, 3_000, RESET),
            row("ig_live", "2026-11-22T11:00:00Z", 4, 2_996, RESET),
        ];
        let s = week_spend(&log, t(NOW), false);
        assert_eq!((s.live, s.history, s.off_data_machine), (0, 0, 54));
        assert_eq!(s.remaining, Some(2_996));
    }

    #[test]
    fn after_a_renewal_nothing_is_spent_and_the_remaining_is_unknown() {
        let log = [row(
            "ig_backfill",
            "2026-11-20T10:00:00Z",
            4_000,
            6_000,
            "2026-11-21T12:00:00Z",
        )];
        assert_eq!(week_spend(&log, t(NOW), true), WeekSpend::default());
        assert_eq!(week_spend(&[], t(NOW), true), WeekSpend::default());
    }

    #[test]
    fn a_usage_stops_before_passing_its_share() {
        let spend = WeekSpend {
            history: 4_900,
            remaining: Some(5_000),
            resets_at: Some(t(RESET)),
            ..WeekSpend::default()
        };
        assert_eq!(check(Usage::History, 100, &spend, &BUDGET), Ok(()));
        let err = check(Usage::History, 101, &spend, &BUDGET).unwrap_err();
        assert!(
            err.contains("4900 de 5000") && err.contains("budget.history"),
            "{err}"
        );
        assert!(err.contains("2026-11-28 12:00 UTC"), "{err}");
        // La parte de otro uso no se toca.
        assert_eq!(check(Usage::Live, 3_400, &spend, &BUDGET), Ok(()));
    }

    #[test]
    fn nobody_eats_the_reserve() {
        let spend = WeekSpend {
            remaining: Some(700),
            ..WeekSpend::default()
        };
        assert_eq!(check(Usage::Live, 100, &spend, &BUDGET), Ok(()));
        let err = check(Usage::Live, 101, &spend, &BUDGET).unwrap_err();
        assert!(err.contains("budget.reserve"), "{err}");
        // Sin observaciones en la ventana, solo manda la parte de cada uso.
        assert_eq!(
            check(Usage::Live, 3_400, &WeekSpend::default(), &BUDGET),
            Ok(())
        );
    }

    #[test]
    fn points_added_between_log_reads_count() {
        let mut spend = WeekSpend {
            live: 3_390,
            remaining: Some(5_000),
            ..WeekSpend::default()
        };
        spend.add(Usage::Live, 10);
        assert_eq!((spend.live, spend.remaining), (3_400, Some(4_990)));
        assert!(check(Usage::Live, 1, &spend, &BUDGET).is_err());
        assert_eq!(check(Usage::Live, 0, &spend, &BUDGET), Ok(()));
    }
}

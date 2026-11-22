//! Presupuesto semanal del cupo por uso (#167): lo gastado sale de `ig_allowance_log`.

use cfds_ig::{AllowanceObservation, LoggedObservation, WeekSpend, week_spend};
use cfds_storage::PgStore;
use chrono::Utc;

/// Lo gastado por uso en la ventana vigente del cupo, según el registro de esta base.
pub async fn spend(
    store: &PgStore,
    data_machine: bool,
) -> Result<WeekSpend, Box<dyn std::error::Error>> {
    let now = Utc::now();
    let log: Vec<_> = store
        .allowance_window(now)
        .await?
        .into_iter()
        .map(|(command, r)| LoggedObservation {
            command,
            observation: AllowanceObservation {
                at: r.observed_at,
                points_returned: r.points_returned,
                remaining: r.remaining,
                total: r.total,
                resets_at: r.resets_at,
            },
        })
        .collect();
    Ok(week_spend(&log, now, data_machine))
}

/// "presupuesto <uso>: gastados X de Y puntos esta semana; quedan Z en la cuenta (reserva R)".
pub fn describe(usage: cfds_ig::Usage, spend: &WeekSpend, budget: &cfds_ig::UsageBudget) -> String {
    let account = spend.remaining.map_or_else(
        || "restante de la cuenta sin observar en esta ventana".to_owned(),
        |r| format!("quedan {r} en la cuenta"),
    );
    format!(
        "presupuesto {}: gastados {} de {} puntos esta semana; {account} (reserva {})",
        usage.field(),
        spend.spent(usage),
        budget.share(usage),
        budget.reserve
    )
}

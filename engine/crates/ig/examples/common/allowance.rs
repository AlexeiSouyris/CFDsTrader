//! Cupo de IG al final de cada carga (#140): se guarda en `ig_allowance_log` (si hay base) y
//! se muestra cuánto queda, cuándo se renueva, lo gastado y lo gastado desde otro lado.

use cfds_ig::{AllowanceObservation, IgProvider, ReqwestTransport, summarize};
use cfds_storage::{AllowanceRecord, PgStore};
use chrono::{DateTime, Utc};

/// Lee lo que dijo IG del cupo desde la última llamada, lo guarda y lo resume. La observación
/// anterior se lee **antes** de guardar las nuevas.
pub async fn record_and_report(
    ig: &IgProvider<ReqwestTransport>,
    store: Option<&PgStore>,
    command: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let log = ig.take_allowance_log().await;
    if log.is_empty() {
        return Ok(());
    }
    let previous = match store {
        Some(s) => s.last_allowance().await?.map(|r| AllowanceObservation {
            at: r.observed_at,
            points_returned: r.points_returned,
            remaining: r.remaining,
            total: r.total,
            resets_at: r.resets_at,
        }),
        None => None,
    };
    if let Some(s) = store {
        let records: Vec<_> = log
            .iter()
            .map(|o| AllowanceRecord {
                observed_at: o.at,
                points_returned: o.points_returned,
                remaining: o.remaining,
                total: o.total,
                resets_at: o.resets_at,
            })
            .collect();
        s.insert_allowance_log(command, &records).await?;
    }
    let Some(sum) = summarize(previous.as_ref(), &log) else {
        return Ok(());
    };
    println!(
        "cupo de IG: quedan {} de {} puntos; se renueva el {} ({})",
        sum.remaining,
        sum.total,
        sum.resets_at.format("%Y-%m-%d %H:%M UTC"),
        until(sum.resets_at, Utc::now())
    );
    println!(
        "esta corrida: {} puntos (precios devueltos por IG)",
        sum.points_returned
    );
    match (sum.elsewhere, previous, store) {
        (Some(e), Some(p), _) => println!(
            "gastado por otro lado desde la última observación ({}): {e} puntos",
            p.at.format("%Y-%m-%d %H:%M UTC")
        ),
        (_, _, None) => println!("sin base: no se registra ni se sabe lo gastado por otro lado"),
        _ => println!(
            "primera observación registrada: lo gastado por otro lado se ve desde la próxima"
        ),
    }
    if sum.one_point_per_price == Some(false) {
        eprintln!(
            "⚠️  el cupo no bajó lo mismo que los precios devueltos: \
             la cuenta de lo gastado por otro lado no es exacta"
        );
    }
    Ok(())
}

/// "en 1 d 18 h", "en 3 h 5 min" o "ya se renovó".
fn until(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let mins = (at - now).num_minutes();
    if mins <= 0 {
        return "ya se renovó".to_owned();
    }
    let (d, h, m) = (mins / 1440, mins % 1440 / 60, mins % 60);
    if d > 0 {
        format!("en {d} d {h} h")
    } else {
        format!("en {h} h {m} min")
    }
}

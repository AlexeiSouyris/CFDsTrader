//! Modo en vivo (#68): mantiene al día las velas de 15m y 1h de los instrumentos del catálogo,
//! pidiendo cada vela por REST apenas cierra (1 punto del cupo semanal por vela).
//!
//! ```text
//! cargo run -p cfds-ig --example ig_live            # corre hasta el kill switch o Ctrl+C
//! cargo run -p cfds-ig --example ig_live -- --once  # un solo ciclo y sale (cron, #124)
//! cargo run -p cfds-ig --example ig_live -- --dry-run  # qué pediría y cuánto, sin conectarse a IG
//! ```
//!
//! - Al arrancar retoma desde la última vela guardada en la base (como mucho
//!   `live.startup_lookback_hours` atrás). Así cada `--once` del cron pide solo las velas que
//!   cerraron desde la corrida anterior (#124).
//! - `--dry-run` muestra las velas que tocaría pedir, los puntos y el presupuesto, y sale sin
//!   credenciales ni conexión a IG.
//!
//! - Con `DATABASE_URL` y `REDIS_URL`, las velas entran por el circuito de ingesta completo:
//!   validación → PostgreSQL → Redis Streams. Sin ellas, es una corrida en seco: valida y
//!   muestra las velas, pero no guarda nada.
//! - Solo pide las velas que el calendario de sesiones espera; fuera de horario no gasta cupo.
//! - **Kill switch** (CLAUDE.md §5b): si una vela cerrada no se puede guardar pasado
//!   `live.give_up_after_secs`, cierra la sesión y termina con error.
//! - **Presupuesto semanal** (#167): antes de cada pedido controla que entre en su parte
//!   (`budget.live` en el Mac mini, `budget.off_data_machine` en otra máquina) y que no toque
//!   `budget.reserve`. Lo gastado sale de `ig_allowance_log`; sin base, de lo pedido en esta
//!   corrida. Si no entra, kill switch: no pide y termina con error.

#[path = "common/allowance.rs"]
mod allowance;
#[path = "common/budget.rs"]
mod budget;

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_core::{InstrumentId, Timeframe};
use cfds_ig::live::{due_points, due_range, next_wake, overdue};
use cfds_ig::{
    Credentials, DATA_MACHINE_ENV, IgConfig, IgProvider, Recent, ReqwestTransport, WeekSpend,
    api_key_from_env, epics_from_catalog, is_data_machine, usage_of,
};
use cfds_ingest::{IngestRequest, ingest, prepare};
use cfds_market_data::{Catalog, ExchangeCalendar, MarketDataProvider, ValidationConfig};
use cfds_storage::{CandlePublisher, PgStore};
use chrono::{TimeDelta, Utc};

const CONFIG: &str = include_str!("../../../../config/ig.toml");
const CATALOG: &str = include_str!("../../../../config/instruments.toml");
const VALIDATION: &str = include_str!("../../../../config/data_validation.toml");

/// Reintento de una vela que no llegó o no validó.
const RETRY: TimeDelta = TimeDelta::seconds(60);
/// Hasta dónde se busca la próxima vela (cubre un fin de semana largo).
const HORIZON: TimeDelta = TimeDelta::days(5);

type Sink = Option<(PgStore, CandlePublisher)>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut once, mut dry_run) = (false, false);
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--once" => once = true,
            "--dry-run" => dry_run = true,
            other => {
                return Err(
                    format!("opción desconocida: {other} (se aceptan --once y --dry-run)").into(),
                );
            }
        }
    }
    let cfg = IgConfig::from_toml(CONFIG)?;
    let catalog = Catalog::from_toml(CATALOG)?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let validation = ValidationConfig::from_toml(VALIDATION)?;
    let live = cfg.live.clone();
    let share = cfg.budget;
    let data_machine = is_data_machine(std::env::var(DATA_MACHINE_ENV).ok().as_deref());
    let usage = usage_of("ig_live", data_machine);
    let delay = TimeDelta::from_std(live.candle_delay)?;
    let give_up = TimeDelta::from_std(live.give_up_after)?;
    let lookback = TimeDelta::from_std(live.startup_lookback)?;

    let mut sink: Sink = match (std::env::var("DATABASE_URL"), std::env::var("REDIS_URL")) {
        (Ok(db), Ok(redis)) => {
            let store = PgStore::connect(&db).await?;
            store.migrate().await?;
            for spec in catalog.instruments.values() {
                store.upsert_instrument(&spec.instrument).await?;
            }
            println!("modo completo: PostgreSQL + Redis");
            Some((store, CandlePublisher::connect(&redis).await?))
        }
        _ => {
            println!("corrida en seco: sin DATABASE_URL/REDIS_URL no se guarda nada");
            None
        }
    };

    let epics = epics_from_catalog(&catalog);
    // Retomar desde la base (#124): sin esto, cada `--once` volvería a pedir todo el lookback.
    let mut last: BTreeMap<(InstrumentId, Timeframe), _> = BTreeMap::new();
    if let Some((store, _)) = &sink {
        let now = Utc::now();
        for id in epics.keys() {
            for &tf in &live.timeframes {
                let stored = store.candles(id, tf, now - lookback, now).await?;
                if let Some(c) = stored.last() {
                    last.insert((id.clone(), tf), c.start);
                }
            }
        }
    }
    if dry_run {
        let now = Utc::now();
        let spend = match &sink {
            Some((store, _)) => budget::spend(store, data_machine).await?,
            None => WeekSpend::default(),
        };
        println!("{}", budget::describe(usage, &spend, &share));
        let mut total = 0;
        for id in epics.keys() {
            for &tf in &live.timeframes {
                let prev = last.get(&(id.clone(), tf)).copied();
                if let Some((from, to)) = due_range(&calendar, id, tf, prev, now, delay, lookback) {
                    let points = due_points(from, to, tf);
                    println!("{id} {tf}: pediría [{from}, {to}), {points} puntos");
                    total += points;
                }
            }
        }
        match cfds_ig::budget::check(usage, total, &spend, &share) {
            Ok(()) => println!("total: {total} puntos; entra en el presupuesto"),
            Err(why) => println!("total: {total} puntos; sin --dry-run frenaría: {why}"),
        }
        println!("--dry-run: no se pide nada a IG");
        return Ok(());
    }
    let transport = ReqwestTransport::new(&cfg, api_key_from_env())?;
    let ig = IgProvider::new(transport, cfg, Credentials::from_env()?, epics.clone());
    // Sin base no hay registro: lo pedido en esta corrida es todo lo que se sabe (#167).
    let mut run_spend = WeekSpend::default();

    let result = loop {
        let mut wake = Utc::now() + HORIZON;
        let mut failure = None;
        // Sin poder leer lo gastado no se pide nada: kill switch (CLAUDE.md §5b).
        let mut spend = match &sink {
            Some((store, _)) => match budget::spend(store, data_machine).await {
                Ok(s) => s,
                Err(e) => {
                    break Err(format!(
                        "kill switch: no se pudo leer ig_allowance_log: {e}"
                    ));
                }
            },
            None => run_spend,
        };
        println!("{}", budget::describe(usage, &spend, &share));
        'cycle: for id in epics.keys() {
            for &tf in &live.timeframes {
                let now = Utc::now();
                let key = (id.clone(), tf);
                let prev = last.get(&key).copied();
                if let Some((from, to)) = due_range(&calendar, id, tf, prev, now, delay, lookback) {
                    // Antes de pedir: si no entra en el presupuesto, kill switch (#167).
                    let points = due_points(from, to, tf);
                    if let Err(why) = cfds_ig::budget::check(usage, points, &spend, &share) {
                        failure = Some(format!("kill switch: {id} {tf} [{from}, {to}): {why}"));
                        break 'cycle;
                    }
                    spend.add(usage, points);
                    run_spend.add(usage, points);
                    let req = IngestRequest {
                        instrument: id.clone(),
                        timeframe: tf,
                        from,
                        to,
                    };
                    match run(&ig, &calendar, &validation, &mut sink, &req).await {
                        Ok(msg) => {
                            println!("{id} {tf} [{from}, {to}): {msg}");
                            last.insert(key, to - tf.duration());
                        }
                        Err(e) if overdue(to, now, give_up) => {
                            failure = Some(format!(
                                "kill switch: {id} {tf} sin datos válidos desde {to}: {e}"
                            ));
                        }
                        Err(e) => {
                            println!("{id} {tf} [{from}, {to}): reintento en 60 s ({e})");
                            wake = wake.min(now + RETRY);
                        }
                    }
                }
                if let Some(w) = next_wake(&calendar, id, tf, Utc::now(), delay, HORIZON) {
                    wake = wake.min(w);
                }
            }
        }
        // Registro del cupo de cada ciclo (#140). Si falla, se avisa: no frena el modo en vivo.
        let store = sink.as_ref().map(|(s, _)| s);
        if let Err(e) = allowance::record_and_report(&ig, store, "ig_live").await {
            eprintln!("⚠️  no se pudo registrar el cupo de IG: {e}");
        }
        if let Some(f) = failure {
            break Err(f);
        }
        if once {
            break Ok(());
        }
        let wait = (wake - Utc::now()).to_std().unwrap_or_default();
        println!("próximo ciclo: {wake} (en {} s)", wait.as_secs());
        tokio::time::sleep(wait).await;
    };
    ig.logout().await?;
    result.map_err(Into::into)
}

async fn run(
    ig: &IgProvider<ReqwestTransport>,
    calendar: &ExchangeCalendar,
    validation: &ValidationConfig,
    sink: &mut Sink,
    req: &IngestRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let recent = Recent(ig);
    let now = Utc::now();
    match sink {
        Some((store, publisher)) => {
            let r = ingest(
                &recent,
                calendar,
                validation,
                store,
                Some(publisher),
                req,
                now,
            )
            .await?;
            Ok(format!(
                "{} nuevas, {} publicadas en Redis",
                r.inserted, r.published
            ))
        }
        None => {
            let candles = recent
                .history(&req.instrument, req.timeframe, req.from, req.to)
                .await?;
            let (ready, open) = prepare(req, candles, calendar, validation, now)?;
            let closes: Vec<String> = ready.iter().map(|c| c.close.to_string()).collect();
            Ok(format!(
                "{} velas válidas ({} abiertas descartadas), cierres {}",
                ready.len(),
                open,
                closes.join(" ")
            ))
        }
    }
}

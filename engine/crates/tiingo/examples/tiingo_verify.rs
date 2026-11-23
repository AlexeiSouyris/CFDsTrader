//! Verificación transitoria de nuestras velas contra un ETF de Tiingo (ADR 0009, opción B; #206).
//!
//! ```text
//! cargo run -q -p cfds-tiingo --example tiingo_verify -- 2026-10-01 2026-10-31
//! cargo run -q -p cfds-tiingo --example tiingo_verify -- 2026-10-01 2026-10-31 --instrument US500
//! cargo run -q -p cfds-tiingo --example tiingo_verify -- 2026-10-01 2026-10-31 --dry-run
//! cargo run -q -p cfds-tiingo --example tiingo_verify -- 2026-10-01 2026-10-31 --calibrate
//! ```
//!
//! - Solo en la máquina que guarda los datos (`CFDS_DATA_MACHINE=1`): el registro de uso vive en su
//!   base, y solo ahí se puede contar el cupo. En otra máquina se niega antes de pedir nada.
//! - Un pedido por ETF (SPY, QQQ, DIA según `config/tiingo.toml`), con los topes controlados antes.
//! - Compara en memoria y guarda **solo el veredicto** de cada día en `tiingo_verifications`. En la
//!   salida muestra el veredicto y los conteos: ningún precio ni número de Tiingo.
//! - `--dry-run`: compara y muestra, pero no guarda veredictos (el uso sí se registra: el pedido se
//!   hizo).
//! - `--calibrate`: como `--dry-run`, y además muestra cuántos días coinciden con cada tolerancia
//!   candidata (solo conteos), para calibrar las tolerancias de `[verify]`.
//! - Cualquier error es kill switch: corta sin reintentar y sale con código 1.
//!
//! Necesita `DATABASE_URL` y `TIINGO_API_KEY` (en el `.env`).

use std::collections::BTreeMap;

use cfds_core::{InstrumentId, Timeframe};
use cfds_storage::PgStore;
use cfds_tiingo::{
    ApiKey, Client, DayVerdict, Method, ReqwestTransport, TiingoConfig, TiingoStore, Verdict,
    compare, our_closes,
};
use chrono::{NaiveDate, TimeDelta, Utc};
use rust_decimal::Decimal;

const CONFIG: &str = "../config/tiingo.toml";
const CANDIDATES: [&str; 9] = [
    "0.02", "0.05", "0.10", "0.15", "0.25", "0.50", "0.75", "1.00", "1.50",
];

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("❌ {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut positional = Vec::new();
    let (mut dry_run, mut calibrate, mut only) = (false, false, None::<InstrumentId>);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dry-run" => dry_run = true,
            "--calibrate" => (dry_run, calibrate) = (true, true),
            "--instrument" => {
                only = Some(args.next().ok_or("--instrument necesita un id")?.parse()?)
            }
            s if s.starts_with("--") => return Err(format!("opción desconocida: {s}").into()),
            s => positional.push(s.to_owned()),
        }
    }
    let [from, to] = positional.as_slice() else {
        return Err("uso: tiingo_verify <desde AAAA-MM-DD> <hasta AAAA-MM-DD> [--instrument ID] [--dry-run] [--calibrate]".into());
    };
    let from: NaiveDate = from.parse()?;
    let to: NaiveDate = to.parse()?;

    // Antes de cualquier otra cosa: solo en la máquina que guarda los datos.
    if !cfds_ig::is_data_machine(std::env::var(cfds_ig::DATA_MACHINE_ENV).ok().as_deref()) {
        return Err(format!(
            "Tiingo solo se usa en la máquina que guarda los datos ({}=1): acá no se cuenta el cupo",
            cfds_ig::DATA_MACHINE_ENV
        )
        .into());
    }
    let cfg = TiingoConfig::from_toml(&std::fs::read_to_string(CONFIG)?)?;
    let today = Utc::now().date_naive();
    if from >= to || to > today {
        return Err("el rango tiene que ser desde < hasta ≤ hoy".into());
    }
    if (to - from).num_days() > i64::from(cfg.verify.max_days) {
        return Err(format!("el rango pasa de verify.max_days ({})", cfg.verify.max_days).into());
    }
    let key = ApiKey::from_env()?;
    let url = std::env::var("DATABASE_URL").map_err(|_| "falta DATABASE_URL")?;
    let store = TiingoStore::connect(&url).await?;
    let pg = PgStore::connect(&url).await?;
    let transport = ReqwestTransport::new(cfg.timeout_secs)?;
    let client = Client::new(cfg.clone(), key, transport);

    let tz = cfg.verify.timezone;
    let start_utc = |d: NaiveDate| {
        d.and_hms_opt(0, 0, 0)
            .and_then(|t| t.and_local_timezone(tz).earliest())
            .map(|t| t.with_timezone(&Utc))
            .ok_or("fecha local inválida")
    };
    for (instrument, etf) in &cfg.pairs {
        if only.as_ref().is_some_and(|o| o != instrument) {
            continue;
        }
        let m15 = pg
            .candles(
                instrument,
                Timeframe::M15,
                start_utc(from)?,
                start_utc(to + TimeDelta::days(1))?,
            )
            .await?;
        let daily = pg
            .daily_candles(instrument, from, to + TimeDelta::days(1))
            .await?;
        let ours = our_closes(&m15, &daily, &cfg.verify);
        let now = Utc::now();
        let rows = client.daily_prices(&store, etf, from, to, now).await?;
        let verdicts = compare(
            &rows,
            &ours,
            cfg.verify.tolerance_m15_pp,
            cfg.verify.tolerance_daily_pp,
        );
        println!(
            "\n{instrument} contra {etf}: {} días del ETF, {} veredictos",
            rows.len(),
            verdicts.len()
        );
        for v in &verdicts {
            println!("  {} {:<11} {}", v.day, v.method.as_str(), label(v.verdict));
        }
        println!("  {}", summary(&verdicts));
        if calibrate {
            println!("  calibración (días que coinciden / días comparados, por tolerancia en pp):");
            for c in CANDIDATES {
                let t: Decimal = c.parse()?;
                let v = compare(&rows, &ours, t, t);
                let count = |m: Method| {
                    let of = v.iter().filter(|d| d.method == m).count();
                    let ok = v
                        .iter()
                        .filter(|d| d.method == m && d.verdict == Verdict::Match)
                        .count();
                    format!("{ok}/{of}")
                };
                println!(
                    "    {c:>5}  m15_close {:>7}  daily_close {:>7}",
                    count(Method::M15Close),
                    count(Method::DailyClose)
                );
            }
        }
        if dry_run {
            println!("  (sin guardar: --dry-run)");
        } else {
            store
                .record_verdicts(instrument, etf, now, &verdicts)
                .await?;
            println!("  veredictos guardados en tiingo_verifications");
        }
        // `rows` se descarta acá: nada de Tiingo sale de la memoria.
    }
    let usage = store.usage(Utc::now()).await?;
    println!(
        "\nUso de Tiingo: {}/{} pedidos en 60 min, {}/{} en 24 h, {}/{} símbolos y {}/{} bytes en 31 días",
        usage.requests_last_hour,
        cfg.budget.requests_per_hour,
        usage.requests_last_day,
        cfg.budget.requests_per_day,
        usage.symbols_last_31_days.len(),
        cfg.budget.symbols_per_31_days,
        usage.bytes_last_31_days,
        cfg.budget.bytes_per_31_days
    );
    Ok(())
}

fn label(v: Verdict) -> &'static str {
    match v {
        Verdict::Match => "coincide",
        Verdict::Mismatch => "NO coincide",
        Verdict::NoData => "sin dato",
    }
}

fn summary(v: &[DayVerdict]) -> String {
    let mut n: BTreeMap<&str, usize> = BTreeMap::new();
    for d in v {
        *n.entry(label(d.verdict)).or_default() += 1;
    }
    n.iter()
        .map(|(k, c)| format!("{c} {k}"))
        .collect::<Vec<_>>()
        .join(", ")
}

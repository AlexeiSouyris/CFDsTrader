//! Propuestas por mes de la estrategia sobre los futuros (spec de ML v1 §10 y decisión 4, #390).
//! **Solo lectura y en memoria**: no escribe nada en la base y no calcula P/L.
//!
//! ```text
//! DATABASE_URL=... cargo run --release -p cfds-backtest --example propuestas_por_mes -- \
//!     [INSTRUMENTO] [desde UTC] [hasta UTC]
//! ```
//! Por omisión, US500 y todo lo bajado de su futuro (`futures_bar_loads`).
//!
//! - Arma las velas de 15m y 1h **en memoria** desde las barras de 1 minuto de Databento, mes por
//!   mes, con la misma función que el ejemplo `futures_candles` de `cfds-ingest`
//!   (`cfds_market_data::futures_candles`): no hace falta haberlas guardado.
//! - Prepara la serie igual que `backtest_run` con la fuente de futuros: validación (§7.8 y §12),
//!   horario, vencimientos y filtro de noticias de alto impacto (§7.2). Sin filtro de spread (no
//!   hay spread medido del CFD) y sin ajuste de noticias del puntaje.
//! - Cuenta las propuestas con `cfds_backtest::proposals` (ver ese módulo): misma decisión y
//!   mismo riesgo que el backtest, etiqueta `y` de la spec de ML (TP1 antes que el stop), con la
//!   cuenta siempre en su capital inicial (sin kill switch ni pérdida diaria).
//! - Imprime tablas en Markdown: por año, por mes, por dirección, por gatillos, por motivo, los
//!   rechazos del riesgo, las ventanas walk-forward de `config/data_split.toml` y cuántos meses
//!   de entrenamiento hacen falta para el mínimo de `config/ml.toml`.
//!
//! El tramo fuera de muestra (los últimos `out_of_sample.months`) se cuenta aparte y no entra en
//! las ventanas de entrenamiento.

#[path = "common/news_filter.rs"]
mod news_filter;

use std::collections::BTreeMap;
use std::path::PathBuf;

use cfds_backtest::{
    BacktestConfig, ExitReason, Label, Overlap, Proposal, ProposalRules, Tally, mark_rollovers,
    proposals, tally_by,
};
use cfds_core::{Candle, InstrumentId};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{
    CandleSources, Catalog, ExchangeCalendar, FuturesFeeds, NewsFilterConfig, SplitConfig,
    TradingWindowConfig, ValidationConfig, check_series, decision_flags, futures_candles, plan,
    trading_window,
};
use cfds_patterns::PatternsConfig;
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_storage::PgStore;
use cfds_strategy::{Configs, Side, StrategyConfig, TriggerKind};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

const SOURCE: &str = "databento";
/// Capital de la cuenta para el tamaño (solo decide los rechazos por tamaño mínimo o margen).
const EQUITY: Decimal = Decimal::from_parts(10000, 0, 0, false, 0);

fn month_start(t: DateTime<Utc>) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(t.year(), t.month(), 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap_or_default(),
    )
}

fn next_month(t: DateTime<Utc>) -> DateTime<Utc> {
    let (y, m) = if t.month() == 12 {
        (t.year() + 1, 1)
    } else {
        (t.year(), t.month() + 1)
    };
    Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(y, m, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .unwrap_or_default(),
    )
}

fn month_index(y: i32, m: u32) -> i32 {
    y * 12 + m as i32 - 1
}

fn trigger_name(t: TriggerKind) -> &'static str {
    match t {
        TriggerKind::CandlePattern => "patrón de velas",
        TriggerKind::DoubleBreak => "doble techo/suelo",
        TriggerKind::LevelRejection => "rechazo en nivel",
        TriggerKind::Sweep => "sweep",
    }
}

fn label_name(l: Label) -> &'static str {
    match l {
        Label::Tp1 => "TP1 (y = 1)",
        Label::Miss(ExitReason::Stop) => "stop (y = 0)",
        Label::Miss(ExitReason::Time) => "salida por tiempo (y = 0)",
        Label::Miss(ExitReason::Forced) => "cierre forzado (y = 0)",
        Label::Miss(_) => "otra salida (y = 0)",
        Label::Unresolved => "sin resolver",
    }
}

fn pct(a: usize, b: usize) -> String {
    if b == 0 {
        "—".to_owned()
    } else {
        format!(
            "{:.1} %",
            Decimal::from(a) * Decimal::ONE_HUNDRED / Decimal::from(b)
        )
    }
}

fn tally_table(title: &str, first: &str, rows: &BTreeMap<String, Tally>) {
    println!("\n### {title}\n");
    println!("| {first} | Propuestas | y = 1 | y = 0 | Sin resolver | % y = 1 |");
    println!("|---|---|---|---|---|---|");
    let mut rows: Vec<_> = rows.iter().collect();
    rows.sort_by_key(|(_, t)| std::cmp::Reverse(t.total));
    for (k, t) in rows {
        println!(
            "| {k} | {} | {} | {} | {} | {} |",
            t.total,
            t.y1,
            t.y0,
            t.unresolved,
            pct(t.y1, t.y1 + t.y0)
        );
    }
}

/// Velas de 15m y 1h del futuro de `id`, armadas en memoria desde las barras de 1 minuto. Quedan
/// bajo `id`, igual que en `backtest_run`: el calendario y la sesión son los del CFD.
async fn candles(
    store: &PgStore,
    id: &InstrumentId,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
) -> Result<(Vec<Candle>, Vec<Candle>), Box<dyn std::error::Error>> {
    let feeds = FuturesFeeds::from_toml(cfg!("databento.toml"))?;
    let feed = feeds.get(id).ok_or("sin futuro en config/databento.toml")?;
    let (loaded_from, loaded_to) = store
        .futures_loaded_range(SOURCE, &feed.dataset, &feed.symbol, id)
        .await?
        .ok_or("sin barras bajadas")?;
    let (from, until) = (from.max(loaded_from), to.min(loaded_to).min(Utc::now()));
    eprintln!(
        "{} ({}): bajado del {loaded_from} al {loaded_to}; se usa del {from} al {until}",
        feed.symbol, feed.dataset
    );
    let (mut m15, mut h1, mut mixed) = (Vec::new(), Vec::new(), 0);
    let mut month = month_start(from);
    while month < until {
        let end = next_month(month).min(until);
        let bars = store
            .futures_bars(
                SOURCE,
                &feed.dataset,
                &feed.symbol,
                id,
                month.max(from),
                end,
            )
            .await?;
        if !bars.is_empty() {
            let f = futures_candles(&bars, id, end)?;
            mixed += f.mixed.len();
            m15.extend(f.m15.into_iter().map(|c| c.candle));
            h1.extend(f.h1.into_iter().map(|c| c.candle));
        }
        month = next_month(month);
    }
    eprintln!(
        "{} velas de 15m y {} de 1h; {mixed} ventanas no se armaron por un vencimiento",
        m15.len(),
        h1.len()
    );
    Ok((m15, h1))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let id: InstrumentId = args.first().map_or("US500", String::as_str).parse()?;
    let from: DateTime<Utc> = match args.get(1) {
        Some(s) => s.parse()?,
        None => "2000-01-01T00:00:00Z".parse()?,
    };
    let to: DateTime<Utc> = match args.get(2) {
        Some(s) => s.parse()?,
        None => Utc::now(),
    };

    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let spec = catalog
        .instruments
        .get(&id)
        .ok_or("instrumento fuera del catálogo")?;
    if catalog.account_currency.value() != Some(&spec.instrument.currency) {
        return Err(
            "la moneda del instrumento no es la de la cuenta: falta el tipo de cambio".into(),
        );
    }
    let limits = InstrumentLimits::from_spec(spec).ok_or("el instrumento no tiene margen")?;
    let rules = ProposalRules {
        instrument: id.clone(),
        strategy: Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml"))?,
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml"))?,
            trend: TrendConfig::from_toml(cfg!("indicators.toml"))?,
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml"))?,
        },
        risk: RiskConfig::from_toml(cfg!("risk.toml"))?,
        limits,
        exits: BacktestConfig::from_toml(cfg!("backtest.toml"))?.exits,
        equity: EQUITY,
        account_to_instrument: Decimal::ONE,
    };
    let sources = CandleSources::from_toml(cfg!("candle_sources.toml"))?;
    let out = sources.instrument_for(cfds_market_data::CandleSource::Futures, &id)?;

    // Solo lectura: la sesión de PostgreSQL no puede escribir.
    let url = std::env::var("DATABASE_URL")?;
    let sep = if url.contains('?') { '&' } else { '?' };
    let store = PgStore::connect(&format!(
        "{url}{sep}options=-c%20default_transaction_read_only%3Don"
    ))
    .await?;

    let (raw_m15, raw_h1) = candles(&store, &id, (from, to)).await?;
    let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    let validation = ValidationConfig::from_toml(cfg!("data_validation.toml"))?;
    let a = check_series(&raw_m15, &calendar, &validation);
    let b = check_series(&raw_h1, &calendar, &validation);
    let suspicious = a.issues.len() + b.issues.len();
    {
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for i in a.issues.iter().chain(&b.issues) {
            let k = format!("{:?}", i.kind);
            let k = k
                .split([' ', '{', '('])
                .next()
                .unwrap_or_default()
                .to_owned();
            *kinds.entry(k).or_insert(0) += 1;
        }
        eprintln!("datos sospechosos por tipo: {kinds:?}");
    }
    let decisions = decision_flags(&a.candles, &[&a, &b]);
    let mut data_errors = [a.data_errors, b.data_errors].concat();
    data_errors.sort();
    let (m15, h1) = (a.candles, b.candles);
    let session = calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml"))?;
    let (mut entries, mut force_close) = trading_window::flags(session, &m15, &window)
        .ok_or("fechas fuera del calendario de feriados (config/holidays)")?;
    for (e, d) in entries.iter_mut().zip(&decisions) {
        *e &= *d;
    }
    let rolls = sources.roll_instants(&id, session, &m15)?;
    mark_rollovers(
        &m15,
        &rolls,
        &mut entries,
        &mut force_close,
        &mut data_errors,
    );
    eprintln!(
        "velas donde se puede entrar (horario y datos): {} de {}",
        entries.iter().filter(|e| **e).count(),
        entries.len()
    );
    let news_cfg = NewsFilterConfig::from_toml(cfg!("news_filter.toml"))?;
    let entries = news_filter::apply(&store, &m15, &entries, &news_cfg).await?;

    let one = proposals(
        &m15,
        &h1,
        &entries,
        &force_close,
        &data_errors,
        &rules,
        Overlap::OneAtATime,
    )
    .map_err(|e| format!("{e:?}"))?;
    let every = proposals(
        &m15,
        &h1,
        &entries,
        &force_close,
        &data_errors,
        &rules,
        Overlap::EveryCandle,
    )
    .map_err(|e| format!("{e:?}"))?;

    let (Some(first), Some(last)) = (m15.first(), m15.last()) else {
        return Err("sin velas".into());
    };
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let ml: toml::Table = toml::from_str(cfg!("ml.toml"))?;
    let min_total = ml["training"]["min_examples"]
        .as_integer()
        .ok_or("ml.toml sin min_examples")? as usize;
    let min_class = ml["training"]["min_per_class"]
        .as_integer()
        .ok_or("ml.toml sin min_per_class")? as usize;
    // Con menos datos que una ventana, no hay walk-forward: todo cuenta como investigación.
    let split = plan(first.start, last.start, &split_cfg).ok();
    let oos_start = split.as_ref().map_or(last.end(), |s| s.out_of_sample.start);
    let windows = split.as_ref().map_or(&[][..], |s| &s.windows[..]);
    let start_of = |p: &Proposal| m15[p.index].start;
    let (research, oos): (Vec<Proposal>, Vec<Proposal>) = one
        .proposals
        .iter()
        .cloned()
        .partition(|p| p.resolved_at <= oos_start);

    println!("# Propuestas por mes: {id} con velas de futuros ({out})\n");
    println!(
        "- Velas de 15m: {} (del {} al {}); de 1h: {}. Datos sospechosos: {suspicious}; \
         errores de datos: {}; vencimientos: {}.",
        m15.len(),
        first.start,
        last.end(),
        h1.len(),
        data_errors.len(),
        rolls.len()
    );
    let all = tally_by(&one.proposals, |_| ())
        .remove(&())
        .unwrap_or_default();
    println!(
        "- **Una posición a la vez (como el backtest):** {} propuestas; y = 1: {}, y = 0: {}, \
         sin resolver: {} ({} de y = 1).",
        all.total,
        all.y1,
        all.y0,
        all.unresolved,
        pct(all.y1, all.y1 + all.y0)
    );
    let ev = tally_by(&every.proposals, |_| ())
        .remove(&())
        .unwrap_or_default();
    println!(
        "- **Toda vela con señal (se superponen):** {} propuestas; y = 1: {}, y = 0: {}.",
        ev.total, ev.y1, ev.y0
    );
    println!(
        "- Fuera de muestra (desde {oos_start}): {} propuestas, solo contadas.",
        oos.len()
    );

    // Por año.
    let by_year = tally_by(&one.proposals, |p| start_of(p).year());
    let by_year_side = tally_by(&one.proposals, |p| {
        (start_of(p).year(), p.side == Side::Long)
    });
    let by_year_every = tally_by(&every.proposals, |p| start_of(p).year());
    println!("\n### Por año (una posición a la vez)\n");
    println!(
        "| Año | Propuestas | Largos | Cortos | y = 1 | y = 0 | Sin resolver | % y = 1 | \
         Por mes | Toda vela con señal |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for (y, t) in &by_year {
        let months = (1..=12)
            .filter(|m| {
                let s = Utc.with_ymd_and_hms(*y, *m, 1, 0, 0, 0).single();
                s.is_some_and(|s| s >= month_start(first.start) && s <= last.start)
            })
            .count()
            .max(1);
        let side = |s| {
            by_year_side
                .get(&(*y, s == Side::Long))
                .map_or(0, |t: &Tally| t.total)
        };
        println!(
            "| {y} | {} | {} | {} | {} | {} | {} | {} | {:.1} | {} |",
            t.total,
            side(Side::Long),
            side(Side::Short),
            t.y1,
            t.y0,
            t.unresolved,
            pct(t.y1, t.y1 + t.y0),
            (Decimal::from(t.total) / Decimal::from(months)).round_dp(1),
            by_year_every.get(y).map_or(0, |t| t.total)
        );
    }

    // Por mes: año × mes.
    let by_month = tally_by(&one.proposals, |p| {
        let s = start_of(p);
        (s.year(), s.month())
    });
    println!("\n### Por mes (una posición a la vez; propuestas / y = 1)\n");
    println!("| Año | ene | feb | mar | abr | may | jun | jul | ago | sep | oct | nov | dic |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for y in by_year.keys() {
        let cells: Vec<String> = (1..=12)
            .map(|m| {
                by_month
                    .get(&(*y, m))
                    .map_or("—".to_owned(), |t| format!("{}/{}", t.total, t.y1))
            })
            .collect();
        println!("| {y} | {} |", cells.join(" | "));
    }

    let side_name = |s: Side| match s {
        Side::Long => "largo",
        Side::Short => "corto",
    };
    let named = |f: &dyn Fn(&Proposal) -> String| tally_by(&one.proposals, f);
    tally_table(
        "Por dirección",
        "Dirección",
        &named(&|p| side_name(p.side).to_owned()),
    );
    tally_table(
        "Por gatillos",
        "Gatillos",
        &named(&|p| {
            p.triggers
                .iter()
                .map(|t| trigger_name(*t))
                .collect::<Vec<_>>()
                .join(" + ")
        }),
    );
    tally_table("Por puntaje", "Puntaje", &named(&|p| p.score.to_string()));
    tally_table(
        "Por motivo",
        "Motivo",
        &named(&|p| label_name(p.label).to_owned()),
    );

    println!("\n### Rechazos del riesgo (señales candidatas que no pasan)\n");
    println!("| Motivo | Una posición a la vez | Toda vela con señal |");
    println!("|---|---|---|");
    for (k, n) in &every.rejected {
        println!(
            "| {k} | {} | {n} |",
            one.rejected.get(k).copied().unwrap_or(0)
        );
    }

    // Ventanas walk-forward: propuestas de entrenamiento con el resultado conocido antes del
    // fin del entrenamiento (purga de la spec de ML §5).
    println!(
        "\n### Ventanas walk-forward ({} meses de entrenamiento; mínimo {min_total} y {min_class} \
         por clase)\n",
        split_cfg.train_months
    );
    println!("| Entrenamiento | Propuestas | y = 1 | y = 0 | ¿Alcanza? |");
    println!("|---|---|---|---|---|");
    let mut reached = 0;
    for w in windows {
        let mut t = Tally::default();
        for p in research
            .iter()
            .filter(|p| start_of(p) >= w.train.start && p.resolved_at <= w.train.end)
        {
            t.add(p.label);
        }
        let ok = t.meets(min_total, min_class);
        reached += usize::from(ok);
        println!(
            "| {} a {} | {} | {} | {} | {} |",
            w.train.start.date_naive(),
            w.train.end.date_naive(),
            t.y1 + t.y0,
            t.y1,
            t.y0,
            if ok { "sí" } else { "no" }
        );
    }
    println!(
        "\n{reached} de {} ventanas alcanzan el mínimo.",
        windows.len()
    );

    // Cuántos meses de entrenamiento harían falta: ventanas de `n` meses que empiezan en cada
    // mes completo antes del fuera de muestra.
    let research_months: BTreeMap<i32, Tally> = {
        let mut m = BTreeMap::new();
        for p in &research {
            let s = start_of(p);
            m.entry(month_index(s.year(), s.month()))
                .or_insert_with(Tally::default)
                .add(p.label);
        }
        m
    };
    let first_m = month_index(first.start.year(), first.start.month())
        + i32::from(first.start != month_start(first.start));
    let end_m = month_index(oos_start.year(), oos_start.month());
    println!("\n### Meses de entrenamiento necesarios (antes del fuera de muestra)\n");
    println!("| Meses | Ventanas | Alcanzan | Mediana de propuestas | Mínimo de y = 1 |");
    println!("|---|---|---|---|---|");
    for n in [6, 12, 18, 24, 36, 48, 60, 72, 96, 120] {
        let mut sums: Vec<Tally> = Vec::new();
        let mut s = first_m;
        while s + n <= end_m {
            let mut t = Tally::default();
            for (_, m) in research_months.range(s..s + n) {
                t.total += m.total;
                t.y1 += m.y1;
                t.y0 += m.y0;
                t.unresolved += m.unresolved;
            }
            sums.push(t);
            s += 1;
        }
        if sums.is_empty() {
            continue;
        }
        let ok = sums
            .iter()
            .filter(|t| t.meets(min_total, min_class))
            .count();
        let mut totals: Vec<usize> = sums.iter().map(|t| t.y1 + t.y0).collect();
        totals.sort_unstable();
        println!(
            "| {n} | {} | {ok} | {} | {} |",
            sums.len(),
            totals[totals.len() / 2],
            sums.iter().map(|t| t.y1).min().unwrap_or(0)
        );
    }
    Ok(())
}

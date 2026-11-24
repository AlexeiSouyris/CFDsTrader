//! Diagnóstico de una corrida (solo lectura, #323): por qué gana o pierde la estrategia en un
//! instrumento. Corre el mismo backtest que `backtest_run` (misma preparación,
//! `common/prepare.rs`) **en memoria**, sin guardar nada, e imprime tablas en Markdown con
//! agregados (nunca precios sueltos).
//!
//! ```text
//! DATABASE_URL=... cargo run --release -p cfds-backtest --example diagnostico -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> <capital inicial> [--fuente ig|futuros]
//! ```
//! Cortes: hora de entrada (UTC), gatillos, dirección, tercio de la volatilidad (el oficial de
//! `daily_vol_terciles` y, si no hay, terciles del ATR de 15m dentro de la muestra, solo
//! descriptivo), día de la semana, motivo de salida, distancia del stop en ATR, MAE y MFE en R,
//! costos por tipo y rechazos del riesgo. Las tasas llevan su IC de Wilson (z de
//! `config/probability.toml`) y se marcan las muestras chicas. No cambia ningún parámetro.

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;

use std::collections::BTreeMap;

use cfds_backtest::{CloseReason, ExitReason, TradeRecord, run_with_news};
use cfds_core::Candle;
use cfds_indicators::{wilder_atr, wilson};
use cfds_market_data::CandleSource;
use cfds_strategy::{Side, TriggerKind};
use chrono::{Datelike, Timelike};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// Muestras con menos operaciones que esto se marcan como chicas.
const SMALL: usize = 30;

/// Lo que se agrega de cada operación, en la moneda del instrumento.
struct Row {
    net: Decimal,
    /// Neto en R (R = tamaño × distancia del stop desde la entrada ejecutada).
    r: Decimal,
}

fn z() -> Decimal {
    let t: toml::Table = toml::from_str(cfg!("probability.toml")).unwrap_or_default();
    t.get("z")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(Decimal::new(196, 2))
}

fn pct(d: Decimal) -> String {
    format!("{:.1} %", d * Decimal::ONE_HUNDRED)
}

fn table(title: &str, groups: &BTreeMap<String, Vec<Row>>, z: Decimal) {
    println!("\n### {title}\n");
    println!(
        "| Grupo | Op. | Gana | IC 95 % | Neto total | Neto medio | R medio | Payoff | |\n|---|---|---|---|---|---|---|---|---|"
    );
    for (k, rows) in groups {
        let n = rows.len();
        let wins: Vec<&Row> = rows.iter().filter(|r| r.net > Decimal::ZERO).collect();
        let losses: Vec<&Row> = rows.iter().filter(|r| r.net < Decimal::ZERO).collect();
        let total: Decimal = rows.iter().map(|r| r.net).sum();
        let mean = total / Decimal::from(n.max(1));
        let r_mean = rows.iter().map(|r| r.r).sum::<Decimal>() / Decimal::from(n.max(1));
        let (lo, hi) = wilson(
            u32::try_from(wins.len()).unwrap_or(u32::MAX),
            u32::try_from(n).unwrap_or(u32::MAX).max(1),
            z,
        );
        let avg = |v: &[&Row]| v.iter().map(|r| r.net).sum::<Decimal>() / Decimal::from(v.len());
        let payoff = if wins.is_empty() || losses.is_empty() {
            "—".to_owned()
        } else {
            format!("{:.2}", avg(&wins) / avg(&losses).abs())
        };
        println!(
            "| {k} | {n} | {} | {}–{} | {:.0} | {:.1} | {:+.2} | {payoff} | {} |",
            pct(Decimal::from(wins.len()) / Decimal::from(n.max(1))),
            pct(lo),
            pct(hi),
            total,
            mean,
            r_mean,
            if n < SMALL { "muestra chica" } else { "" }
        );
    }
}

fn quantiles(mut v: Vec<Decimal>) -> String {
    if v.is_empty() {
        return "sin datos".into();
    }
    v.sort();
    let q = |p: usize| v[(v.len() - 1) * p / 100];
    format!(
        "p10 {:.2} · p25 {:.2} · mediana {:.2} · p75 {:.2} · p90 {:.2} (n = {})",
        q(10),
        q(25),
        q(50),
        q(75),
        q(90),
        v.len()
    )
}

fn side_name(s: Side) -> &'static str {
    match s {
        Side::Long => "largo",
        Side::Short => "corto",
    }
}

fn trigger_name(t: TriggerKind) -> &'static str {
    match t {
        TriggerKind::CandlePattern => "patrón de velas",
        TriggerKind::DoubleBreak => "doble techo/suelo",
        TriggerKind::LevelRejection => "rechazo en nivel",
        TriggerKind::Sweep => "sweep",
    }
}

fn exit_name(r: CloseReason) -> &'static str {
    match r {
        CloseReason::Exit(ExitReason::Stop) => "stop",
        CloseReason::Exit(ExitReason::Tp1) => "TP1",
        CloseReason::Exit(ExitReason::BreakEven) => "break-even",
        CloseReason::Exit(ExitReason::Trailing) => "trailing",
        CloseReason::Exit(ExitReason::Time) => "por tiempo",
        CloseReason::Exit(ExitReason::Forced) => "cierre forzado",
        CloseReason::EndOfData => "fin de datos",
    }
}

/// MAE y MFE en R: el peor y el mejor recorrido entre la vela de entrada y la de la última
/// salida (incluidas: cota conservadora, con velas de 15m no se sabe el orden dentro de la vela).
fn mae_mfe(t: &TradeRecord, m15: &[Candle]) -> Option<(Decimal, Decimal)> {
    let last = t.exits.last()?.index;
    let span = m15.get(t.entry_index..=last)?;
    let dist = (t.entry_price - t.stop).abs();
    if dist.is_zero() {
        return None;
    }
    let hi = span.iter().map(|c| c.high).max()?;
    let lo = span.iter().map(|c| c.low).min()?;
    Some(match t.side {
        Side::Long => ((t.entry_price - lo) / dist, (hi - t.entry_price) / dist),
        Side::Short => ((hi - t.entry_price) / dist, (t.entry_price - lo) / dist),
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let (source, all) = match all.iter().position(|a| a == "--fuente") {
        None => (CandleSource::Ig, all),
        Some(i) => {
            let s = all
                .get(i + 1)
                .and_then(|s| CandleSource::parse(s))
                .ok_or("--fuente ig|futuros")?;
            (s, [&all[..i], all.get(i + 2..).unwrap_or(&[])].concat())
        }
    };
    let [instrument, from, to, equity] = all.as_slice() else {
        return Err(
            "uso: <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> [--fuente ig|futuros]".into(),
        );
    };
    let id: cfds_core::InstrumentId = instrument.parse()?;
    let (from, to) = (from.parse()?, to.parse()?);
    let equity: Decimal = equity.parse()?;
    let p = prepare::prepare(&id, source, (from, to), equity, None, false).await?;
    let s = &p.setup;
    let report = run_with_news(
        &p.m15,
        &p.h1,
        &p.entries,
        &p.force_close,
        &p.data_errors,
        &p.news,
        p.published_from,
        s,
    )
    .map_err(|e| format!("{e:?}"))?;
    let z = z();
    let atr = wilder_atr(&p.m15, s.strategy.patterns.candles.atr_period);
    let trades = &report.trades;
    println!(
        "## Corrida\n\n{} ({}), {} a {}: {} operaciones, capital {} → {:.0}, {} eventos del kill \
         switch, {} señales rechazadas.",
        p.out,
        p.sources.label(source),
        from.date_naive(),
        to.date_naive(),
        trades.len(),
        s.initial_equity,
        report.final_equity,
        report.kill_switch_events.len(),
        report.rejected.len()
    );

    let row = |t: &TradeRecord| {
        let risk = t.size * (t.entry_price - t.stop).abs();
        Row {
            // El recorrido ya convirtió todo: si terminó, hay tipo al cierre de cada operación.
            net: s
                .to_account(t.net_pnl, t.closed_at())
                .unwrap_or(Decimal::ZERO),
            r: if risk.is_zero() {
                Decimal::ZERO
            } else {
                t.net_pnl / risk
            },
        }
    };
    let by = |key: &dyn Fn(&TradeRecord) -> String| {
        let mut g: BTreeMap<String, Vec<Row>> = BTreeMap::new();
        for t in trades {
            g.entry(key(t)).or_default().push(row(t));
        }
        g
    };

    println!("\n## Cortes del P/L neto (moneda de la cuenta)");
    table("Todas", &by(&|_| "todas".into()), z);
    table(
        "Hora de entrada (UTC)",
        &by(&|t| format!("{:02}:00", t.entry_time.hour())),
        z,
    );
    table(
        "Gatillos de la señal",
        &by(&|t| {
            let mut k: Vec<&str> = t.triggers.iter().map(|g| trigger_name(*g)).collect();
            k.sort();
            k.dedup();
            k.join(" + ")
        }),
        z,
    );
    table("Dirección", &by(&|t| side_name(t.side).into()), z);
    table(
        "Día de la semana (UTC)",
        &by(&|t| {
            let d = t.entry_time.weekday();
            format!("{} {d}", d.number_from_monday())
        }),
        z,
    );
    table(
        "Motivo de salida (secuencia)",
        &by(&|t| {
            t.exits
                .iter()
                .map(|e| exit_name(e.reason))
                .collect::<Vec<_>>()
                .join(" → ")
        }),
        z,
    );
    let dist = |t: &TradeRecord| {
        atr.get(t.signal_index)
            .copied()
            .flatten()
            .filter(|a| !a.is_zero())
            .map(|a| (t.reference - t.stop).abs() / a)
    };
    table(
        "Distancia del stop (en ATR de 15m al decidir)",
        &by(&|t| match dist(t) {
            None => "sin ATR".into(),
            Some(d) if d < Decimal::ONE => "a. < 1".into(),
            Some(d) if d < Decimal::new(15, 1) => "b. 1–1,5".into(),
            Some(d) if d < Decimal::TWO => "c. 1,5–2".into(),
            Some(d) if d < Decimal::new(25, 1) => "d. 2–2,5".into(),
            Some(_) => "e. 2,5–3".into(),
        }),
        z,
    );

    // Tercio oficial (daily_vol_terciles) del último día publicado hasta la señal.
    let (window, years) = {
        let t: toml::Table = toml::from_str(cfg!("indicators.toml"))?;
        let d = t
            .get("daily")
            .and_then(|v| v.as_table())
            .ok_or("indicators.toml [daily]")?;
        let get = |k: &str| {
            d.get(k)
                .and_then(|v| v.as_integer())
                .and_then(|v| u32::try_from(v).ok())
        };
        (
            get("realized_vol_window").ok_or("realized_vol_window")?,
            get("vol_tercile_years").ok_or("vol_tercile_years")?,
        )
    };
    let terciles = p
        .store
        .daily_vol_terciles(&p.out, window, years, from.date_naive(), to.date_naive())
        .await?;
    let official = |t: &TradeRecord| {
        terciles
            .iter()
            .rev()
            .find(|x| x.published_at <= t.signal_at)
            .and_then(|x| x.tercile)
            .map(|x| x.as_str().to_owned())
    };
    let with_official = trades.iter().filter(|t| official(t).is_some()).count();
    println!(
        "\nTercio oficial de la volatilidad (`daily_vol_terciles`, {years} años de historia): {} de {} operaciones con dato.",
        with_official,
        trades.len()
    );
    if with_official > 0 {
        table(
            "Tercio oficial de la volatilidad",
            &by(&|t| official(t).unwrap_or_else(|| "sin dato".into())),
            z,
        );
    }
    // Sustituto descriptivo: terciles del ATR de 15m al decidir, dentro de la propia muestra.
    let mut atrs: Vec<Decimal> = trades
        .iter()
        .filter_map(|t| atr.get(t.signal_index).copied().flatten())
        .collect();
    atrs.sort();
    if atrs.len() >= 3 {
        let (c1, c2) = (atrs[atrs.len() / 3], atrs[2 * atrs.len() / 3]);
        table(
            "Tercio del ATR de 15m dentro de la muestra (descriptivo: usa toda la muestra, no sirve para decidir)",
            &by(&|t| match atr.get(t.signal_index).copied().flatten() {
                None => "sin ATR".into(),
                Some(a) if a < c1 => "1 bajo".into(),
                Some(a) if a < c2 => "2 medio".into(),
                Some(_) => "3 alto".into(),
            }),
            z,
        );
    }

    println!("\n## MAE y MFE en R (por operación)\n");
    let (mut mae, mut mfe, mut mfe_losers) = (Vec::new(), Vec::new(), Vec::new());
    for t in trades {
        if let Some((a, f)) = mae_mfe(t, &p.m15) {
            mae.push(a);
            mfe.push(f);
            if t.net_pnl < Decimal::ZERO {
                mfe_losers.push(f);
            }
        }
    }
    println!("- MAE: {}", quantiles(mae));
    println!("- MFE: {}", quantiles(mfe));
    println!("- MFE de las perdedoras: {}", quantiles(mfe_losers));
    let tp1_r = s.backtest.exits.tp1_r;
    println!("- (TP1 está a {tp1_r} R)");

    println!("\n## Costos (moneda del instrumento)\n");
    let cfg = &s.backtest.costs;
    let half = s.costs.spread / Decimal::TWO;
    let slip = cfg.slippage_spread_fraction * s.costs.spread + cfg.slippage_ticks * s.costs.tick;
    let (mut spread, mut slippage) = (Decimal::ZERO, Decimal::ZERO);
    for t in trades {
        // Entrada + cada salida parcial.
        let parts: Decimal = t.size + t.exits.iter().map(|e| t.size * e.fraction).sum::<Decimal>();
        spread += parts * half;
        slippage += parts * slip;
    }
    let mid: Decimal = trades.iter().map(|t| t.mid_pnl).sum();
    let gross: Decimal = trades.iter().map(|t| t.gross_pnl).sum();
    let commission: Decimal = trades.iter().map(|t| t.commission).sum();
    let funding: Decimal = trades.iter().map(|t| t.funding).sum();
    let net: Decimal = trades.iter().map(|t| t.net_pnl).sum();
    let check = (mid - gross) - (spread + slippage);
    let costs = spread + slippage + commission + funding;
    let share = |x: Decimal| {
        if mid.is_zero() {
            "—".to_owned()
        } else {
            pct(x / mid.abs())
        }
    };
    println!("| Concepto | Total | Sobre el P/L a precio medio (valor absoluto) |\n|---|---|---|");
    println!("| P/L a precio medio (sin costos) | {mid:.0} | |");
    println!("| Spread | {:.0} | {} |", spread, share(spread));
    println!("| Slippage | {:.0} | {} |", slippage, share(slippage));
    println!("| Comisión | {:.0} | {} |", commission, share(commission));
    println!(
        "| Financiación (swap) | {:.0} | {} |",
        funding,
        share(funding)
    );
    println!("| Costos totales | {:.0} | {} |", costs, share(costs));
    println!("| P/L neto | {net:.0} | |");
    println!(
        "\nControl: spread + slippage = P/L medio − P/L bruto (diferencia {:.6}).",
        check
    );
    let per_trade = costs / Decimal::from(trades.len().max(1));
    let risk_mean = trades
        .iter()
        .map(|t| t.size * (t.entry_price - t.stop).abs())
        .sum::<Decimal>()
        / Decimal::from(trades.len().max(1));
    if !risk_mean.is_zero() {
        println!(
            "Costo medio por operación: {:.2} R (con el riesgo medio por operación como R).",
            per_trade / risk_mean
        );
    }

    println!("\n## Rechazos del riesgo\n");
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut far_hours: BTreeMap<u32, usize> = BTreeMap::new();
    for r in &report.rejected {
        *reasons.entry(format!("{:?}", r.reason)).or_default() += 1;
        if r.reason == cfds_risk::Rejection::StopTooFar
            && let Some(c) = p.m15.get(r.index)
        {
            *far_hours.entry(c.end().hour()).or_default() += 1;
        }
    }
    println!("| Motivo | Señales |\n|---|---|");
    for (k, v) in &reasons {
        println!("| {k} | {v} |");
    }
    let far: usize = far_hours.values().sum();
    if far > 0 {
        println!(
            "\nStopTooFar por hora de la decisión (UTC):\n\n| Hora | Señales | Parte |\n|---|---|---|"
        );
        for (h, n) in &far_hours {
            println!(
                "| {h:02}:00 | {n} | {} |",
                pct(Decimal::from(*n) / Decimal::from(far))
            );
        }
    }
    Ok(())
}

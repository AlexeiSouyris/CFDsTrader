//! Estudio de variantes en backtest (#359), **solo lectura y solo estudio**: no cambia ninguna
//! config ni guarda nada. Corre en memoria cada variante de un archivo TOML (`[[variant]]`, ver
//! `cfds_backtest::variants`) sobre los tramos de investigación e imprime tablas en Markdown.
//!
//! ```text
//! DATABASE_URL=... cargo run --release -p cfds-backtest --example variantes -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <tamaño B&H> <variantes.toml> \
//!     [--fuente ig|futuros] [--sin-noticias]
//! ```
//! Tramos: las ventanas de **prueba** del walk-forward de `config/data_split.toml`. Si los datos
//! no alcanzan para ninguna ventana, bloques de `validation_months` antes del fuera de muestra
//! (`research_blocks`), y lo dice. Nunca toca el fuera de muestra: cada tramo pasa por la misma
//! guardia que el walk-forward. Cada tramo arranca del capital inicial, con el calentamiento de
//! `config/evaluation.toml` y todos los costos (spread, slippage, comisión y financiación).

#[path = "common/news_filter.rs"]
mod news_filter;
#[path = "common/prepare.rs"]
mod prepare;

use cfds_backtest::{
    ConfigTexts, EvaluationConfig, TradeStats, entry_filter, evaluate_period_with, research_blocks,
    setup_for, variants_from_toml,
};
use cfds_indicators::wilson;
use cfds_market_data::{CandleSource, DataSplit, Period, SplitConfig, plan};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn z() -> Decimal {
    let t: toml::Table = toml::from_str(cfg!("probability.toml")).unwrap_or_default();
    t.get("z")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(Decimal::new(196, 2))
}

fn pct(d: Option<Decimal>) -> String {
    d.map_or("—".into(), |d| {
        format!("{:.1} %", d * Decimal::ONE_HUNDRED)
    })
}

fn num(d: Option<Decimal>) -> String {
    d.map_or("—".into(), |d| format!("{d:.2}"))
}

fn ci(s: &TradeStats, z: Decimal) -> String {
    if s.trades == 0 {
        return "—".into();
    }
    let n = u32::try_from(s.trades).unwrap_or(u32::MAX);
    let w = u32::try_from(s.wins).unwrap_or(u32::MAX);
    let (lo, hi) = wilson(w, n, z);
    format!("{}–{}", pct(Some(lo)), pct(Some(hi)))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let without_news = all.iter().any(|a| a == "--sin-noticias");
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
    let args: Vec<&str> = all
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with("--"))
        .collect();
    let [instrument, from, to, equity, size, file] = args.as_slice() else {
        return Err(
            "uso: <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <tamaño B&H> \
                    <variantes.toml> [--fuente ig|futuros] [--sin-noticias]"
                .into(),
        );
    };
    let variants = variants_from_toml(&std::fs::read_to_string(file)?)
        .map_err(|e| format!("{file}: {e:?}"))?;
    let id: cfds_core::InstrumentId = instrument.parse()?;
    let (from, to) = (from.parse()?, to.parse()?);
    let (equity, size): (Decimal, Decimal) = (equity.parse()?, size.parse()?);
    let p = prepare::prepare(&id, source, (from, to), equity, None, without_news).await?;
    let (first, last) = match (p.m15.first(), p.m15.last()) {
        (Some(a), Some(b)) => (a.start, b.start),
        _ => return Err("sin velas en el período".into()),
    };
    let split_cfg = SplitConfig::from_toml(cfg!("data_split.toml"))?;
    let eval =
        EvaluationConfig::from_toml(cfg!("evaluation.toml")).map_err(|e| format!("{e:?}"))?;
    // Tramos: ventanas de prueba del walk-forward, o bloques antes del fuera de muestra.
    let (periods, holdout, mode): (Vec<Period>, _, String) = match plan(first, last, &split_cfg) {
        Ok(split) => (
            split.windows.iter().map(|w| w.validation).collect(),
            split.out_of_sample,
            format!(
                "ventanas de prueba del walk-forward ({} meses de entrenamiento, {} de prueba, \
                 paso {})",
                split_cfg.train_months, split_cfg.validation_months, split_cfg.step_months
            ),
        ),
        Err(e) => {
            let (blocks, oos) = research_blocks(first, last, &split_cfg);
            let oos = oos.ok_or("no se puede calcular el fuera de muestra")?;
            (
                blocks,
                Period {
                    start: oos,
                    end: last,
                },
                format!(
                    "SIN walk-forward ({e}: hacen falta {} meses más {} de fuera de muestra y hay \
                     velas del {} al {}). Bloques de {} meses antes del fuera de muestra, sin \
                     entrenamiento",
                    split_cfg.train_months + split_cfg.validation_months,
                    split_cfg.out_of_sample_months,
                    first.date_naive(),
                    last.date_naive(),
                    split_cfg.validation_months
                ),
            )
        }
    };
    if periods.is_empty() {
        return Err("no hay ningún tramo de investigación antes del fuera de muestra".into());
    }
    // La misma guardia del walk-forward: ningún tramo llega al fuera de muestra.
    let guard = DataSplit {
        windows: Vec::new(),
        out_of_sample: holdout,
    };
    for period in &periods {
        cfds_backtest::research_period(*period, &guard).map_err(|e| format!("{e:?}"))?;
    }
    let inputs = cfds_backtest::Inputs {
        m15: &p.m15,
        h1: &p.h1,
        entries: &p.entries,
        force_close: &p.force_close,
        data_errors: &p.data_errors,
        news: &p.news,
        published_from: p.published_from,
    };
    let texts = ConfigTexts {
        strategy: cfg!("strategy.toml"),
        backtest: cfg!("backtest.toml"),
        risk: cfg!("risk.toml"),
    };
    let z = z();
    println!(
        "\n## {} ({})\n\nTramos: {mode}. Fuera de muestra desde {} (no se usa). {} variantes.",
        p.out,
        p.sources.label(source),
        holdout.start.date_naive(),
        variants.len()
    );

    let mut summary = Vec::new();
    let mut detail = Vec::new();
    for v in &variants {
        let setup = setup_for(&p.setup, &texts, v).map_err(|e| format!("{e:?}"))?;
        let mut total = TradeStats::default();
        let (mut worst_dd, mut positive, mut with_data) = (None::<Decimal>, 0, 0);
        for period in &periods {
            let (r, report) =
                evaluate_period_with(&inputs, *period, &setup, eval.warmup, size, |sl, e| {
                    entry_filter(sl, &e, &setup, v)
                })
                .map_err(|e| format!("{e:?}"))?;
            let st = report.map_or_else(TradeStats::default, |rep| {
                TradeStats::of(&rep.trades, setup.account_to_instrument)
            });
            total.add(&st);
            let dd = r.metrics.as_ref().map(|m| m.max_drawdown);
            if let Some(d) = dd {
                with_data += 1;
                worst_dd = Some(worst_dd.map_or(d, |w: Decimal| w.max(d)));
                if st.net > Decimal::ZERO {
                    positive += 1;
                }
            }
            detail.push(format!(
                "| {} | {} → {} | {} | {} | {} | {:.0} | {:.0} | {:.0} | {} | {} | {} |",
                v.name,
                period.start.date_naive(),
                period.end.date_naive(),
                st.trades,
                pct(st.win_rate()),
                num(st.payoff()),
                st.mid_pnl,
                st.costs,
                st.net,
                pct(dd),
                pct(r.metrics.as_ref().map(|m| m.total_return)),
                pct(r.buy_and_hold_return)
            ));
        }
        summary.push(format!(
            "| {} | {} | {} | {} | {} | {:.0} | {:.0} | {:.0} | {} | {positive} de {with_data} |",
            v.name,
            total.trades,
            pct(total.win_rate()),
            ci(&total, z),
            num(total.payoff()),
            total.mid_pnl,
            total.costs,
            total.net,
            pct(worst_dd),
        ));
    }
    println!(
        "\n### Todos los tramos sumados\n\n| Variante | Op. | Gana | IC 95 % | Payoff | P/L sin \
         costos | Costos | Neto | Peor drawdown | Tramos con neto > 0 |\n|---|---|---|---|---|---|---|---|---|---|"
    );
    for l in &summary {
        println!("{l}");
    }
    println!(
        "\n### Por tramo\n\n| Variante | Tramo | Op. | Gana | Payoff | P/L sin costos | Costos | \
         Neto | Drawdown | Retorno | Comprar y mantener |\n|---|---|---|---|---|---|---|---|---|---|---|"
    );
    for l in &detail {
        println!("{l}");
    }
    Ok(())
}

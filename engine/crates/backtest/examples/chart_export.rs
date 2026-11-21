//! Exporta velas reales de PostgreSQL con sus indicadores, puntajes, pivotes y las operaciones
//! del backtest (`contracts/chart_export.v1`, #81). Lo leen la comparación con la referencia
//! (`tools/verify/`) y el visor; ninguno recalcula nada.
//!
//! ```text
//! DATABASE_URL=... cargo run -p cfds-backtest --example chart_export -- \
//!     <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <archivo.json>
//! ```
//! El archivo tiene precios de IG: no se commitea (el repo es público).

use cfds_backtest::{BacktestConfig, chart_export, run_checked, setup_from_catalog};
use cfds_core::Timeframe;
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{
    Catalog, ExchangeCalendar, SpreadFilterConfig, TradingWindowConfig, ValidationConfig,
    check_series, spread_flags, trading_window,
};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_storage::PgStore;
use cfds_strategy::{Configs, StrategyConfig};
use rust_decimal::Decimal;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [instrument, from, to, equity, out] = args.as_slice() else {
        return Err("uso: <INSTRUMENTO> <desde UTC> <hasta UTC> <capital> <archivo.json>".into());
    };
    let id = instrument.parse()?;
    let (from, to) = (from.parse()?, to.parse()?);
    let equity: Decimal = equity.parse()?;
    let catalog = Catalog::from_toml(cfg!("instruments.toml"))?;
    let setup = setup_from_catalog(
        &catalog,
        &id,
        Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml"))?,
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml"))?,
            trend: TrendConfig::from_toml(cfg!("indicators.toml"))?,
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml"))?,
        },
        RiskConfig::from_toml(cfg!("risk.toml"))?,
        BacktestConfig::from_toml(cfg!("backtest.toml"))?,
        equity,
    )?;
    let store = PgStore::connect(&std::env::var("DATABASE_URL")?).await?;
    let raw_m15 = store.candles(&id, Timeframe::M15, from, to).await?;
    let raw_h1 = store.candles(&id, Timeframe::H1, from, to).await?;
    let config_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&config_dir, catalog.exchange_map())?;
    // Spec §7.8 y §12: se descartan las velas sospechosas y, después de cada error de datos,
    // no se abre hasta volver a tener el calentamiento completo.
    let validation = ValidationConfig::from_toml(cfg!("data_validation.toml"))?;
    let (m15, h1, data_errors) = {
        let a = check_series(&raw_m15, &calendar, &validation);
        let b = check_series(&raw_h1, &calendar, &validation);
        for i in a.issues.iter().chain(&b.issues) {
            eprintln!("⚠️  error de datos en {}: {}", i.start, i.kind);
        }
        let mut errors = [a.data_errors, b.data_errors].concat();
        errors.sort();
        (a.candles, b.candles, errors)
    };
    let session = calendar.session_for(&id).ok_or("instrumento sin bolsa")?;
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml"))?;
    let (entries, force_close) =
        trading_window::flags(session, &m15, &window).ok_or("fechas fuera del calendario")?;
    // Filtro de spread (spec §7.3): necesita el spread de cierre guardado (#95), también de los
    // días anteriores al período. Sin ese dato no se abre.
    let spread_cfg = SpreadFilterConfig::from_toml(cfg!("spread_filter.toml"))?;
    let spreads = store
        .spreads(&id, Timeframe::M15, from - spread_cfg.lookback, to)
        .await?;
    let spread_ok = spread_flags(&m15, &spreads, session.tz, &spread_cfg);
    eprintln!(
        "filtro de spread: {} de {} velas no dejan abrir",
        spread_ok.iter().filter(|ok| !**ok).count(),
        spread_ok.len()
    );
    let entries: Vec<bool> = entries
        .iter()
        .zip(&spread_ok)
        .map(|(e, s)| *e && *s)
        .collect();
    // Filtro de noticias (spec §7.2, `cfds_market_data::news_flags`): la fuente del calendario
    // económico es de la fase 4b y necesita su ADR (spec §9). Sin eventos no se aplica: avisarlo.
    eprintln!(
        "⚠️  filtro de noticias de alto impacto (§7.2) SIN DATOS: falta la fuente del calendario \
         (fase 4b). El resultado no lo descuenta."
    );
    let report = run_checked(&m15, &h1, &entries, &force_close, &data_errors, &setup)
        .map_err(|e| format!("{e:?}"))?;
    let export =
        chart_export(&m15, &h1, &data_errors, &setup, &report).map_err(|e| format!("{e:?}"))?;
    std::fs::write(out, serde_json::to_string(&export)?)?;
    println!(
        "{out}: {} velas, {} pivotes, {} operaciones",
        export.candles.len(),
        export.pivots.len(),
        export.trades.len()
    );
    Ok(())
}

//! `setup_from_catalog` (#76) y el criterio de aprobación de la Fase 3: comprar y mantener sobre
//! **datos reales** del DE40 (IG demo), calculado a mano.

use cfds_backtest::{
    BacktestConfig, Commission, InstrumentCosts, SetupError, buy_and_hold, setup_from_catalog,
};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::Catalog;
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, StrategyConfig};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

const CATALOG: &str = cfg!("instruments.toml");

fn strategy() -> Configs {
    Configs {
        patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
        liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
        trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
        strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
    }
}

fn build(catalog: &str, id: &str, equity: Decimal) -> Result<cfds_backtest::Setup, SetupError> {
    setup_from_catalog(
        &Catalog::from_toml(catalog).unwrap(),
        &id.parse::<InstrumentId>().unwrap(),
        strategy(),
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        equity,
    )
}

/// Reemplaza `from` por `to` dentro de la sección de `id`.
fn in_section(id: &str, from: &str, to: &str) -> String {
    let at = CATALOG.find(&format!("[instruments.{id}]")).unwrap();
    let pos = at + CATALOG[at..].find(from).unwrap();
    format!("{}{}{}", &CATALOG[..pos], to, &CATALOG[pos + from.len()..])
}

#[test]
fn de40_setup_comes_only_from_the_configuration() {
    let s = build(CATALOG, "DE40", dec!(10000)).unwrap();
    assert_eq!(
        s.costs,
        InstrumentCosts {
            spread: dec!(1.8),
            tick: dec!(0.1),
            commission: Commission::None,
            funding_admin_fee_pct: dec!(3),
            funding_day_count: dec!(360),
        }
    );
    assert_eq!(
        s.funding_rates.at(chrono::Utc::now()),
        Some(dec!(2.441)),
        "€STR del BCE (fuente ecb-estr)"
    );
    assert_eq!(s.limits.margin_pct, dec!(5));
    assert_eq!(s.limits.min_deal_size, Some(dec!(0.5)));
    assert_eq!(
        s.account_to_instrument,
        cfds_backtest::AccountFx::Same,
        "contrato y cuenta en EUR"
    );
    assert_eq!(s.funding_cutoff.to_string(), "23:00:00");
    assert_eq!(s.funding_tz, chrono_tz::Europe::Madrid);
    assert_eq!(s.day_tz, chrono_tz::Europe::Madrid);
    assert_eq!(s.initial_equity, dec!(10000));
}

#[test]
fn pending_broker_terms_block_the_run_and_say_what_is_missing() {
    let pending = in_section(
        "US100",
        r#"funding_benchmark = { value = "SOFR", source = "ig-chat-sofr" }"#,
        r#"funding_benchmark = { pending = "sin confirmar" }"#,
    );
    let Err(SetupError::Pending(p)) = build(&pending, "US100", dec!(10000)) else {
        panic!("sin tipo de referencia confirmado no se simula");
    };
    let fields: Vec<_> = p.iter().map(|f| (f.item.as_str(), f.field)).collect();
    assert_eq!(fields, [("US100", "funding_benchmark")]);

    let no_rate = CATALOG.replacen(
        r#"ESTR = { value = "2.441", source = "ecb-estr" }"#,
        r#"ESTR = { pending = "sin dato" }"#,
        1,
    );
    assert_ne!(no_rate, CATALOG);
    let Err(SetupError::Pending(p)) = build(&no_rate, "DE40", dec!(10000)) else {
        panic!("sin el valor del €STR no se simula");
    };
    assert_eq!(p[0].field, "rate");

    let no_margin = in_section(
        "DE40",
        r#"margin_retail_pct = { value = "5", source = "ig-es-indices" }"#,
        r#"margin_retail_pct = { pending = "x" }"#,
    );
    let Err(SetupError::Pending(p)) = build(&no_margin, "DE40", dec!(10000)) else {
        panic!("sin margen no se simula");
    };
    assert_eq!(p.len(), 1, "{p:?}");
    assert_eq!(p[0].field, "margin_retail_pct");
}

#[test]
fn us_indices_simulate_with_the_documented_sofr() {
    // #655: con SOFR confirmado, los tres índices de EE. UU. ya no quedan bloqueados. Sin la
    // serie diaria guardada, el respaldo es la constante documentada (nyfed-sofr).
    for id in ["US100", "US500", "US30"] {
        let s = build(CATALOG, id, dec!(10000)).unwrap();
        assert_eq!(
            s.funding_rates.at(chrono::Utc::now()),
            Some(dec!(3.94)),
            "{id}"
        );
        assert_eq!(s.costs.funding_admin_fee_pct, dec!(3), "{id}");
        assert_eq!(s.costs.funding_day_count, dec!(360), "{id}");
        assert_eq!(
            s.account_to_instrument,
            cfds_backtest::AccountFx::Same,
            "{id}"
        );
    }
}

#[test]
fn a_contract_in_another_currency_needs_an_exchange_rate() {
    let usd = in_section("DE40", r#"currency = "EUR""#, r#"currency = "USD""#);
    // ADR 0013 §4: el recorrido convierte con la serie del par CUENTA+INSTRUMENTO. Sin
    // cargarla, falta el tipo en todo momento (nunca uno inventado).
    let setup = build(&usd, "DE40", dec!(10000)).unwrap();
    assert_eq!(
        setup.account_to_instrument,
        cfds_backtest::AccountFx::Missing {
            pair: "EURUSD".into()
        }
    );
    let at = "2026-11-24T12:00:00Z".parse().unwrap();
    assert_eq!(
        setup.to_account(dec!(100), at),
        Err(cfds_backtest::RunError::MissingFxRate(at))
    );
}

#[test]
fn unknown_instruments_and_non_positive_equity_are_rejected() {
    assert!(matches!(
        build(CATALOG, "FR40", dec!(1)),
        Err(SetupError::Unknown(_))
    ));
    assert!(matches!(
        build(CATALOG, "DE40", dec!(0)),
        Err(SetupError::InvalidEquity(_))
    ));
}

fn de40(start: &str, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "DE40".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: start.parse().unwrap(),
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

/// **Criterio de aprobación de la Fase 3.** Velas reales del DE40 (IG demo, `IX.D.DAX.IFE.IP`,
/// precio medio), cargadas con `ig_backfill`: la primera y la última de 15m del 09-11 al 20-11.
/// Cálculo a mano en `docs/research/fase-3-comprar-y-mantener.md`.
#[test]
fn buy_and_hold_on_real_de40_matches_the_hand_calculation() {
    let s = build(CATALOG, "DE40", dec!(10000)).unwrap();
    let candles = [
        de40(
            "2026-11-09T00:00:00Z",
            dec!(25669.3),
            dec!(25705.4),
            dec!(25655.0),
            dec!(25699.2),
        ),
        de40(
            "2026-11-20T21:45:00Z",
            dec!(24630.1),
            dec!(24643.4),
            dec!(24611.7),
            dec!(24614.2),
        ),
    ];
    let bh = buy_and_hold(&candles, dec!(1), &s).unwrap();
    // Por ejecución: spread/2 + 0,1 × spread + 1 tick = 0,9 + 0,18 + 0,1 = 1,18 puntos.
    assert_eq!(bh.entry_price, dec!(25670.48)); // 25669,3 + 1,18
    assert_eq!(bh.exit_price, dec!(24613.02)); // 24614,2 − 1,18
    assert_eq!(bh.gross_pnl, dec!(-1057.46));
    // Cortes de las 23:00 de Madrid (22:00 UTC) del 9 al 20 de noviembre: 12 noches.
    assert_eq!(bh.nights, 12);
    // 12 × 25669,3 × 1 × (2,441 + 3) % / 360 = 46,555553 766 666…
    assert_eq!(bh.funding, dec!(46.555553766666666666666666667));
    assert_eq!(bh.commission, dec!(0));
    assert_eq!(bh.net_pnl, dec!(-1104.0155537666666666666666667));
}

#[test]
fn gold_is_blocked_by_its_pending_terms() {
    // ADR 0013 (#418): el oro está en el catálogo, pero con datos de IG pendientes no se simula.
    let catalog =
        cfds_market_data::Catalog::from_toml(include_str!("../../../../config/instruments.toml"))
            .unwrap();
    let err =
        cfds_backtest::market_terms_from_catalog(&catalog, &"XAUUSD".parse().unwrap()).unwrap_err();
    let cfds_backtest::SetupError::Pending(p) = err else {
        panic!("esperaba pendientes, no {err:?}");
    };
    let fields: Vec<_> = p.iter().map(|f| f.field).collect();
    for f in [
        "funding_basis",
        "spread_typical_points",
        "tick",
        "min_deal_size",
    ] {
        assert!(fields.contains(&f), "{f}: {fields:?}");
    }
}

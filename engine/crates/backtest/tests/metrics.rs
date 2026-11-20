//! Métricas calculables a mano (ROADMAP Fase 5, #58).

use cfds_backtest::metrics::{
    daily_returns, evaluate, exposure, max_drawdown, profit_factor, sharpe, sortino, win_rate,
};
use cfds_backtest::{BacktestConfig, MetricsConfig};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::{Decimal, dec};

fn cfg() -> MetricsConfig {
    BacktestConfig::from_toml(include_str!("../../../../config/backtest.toml"))
        .unwrap()
        .metrics
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
}

/// 100 → 110 → 99 → 108,9: retornos +10 %, −10 %, +10 %.
fn curve() -> Vec<(NaiveDate, Decimal)> {
    vec![
        (day(16), dec!(110)),
        (day(17), dec!(99)),
        (day(18), dec!(108.9)),
    ]
}

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

#[test]
fn the_repository_config() {
    assert_eq!(
        cfg(),
        MetricsConfig {
            trading_days_per_year: 252,
            risk_free_annual_pct: dec!(0),
        }
    );
}

#[test]
fn daily_returns_start_from_the_initial_equity() {
    assert_eq!(
        daily_returns(dec!(100), &curve()),
        vec![dec!(0.1), dec!(-0.1), dec!(0.1)]
    );
}

#[test]
fn max_drawdown_from_peak_to_trough() {
    // Pico 110, valle 99: 10 %.
    assert_eq!(max_drawdown(dec!(100), &curve()), dec!(0.1));
    // Si nunca cae, 0; y una caída desde el capital inicial también cuenta.
    assert_eq!(max_drawdown(dec!(100), &[(day(16), dec!(120))]), dec!(0));
    assert_eq!(max_drawdown(dec!(100), &[(day(16), dec!(80))]), dec!(0.2));
}

#[test]
fn sharpe_by_hand() {
    // Media 1/30, desvío muestral √(1/75): 1/30 × √75 × √252 = 4,5826.
    let s = sharpe(&daily_returns(dec!(100), &curve()), &cfg()).unwrap();
    assert_eq!(s.round_dp(4), dec!(4.5826));
}

#[test]
fn sortino_only_penalizes_the_downside() {
    // Desvío a la baja √(0,01 / 3): 1/30 / √(1/300) × √252 = 9,1652.
    let s = sortino(&daily_returns(dec!(100), &curve()), &cfg()).unwrap();
    assert_eq!(s.round_dp(4), dec!(9.1652));
    // Sin días negativos no está definido.
    assert_eq!(sortino(&[dec!(0.01), dec!(0.02)], &cfg()), None);
}

#[test]
fn the_risk_free_rate_is_subtracted() {
    // 25,2 % anual = 0,1 % diario: el Sharpe baja 0,001 / √(1/75) × √252 = 0,1375.
    let rf = MetricsConfig {
        risk_free_annual_pct: dec!(25.2),
        ..cfg()
    };
    let r = daily_returns(dec!(100), &curve());
    let diff = sharpe(&r, &cfg()).unwrap() - sharpe(&r, &rf).unwrap();
    assert_eq!(diff.round_dp(4), dec!(0.1375));
}

#[test]
fn sharpe_is_undefined_without_volatility_or_with_one_day() {
    assert_eq!(sharpe(&[dec!(0.01)], &cfg()), None);
    assert_eq!(sharpe(&[dec!(0.01), dec!(0.01)], &cfg()), None);
    assert_eq!(sharpe(&[], &cfg()), None);
}

#[test]
fn win_rate_and_profit_factor() {
    let net = [dec!(5), dec!(-2), dec!(3), dec!(0)];
    // Un neto de 0 no es ganador.
    assert_eq!(win_rate(&net), Some(dec!(0.5)));
    assert_eq!(profit_factor(&net), Some(dec!(4)));
    // Sin pérdidas, el profit factor no está definido; sin operaciones, tampoco el win rate.
    assert_eq!(profit_factor(&[dec!(1)]), None);
    assert_eq!(win_rate(&[]), None);
}

#[test]
fn exposure_counts_overlapping_positions_once() {
    let (start, end) = (utc("2026-11-19T10:00:00Z"), utc("2026-11-19T14:00:00Z"));
    let iv = [
        (utc("2026-11-19T10:00:00Z"), utc("2026-11-19T11:00:00Z")),
        (utc("2026-11-19T10:30:00Z"), utc("2026-11-19T12:00:00Z")),
        // Se recorta al final del período.
        (utc("2026-11-19T13:00:00Z"), utc("2026-11-19T15:00:00Z")),
    ];
    // 10–12 (2 h) + 13–14 (1 h) de 4 h.
    assert_eq!(exposure(&iv, start, end), Some(dec!(0.75)));
    assert_eq!(exposure(&[], start, end), Some(dec!(0)));
    assert_eq!(exposure(&iv, end, start), None);
}

#[test]
fn evaluate_puts_it_all_together() {
    let m = evaluate(
        dec!(100),
        dec!(108.9),
        &curve(),
        &[dec!(10), dec!(-11), dec!(9.9)],
        &[],
        (utc("2026-11-16T00:00:00Z"), utc("2026-11-19T00:00:00Z")),
        &cfg(),
    );
    assert_eq!(m.total_return, dec!(0.089));
    assert_eq!(m.max_drawdown, dec!(0.1));
    assert_eq!(m.trades, 3);
    assert_eq!(m.win_rate.unwrap().round_dp(4), dec!(0.6667));
    assert_eq!(m.profit_factor.unwrap(), dec!(19.9) / dec!(11));
    assert_eq!(m.exposure, Some(dec!(0)));
    assert!(m.sharpe.is_some() && m.sortino.is_some());
}

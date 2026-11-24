//! Base de los futuros en la financiación (#467): lo que el precio del futuro ya descontó se resta
//! de la financiación del CFD, noche por noche, con el signo del lado.

use cfds_backtest::{
    BacktestConfig, Commission, DailyRates, FundingRates, FuturesBasis, InstrumentCosts, Setup,
    basis_carry, buy_and_hold, execute, roll_jumps, splice_rates,
};
use cfds_core::{Candle, ReferenceRate, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::RateSegment;
use cfds_patterns::PatternsConfig;
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_strategy::{
    Component, Configs, Score, Side, Signal, StrategyConfig, Trigger, TriggerKind,
};
use chrono::{DateTime, NaiveDate, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn setup() -> Setup {
    Setup {
        instrument: "US100".parse().unwrap(),
        strategy: Configs {
            patterns: PatternsConfig::from_toml(include_str!("../../../../config/patterns.toml"))
                .unwrap(),
            liquidity: LiquidityConfig::from_toml(include_str!(
                "../../../../config/liquidity.toml"
            ))
            .unwrap(),
            trend: TrendConfig::from_toml(include_str!("../../../../config/indicators.toml"))
                .unwrap(),
            strategy: StrategyConfig::from_toml(include_str!("../../../../config/strategy.toml"))
                .unwrap(),
        },
        risk: RiskConfig::from_toml(include_str!("../../../../config/risk.toml")).unwrap(),
        // Valores de ejemplo: el mínimo real del broker llega con #8.
        limits: InstrumentLimits {
            max_leverage: dec!(10),
            margin_pct: dec!(5),
            min_deal_size: Some(dec!(0.0001)),
        },
        // Spread 1 y tick 0,1 de ejemplo: cada ejecución paga 0,5 + 0,1 × 1 + 0,1 = 0,7.
        costs: InstrumentCosts {
            spread: dec!(1),
            tick: dec!(0.1),
            commission: Commission::None,
            funding_admin_fee_pct: dec!(2.5),
            funding_day_count: dec!(360),
        },
        backtest: BacktestConfig::from_toml(include_str!("../../../../config/backtest.toml"))
            .unwrap(),
        initial_equity: dec!(1000),
        account_to_instrument: cfds_backtest::AccountFx::Same,
        // 1,1 % + 2,5 % de tasa = 3,6 % anual para los largos.
        funding_rates: FundingRates::Constant(dec!(1.1)),
        funding_cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
        funding_tz: chrono_tz::Europe::Madrid,
        futures_basis: None,
        day_tz: chrono_tz::Europe::Madrid,
    }
}

fn bar(t0: DateTime<Utc>, i: usize, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: t0 + TimeDelta::minutes(15 * i as i64),
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

/// Señal larga en la vela 0 con stop 98. La 1 entra a 100 (ejecutado 100,7; R = 2,7;
/// TP1 = 106,1); la 2 toca el TP1; la 3 vuelve a la entrada.
fn trade_candles(t0: DateTime<Utc>) -> Vec<Candle> {
    vec![
        bar(t0, 0, dec!(100), dec!(100.5), dec!(99.5), dec!(100)),
        bar(t0, 1, dec!(100), dec!(101), dec!(99.5), dec!(100.5)),
        bar(t0, 2, dec!(100.5), dec!(107), dec!(100.8), dec!(106.5)),
        bar(t0, 3, dec!(106), dec!(106.2), dec!(100.6), dec!(101)),
    ]
}

fn signal(side: Side, stop: Decimal) -> Signal {
    Signal {
        side,
        index: 0,
        score: Score {
            total: 6,
            breakdown: vec![(Component::Trend, 2)],
            candidate: true,
        },
        reference: dec!(100),
        stop,
        r: dec!(2),
        triggers: vec![Trigger {
            kind: TriggerKind::Sweep,
            structure: dec!(99),
        }],
    }
}

/// ATR grande: el trailing no supera la entrada en este escenario.
fn big_atr(n: usize) -> Vec<Option<Decimal>> {
    vec![Some(dec!(10)); n]
}

fn basis(rate: Decimal) -> FuturesBasis {
    FuturesBasis {
        rate: FundingRates::Constant(rate),
        dividends: None,
        rolls: vec![],
    }
}

fn with_basis(b: FuturesBasis) -> Setup {
    Setup {
        futures_basis: Some(b),
        ..setup()
    }
}

fn cutoffs(n: usize) -> Vec<DateTime<Utc>> {
    (0..n)
        .map(|i| utc("2026-11-16T22:00:00Z") + TimeDelta::days(i as i64))
        .collect()
}

#[test]
fn the_carry_is_the_basis_rate_per_night_with_the_sign_of_the_side() {
    let s = setup();
    // 3 noches × 100 × 2 × 1,8 % / 360 = 0,03.
    let b = basis(dec!(1.8));
    assert_eq!(
        basis_carry(Side::Long, dec!(100), dec!(2), &cutoffs(3), &b, &s.costs),
        Ok(dec!(0.03))
    );
    assert_eq!(
        basis_carry(Side::Short, dec!(100), dec!(2), &cutoffs(3), &b, &s.costs),
        Ok(dec!(-0.03))
    );
    assert_eq!(
        basis_carry(Side::Long, dec!(100), dec!(2), &[], &b, &s.costs),
        Ok(dec!(0))
    );
}

#[test]
fn with_dividends_the_carry_is_rate_minus_dividends() {
    let s = setup();
    let b = FuturesBasis {
        rate: FundingRates::Constant(dec!(4.6)),
        dividends: Some(FundingRates::Constant(dec!(1))),
        rolls: vec![],
    };
    // (4,6 − 1) × 100 × 1 / 100 / 360 = 0,01 por noche.
    assert_eq!(b.at(utc("2026-11-16T22:00:00Z")), Some(dec!(3.6)));
    assert_eq!(
        basis_carry(Side::Long, dec!(100), dec!(1), &cutoffs(2), &b, &s.costs),
        Ok(dec!(0.02))
    );
}

fn rate(series: &str, day: &str, pct: Decimal) -> ReferenceRate {
    let d: NaiveDate = day.parse().unwrap();
    ReferenceRate {
        series: series.into(),
        ref_date: d,
        published_at: d.and_hms_opt(12, 0, 0).unwrap().and_utc() + TimeDelta::days(1),
        rate_pct: pct,
    }
}

#[test]
fn a_night_without_a_published_basis_rate_is_an_error() {
    let s = setup();
    let daily = DailyRates::new(vec![rate("SOFR", "2026-11-18", dec!(4))]).unwrap();
    let b = FuturesBasis {
        rate: FundingRates::Daily(daily),
        dividends: None,
        rolls: vec![],
    };
    // El valor del 18 vale desde el 19 a las 12:00 UTC: las noches del 16 y 17 no tienen tasa.
    assert_eq!(
        basis_carry(Side::Long, dec!(100), dec!(1), &cutoffs(4), &b, &s.costs),
        Err(utc("2026-11-16T22:00:00Z"))
    );
}

#[test]
fn spliced_series_switch_at_the_segment_date() {
    let segments = vec![
        RateSegment {
            series: "EFFR".into(),
            from: None,
        },
        RateSegment {
            series: "SOFR".into(),
            from: NaiveDate::from_ymd_opt(2018, 4, 2),
        },
    ];
    let effr = vec![
        rate("EFFR", "2018-03-29", dec!(1.68)),
        rate("EFFR", "2018-04-02", dec!(1.68)),
    ];
    let sofr = vec![
        rate("SOFR", "2018-03-29", dec!(9)), // antes de su tramo: no entra
        rate("SOFR", "2018-04-02", dec!(1.80)),
    ];
    let d = splice_rates(&segments, vec![effr, sofr], "EFFR→SOFR").unwrap();
    assert_eq!(d.series(), "EFFR→SOFR");
    assert_eq!(d.at(utc("2018-03-31T00:00:00Z")), Some(dec!(1.68)));
    assert_eq!(d.at(utc("2018-04-04T00:00:00Z")), Some(dec!(1.80)));
    assert!(splice_rates(&segments, vec![vec![], vec![]], "X").is_none());
}

#[test]
fn an_overnight_trade_on_futures_pays_only_what_the_basis_did_not_discount() {
    // La misma operación que en `runner.rs`: dos partes de 1 que pasan una noche. El CFD cobra
    // 3,6 % (1,1 + 2,5); la base del futuro ya descontó 1,1 %: queda la tasa de 2,5 %.
    let c = trade_candles(utc("2026-11-19T21:30:00Z"));
    let run = |s: &Setup| {
        execute(
            &signal(Side::Long, dec!(98)),
            dec!(2),
            dec!(0.2),
            dec!(10),
            &c,
            &big_atr(4),
            &[],
            s,
        )
        .unwrap()
    };
    let cfd = run(&setup());
    let fut = run(&with_basis(basis(dec!(1.1))));
    assert_eq!(cfd.funding, dec!(0.02));
    // 2 × 1 × 100 × 2,5 % / 360.
    assert_eq!(fut.funding.round_dp(12), (dec!(5) / dec!(360)).round_dp(12));
    assert_eq!(fut.net_pnl - cfd.net_pnl, cfd.funding - fut.funding);
    // Un corto recibe de menos: la base le resta lo que el precio ya le había dado.
    let short = |s: &Setup| {
        execute(
            &signal(Side::Short, dec!(102)),
            dec!(2),
            dec!(0.2),
            dec!(10),
            &c,
            &big_atr(4),
            &[],
            s,
        )
        .unwrap()
    };
    let (plain, fut) = (short(&setup()), short(&with_basis(basis(dec!(1.1)))));
    // El corto paga 2,5 − 1,1 = 1,4 % en el CFD; con la base, 1,4 + 1,1 = 2,5 %.
    assert!(
        fut.funding > plain.funding,
        "{} {}",
        fut.funding,
        plain.funding
    );
}

#[test]
fn an_intraday_trade_on_futures_does_not_change() {
    // De 10:00 a 11:00 UTC: no cruza las 23:00 de Madrid.
    let c = trade_candles(utc("2026-11-19T10:00:00Z"));
    let run = |s: &Setup| {
        execute(
            &signal(Side::Long, dec!(98)),
            dec!(2),
            dec!(0.2),
            dec!(10),
            &c,
            &big_atr(4),
            &[],
            s,
        )
        .unwrap()
    };
    assert_eq!(run(&setup()), run(&with_basis(basis(dec!(5)))));
}

#[test]
fn buy_and_hold_on_futures_removes_the_roll_jump_and_the_basis() {
    // 21:30 a 22:30 UTC: 1 noche. El contrato nuevo empieza en la vela 2 con un salto de +3.
    let t0 = utc("2026-11-19T21:30:00Z");
    let candles = vec![
        bar(t0, 0, dec!(100), dec!(103), dec!(99), dec!(102)),
        bar(t0, 1, dec!(102), dec!(106), dec!(101), dec!(105)),
        bar(t0, 2, dec!(108), dec!(110), dec!(104), dec!(108)),
        bar(t0, 3, dec!(108), dec!(111), dec!(107), dec!(110)),
    ];
    let roll = candles[2].start;
    assert_eq!(roll_jumps(&candles, &[roll]), dec!(3));
    // Un vencimiento fuera de la serie no cuenta.
    assert_eq!(roll_jumps(&candles, &[t0 - TimeDelta::days(1)]), dec!(0));
    let cfd = buy_and_hold(&candles, dec!(2), &setup()).unwrap();
    let b = FuturesBasis {
        rolls: vec![roll],
        ..basis(dec!(1.1))
    };
    let fut = buy_and_hold(&candles, dec!(2), &with_basis(b)).unwrap();
    // Bruto: 17,2 − 2 × 3 = 11,2. Financiación: 1 noche × 100 × 2 × (3,6 − 1,1) % / 360.
    assert_eq!(fut.gross_pnl, cfd.gross_pnl - dec!(6));
    assert_eq!(fut.funding.round_dp(12), (dec!(5) / dec!(360)).round_dp(12));
    assert_eq!(fut.net_pnl, fut.gross_pnl - fut.funding);
}

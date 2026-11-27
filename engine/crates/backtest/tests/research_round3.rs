//! Estudios de la ronda 3 (#493): exposición manejada por volatilidad (V), cascada al cruzar un
//! número redondo (M1), el descriptivo de la deriva nocturna y el alfa con errores HC1. Series
//! sintéticas sobre el calendario real de XNYS; nunca la base real.

use std::path::PathBuf;

use cfds_backtest::research::{
    Levels, ResearchConfig, buy_and_hold_returns, managed_returns, monthly_reviews,
    review_variances, round_number_plans, session_close_indices, stats, vol_constant, vol_weights,
    window_returns,
};
use cfds_backtest::{BacktestConfig, Setup, apply_funding_scenario, setup_from_catalog};
use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_indicators::TrendConfig as IndicatorTrend;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{Catalog, ExchangeCalendar, Session, TradingWindowConfig, trading_window};
use cfds_patterns::PatternsConfig;
use cfds_risk::RiskConfig;
use cfds_strategy::{Configs, Side, StrategyConfig};
use chrono::{DateTime, NaiveDate, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn us500() -> InstrumentId {
    "US500".parse().unwrap()
}

fn calendar() -> ExchangeCalendar {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap()
}

fn research() -> ResearchConfig {
    ResearchConfig::from_toml(cfg!("research.toml")).unwrap()
}

fn setup() -> Setup {
    let mut catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    apply_funding_scenario(&mut catalog, &us500(), "TEST", dec!(4)).unwrap();
    setup_from_catalog(
        &catalog,
        &us500(),
        Configs {
            patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
            liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
            trend: IndicatorTrend::from_toml(cfg!("indicators.toml")).unwrap(),
            strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
        },
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        dec!(100000),
    )
    .unwrap()
}

fn candle(start: DateTime<Utc>, o: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: us500(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: o.max(c) + dec!(1),
        low: o.min(c) - dec!(1),
        close: c,
        volume: None,
    }
}

/// Velas de 15m de la sesión de contado de cada día con sesión en `[from, to]`; `price(k)` es el
/// cierre de la vela `k` y la apertura es el cierre anterior.
fn series(
    session: &Session,
    from: NaiveDate,
    to: NaiveDate,
    price: impl Fn(usize) -> Decimal,
) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();
    let mut d = from;
    while d <= to {
        if let Some(Some((open, close))) = session.hours(d) {
            let mut s = open;
            while s + TimeDelta::minutes(15) <= close {
                let k = out.len();
                let o = out.last().map_or(price(0), |c| c.close);
                out.push(candle(s, o, price(k)));
                s += TimeDelta::minutes(15);
            }
        }
        d = d.succ_opt().unwrap();
    }
    out
}

/// Velas contiguas de 15m desde `start` con los cierres `closes` (apertura = cierre anterior).
fn run(start: &str, closes: &[Decimal]) -> Vec<Candle> {
    let t0 = at(start);
    closes
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let o = if k == 0 { *c } else { closes[k - 1] };
            candle(
                t0 + TimeDelta::minutes(15) * i32::try_from(k).unwrap(),
                o,
                *c,
            )
        })
        .collect()
}

fn flags(session: &Session, m15: &[Candle]) -> (Vec<bool>, Vec<bool>) {
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml")).unwrap();
    trading_window::flags(session, m15, &window).unwrap()
}

// ── configuración ──────────────────────────────────────────────────────────────────────────

#[test]
fn the_round3_config_loads_and_rejects_bad_values() {
    let c = research();
    assert_eq!(
        (c.vol_managed.cap, c.vol_managed.min_training_pairs),
        (dec!(2), 12)
    );
    let r = &c.round_number;
    assert_eq!(
        (r.horizon_minutes, r.atr_period, r.stop_atr),
        (120, 14, dec!(2))
    );
    assert_eq!(r.step_for("US500"), Some(dec!(50)));
    assert_eq!(r.step_for("US100"), Some(dec!(100)));
    assert_eq!(r.step_for("US30"), Some(dec!(500)));
    assert_eq!(r.step_for("US2000"), None);
    assert_eq!(
        (c.overnight_drift.from, c.overnight_drift.to),
        (
            NaiveTime::from_hms_opt(2, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(3, 0, 0).unwrap()
        )
    );
    assert_eq!(c.rule_round3.alpha, dec!(0.0125));
    assert_eq!(c.rule_round3.dsr_trials, 354);
    // Las reglas de las rondas 1 y 2 no cambian.
    assert_eq!((c.rule.dsr_trials, c.rule_round2.dsr_trials), (350, 352));
    // Programa Nasdaq (#526): las 354 de la ronda 3 más H1 y H2, mitades desde 2021.
    assert_eq!(c.rule_nasdaq.alpha, dec!(0.0125));
    assert_eq!(c.rule_nasdaq.dsr_trials, 356);
    assert_eq!(c.rule_nasdaq.second_half_from, day("2021-01-01"));
    for (from, to) in [
        ("horizon_minutes = 120", "horizon_minutes = 100"),
        ("cap = \"2\"", "cap = \"0\""),
        ("US30 = \"500\"", "US30 = \"-500\""),
        ("from = \"02:00\"", "from = \"04:00\""),
        ("min_training_pairs = 12", "min_training_pairs = 1"),
    ] {
        let bad = cfg!("research.toml").replace(from, to);
        assert!(ResearchConfig::from_toml(&bad).is_err(), "{to}");
    }
}

// ── alfa con errores HC1 ───────────────────────────────────────────────────────────────────

#[test]
fn the_hc1_alpha_matches_a_hand_computation() {
    let x = [0.01, -0.02, 0.03, 0.0, 0.015, -0.01];
    let y = [0.012, -0.015, 0.02, 0.004, 0.02, -0.004];
    let a = stats::alpha_hc1(&y, &x).unwrap();
    // Calculado aparte con (X'X)⁻¹ X' diag(e²) X (X'X)⁻¹ × n / (n − 2).
    assert!((a.alpha - 0.003_066_838_046_272_494_6).abs() < 1e-12);
    assert!((a.beta - 0.743_958_868_894_601_5).abs() < 1e-12);
    assert!((a.se_alpha - 0.001_524_934_570_081_782_5).abs() < 1e-12);
    assert!((a.t - 2.011_127_628_976_251_6).abs() < 1e-9);
    assert!(a.p_value > 0.0 && a.p_value < 0.1);
    // Sin variación en x o con pocos datos no hay regresión.
    assert!(stats::alpha_hc1(&[1.0, 2.0, 3.0], &[1.0, 1.0, 1.0]).is_none());
    assert!(stats::alpha_hc1(&[1.0, 2.0], &[1.0, 2.0]).is_none());
}

// ── familia V ──────────────────────────────────────────────────────────────────────────────

#[test]
fn the_realized_variance_sums_squared_daily_returns_since_the_previous_review() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    // Cada día sube 1 % al cierre de su última vela; el resto del día está plano.
    let m15 = {
        let mut out: Vec<Candle> = Vec::new();
        let mut level = dec!(1000);
        let mut d = day("2024-04-01");
        while d <= day("2024-06-28") {
            if let Some(Some((open, close))) = s.hours(d) {
                let mut t = open;
                while t + TimeDelta::minutes(15) <= close {
                    let o = out.last().map_or(level, |c| c.close);
                    let last = t + TimeDelta::minutes(30) > close;
                    if last {
                        level *= dec!(1.01);
                    }
                    out.push(candle(t, o, level));
                    t += TimeDelta::minutes(15);
                }
            }
            d = d.succ_opt().unwrap();
        }
        out
    };
    let (entries, _) = flags(s, &m15);
    let reviews: Vec<usize> = monthly_reviews(&m15, s, &entries)
        .into_iter()
        .map(|(_, t)| t)
        .collect();
    let closes = session_close_indices(&m15, s);
    assert_eq!(closes.len(), 63);
    let rv = review_variances(&m15, &[], &reviews, &closes);
    assert_eq!(rv[0], None);
    // Abril 30 (martes, 15:45) → mayo 31 (viernes: la revisión pasa al lunes 03-06 a las 09:45).
    // Entre medio: el cierre del 30-04 (+1 %) y los 22 días de mayo (+1 % cada uno), más el
    // tramo plano del lunes. Son 23 retornos de 1 % y algunos de 0.
    let v = rv[1].unwrap();
    assert!((v - dec!(0.0023)).abs() < dec!(0.0000001), "{v}");
}

#[test]
fn a_constant_volatility_gives_a_weight_of_one_and_the_cap_binds() {
    // Con la misma varianza en todas las revisiones, c = RV y el peso es 1.
    let m15 = run(
        "2024-01-02T14:30:00Z",
        &(0..40)
            .map(|k| if k % 2 == 0 { dec!(100) } else { dec!(101) })
            .collect::<Vec<_>>(),
    );
    let reviews: Vec<usize> = (0..40).step_by(2).collect();
    let rv: Vec<Option<Decimal>> = (0..reviews.len())
        .map(|k| (k > 0).then_some(dec!(0.0004)))
        .collect();
    // Los retornos de revisión a revisión son 0: desvío cero, sin constante.
    assert_eq!(
        vol_constant(
            &m15,
            &[],
            &reviews,
            &rv,
            at("2024-01-01T00:00:00Z"),
            at("2025-01-01T00:00:00Z"),
            5
        ),
        None
    );
    let reviews: Vec<usize> = (0..40).step_by(3).collect();
    let rv: Vec<Option<Decimal>> = (0..reviews.len())
        .map(|k| (k > 0).then_some(dec!(0.0004)))
        .collect();
    let c = vol_constant(
        &m15,
        &[],
        &reviews,
        &rv,
        at("2024-01-01T00:00:00Z"),
        at("2025-01-01T00:00:00Z"),
        5,
    )
    .unwrap();
    assert!((c - dec!(0.0004)).abs() < dec!(0.000000001), "{c}");
    let w = vol_weights(&rv, c, dec!(2));
    assert_eq!(w[0], None);
    assert!(
        w[1..]
            .iter()
            .all(|x| (x.unwrap() - Decimal::ONE).abs() < dec!(0.000001))
    );
    // Una varianza diez veces menor: el peso querría ser 10 y queda en el tope.
    let low = vol_weights(&[Some(dec!(0.00004))], c, dec!(2));
    assert_eq!(low[0], Some(dec!(2)));
    // Pocos pares en el entrenamiento: sin constante.
    assert_eq!(
        vol_constant(
            &m15,
            &[],
            &reviews,
            &rv,
            at("2024-01-01T00:00:00Z"),
            at("2025-01-01T00:00:00Z"),
            50
        ),
        None
    );
}

#[test]
fn weight_one_without_rebalances_is_exactly_buy_and_hold() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let setup = setup();
    let (from, to) = (at("2024-04-01T00:00:00Z"), at("2024-07-01T00:00:00Z"));
    let m15 = series(s, day("2024-03-28"), day("2024-06-28"), |k| {
        dec!(5000) + Decimal::from(k % 37) - Decimal::from(k % 11)
    });
    let roll = at("2024-06-21T13:30:00Z");
    // Una decisión antes de la ventana (28-03), ninguna adentro.
    let before = m15
        .iter()
        .position(|c| c.start >= at("2024-03-28T19:00:00Z"))
        .unwrap();
    for rolls in [vec![], vec![roll]] {
        let hold = buy_and_hold_returns(&m15, s, &rolls, &setup, from, to).unwrap();
        let managed = managed_returns(
            &m15,
            s,
            &rolls,
            &setup,
            (from, to),
            &[(before, Decimal::ONE)],
            Decimal::ONE,
        )
        .unwrap();
        assert_eq!(hold, managed);
    }
    // Sin ninguna decisión antes de la entrada no hay serie.
    let none = managed_returns(&m15, s, &[], &setup, (from, to), &[], Decimal::ONE).unwrap();
    assert!(none.is_empty());
}

#[test]
fn the_weight_scales_the_exposure_and_rebalances_at_the_next_open() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let setup = setup();
    let (from, to) = (at("2024-04-01T00:00:00Z"), at("2024-06-01T00:00:00Z"));
    // Plano en 5000; sube a 5500 en la primera vela del 02-05 (vela 624: 24 días × 26) y queda ahí.
    let m15 = series(s, day("2024-03-28"), day("2024-05-31"), |k| {
        if k < 624 { dec!(5000) } else { dec!(5500) }
    });
    let jump = m15.iter().position(|c| c.close == dec!(5500)).unwrap();
    let before = m15
        .iter()
        .position(|c| c.start >= at("2024-03-28T19:00:00Z"))
        .unwrap();
    let full = managed_returns(
        &m15,
        s,
        &[],
        &setup,
        (from, to),
        &[(before, dec!(1))],
        dec!(1),
    )
    .unwrap();
    let half = managed_returns(
        &m15,
        s,
        &[],
        &setup,
        (from, to),
        &[(before, dec!(0.5))],
        dec!(1),
    )
    .unwrap();
    // El mes del salto: con medio peso, la mitad del retorno (los costos también son la mitad).
    let (rf, rh) = (full[1].1, half[1].1);
    assert!(rf > dec!(0.09));
    assert!((rh - rf / dec!(2)).abs() < dec!(0.002), "{rh} vs {rf}");
    // Peso 0 hasta una revisión decidida en la vela del salto: entra en la apertura siguiente y
    // no gana el salto.
    let late = managed_returns(
        &m15,
        s,
        &[],
        &setup,
        (from, to),
        &[(before, dec!(0)), (jump, dec!(1))],
        dec!(1),
    )
    .unwrap();
    assert_eq!(late[0].1, Decimal::ZERO);
    assert!(
        late[1].1 < Decimal::ZERO && late[1].1 > dec!(-0.01),
        "{}",
        late[1].1
    );
    // Decidida una vela antes del salto: lo gana.
    let early = managed_returns(
        &m15,
        s,
        &[],
        &setup,
        (from, to),
        &[(before, dec!(0)), (jump - 1, dec!(1))],
        dec!(1),
    )
    .unwrap();
    assert!(early[1].1 > dec!(0.09));
    // Con costos × 1,5 el retorno es menor.
    let stressed = managed_returns(
        &m15,
        s,
        &[],
        &setup,
        (from, to),
        &[(before, dec!(1))],
        dec!(1.5),
    )
    .unwrap();
    assert!(stressed[0].1 < full[0].1 && stressed[1].1 < full[1].1);
}

// ── familia M1 ─────────────────────────────────────────────────────────────────────────────

fn m1_case(closes: &[Decimal]) -> Vec<Candle> {
    // Lunes 03-06-2024, desde las 10:00 ET (14:00 UTC): todas las decisiones están permitidas.
    run("2024-06-03T14:00:00Z", closes)
}

fn atr(n: usize) -> Vec<Option<Decimal>> {
    vec![Some(dec!(4)); n]
}

fn levels50(offset: Decimal) -> Levels {
    Levels {
        step: dec!(50),
        offset,
    }
}

#[test]
fn crossing_a_round_number_up_is_a_long_with_a_two_hour_exit() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let mut closes = vec![dec!(5040), dec!(5045), dec!(5050)];
    closes.extend(std::iter::repeat_n(dec!(5052), 12));
    let m15 = m1_case(&closes);
    let (entries, _) = flags(s, &m15);
    let plans = round_number_plans(
        &m15,
        &entries,
        &atr(m15.len()),
        &[],
        levels50(dec!(0)),
        dec!(2),
        TimeDelta::minutes(120),
    );
    assert_eq!(plans.len(), 1);
    let p = &plans[0];
    assert_eq!((p.signal_index, p.side), (2, Side::Long));
    assert_eq!(
        (p.reference, p.stop, p.risk_atr),
        (dec!(5050), dec!(5042), dec!(4))
    );
    // Decisión al cierre de 10:45 ET; salida al cierre de la vela que termina a las 12:45 ET.
    assert_eq!(
        m15[p.exit_index].end(),
        m15[2].end() + TimeDelta::minutes(120)
    );
    // El control (niveles 5025, 5075…) no ve ningún cruce acá.
    let control = round_number_plans(
        &m15,
        &entries,
        &atr(m15.len()),
        &[],
        levels50(dec!(25)),
        dec!(2),
        TimeDelta::minutes(120),
    );
    assert!(control.is_empty());
}

#[test]
fn crossing_down_is_a_short_and_touching_without_crossing_is_nothing() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let mut closes = vec![dec!(5060), dec!(5050), dec!(5051), dec!(5049)];
    closes.extend(std::iter::repeat_n(dec!(5049), 12));
    let m15 = m1_case(&closes);
    let (entries, _) = flags(s, &m15);
    let plans = round_number_plans(
        &m15,
        &entries,
        &atr(m15.len()),
        &[],
        levels50(dec!(0)),
        dec!(2),
        TimeDelta::minutes(120),
    );
    // 5060 → 5050: cierra en el nivel (R ≥ C[t]): bajista. 5050 → 5051: alcista? No: el nivel
    // tiene que estar por encima de C[t−1]. 5051 → 5049: cruza 5050 hacia abajo.
    let sides: Vec<(usize, Side)> = plans.iter().map(|p| (p.signal_index, p.side)).collect();
    assert_eq!(sides, vec![(1, Side::Short), (3, Side::Short)]);
    assert_eq!(plans[0].stop, dec!(5058));
}

#[test]
fn gaps_rollovers_blocked_decisions_and_short_horizons_skip_the_cross() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let mut closes = vec![dec!(5040), dec!(5055)];
    closes.extend(std::iter::repeat_n(dec!(5055), 12));
    let m15 = m1_case(&closes);
    let (entries, _) = flags(s, &m15);
    let base = |m15: &[Candle], entries: &[bool], rolls: &[DateTime<Utc>], h: i64| {
        round_number_plans(
            m15,
            entries,
            &atr(m15.len()),
            rolls,
            levels50(dec!(0)),
            dec!(2),
            TimeDelta::minutes(h),
        )
    };
    assert_eq!(base(&m15, &entries, &[], 120).len(), 1);
    // Vencimiento entre t − 1 y t.
    assert!(base(&m15, &entries, &[m15[1].start], 120).is_empty());
    // Decisión no permitida.
    let mut blocked = entries.clone();
    blocked[1] = false;
    assert!(base(&m15, &blocked, &[], 120).is_empty());
    // Hueco entre t − 1 y t.
    let mut gap = m15.clone();
    gap.remove(0);
    let mut shifted = m1_case(&[dec!(5040)]);
    shifted.extend(gap.into_iter().map(|mut c| {
        c.start += TimeDelta::minutes(15);
        c
    }));
    let (e2, _) = flags(s, &shifted);
    assert!(base(&shifted, &e2, &[], 120).is_empty());
    // Sin ninguna vela después de t + 1 dentro del horizonte.
    assert!(base(&m15, &entries, &[], 15).is_empty());
    // Sin ATR.
    let none = round_number_plans(
        &m15,
        &entries,
        &vec![None; m15.len()],
        &[],
        levels50(dec!(0)),
        dec!(2),
        TimeDelta::minutes(120),
    );
    assert!(none.is_empty());
}

#[test]
fn round_number_signals_do_not_repaint_when_more_candles_arrive() {
    let cal = calendar();
    let s = cal.session_for(&us500()).unwrap();
    let m15 = series(s, day("2024-06-03"), day("2024-06-14"), |k| {
        dec!(5000) + Decimal::from((k * 7) % 90)
    });
    let (entries, _) = flags(s, &m15);
    let a = atr(m15.len());
    let full = round_number_plans(
        &m15,
        &entries,
        &a,
        &[],
        levels50(dec!(0)),
        dec!(2),
        TimeDelta::minutes(120),
    );
    assert!(!full.is_empty());
    let cut = m15.len() / 2;
    let part = round_number_plans(
        &m15[..cut],
        &entries[..cut],
        &a[..cut],
        &[],
        levels50(dec!(0)),
        dec!(2),
        TimeDelta::minutes(120),
    );
    // Los planes del pedazo cuya salida ya estaba en el pedazo son idénticos.
    let settled: Vec<_> = part.iter().filter(|p| p.exit_index + 1 < cut).collect();
    assert!(!settled.is_empty());
    for p in settled {
        assert!(full.contains(p));
    }
}

// ── deriva nocturna ────────────────────────────────────────────────────────────────────────

#[test]
fn the_overnight_window_is_two_to_three_new_york_time_on_weekdays() {
    let ny: chrono_tz::Tz = "America/New_York".parse().unwrap();
    let (from, to) = (
        NaiveTime::from_hms_opt(2, 0, 0).unwrap(),
        NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
    );
    // Martes 05-03-2024 (EST, 02:00 ET = 07:00 UTC) y martes 12-03-2024 (EDT, 06:00 UTC).
    let mut m15 = run(
        "2024-03-05T07:00:00Z",
        &[dec!(100), dec!(101), dec!(101), dec!(102)],
    );
    m15.extend(run(
        "2024-03-12T06:00:00Z",
        &[dec!(200), dec!(199), dec!(199), dec!(198)],
    ));
    // Sábado 09-03: no cuenta.
    m15.extend(run(
        "2024-03-09T07:00:00Z",
        &[dec!(50), dec!(60), dec!(60), dec!(60)],
    ));
    m15.sort_by_key(|c| c.start);
    let r = window_returns(&m15, &[], ny, from, to);
    assert_eq!(r.len(), 2);
    // 05-03: abre 100 a las 07:00 UTC y cierra 102 a las 08:00 UTC.
    assert_eq!(r[0], (day("2024-03-05"), dec!(0.02)));
    // 12-03: abre 200 a las 06:00 UTC, cierra 198 a las 07:00 UTC.
    assert_eq!(r[1], (day("2024-03-12"), dec!(-0.01)));
    // Falta una vela: el día no cuenta. Un vencimiento adentro, tampoco.
    let mut missing = m15.clone();
    missing.retain(|c| c.start != at("2024-03-05T07:30:00Z"));
    assert_eq!(window_returns(&missing, &[], ny, from, to).len(), 1);
    assert_eq!(
        window_returns(&m15, &[at("2024-03-05T07:30:00Z")], ny, from, to).len(),
        1
    );
}

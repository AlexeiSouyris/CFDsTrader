//! Calificación del replay (#722): proceso antes que resultado. Una ganadora sin stop reprueba;
//! una perdedora bien hecha aprueba; el R se muestra con costos y no decide nada.

mod common;

use cfds_academy::AcademyConfig;
use cfds_academy::grade::{ExitKind, GradeConfig, ReplayCosts, Rule, grade};
use cfds_academy::replay::{
    Draw, MarkKind, MarkRequest, OrderRequest, OrderSide, ProcessRules, ReplayKind, ReplaySession,
};
use cfds_backtest::{BacktestConfig, Commission, InstrumentCosts};
use cfds_core::{Candle, Timeframe};
use cfds_market_data::Period;
use cfds_patterns::PatternsConfig;
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use common::{REAL, assert_valid};
use rust_decimal::{Decimal, dec};

const PATTERNS: &str = include_str!("../../../../config/patterns.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");

fn t0() -> DateTime<Utc> {
    "2013-03-04T08:00:00Z".parse().unwrap()
}

/// Velas de 15m desde los precios medios: cada vela va del anterior al siguiente, con medio punto
/// de mecha del lado al que va y un cuarto del otro (así una punta tiene una sola vela máxima).
fn bars(mids: &[Decimal]) -> Vec<Candle> {
    mids.windows(2)
        .enumerate()
        .map(|(i, w)| Candle {
            instrument_id: "US500".parse().unwrap(),
            timeframe: Timeframe::M15,
            start: t0() + TimeDelta::minutes(15 * i64::try_from(i).unwrap()),
            open: w[0],
            high: w[0].max(w[1]) + if w[1] >= w[0] { dec!(0.5) } else { dec!(0.25) },
            low: w[0].min(w[1]) - if w[1] <= w[0] { dec!(0.5) } else { dec!(0.25) },
            close: w[1],
            volume: None,
        })
        .collect()
}

fn session(kind: ReplayKind, candles: Vec<Candle>, warmup: usize) -> ReplaySession {
    let draw = Draw {
        instrument_id: "US500".parse().unwrap(),
        timeframe: Timeframe::M15,
        research: Period {
            start: t0(),
            end: t0() + TimeDelta::days(400),
        },
    };
    ReplaySession::new(
        "rp-0000000000000001".into(),
        kind,
        None,
        1,
        draw,
        candles,
        warmup,
    )
    .unwrap()
}

/// 30 velas planas alrededor de 100 (ATR de 1,25 con estas mechas) y después `tail`.
fn flat_then(tail: &[Decimal]) -> Vec<Decimal> {
    let mut mids: Vec<Decimal> = (0..31)
        .map(|i| if i % 2 == 0 { dec!(100) } else { dec!(100.5) })
        .collect();
    mids.extend_from_slice(tail);
    mids
}

fn costs() -> ReplayCosts {
    ReplayCosts {
        costs: InstrumentCosts {
            spread: dec!(0.9),
            tick: dec!(0.1),
            commission: Commission::None,
            funding_admin_fee_pct: dec!(3),
            funding_day_count: dec!(360),
        },
        execution: BacktestConfig::from_toml(BACKTEST).unwrap().costs,
        funding_cutoff: NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
        funding_tz: chrono_tz::UTC,
    }
}

fn run(s: &ReplaySession, trades_per_exam: usize) -> cfds_academy::grade::ExamResult {
    let cfg = AcademyConfig::from_toml(REAL).unwrap();
    let mut replay = cfg.replay.clone();
    replay.trades_per_exam = trades_per_exam;
    let patterns = PatternsConfig::from_toml(PATTERNS).unwrap();
    let rules = ProcessRules {
        stop_min_atr: dec!(0.5),
        stop_max_atr: dec!(3),
        max_risk_pct: dec!(1),
        provisional: true,
    };
    let g = GradeConfig {
        replay: &replay,
        rules,
        patterns: &patterns,
        pass_pct: cfg.catalog.thresholds.quiz_pass_pct,
        catalog_version: &cfg.catalog.catalog_version,
    };
    let r = grade(s, g, &costs(), "2026-11-26T23:00:00Z".parse().unwrap()).unwrap();
    assert_valid(
        "exam_result.v1.schema.json",
        &serde_json::to_value(&r).unwrap(),
    );
    r
}

fn long(stop: Option<Decimal>, target: Option<Decimal>, risk: Decimal, plan: &str) -> OrderRequest {
    OrderRequest {
        side: OrderSide::Long,
        stop,
        target,
        risk_pct: risk,
        plan: plan.into(),
    }
}

fn failed(rules: &[cfds_academy::grade::RuleCheck]) -> Vec<Rule> {
    rules.iter().filter(|r| !r.passed).map(|r| r.rule).collect()
}

#[test]
fn a_winner_without_stop_fails_and_a_well_made_loser_passes() {
    // Sube a 106 (toca el objetivo de 105).
    let up = flat_then(&[dec!(101), dec!(103), dec!(105.5), dec!(106), dec!(106)]);
    let mut s = session(ReplayKind::Trades, bars(&up), 30);
    s.order(long(None, Some(dec!(105)), dec!(1), "Compro la ruptura"))
        .unwrap();
    s.finish().unwrap();
    let r = run(&s, 1);
    assert_eq!(r.passed, Some(false));
    assert_eq!(
        failed(&r.trades[0].rules),
        [Rule::StopPresent, Rule::StopSide, Rule::StopInRange]
    );
    let o = r.trades[0].outcome.as_ref().unwrap();
    assert_eq!(o.exit, ExitKind::Target);
    assert_eq!(o.r_gross, None, "sin stop no hay R");

    // Baja a 97: el stop de 98 salta. Proceso completo: aprueba aunque pierde.
    let down = flat_then(&[dec!(100), dec!(99), dec!(97.5), dec!(97), dec!(97)]);
    let mut s = session(ReplayKind::Trades, bars(&down), 30);
    s.order(long(
        Some(dec!(98)),
        None,
        dec!(1),
        "Compro el soporte; stop debajo",
    ))
    .unwrap();
    s.finish().unwrap();
    let r = run(&s, 1);
    assert_eq!(r.passed, Some(true), "{:?}", r.trades[0].rules);
    let o = r.trades[0].outcome.as_ref().unwrap();
    assert_eq!(o.exit, ExitKind::Stop);
    let (g, c, n) = (o.r_gross.unwrap(), o.costs_r.unwrap(), o.r_net.unwrap());
    assert!(
        g < Decimal::ZERO && c > Decimal::ZERO && n == g - c,
        "{g} {c} {n}"
    );
    // Entrada en la apertura de t+1 con medio spread (0,45) más slippage (0,1 × 0,9 + 1 tick).
    assert_eq!(o.entry_index, 30);
    assert_eq!(o.entry_price, dec!(100.64));
}

#[test]
fn each_process_rule_is_checked_with_what_was_known_at_the_decision() {
    let tail: Vec<Decimal> = (0..40).map(|i| dec!(100) + Decimal::from(i % 3)).collect();
    let mut s = session(ReplayKind::Trades, bars(&flat_then(&tail)), 30);
    let cases = [
        (
            long(Some(dec!(99.9)), None, dec!(1), "plan"),
            vec![Rule::StopInRange],
        ),
        (
            long(Some(dec!(80)), None, dec!(1), "plan"),
            vec![Rule::StopInRange],
        ),
        (
            long(Some(dec!(102)), None, dec!(1), "plan"),
            vec![Rule::StopSide, Rule::StopInRange],
        ),
        (
            long(Some(dec!(98)), None, dec!(2), "plan"),
            vec![Rule::RiskWithinLimit],
        ),
        (
            long(Some(dec!(98)), None, dec!(1), "   "),
            vec![Rule::PlanWritten],
        ),
    ];
    for (order, _) in &cases {
        s.order(order.clone()).unwrap();
        s.step(8, 100).unwrap();
    }
    s.finish().unwrap();
    let r = run(&s, 10);
    assert_eq!(r.passed, Some(false));
    assert_eq!(failed(&r.rules), [Rule::EnoughTrades]);
    for (i, (_, expected)) in cases.iter().enumerate() {
        let mut want = expected.clone();
        // Las que no tocan el stop siguen abiertas hasta el final: la siguiente choca con ella.
        if i > 0 && r.trades[i - 1].outcome.as_ref().unwrap().exit_index > s.orders[i].cursor {
            want.push(Rule::OnePosition);
        }
        assert_eq!(failed(&r.trades[i].rules), want, "entrada {}", i + 1);
    }
}

#[test]
fn holding_overnight_pays_the_admin_fee_in_r() {
    // 60 velas de 15m desde las 08:00: cruza el corte de las 22:00 UTC.
    let tail: Vec<Decimal> = (0..60).map(|_| dec!(100)).collect();
    let mut s = session(ReplayKind::Practice, bars(&flat_then(&tail)), 30);
    s.order(long(Some(dec!(98)), None, dec!(1), "plan"))
        .unwrap();
    s.finish().unwrap();
    let r = run(&s, 1);
    assert_eq!(r.passed, None, "una práctica no tiene nota");
    let o = r.trades[0].outcome.as_ref().unwrap();
    assert_eq!(o.exit, ExitKind::End);
    // Spread y slippage de ida y vuelta: 2 × (0,45 + 0,19) = 1,28 puntos; una noche de tasa
    // administrativa: 100 × 3 % / 360 = 0,008333…; sobre un R de 2 puntos (100 − 98).
    let expected =
        ((dec!(1.28) + dec!(100) * dec!(3) / dec!(100) / dec!(360)) / dec!(2)).round_dp(4);
    assert_eq!(o.costs_r.unwrap(), expected.normalize());
}

#[test]
fn levels_are_graded_against_the_engine_with_only_the_bars_up_to_the_cursor() {
    // Ondas entre 90 y 110 (pivotes en 110,5 y 89,5); después del cursor, ondas hasta 120.
    let wave = |top: i64| -> Vec<Decimal> {
        (0..10)
            .map(|i| {
                Decimal::from(if i <= 5 {
                    90 + 4 * i
                } else {
                    top - 4 * (i - 5)
                })
            })
            .collect()
    };
    let mut mids = Vec::new();
    for _ in 0..8 {
        mids.extend(wave(110));
    }
    let seen = mids.len() - 1;
    for _ in 0..6 {
        mids.extend((0..10).map(|i| {
            Decimal::from(if i <= 5 {
                100 + 4 * i
            } else {
                120 - 4 * (i - 5)
            })
        }));
    }
    let candles = bars(&mids);
    let mut s = session(ReplayKind::Levels, candles, seen - 1);
    let mark = |p: Decimal| MarkRequest {
        kind: MarkKind::Level,
        price: p,
        bar_index: None,
    };
    s.mark(mark(dec!(110))).unwrap();
    s.mark(mark(dec!(90))).unwrap();
    s.finish().unwrap();
    let r = run(&s, 1);
    let l = r.levels.as_ref().unwrap();
    assert!(
        l.engine_levels.iter().all(|p| *p < dec!(115)),
        "un nivel de después del cursor: {:?}",
        l.engine_levels
    );
    assert_eq!((l.marked, l.matched_marks), (2, 2), "{l:?}");
    assert_eq!(l.precision_pct, Some(dec!(100)));
    assert_eq!(r.passed, Some(true));

    let mut s = session(ReplayKind::Levels, bars(&mids), seen - 1);
    s.mark(mark(dec!(110))).unwrap();
    s.mark(mark(dec!(100))).unwrap();
    s.finish().unwrap();
    let r = run(&s, 1);
    assert_eq!(r.levels.as_ref().unwrap().precision_pct, Some(dec!(50)));
    assert_eq!(r.passed, Some(false));
    assert_eq!(failed(&r.rules), [Rule::LevelPrecision]);
}

#[test]
fn an_open_session_cannot_be_graded() {
    let s = session(
        ReplayKind::Trades,
        bars(&flat_then(&[dec!(100), dec!(100)])),
        30,
    );
    let cfg = AcademyConfig::from_toml(REAL).unwrap();
    let patterns = PatternsConfig::from_toml(PATTERNS).unwrap();
    let rules = ProcessRules {
        stop_min_atr: dec!(0.5),
        stop_max_atr: dec!(3),
        max_risk_pct: dec!(1),
        provisional: true,
    };
    let g = GradeConfig {
        replay: &cfg.replay,
        rules,
        patterns: &patterns,
        pass_pct: dec!(80),
        catalog_version: "x",
    };
    assert!(grade(&s, g, &costs(), t0()).is_err());
}

//! Probabilidad mostrada de la spec §13 (#175).

use cfds_core::InstrumentId;
use cfds_journal::probability::{
    Group, ProbabilityConfig, ProbabilityError, ProbabilityTable, Shown, wilson,
};
use cfds_journal::{
    BreakdownItem, Direction, Exit, Outcome, OutcomeKind, Proposal, Reason, Signal, Source,
};
use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal::dec;

const REAL: &str = include_str!("../../../../config/probability.toml");

fn cfg() -> ProbabilityConfig {
    ProbabilityConfig::from_toml(REAL).unwrap()
}

fn t0() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 1, 6, 15, 0, 0).unwrap()
}

/// Corte = inicio del tramo fuera de muestra.
fn cutoff() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

/// Propuesta `i` con sus gatillos y puntaje, y su resultado que termina en `ends`.
fn pair(
    i: u32,
    triggers: &[Signal],
    score: i32,
    hit: bool,
    ends: DateTime<Utc>,
) -> (Proposal, Outcome) {
    let at = t0() + Duration::minutes(15 * i64::from(i));
    let id = format!("bt:US100:{i}:long");
    let p = Proposal {
        id: id.clone(),
        source: Source::Backtest,
        run_id: "bt".into(),
        instrument_id: "US100".parse::<InstrumentId>().unwrap(),
        side: Direction::Long,
        signal_at: at,
        reference: dec!(100),
        stop: dec!(99),
        tp1: dec!(102),
        size: dec!(1),
        leverage: dec!(1),
        score,
        breakdown: vec![BreakdownItem {
            component: Signal::Trend,
            points: 2,
        }],
        triggers: triggers.to_vec(),
        shown: None,
    };
    let o = Outcome {
        proposal_id: id,
        kind: OutcomeKind::Simulated,
        entry_at: at + Duration::minutes(15),
        entry_price: dec!(100),
        exits: vec![Exit {
            at: ends,
            price: if hit { dec!(102) } else { dec!(99) },
            fraction: dec!(1),
            reason: if hit { Reason::Tp1 } else { Reason::Stop },
        }],
        commission: dec!(0),
        funding: dec!(0),
        gross_pnl: dec!(0),
        net_pnl: dec!(0),
        hit_tp1: hit,
        hit_stop: !hit,
        exit_reason: if hit { Reason::Tp1 } else { Reason::Stop },
    };
    (p, o)
}

/// `n` pares de sweep con puntaje `score`, `hits` de ellos aciertos, todos antes del corte.
fn sweeps(n: u32, hits: u32, score: i32) -> Vec<(Proposal, Outcome)> {
    (0..n)
        .map(|i| {
            pair(
                i,
                &[Signal::Sweep],
                score,
                i < hits,
                t0() + Duration::days(i64::from(i) + 1),
            )
        })
        .collect()
}

#[test]
fn the_shipped_config_is_valid() {
    let c = cfg();
    assert_eq!(c.min_cases, 30);
    assert_eq!(c.z, dec!(1.96));
    assert_eq!(c.score_bands, vec![5, 6, 7, 9]);
}

#[test]
fn estimate_with_wilson_interval_from_thirty_cases() {
    let table = ProbabilityTable::build(&sweeps(30, 18, 6), cutoff(), &cfg()).unwrap();
    let probe = sweeps(1, 0, 6).remove(0).0;
    match table.shown(&probe) {
        Shown::Estimate {
            probability,
            low,
            high,
            hits,
            cases,
        } => {
            assert_eq!((hits, cases), (18, 30));
            assert_eq!(probability, dec!(0.6));
            // Wilson 95 % de 18/30: [0,4232; 0,7541] (calculado aparte).
            assert_eq!(low, dec!(0.4232));
            assert_eq!(high, dec!(0.7541));
        }
        other => panic!("esperaba una estimación: {other:?}"),
    }
}

#[test]
fn fewer_than_min_cases_is_insufficient() {
    let table = ProbabilityTable::build(&sweeps(29, 29, 6), cutoff(), &cfg()).unwrap();
    let probe = sweeps(1, 0, 6).remove(0).0;
    assert_eq!(
        table.shown(&probe),
        Shown::Insufficient {
            cases: 29,
            needed: 30
        }
    );
}

#[test]
fn no_cases_and_no_group_are_insufficient() {
    let table = ProbabilityTable::build(&[], cutoff(), &cfg()).unwrap();
    let probe = sweeps(1, 0, 6).remove(0).0;
    assert_eq!(
        table.shown(&probe),
        Shown::Insufficient {
            cases: 0,
            needed: 30
        }
    );
    // Puntaje por debajo del primer rango: sin grupo.
    let low = sweeps(1, 0, 4).remove(0).0;
    assert_eq!(cfg().group(&low), None);
}

#[test]
fn all_hits_or_none_stay_inside_zero_and_one() {
    for hits in [0, 40] {
        let table = ProbabilityTable::build(&sweeps(40, hits, 6), cutoff(), &cfg()).unwrap();
        let probe = sweeps(1, 0, 6).remove(0).0;
        let Shown::Estimate {
            low,
            high,
            probability,
            ..
        } = table.shown(&probe)
        else {
            panic!("esperaba una estimación")
        };
        assert!(Decimal::ZERO <= low && low <= probability && probability <= high);
        assert!(high <= Decimal::ONE);
        if hits == 0 {
            assert_eq!(low, Decimal::ZERO);
        } else {
            assert_eq!(high, Decimal::ONE);
        }
    }
}

#[test]
fn outcomes_that_end_after_the_cutoff_do_not_count() {
    let mut pairs = sweeps(30, 30, 6);
    // Uno termina un segundo después del inicio del fuera de muestra: no cuenta.
    pairs.push(pair(
        99,
        &[Signal::Sweep],
        6,
        false,
        cutoff() + Duration::seconds(1),
    ));
    // Uno termina justo en el corte: cuenta.
    pairs.push(pair(98, &[Signal::Sweep], 6, false, cutoff()));
    let table = ProbabilityTable::build(&pairs, cutoff(), &cfg()).unwrap();
    let g = Group {
        trigger: Signal::Sweep,
        band: 1,
    };
    assert_eq!(table.counts(g), (30, 31));
}

#[test]
fn groups_by_preferred_trigger_and_score_band() {
    let c = cfg();
    let both = pair(1, &[Signal::CandlePattern, Signal::Sweep], 7, true, t0()).0;
    assert_eq!(
        c.group(&both),
        Some(Group {
            trigger: Signal::Sweep,
            band: 2
        })
    );
    let candle = pair(
        2,
        &[Signal::CandlePattern, Signal::LevelRejection],
        8,
        true,
        t0(),
    )
    .0;
    assert_eq!(
        c.group(&candle),
        Some(Group {
            trigger: Signal::LevelRejection,
            band: 2
        })
    );
    let high = pair(3, &[Signal::DoubleBreak], 12, true, t0()).0;
    assert_eq!(
        c.group(&high),
        Some(Group {
            trigger: Signal::DoubleBreak,
            band: 3
        })
    );
}

#[test]
fn other_groups_do_not_mix() {
    let mut pairs = sweeps(30, 30, 6);
    pairs.extend((100..130).map(|i| pair(i, &[Signal::CandlePattern], 6, false, t0())));
    let table = ProbabilityTable::build(&pairs, cutoff(), &cfg()).unwrap();
    let candle = pair(500, &[Signal::CandlePattern], 6, false, t0()).0;
    let Shown::Estimate { hits, cases, .. } = table.shown(&candle) else {
        panic!()
    };
    assert_eq!((hits, cases), (0, 30));
    // Otro rango de puntaje del mismo gatillo: sin casos.
    let sweep9 = pair(501, &[Signal::Sweep], 9, false, t0()).0;
    assert_eq!(
        table.shown(&sweep9),
        Shown::Insufficient {
            cases: 0,
            needed: 30
        }
    );
}

#[test]
fn mismatched_or_non_backtest_pairs_are_errors() {
    let (p, mut o) = pair(1, &[Signal::Sweep], 6, true, t0());
    o.proposal_id = "otro".into();
    assert!(matches!(
        ProbabilityTable::build(&[(p.clone(), o)], cutoff(), &cfg()),
        Err(ProbabilityError::Mismatch { .. })
    ));
    let (mut p2, o2) = pair(2, &[Signal::Sweep], 6, true, t0());
    p2.source = Source::Paper;
    assert!(matches!(
        ProbabilityTable::build(&[(p2, o2)], cutoff(), &cfg()),
        Err(ProbabilityError::NotBacktest(_))
    ));
}

#[test]
fn wilson_matches_a_known_value() {
    // 5/10 con z = 1,96: [0,2366; 0,7634].
    let (lo, hi) = wilson(5, 10, dec!(1.96));
    assert_eq!(
        (lo.round_dp(4), hi.round_dp(4)),
        (dec!(0.2366), dec!(0.7634))
    );
}

#[test]
fn invalid_config_is_rejected() {
    let base = "min_cases = 30\nz = \"1.96\"\nscore_bands = [5, 6, 7, 9]\ntrigger_priority = [\"sweep\", \"double_break\", \"level_rejection\", \"candle_pattern\"]\n";
    assert!(ProbabilityConfig::from_toml(base).is_ok());
    for (from, to) in [
        ("min_cases = 30", "min_cases = 0"),
        ("z = \"1.96\"", "z = \"0\""),
        ("z = \"1.96\"", "z = \"abc\""),
        ("[5, 6, 7, 9]", "[5, 5, 7]"),
        ("[5, 6, 7, 9]", "[]"),
        ("\"sweep\", \"double_break\"", "\"sweep\", \"sweep\""),
        ("\"sweep\", \"double_break\"", "\"trend\", \"double_break\""),
        ("min_cases = 30", "min_cases = 30\nextra = 1"),
    ] {
        let text = base.replace(from, to);
        assert!(
            matches!(
                ProbabilityConfig::from_toml(&text),
                Err(ProbabilityError::Config(_))
            ),
            "debería fallar: {to}"
        );
    }
}

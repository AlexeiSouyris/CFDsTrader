//! Una prueba por cada regla de puntaje (spec §6) y de stop (spec §8), con los espejos (#45).

use cfds_indicators::Trend;
use cfds_strategy::{
    Component, Evidence, ScoreConfig, ScoreError, Side, StopConfig, StrategyConfig, Trigger,
    TriggerKind, risk_per_unit, score, structural_stop,
};
use rust_decimal::{Decimal, dec};

fn cfg() -> StrategyConfig {
    StrategyConfig::from_toml(include_str!("../../../../config/strategy.toml")).unwrap()
}

fn sc() -> ScoreConfig {
    cfg().score
}

fn stop_cfg() -> StopConfig {
    cfg().stop
}

fn trig(kind: TriggerKind, structure: Decimal) -> Trigger {
    Trigger { kind, structure }
}

/// Evidencia mínima: un martillo, tendencia neutral, nada más. Puntaje 1.
fn base(side: Side) -> Evidence {
    Evidence {
        side,
        trend_1h: Trend::Neutral,
        sma_side: false,
        triggers: vec![trig(TriggerKind::CandlePattern, dec!(100))],
        fvg_or_order_block: false,
        recent_choch: false,
        news: 0,
    }
}

fn total(e: &Evidence) -> i32 {
    score(e, &sc()).unwrap().unwrap().total
}

fn opposite(t: Trend) -> Trend {
    match t {
        Trend::Up => Trend::Down,
        Trend::Down => Trend::Up,
        Trend::Neutral => Trend::Neutral,
    }
}

/// Corre la misma comprobación para el largo y su espejo corto.
fn both_sides(check: impl Fn(Side, Trend)) {
    check(Side::Long, Trend::Up);
    check(Side::Short, Trend::Down);
}

// ---------- Puntaje ----------

#[test]
fn without_a_trigger_there_is_no_signal() {
    both_sides(|side, with| {
        let e = Evidence {
            triggers: vec![],
            trend_1h: with,
            sma_side: true,
            fvg_or_order_block: true,
            recent_choch: true,
            news: 2,
            ..base(side)
        };
        assert_eq!(score(&e, &sc()), Ok(None));
    });
}

#[test]
fn trend_with_adds_2_against_subtracts_3_neutral_adds_0() {
    both_sides(|side, with| {
        assert_eq!(total(&base(side)), 1);
        assert_eq!(
            total(&Evidence {
                trend_1h: with,
                ..base(side)
            }),
            3
        );
        let against = Evidence {
            trend_1h: opposite(with),
            ..base(side)
        };
        assert_eq!(total(&against), -2);
    });
}

#[test]
fn right_side_of_the_sma_adds_1() {
    both_sides(|side, _| {
        assert_eq!(
            total(&Evidence {
                sma_side: true,
                ..base(side)
            }),
            2
        );
    });
}

#[test]
fn each_trigger_has_its_weight() {
    for (kind, expected) in [
        (TriggerKind::CandlePattern, 1),
        (TriggerKind::DoubleBreak, 2),
        (TriggerKind::LevelRejection, 1),
        (TriggerKind::Sweep, 2),
    ] {
        both_sides(|side, _| {
            let e = Evidence {
                triggers: vec![trig(kind, dec!(100))],
                ..base(side)
            };
            assert_eq!(total(&e), expected, "{kind:?}");
        });
    }
}

#[test]
fn a_trigger_kind_counts_once() {
    // Martillo y envolvente en la misma vela: "martillo o envolvente" suma 1.
    let e = Evidence {
        triggers: vec![
            trig(TriggerKind::CandlePattern, dec!(100)),
            trig(TriggerKind::CandlePattern, dec!(99)),
        ],
        ..base(Side::Long)
    };
    assert_eq!(total(&e), 1);
}

#[test]
fn fvg_or_order_block_and_recent_choch_add_1_each() {
    both_sides(|side, _| {
        let fvg = Evidence {
            fvg_or_order_block: true,
            ..base(side)
        };
        assert_eq!(total(&fvg), 2);
        let choch = Evidence {
            recent_choch: true,
            ..base(side)
        };
        assert_eq!(total(&choch), 2);
    });
}

#[test]
fn news_adjust_between_minus_2_and_plus_2() {
    for news in -2..=2 {
        assert_eq!(
            total(&Evidence {
                news,
                ..base(Side::Long)
            }),
            1 + news
        );
    }
    for news in [-3, 3] {
        assert_eq!(
            score(
                &Evidence {
                    news,
                    ..base(Side::Long)
                },
                &sc()
            ),
            Err(ScoreError::NewsOutOfRange {
                news,
                min: -2,
                max: 2
            })
        );
    }
}

#[test]
fn a_score_of_5_is_a_candidate_and_4_is_not() {
    both_sides(|side, with| {
        // Tendencia a favor (2) + martillo (1) + SMA (1) = 4.
        let four = Evidence {
            trend_1h: with,
            sma_side: true,
            ..base(side)
        };
        let s = score(&four, &sc()).unwrap().unwrap();
        assert_eq!((s.total, s.candidate), (4, false));
        let five = Evidence {
            recent_choch: true,
            ..four
        };
        let s = score(&five, &sc()).unwrap().unwrap();
        assert_eq!((s.total, s.candidate), (5, true));
    });
}

#[test]
fn the_breakdown_explains_the_total() {
    let e = Evidence {
        trend_1h: Trend::Down,
        sma_side: true,
        triggers: vec![
            trig(TriggerKind::Sweep, dec!(95)),
            trig(TriggerKind::DoubleBreak, dec!(96)),
        ],
        fvg_or_order_block: true,
        recent_choch: true,
        news: -1,
        ..base(Side::Long)
    };
    let s = score(&e, &sc()).unwrap().unwrap();
    assert_eq!(
        s.breakdown,
        vec![
            (Component::Trend, -3),
            (Component::SmaSide, 1),
            (Component::Trigger(TriggerKind::DoubleBreak), 2),
            (Component::Trigger(TriggerKind::Sweep), 2),
            (Component::FvgOrOrderBlock, 1),
            (Component::RecentChoch, 1),
            (Component::News, -1),
        ]
    );
    assert_eq!(s.total, 3);
    assert_eq!(s.total, s.breakdown.iter().map(|(_, w)| w).sum::<i32>());
}

// ---------- Stop y R ----------

#[test]
fn long_stop_is_the_lowest_structure_minus_half_atr() {
    let triggers = [
        trig(TriggerKind::CandlePattern, dec!(99)),
        trig(TriggerKind::Sweep, dec!(97.5)),
    ];
    let stop = structural_stop(Side::Long, &triggers, dec!(2), &stop_cfg());
    assert_eq!(stop, Some(dec!(96.5)));
    assert_eq!(
        risk_per_unit(Side::Long, dec!(101), dec!(96.5)),
        Some(dec!(4.5))
    );
}

#[test]
fn short_stop_is_the_highest_structure_plus_half_atr() {
    let triggers = [
        trig(TriggerKind::CandlePattern, dec!(101)),
        trig(TriggerKind::Sweep, dec!(102.5)),
    ];
    let stop = structural_stop(Side::Short, &triggers, dec!(2), &stop_cfg());
    assert_eq!(stop, Some(dec!(103.5)));
    assert_eq!(
        risk_per_unit(Side::Short, dec!(99), dec!(103.5)),
        Some(dec!(4.5))
    );
}

#[test]
fn no_stop_without_triggers_or_with_a_non_positive_atr() {
    let t = [trig(TriggerKind::CandlePattern, dec!(99))];
    assert_eq!(structural_stop(Side::Long, &[], dec!(2), &stop_cfg()), None);
    assert_eq!(structural_stop(Side::Long, &t, dec!(0), &stop_cfg()), None);
    assert_eq!(
        structural_stop(Side::Short, &t, dec!(-1), &stop_cfg()),
        None
    );
}

#[test]
fn a_stop_on_the_wrong_side_has_no_risk_defined() {
    assert_eq!(risk_per_unit(Side::Long, dec!(100), dec!(100)), None);
    assert_eq!(risk_per_unit(Side::Long, dec!(100), dec!(101)), None);
    assert_eq!(risk_per_unit(Side::Short, dec!(100), dec!(99)), None);
}

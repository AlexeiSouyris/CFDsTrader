//! Informe de aprendizaje continuo (fase 8b, #327): calibración verificable a mano, aporte por
//! señal con muestras chicas marcadas y propuestas de cambio pendientes de voto. Datos
//! sintéticos, no de mercado.

use cfds_core::InstrumentId;
use cfds_journal::learning::{
    ChangeKind, Inputs, LearningConfig, LearningReport, PENDING_VOTE, report,
};
use cfds_journal::probability::{ProbabilityConfig, ProbabilityTable, wilson};
use cfds_journal::{
    BreakdownItem, Direction, Exit, Outcome, OutcomeKind, Proposal, Reason, Signal, Source,
};
use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::{Decimal, dec};

const PROBABILITY: &str = include_str!("../../../../config/probability.toml");
const LEARNING: &str = include_str!("../../../../config/learning.toml");

fn prob() -> ProbabilityConfig {
    ProbabilityConfig::from_toml(PROBABILITY).unwrap()
}

fn cfg() -> LearningConfig {
    LearningConfig::from_toml(LEARNING).unwrap()
}

fn t0() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 1, 6, 15, 0, 0).unwrap()
}

fn cutoff() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

/// Una propuesta con su resultado. `net` en moneda; con R = |100 − 99| × 1 = 1, `net` es R.
#[allow(clippy::too_many_arguments)]
fn pair(
    source: Source,
    i: u32,
    start: DateTime<Utc>,
    trigger: Signal,
    extra: &[Signal],
    hit: bool,
    net: Decimal,
) -> (Proposal, Outcome) {
    let at = start + Duration::minutes(15 * i64::from(i));
    let id = format!("{source:?}:US100:{i}:long");
    let mut breakdown = vec![BreakdownItem {
        component: Signal::Trend,
        points: 2,
    }];
    breakdown.extend(extra.iter().map(|&s| BreakdownItem {
        component: s,
        points: 1,
    }));
    let p = Proposal {
        id: id.clone(),
        source,
        run_id: "r".into(),
        instrument_id: "US100".parse::<InstrumentId>().unwrap(),
        side: Direction::Long,
        signal_at: at,
        reference: dec!(100),
        stop: dec!(99),
        tp1: dec!(102),
        size: dec!(1),
        leverage: dec!(1),
        score: 5,
        breakdown,
        triggers: vec![trigger],
    };
    let o = Outcome {
        proposal_id: id,
        kind: OutcomeKind::Simulated,
        entry_at: at + Duration::minutes(15),
        entry_price: dec!(100),
        exits: vec![Exit {
            at: at + Duration::hours(1),
            price: dec!(100) + net,
            fraction: dec!(1),
            reason: if hit { Reason::Tp1 } else { Reason::Stop },
        }],
        commission: dec!(0),
        funding: dec!(0),
        gross_pnl: net,
        net_pnl: net,
        hit_tp1: hit,
        hit_stop: !hit,
        exit_reason: if hit { Reason::Tp1 } else { Reason::Stop },
    };
    (p, o)
}

/// Entrenamiento: 40 de vela (candle_pattern) con 20 aciertos (se muestra 50 %) y 40 de sweep
/// con 32 (se muestra 80 %).
fn training() -> ProbabilityTable {
    let mut pairs = Vec::new();
    for i in 0..40 {
        pairs.push(pair(
            Source::Backtest,
            i,
            t0(),
            Signal::CandlePattern,
            &[],
            i % 2 == 0,
            dec!(1),
        ));
        pairs.push(pair(
            Source::Backtest,
            100 + i,
            t0(),
            Signal::Sweep,
            &[],
            i % 5 != 0,
            dec!(1),
        ));
    }
    ProbabilityTable::build(&pairs, cutoff(), &prob()).unwrap()
}

fn build(pairs: &[(Proposal, Outcome)]) -> LearningReport {
    let table = training();
    let (from, to) = (cutoff(), cutoff() + Duration::days(90));
    report(&Inputs {
        pairs,
        table: &table,
        probability: &prob(),
        cfg: &cfg(),
        from,
        to,
        created_at: to,
    })
}

fn validator() -> jsonschema::Validator {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/learning_report.v1.schema.json");
    let text = std::fs::read_to_string(path).unwrap();
    jsonschema::validator_for(&serde_json::from_str(&text).unwrap()).unwrap()
}

fn assert_contract(r: &LearningReport) {
    let doc = serde_json::to_value(r).unwrap();
    let errors: Vec<String> = validator()
        .iter_errors(&doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn the_repo_config_is_valid_and_bad_ones_are_rejected() {
    let c = cfg();
    assert_eq!(c.buckets.first(), Some(&Decimal::ZERO));
    assert_eq!(c.buckets.last(), Some(&Decimal::ONE));
    for (old, new) in [
        ("\"0\", \"0.1\"", "\"0.1\", \"0\""),
        ("\"0.9\", \"1\"", "\"0.9\""),
        ("sample_per_bucket = 5", "sample_per_bucket = 0"),
        ("min_cases = 30", "min_cases = 0"),
    ] {
        let bad = LEARNING.replacen(old, new, 1);
        assert_ne!(bad, LEARNING);
        assert!(LearningConfig::from_toml(&bad).is_err(), "{new}");
    }
}

#[test]
fn calibration_can_be_checked_by_hand_and_a_miss_becomes_a_proposal() {
    // Paper: 40 de vela (se mostró 50 %) con solo 10 aciertos, y 40 de sweep (80 %) con 32.
    let mut pairs = Vec::new();
    for i in 0..40 {
        pairs.push(pair(
            Source::Paper,
            i,
            cutoff(),
            Signal::CandlePattern,
            &[],
            i % 4 == 0,
            dec!(1),
        ));
        pairs.push(pair(
            Source::Paper,
            100 + i,
            cutoff(),
            Signal::Sweep,
            &[],
            i % 5 != 0,
            dec!(1),
        ));
    }
    let r = build(&pairs);
    assert_contract(&r);
    assert_eq!(r.proposals, 80);
    assert_eq!(r.sources, vec![Source::Paper]);
    // Rango [0,5, 0,6): 40 casos, 10 aciertos → 25 %, Wilson con z = 1,96, a mano.
    let half = &r.calibration.rows[5];
    assert_eq!((half.from, half.to), (dec!(0.5), dec!(0.6)));
    assert_eq!((half.cases, half.hits), (40, 10));
    assert_eq!(half.mean_shown, Some(dec!(0.5)));
    assert_eq!(half.frequency, Some(dec!(0.25)));
    let (lo, hi) = wilson(10, 40, dec!(1.96));
    assert_eq!(half.low, Some(lo.round_dp(4)));
    assert_eq!(half.high, Some(hi.round_dp(4)));
    assert!(!half.insufficient);
    // Rango [0,8, 0,9): calibrado (80 % mostrado, 80 % real).
    let eighty = &r.calibration.rows[8];
    assert_eq!((eighty.cases, eighty.hits), (40, 32));
    // La muestra: 5 por rango, las primeras en el tiempo, con lo mostrado y lo que pasó.
    let s: Vec<_> = r
        .calibration
        .sample
        .iter()
        .filter(|s| s.bucket == 5)
        .collect();
    assert_eq!(s.len(), 5);
    assert!(s.iter().all(|x| x.shown == dec!(0.5)));
    assert_eq!(
        s.iter().map(|x| x.hit_tp1).collect::<Vec<_>>(),
        [true, false, false, false, true]
    );
    // Solo el rango descalibrado genera propuesta, pendiente de voto y citando la evidencia.
    let recal: Vec<_> = r
        .change_proposals
        .iter()
        .filter(|c| c.kind == ChangeKind::Recalibrate)
        .collect();
    assert_eq!(recal.len(), 1);
    assert_eq!(recal[0].evidence, "calibration.rows[5]");
    assert_eq!(recal[0].status, PENDING_VOTE);
    assert!(recal[0].text.contains("pasó 25.0%"), "{}", recal[0].text);
}

#[test]
fn fewer_than_30_cases_is_insufficient_and_proposes_nothing() {
    let pairs: Vec<_> = (0..20)
        .map(|i| {
            pair(
                Source::Paper,
                i,
                cutoff(),
                Signal::CandlePattern,
                &[],
                false,
                dec!(-1),
            )
        })
        .collect();
    let r = build(&pairs);
    assert_contract(&r);
    assert!(r.calibration.rows[5].insufficient);
    assert_eq!(r.calibration.rows[5].cases, 20);
    assert!(
        r.change_proposals
            .iter()
            .all(|c| c.kind != ChangeKind::Recalibrate)
    );
    // Las propuestas sin probabilidad mostrada se cuentan aparte (sin gatillo con datos).
    let other: Vec<_> = (0..3)
        .map(|i| {
            pair(
                Source::Paper,
                500 + i,
                cutoff(),
                Signal::DoubleBreak,
                &[],
                true,
                dec!(2),
            )
        })
        .collect();
    assert_eq!(build(&other).calibration.without_probability, 3);
}

#[test]
fn a_signal_that_does_not_help_is_proposed_for_review_and_small_samples_are_flagged() {
    // 40 con noticias (pierden 1 R) y 40 sin (ganan 1 R); 5 con fvg (muestra chica).
    let mut pairs = Vec::new();
    for i in 0..40 {
        pairs.push(pair(
            Source::Paper,
            i,
            cutoff(),
            Signal::CandlePattern,
            &[Signal::News],
            false,
            dec!(-1),
        ));
        let fvg: &[Signal] = if i < 5 {
            &[Signal::FvgOrOrderBlock]
        } else {
            &[]
        };
        pairs.push(pair(
            Source::Paper,
            100 + i,
            cutoff(),
            Signal::CandlePattern,
            fvg,
            true,
            dec!(1),
        ));
    }
    let r = build(&pairs);
    assert_contract(&r);
    let news = r.signals.iter().find(|s| s.signal == Signal::News).unwrap();
    assert_eq!((news.with.cases, news.without.cases), (40, 40));
    assert_eq!(news.with.mean_r, Some(dec!(-1)));
    assert_eq!(news.without.mean_r, Some(dec!(1)));
    assert_eq!(news.difference_r, Some(dec!(-2)));
    assert!(!news.small_sample);
    let fvg = r
        .signals
        .iter()
        .find(|s| s.signal == Signal::FvgOrOrderBlock)
        .unwrap();
    assert!(fvg.small_sample);
    let review: Vec<_> = r
        .change_proposals
        .iter()
        .filter(|c| c.kind == ChangeKind::ReviewSignalWeight)
        .collect();
    assert_eq!(review.len(), 1, "{review:?}");
    assert!(review[0].text.contains("news"));
    assert_eq!(review[0].status, PENDING_VOTE);
    // La tendencia está en todas: sin casos "sin ella", muestra chica y sin propuesta.
    let trend = r
        .signals
        .iter()
        .find(|s| s.signal == Signal::Trend)
        .unwrap();
    assert_eq!(trend.without.cases, 0);
    assert!(trend.small_sample);
}

#[test]
fn the_report_is_pure() {
    let pairs: Vec<_> = (0..35)
        .map(|i| {
            pair(
                Source::Backtest,
                i,
                cutoff(),
                Signal::Sweep,
                &[],
                i % 3 == 0,
                dec!(0.5),
            )
        })
        .collect();
    let a = build(&pairs);
    let mut reversed = pairs.clone();
    reversed.reverse();
    assert_eq!(
        a,
        build(&reversed),
        "el orden de entrada no cambia el informe"
    );
}

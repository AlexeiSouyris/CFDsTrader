//! Propuestas por mes sin P/L (#390): las mismas decisiones que el backtest, etiqueta de la spec
//! de ML (TP1 antes que el stop) y sin look-ahead.

use cfds_backtest::{
    BacktestConfig, CloseReason, Commission, ExitReason, FundingRates, InstrumentCosts, Label,
    Overlap, Proposal, ProposalRules, Setup, Tally, proposals, run, tally_by,
};
use cfds_core::{Candle, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_patterns::PatternsConfig;
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, Datelike, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn configs() -> Configs {
    Configs {
        patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
        liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
        trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
        strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
    }
}

fn limits() -> InstrumentLimits {
    InstrumentLimits {
        max_leverage: dec!(10),
        margin_pct: dec!(5),
        min_deal_size: Some(dec!(0.0001)),
    }
}

fn rules() -> ProposalRules {
    ProposalRules {
        instrument: "US500".parse().unwrap(),
        strategy: configs(),
        risk: RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        limits: limits(),
        exits: BacktestConfig::from_toml(cfg!("backtest.toml"))
            .unwrap()
            .exits,
        equity: dec!(1000),
        account_to_instrument: dec!(1),
    }
}

/// El backtest con costos cero y sin límites de la cuenta que dependan del P/L: así sus
/// operaciones tienen que ser exactamente las propuestas.
fn frictionless_setup() -> Setup {
    let r = rules();
    let mut risk = r.risk;
    risk.max_drawdown_pct = dec!(99);
    risk.max_daily_loss_pct = dec!(99);
    let mut backtest = BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap();
    backtest.costs.slippage_spread_fraction = dec!(0);
    backtest.costs.slippage_ticks = dec!(0);
    Setup {
        instrument: r.instrument,
        strategy: r.strategy,
        risk,
        limits: r.limits,
        costs: InstrumentCosts {
            spread: dec!(0),
            tick: dec!(0.1),
            commission: Commission::None,
            funding_admin_fee_pct: dec!(0),
            funding_day_count: dec!(360),
        },
        backtest,
        initial_equity: r.equity,
        account_to_instrument: dec!(1),
        funding_rates: FundingRates::Constant(dec!(0)),
        funding_cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
        funding_tz: chrono_tz::Europe::Madrid,
        day_tz: chrono_tz::Europe::Madrid,
    }
}

fn bar(t0: DateTime<Utc>, i: usize, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US500".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: t0 + TimeDelta::minutes(15 * i as i64),
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

/// La misma caminata de `tests/runner.rs`: tramos de 150 velas con deriva alterna.
fn random_walk(n: usize, seed: u64) -> Vec<Candle> {
    let mut state = seed;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let t0: DateTime<Utc> = "2026-01-05T00:00:00Z".parse().unwrap();
    let mut close = dec!(1000);
    (0..n)
        .map(|i| {
            let drift = if (i / 150) % 2 == 0 { 1 } else { -1 };
            let open = close;
            close = open + Decimal::new(next(41) - 20 + drift * 2, 1);
            let up = Decimal::new(next(25), 1);
            let down = Decimal::new(next(25), 1);
            bar(
                t0,
                i,
                open,
                open.max(close) + up,
                open.min(close) - down,
                close,
            )
        })
        .collect()
}

fn resample_h1(m15: &[Candle]) -> Vec<Candle> {
    m15.chunks_exact(4)
        .map(|g| Candle {
            instrument_id: g[0].instrument_id.clone(),
            timeframe: Timeframe::H1,
            start: g[0].start,
            open: g[0].open,
            high: g.iter().map(|c| c.high).max().unwrap(),
            low: g.iter().map(|c| c.low).min().unwrap(),
            close: g[3].close,
            volume: None,
        })
        .collect()
}

fn series() -> (Vec<Candle>, Vec<Candle>) {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    (m15, h1)
}

/// Lo que no depende del resultado: la señal.
fn signal_key(
    p: &Proposal,
) -> (
    usize,
    cfds_strategy::Side,
    i32,
    Vec<cfds_strategy::TriggerKind>,
) {
    (p.index, p.side, p.score, p.triggers.clone())
}

#[test]
fn one_at_a_time_proposals_are_the_frictionless_backtest_trades() {
    let (m15, h1) = series();
    let report = proposals(&m15, &h1, &[], &[], &[], &rules(), Overlap::OneAtATime).unwrap();
    let trades = run(&m15, &h1, &[], &[], &frictionless_setup())
        .unwrap()
        .trades;
    assert!(trades.len() > 5, "solo {} operaciones", trades.len());
    assert_eq!(report.proposals.len(), trades.len());
    for (p, t) in report.proposals.iter().zip(&trades) {
        assert_eq!(
            (p.index, p.side, p.score),
            (t.signal_index, t.side, t.score)
        );
        assert_eq!(p.signal_at, t.signal_at);
        assert_eq!(
            (p.reference, p.stop, p.tp1, &p.breakdown),
            (t.reference, t.stop, t.tp1, &t.breakdown)
        );
        assert!(p.size > Decimal::ZERO && p.leverage > Decimal::ZERO);
        let hit_tp1 = t
            .exits
            .iter()
            .any(|e| e.reason == CloseReason::Exit(ExitReason::Tp1));
        match p.label {
            Label::Tp1 => assert!(hit_tp1, "{p:?}"),
            Label::Miss(reason) => {
                assert!(!hit_tp1);
                assert_eq!(t.exits.last().unwrap().reason, CloseReason::Exit(reason));
            }
            Label::Unresolved => {
                assert_eq!(t.exits.last().unwrap().reason, CloseReason::EndOfData)
            }
        }
        assert_eq!(p.resolved_index, t.exits.last().unwrap().index);
    }
}

#[test]
fn every_candle_mode_contains_the_one_at_a_time_proposals() {
    let (m15, h1) = series();
    let one = proposals(&m15, &h1, &[], &[], &[], &rules(), Overlap::OneAtATime).unwrap();
    let all = proposals(&m15, &h1, &[], &[], &[], &rules(), Overlap::EveryCandle).unwrap();
    assert!(all.proposals.len() > one.proposals.len());
    for p in &one.proposals {
        assert!(all.proposals.contains(p), "{p:?}");
    }
    // En el modo del backtest, nunca se decide antes de conocer el resultado anterior.
    for w in one.proposals.windows(2) {
        assert!(w[1].index >= w[0].resolved_index);
    }
}

#[test]
fn changing_the_future_never_changes_a_past_signal() {
    // Sin look-ahead (spec de ML §7.1): se cortan las velas desde `k`. Las señales anteriores
    // son las mismas; solo puede cambiar la etiqueta de las que no se resolvieron antes de `k`.
    let (m15, h1) = series();
    let full = proposals(&m15, &h1, &[], &[], &[], &rules(), Overlap::EveryCandle).unwrap();
    for k in [1200, 2000, 2600] {
        let cut = &m15[..k];
        let cut_h1: Vec<Candle> = h1
            .iter()
            .filter(|c| c.end() <= cut[k - 1].end())
            .cloned()
            .collect();
        let part = proposals(cut, &cut_h1, &[], &[], &[], &rules(), Overlap::EveryCandle).unwrap();
        let before: Vec<&Proposal> = full.proposals.iter().filter(|p| p.index < k - 1).collect();
        assert_eq!(part.proposals.len(), before.len(), "k = {k}");
        for (a, b) in part.proposals.iter().zip(before) {
            assert_eq!(signal_key(a), signal_key(b));
            if b.resolved_index < k {
                assert_eq!(a.label, b.label);
            } else if a.label != Label::Tp1 {
                assert_eq!(a.label, Label::Unresolved);
            }
        }
    }
}

#[test]
fn closed_entry_windows_and_data_errors_block_proposals() {
    let (m15, h1) = series();
    let free = proposals(&m15, &h1, &[], &[], &[], &rules(), Overlap::EveryCandle).unwrap();
    let first = free.proposals[0].index;
    let mut entries = vec![true; m15.len()];
    entries[first] = false;
    let r = proposals(
        &m15,
        &h1,
        &entries,
        &[],
        &[],
        &rules(),
        Overlap::EveryCandle,
    )
    .unwrap();
    assert!(r.proposals.iter().all(|p| p.index != first));
    // Después de un error de datos, nada hasta completar el calentamiento (200 velas).
    let r = proposals(
        &m15,
        &h1,
        &[],
        &[],
        &[m15[first].start],
        &rules(),
        Overlap::EveryCandle,
    )
    .unwrap();
    assert!(r.proposals.iter().all(|p| p.index >= first + 200));
}

#[test]
fn an_unconfirmed_broker_minimum_rejects_everything_and_says_why() {
    let (m15, h1) = series();
    let mut r = rules();
    r.limits.min_deal_size = None;
    let report = proposals(&m15, &h1, &[], &[], &[], &r, Overlap::EveryCandle).unwrap();
    assert!(report.proposals.is_empty());
    assert!(report.rejected["MinDealSizeUnknown"] > 0);
}

#[test]
fn tallies_count_classes_and_check_the_minimum() {
    let (m15, h1) = series();
    let report = proposals(&m15, &h1, &[], &[], &[], &rules(), Overlap::EveryCandle).unwrap();
    let by_month = tally_by(&report.proposals, |p| {
        (p.signal_at.year(), p.signal_at.month())
    });
    let total: usize = by_month.values().map(|t| t.total).sum();
    assert_eq!(total, report.proposals.len());
    for t in by_month.values() {
        assert_eq!(t.total, t.y1 + t.y0 + t.unresolved);
    }
    let t = Tally {
        total: 320,
        y1: 60,
        y0: 250,
        unresolved: 10,
    };
    assert!(t.meets(300, 50));
    assert!(!t.meets(311, 50), "las sin resolver no cuentan");
    assert!(!t.meets(300, 61));
}

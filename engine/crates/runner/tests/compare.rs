//! Comparación paper vs backtest (fase 8, #315): una corrida idéntica da cero, una operación
//! faltante, una diferencia de swap, el cierre de fin de semana y un registro paper sintético
//! hecho con el núcleo del runner sobre velas históricas (sintéticas, con semilla fija).

use cfds_backtest::{
    BacktestConfig, CloseReason, Commission, FundingRates, InstrumentCosts, Report, Setup,
    run_checked,
};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, MarketSetup, PaperBroker, PaperExit, PaperTrade,
};
use cfds_core::{
    Candle, ClientId, InstrumentId, Order, OrderRejection, OrderReport, OrderSide, OrderStatus,
    PaperMode, Timeframe,
};
use cfds_liquidity::Side;
use cfds_market_data::{SessionCalendar, ValidationConfig};
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_runner::compare::{Causes, Comparison, Inputs, compare, entries_from};
use cfds_runner::{Feed, Runner, RunnerConfig, strategy_from_toml};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn id() -> InstrumentId {
    "US100".parse().unwrap()
}

fn random_walk(n: usize, seed: u64) -> Vec<Candle> {
    let mut state = seed;
    let mut next = move |m: i64| -> i64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % m as u64) as i64
    };
    let t0 = utc("2026-01-05T00:00:00Z");
    let mut close = dec!(1000);
    (0..n)
        .map(|i| {
            let drift = if (i / 150) % 2 == 0 { 1 } else { -1 };
            let open = close;
            close = open + Decimal::new(next(41) - 20 + drift * 2, 1);
            let up = Decimal::new(next(25), 1);
            let down = Decimal::new(next(25), 1);
            Candle {
                instrument_id: id(),
                timeframe: Timeframe::M15,
                start: t0 + TimeDelta::minutes(15 * i as i64),
                open,
                high: open.max(close) + up,
                low: open.min(close) - down,
                close,
                volume: None,
            }
        })
        .collect()
}

fn resample_h1(m15: &[Candle]) -> Vec<Candle> {
    m15.chunks_exact(4)
        .map(|g| Candle {
            instrument_id: id(),
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

fn limits() -> InstrumentLimits {
    InstrumentLimits {
        max_leverage: dec!(10),
        margin_pct: dec!(5),
        min_deal_size: Some(dec!(0.01)),
    }
}

fn costs() -> InstrumentCosts {
    InstrumentCosts {
        spread: dec!(1),
        tick: dec!(0.1),
        commission: Commission::None,
        funding_admin_fee_pct: dec!(2.5),
        funding_day_count: dec!(365),
    }
}

fn backtest_cfg() -> BacktestConfig {
    let mut b = BacktestConfig::from_toml(cfg!("backtest.toml")).unwrap();
    // Como en vivo: el kill switch no se reactiva.
    b.kill_switch_reset_days = None;
    b
}

fn broker_cfg() -> BrokerConfig {
    let b = backtest_cfg();
    BrokerConfig::from_toml(
        cfg!("broker.toml"),
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        b.costs,
        b.exits,
        b.day_tz,
    )
    .unwrap()
}

fn setup(rate: Decimal) -> Setup {
    let b = backtest_cfg();
    Setup {
        instrument: id(),
        strategy: strategy_from_toml(
            cfg!("patterns.toml"),
            cfg!("liquidity.toml"),
            cfg!("indicators.toml"),
            cfg!("strategy.toml"),
        )
        .unwrap(),
        risk: RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        limits: limits(),
        costs: costs(),
        day_tz: b.day_tz,
        backtest: b,
        initial_equity: broker_cfg().initial_equity,
        account_to_instrument: cfds_backtest::AccountFx::Same,
        funding_rates: FundingRates::Constant(rate),
        funding_cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
        funding_tz: chrono_tz::Europe::Madrid,
        futures_basis: None,
    }
}

fn broker(candles: &[Candle], rate: Decimal) -> PaperBroker {
    let m = MarketSetup {
        limits: limits(),
        costs: costs(),
        account_to_instrument: cfds_backtest::AccountFx::Same,
        candles: candles.to_vec(),
        funding: FundingSetup {
            cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            tz: chrono_tz::Europe::Madrid,
            rates: FundingRates::Constant(rate),
        },
    };
    PaperBroker::new(broker_cfg(), [(id(), m)].into_iter().collect())
}

fn side(s: Side) -> OrderSide {
    match s {
        Side::Long => OrderSide::Long,
        Side::Short => OrderSide::Short,
    }
}

/// Un registro paper hecho a mano igual al backtest: una orden ejecutada por operación, decidida
/// en la misma vela, con la misma ejecución. `skip` saca una; `swap` suma financiación a todas.
fn synthetic(
    report: &Report,
    skip: Option<usize>,
    swap: Decimal,
) -> (Vec<(Order, OrderReport)>, Vec<PaperTrade>) {
    let (mut journal, mut paper) = (Vec::new(), Vec::new());
    for (k, t) in report.trades.iter().enumerate() {
        if Some(k) == skip {
            continue;
        }
        let order_id: ClientId = format!("bt:{k}").parse().unwrap();
        journal.push((
            Order {
                order_id: order_id.clone(),
                mode: PaperMode::Paper,
                instrument_id: id(),
                side: side(t.side),
                stop: t.stop,
                tp1: None,
                proposal_id: None,
                created_at: t.signal_at,
            },
            OrderReport {
                order_id: order_id.clone(),
                status: OrderStatus::Filled,
                rejection: None,
                position_id: Some(format!("pos:{order_id}").parse().unwrap()),
                size: Some(t.size),
                fill_price: Some(t.entry_price),
                commission: Some(Decimal::ZERO),
                at: t.entry_time,
            },
        ));
        paper.push(PaperTrade {
            order_id,
            instrument: id(),
            side: side(t.side),
            opened_at: t.entry_time,
            entry_mid: t.entry_mid,
            entry_fill: t.entry_price,
            size: t.size,
            stop: t.stop,
            exits: t
                .exits
                .iter()
                .map(|e| PaperExit {
                    time: e.time,
                    mid: e.mid,
                    price: e.price,
                    fraction: e.fraction,
                    end_of_data: e.reason == CloseReason::EndOfData,
                    reason: match e.reason {
                        CloseReason::Exit(r) => Some(r),
                        CloseReason::EndOfData => None,
                    },
                })
                .collect(),
            commission: t.commission,
            funding: t.funding + swap,
            gross_pnl: t.gross_pnl,
            net_pnl: t.net_pnl - swap,
        });
    }
    (journal, paper)
}

fn compare_with(
    m15: &[Candle],
    report: &Report,
    journal: &[(Order, OrderReport)],
    paper: &[PaperTrade],
) -> Comparison {
    let now = m15.last().unwrap().end();
    compare(&Inputs {
        instrument: &id(),
        from: m15[0].start,
        to: now,
        journal,
        paper,
        backtest: report,
        m15,
        created_at: now,
    })
}

fn run_compare(m15: &[Candle], report: &Report, b: &PaperBroker) -> Comparison {
    let now = m15.last().unwrap().end();
    compare_with(m15, report, b.journal(), &b.trade_outcomes(now))
}

fn assert_explained(c: &Comparison) {
    assert_contract(c);
    assert_eq!(c.causes.total(), c.difference, "la suma de las causas");
    assert_eq!(c.difference, c.paper.net_pnl - c.backtest.net_pnl);
}

fn walk() -> (Vec<Candle>, Vec<Candle>) {
    let m15 = random_walk(1500, 11);
    let h1 = resample_h1(&m15);
    (m15, h1)
}

#[test]
fn an_identical_run_has_zero_difference() {
    let (m15, h1) = walk();
    let report = run_checked(&m15, &h1, &[], &[], &[], &setup(dec!(2))).unwrap();
    assert!(
        report.trades.len() > 2,
        "{} operaciones",
        report.trades.len()
    );
    let (journal, paper) = synthetic(&report, None, Decimal::ZERO);
    let c = compare_with(&m15, &report, &journal, &paper);
    assert_explained(&c);
    assert_eq!(c.matched.len(), report.trades.len());
    assert!(c.missing.is_empty() && c.extra.is_empty());
    assert_eq!(c.difference, Decimal::ZERO);
    assert_eq!(c.causes, Causes::default());
    for m in &c.matched {
        assert_eq!(m.difference.net_pnl, Decimal::ZERO);
        assert_eq!(m.difference.entry_price, Decimal::ZERO);
        assert_eq!(m.difference.exit_price, Decimal::ZERO);
        assert_eq!(m.difference.size, Decimal::ZERO);
    }
}

#[test]
fn a_trade_missing_in_paper_is_reported_and_explained() {
    let (m15, h1) = walk();
    let report = run_checked(&m15, &h1, &[], &[], &[], &setup(dec!(2))).unwrap();
    let last = report.trades.len() - 1;
    let (journal, paper) = synthetic(&report, Some(last), Decimal::ZERO);
    let c = compare_with(&m15, &report, &journal, &paper);
    assert_explained(&c);
    assert_eq!(c.missing.len(), 1);
    assert_eq!(c.missing[0].decision_at, report.trades[last].signal_at);
    assert_eq!(c.missing[0].paper, "no_order");
    assert_eq!(
        c.causes.missing_in_paper,
        -report.trades[last].net_pnl.round_dp(8)
    );
    assert_eq!(c.difference, c.causes.missing_in_paper);
    assert_eq!(c.matched.len(), last);
}

#[test]
fn a_risk_rejection_in_paper_is_its_own_cause() {
    let (m15, h1) = walk();
    let report = run_checked(&m15, &h1, &[], &[], &[], &setup(dec!(2))).unwrap();
    let (mut journal, mut paper) = synthetic(&report, None, Decimal::ZERO);
    // El riesgo del broker rechazó la primera.
    let (_, rep) = &mut journal[0];
    rep.status = OrderStatus::Rejected;
    rep.rejection = Some(OrderRejection::DailyLossLimit);
    rep.position_id = None;
    rep.size = None;
    rep.fill_price = None;
    rep.commission = None;
    paper.remove(0);
    let c = compare_with(&m15, &report, &journal, &paper);
    assert_explained(&c);
    assert_eq!(c.missing.len(), 1);
    assert_eq!(c.missing[0].paper, "daily_loss_limit");
    assert_eq!(
        c.causes.risk_rejection,
        -report.trades[0].net_pnl.round_dp(8)
    );
    assert_eq!(c.causes.missing_in_paper, Decimal::ZERO);
}

#[test]
fn a_swap_difference_goes_to_the_swap_cause() {
    let (m15, h1) = walk();
    let report = run_checked(&m15, &h1, &[], &[], &[], &setup(dec!(2))).unwrap();
    let (journal, paper) = synthetic(&report, None, dec!(0.25));
    let c = compare_with(&m15, &report, &journal, &paper);
    assert_explained(&c);
    let n = Decimal::from(report.trades.len());
    assert_eq!(c.causes.swap, dec!(-0.25) * n);
    assert_eq!(c.difference, c.causes.swap);
    assert_eq!(c.causes.fill_price, Decimal::ZERO);
    assert_eq!(c.causes.size, Decimal::ZERO);
}

#[test]
fn the_weekend_close_of_the_backtest_is_its_own_cause() {
    let (m15, h1) = walk();
    let clean = run_checked(&m15, &h1, &[], &[], &[], &setup(dec!(2))).unwrap();
    // Una operación que dura al menos dos velas: el backtest la cierra a la fuerza en la vela
    // siguiente a la entrada (como un viernes); paper (sin cierre de fin de semana) la sigue.
    let k = clean
        .trades
        .iter()
        .position(|t| t.exits.last().unwrap().index >= t.entry_index + 2)
        .unwrap();
    let t = &clean.trades[k];
    let mut force = vec![false; m15.len()];
    force[t.entry_index + 1] = true;
    let report = run_checked(&m15, &h1, &[], &force, &[], &setup(dec!(2))).unwrap();
    let (journal, paper) = synthetic(&clean, None, Decimal::ZERO);
    let c = compare_with(&m15, &report, &journal, &paper);
    assert_explained(&c);
    let m = c
        .matched
        .iter()
        .find(|m| m.decision_at == t.signal_at)
        .expect("emparejada");
    assert!(m.weekend_close);
    assert_eq!(c.causes.weekend_close, m.difference.net_pnl);
    assert!(!m.difference.net_pnl.is_zero());
    // Las anteriores al cierre forzado son idénticas.
    for m in c.matched.iter().filter(|m| m.decision_at < t.signal_at) {
        assert_eq!(m.difference.net_pnl, Decimal::ZERO);
    }
}

/// El broker simulado real, con las órdenes del backtest: da lo mismo que el backtest (#319).
#[test]
fn the_real_paper_broker_against_the_backtest_is_explained() {
    let (m15, h1) = walk();
    let report = run_checked(&m15, &h1, &[], &[], &[], &setup(dec!(2))).unwrap();
    let mut b = broker(&m15, dec!(2));
    for (k, t) in report.trades.iter().enumerate() {
        b.submit(&Order {
            order_id: format!("bt:{k}").parse().unwrap(),
            mode: PaperMode::Paper,
            instrument_id: id(),
            side: side(t.side),
            stop: t.stop,
            tp1: None,
            proposal_id: None,
            created_at: t.signal_at,
        });
    }
    let c = run_compare(&m15, &report, &b);
    assert_explained(&c);
    // Misma decisión, misma vela y mismo precio de entrada en todas.
    assert_eq!(c.matched.len(), report.trades.len());
    // #319: el broker simula las salidas igual que el backtest (desde el precio ejecutado).
    assert_eq!(c.causes.fill_price, Decimal::ZERO, "{:#?}", c.causes);
    assert_eq!(c.causes.size, Decimal::ZERO);
    assert_eq!(c.difference, Decimal::ZERO);
    for m in &c.matched {
        assert_eq!(m.difference.entry_price, Decimal::ZERO);
        assert_eq!(m.difference.exit_price, Decimal::ZERO);
        assert_eq!(m.difference.size, Decimal::ZERO);
        assert_eq!(m.paper.exit_time, m.backtest.exit_time);
    }
}

/// Mercado abierto todo el tiempo (las velas sintéticas son continuas).
struct AlwaysOpen;

impl SessionCalendar for AlwaysOpen {
    fn expected_starts(
        &self,
        _: &InstrumentId,
        tf: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Option<Vec<DateTime<Utc>>> {
        let step = tf.duration().num_seconds();
        let first = from.timestamp().div_euclid(step) * step
            + if from.timestamp().rem_euclid(step) == 0 {
                0
            } else {
                step
            };
        Some(
            (first..to.timestamp())
                .step_by(step as usize)
                .map(|s| DateTime::from_timestamp(s, 0).unwrap())
                .collect(),
        )
    }
}

#[test]
fn a_synthetic_paper_journal_from_the_runner_is_explained() {
    let (m15, h1) = walk();
    let strategy = setup(dec!(2)).strategy;
    let mut runner = Runner::new(
        RunnerConfig::from_toml(cfg!("runner.toml")).unwrap(),
        strategy,
        ValidationConfig::from_toml(cfg!("data_validation.toml")).unwrap(),
    );
    // El runner, vela por vela, con una ventana como la del gateway (lookback).
    const WINDOW: usize = 320;
    let mut b = broker(&m15, dec!(2));
    for last in WINDOW..m15.len() {
        let from = last + 1 - WINDOW;
        let window = &m15[from..=last];
        let h1w: Vec<Candle> = h1
            .iter()
            .filter(|c| c.start >= window[0].start && c.end() <= window[WINDOW - 1].end())
            .cloned()
            .collect();
        let step = runner.step(
            &mut b,
            Feed {
                instrument: &id(),
                m15: window,
                h1: &h1w,
            },
            &AlwaysOpen,
            true,
            |_, _| true,
            |_, _| 0,
            m15[last].end(),
        );
        if let Some(o) = &step.order {
            b.submit(o);
        }
    }
    let report = run_checked(&m15, &h1, &[], &[], &[], &setup(dec!(2))).unwrap();
    let c = run_compare(&m15, &report, &b);
    assert_explained(&c);
    assert!(c.paper.trades > 0, "el runner operó");
    assert!(!c.matched.is_empty(), "{c:#?}");
    // Lo emparejado se decidió en la misma vela y con la misma dirección.
    for m in &c.matched {
        assert!(m.decision_at >= m15[WINDOW].end());
    }
    assert_contract(&c);
}

/// Cada comparación cumple `contracts/paper_comparison.v1`.
#[test]
fn entries_before_from_are_only_warmup() {
    let (m15, _) = walk();
    let from = m15[10].end();
    // Sin marca cuenta como permitida (como en el backtest); la marca `false` se respeta.
    let mut given = vec![true; 20];
    given[15] = false;
    let e = entries_from(&m15, &given, from);
    assert_eq!(e.len(), m15.len());
    for (i, ok) in e.iter().enumerate() {
        assert_eq!(*ok, i >= 10 && i != 15, "vela {i}");
    }
    // Sin look-ahead: el resultado de una vela no cambia al llegar velas nuevas.
    let shorter = entries_from(&m15[..100], &given, from);
    assert_eq!(shorter[..], e[..100]);
}

/// #440: paper decide desde `from` con el capital de la cuenta en `from`. Si el backtest opera
/// en el calentamiento, llega a `from` con otro capital y las mismas decisiones salen con otro
/// tamaño; sin entradas antes de `from`, la misma corrida da diferencia cero.
#[test]
fn the_backtest_does_not_trade_during_the_paper_warmup() {
    let (m15, h1) = walk();
    let full = run_checked(&m15, &h1, &[], &[], &[], &setup(dec!(2))).unwrap();
    assert!(full.trades.len() > 4, "{} operaciones", full.trades.len());
    let from = full.trades[full.trades.len() / 2].signal_at;
    assert!(full.trades.iter().any(|t| t.signal_at < from));

    let entries = entries_from(&m15, &[], from);
    let report = run_checked(&m15, &h1, &entries, &[], &[], &setup(dec!(2))).unwrap();
    assert!(!report.trades.is_empty());
    assert!(report.trades.iter().all(|t| t.signal_at >= from));

    // Paper: las mismas decisiones desde `from`, con el capital inicial.
    let (journal, paper) = synthetic(&report, None, Decimal::ZERO);
    let now = m15.last().unwrap().end();
    let with = |backtest: &Report| {
        compare(&Inputs {
            instrument: &id(),
            from,
            to: now,
            journal: &journal,
            paper: &paper,
            backtest,
            m15: &m15,
            created_at: now,
        })
    };
    let c = with(&report);
    assert_explained(&c);
    assert_eq!(c.matched.len(), report.trades.len());
    assert!(c.missing.is_empty() && c.extra.is_empty());
    assert_eq!(c.difference, Decimal::ZERO);
    assert_eq!(c.causes, Causes::default());

    // Lo que pasaba antes: el backtest que operó en el calentamiento fabrica «tamaño».
    let before = with(&full);
    assert_explained(&before);
    assert_ne!(before.causes.size, Decimal::ZERO);
}

fn assert_contract(c: &Comparison) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/paper_comparison.v1.schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let v = jsonschema::validator_for(&schema).unwrap();
    let doc = serde_json::to_value(c).unwrap();
    let errors: Vec<String> = v.iter_errors(&doc).map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
}

/// El broker simulado real aplica el mismo cierre de fin de semana que el backtest (#334): con
/// las mismas marcas de la sesión, cruzando fines de semana, la causa `weekend_close` da cero.
#[test]
fn the_real_paper_broker_closes_on_weekends_like_the_backtest() {
    use cfds_market_data::{Catalog, ExchangeCalendar, TradingWindowConfig, trading_window};
    let (m15, h1) = walk();
    let catalog = Catalog::from_toml(cfg!("instruments.toml")).unwrap();
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    let calendar = ExchangeCalendar::load(&dir, catalog.exchange_map()).unwrap();
    let session = calendar.session_for(&id()).unwrap();
    let window = TradingWindowConfig::from_toml(cfg!("trading_window.toml")).unwrap();
    let (_, force) = trading_window::flags(session, &m15, &window).unwrap();
    let report = run_checked(&m15, &h1, &[], &force, &[], &setup(dec!(2))).unwrap();
    let forced = report
        .trades
        .iter()
        .filter(|t| {
            t.exits
                .iter()
                .any(|e| e.reason == CloseReason::Exit(cfds_backtest::ExitReason::Forced))
        })
        .count();
    assert!(
        forced > 0,
        "la serie tiene que cruzar un cierre de semana con una posición"
    );
    let markets = broker(&m15, dec!(2)).markets().clone();
    let cfg = broker_cfg().with_calendar(&calendar, [&id()]).unwrap();
    let mut b = PaperBroker::new(cfg, markets);
    for (k, t) in report.trades.iter().enumerate() {
        b.submit(&Order {
            order_id: format!("bt:{k}").parse().unwrap(),
            mode: PaperMode::Paper,
            instrument_id: id(),
            side: side(t.side),
            stop: t.stop,
            tp1: None,
            proposal_id: None,
            created_at: t.signal_at,
        });
    }
    let c = run_compare(&m15, &report, &b);
    assert_explained(&c);
    assert_eq!(c.matched.len(), report.trades.len());
    assert_eq!(c.causes.weekend_close, Decimal::ZERO, "{:#?}", c.causes);
    assert_eq!(c.difference, Decimal::ZERO);
    for m in &c.matched {
        assert_eq!(m.paper.exit_time, m.backtest.exit_time);
        assert_eq!(m.difference.net_pnl, Decimal::ZERO);
    }
}

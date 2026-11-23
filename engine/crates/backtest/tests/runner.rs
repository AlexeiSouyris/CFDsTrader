//! Recorrido completo (#55): comprar y mantener y una operación calculados a mano, más los
//! invariantes del recorrido sobre una serie larga.

use cfds_backtest::{
    BacktestConfig, CloseReason, Commission, ExitReason, FundingRates, InstrumentCosts, Setup,
    buy_and_hold, execute, mark_rollovers, pick, run, run_checked, run_with_news,
};
use cfds_core::{Candle, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_patterns::PatternsConfig;
use cfds_risk::{InstrumentLimits, Rejection, RiskConfig};
use cfds_strategy::{
    Component, Configs, Score, Side, Signal, StrategyConfig, Trigger, TriggerKind,
};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
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
        account_to_instrument: dec!(1),
        // 1,1 % + 2,5 % de tasa = 3,6 % anual para los largos.
        funding_rates: FundingRates::Constant(dec!(1.1)),
        funding_cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
        funding_tz: chrono_tz::Europe::Madrid,
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

// ---------- Comprar y mantener ----------

#[test]
fn buy_and_hold_matches_the_hand_calculation() {
    // 21:30 a 22:30 UTC del 19-11-2026: cruza las 23:00 de Madrid (22:00 UTC) → 1 noche.
    let t0 = utc("2026-11-19T21:30:00Z");
    let candles = vec![
        bar(t0, 0, dec!(100), dec!(103), dec!(99), dec!(102)),
        bar(t0, 1, dec!(102), dec!(106), dec!(101), dec!(105)),
        bar(t0, 2, dec!(105), dec!(110), dec!(104), dec!(108)),
        bar(t0, 3, dec!(108), dec!(111), dec!(107), dec!(110)),
    ];
    let b = buy_and_hold(&candles, dec!(2), &setup()).unwrap();
    // Entrada 100 + 0,7 = 100,7; salida 110 − 0,7 = 109,3; bruto 2 × 8,6 = 17,2.
    assert_eq!((b.entry_price, b.exit_price), (dec!(100.7), dec!(109.3)));
    assert_eq!(b.gross_pnl, dec!(17.2));
    // Financiación: 1 noche × 100 × 2 × 3,6 % / 360 = 0,02. Sin comisión (índice).
    assert_eq!(
        (b.nights, b.commission, b.funding),
        (1, dec!(0), dec!(0.02))
    );
    assert_eq!(b.net_pnl, dec!(17.18));
    assert_eq!(buy_and_hold(&[], dec!(2), &setup()), None);
}

// ---------- Una operación, a mano ----------

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

#[test]
fn one_trade_matches_the_hand_calculation_in_every_field() {
    let c = trade_candles(utc("2026-11-19T14:30:00Z"));
    let r = execute(
        &signal(Side::Long, dec!(98)),
        dec!(2),
        dec!(0.2),
        dec!(10),
        &c,
        &big_atr(4),
        &[],
        &setup(),
    )
    .unwrap();
    assert_eq!((r.signal_index, r.entry_index), (0, 1));
    assert_eq!(r.entry_time, utc("2026-11-19T14:45:00Z"));
    assert_eq!((r.entry_mid, r.entry_price), (dec!(100), dec!(100.7)));
    assert_eq!(
        (r.size, r.leverage, r.margin),
        (dec!(2), dec!(0.2), dec!(10))
    );
    assert_eq!((r.score, r.triggers.clone()), (6, vec![TriggerKind::Sweep]));
    // La propuesta muestra la referencia (cierre de la señal) y su TP1: 100 + 2 × (100 − 98).
    assert_eq!((r.reference, r.tp1), (dec!(100), dec!(104)));
    assert_eq!(r.breakdown, vec![(Component::Trend, 2)]);
    let exits: Vec<_> = r
        .exits
        .iter()
        .map(|e| (e.index, e.mid, e.price, e.fraction, e.reason))
        .collect();
    assert_eq!(
        exits,
        vec![
            (
                2,
                dec!(106.1),
                dec!(105.4),
                dec!(0.5),
                CloseReason::Exit(ExitReason::Tp1)
            ),
            (
                3,
                dec!(100.7),
                dec!(100.0),
                dec!(0.5),
                CloseReason::Exit(ExitReason::BreakEven)
            ),
        ]
    );
    assert_eq!(r.exits[1].time, utc("2026-11-19T15:30:00Z"));
    // A precio medio: 1 × 6,1 + 1 × 0,7 = 6,8. Ejecutado: 1 × 4,7 + 1 × (−0,7) = 4.
    assert_eq!((r.mid_pnl, r.gross_pnl), (dec!(6.8), dec!(4)));
    // Intradía: sin financiación. Índice: sin comisión.
    assert_eq!(
        (r.commission, r.funding, r.net_pnl),
        (dec!(0), dec!(0), dec!(4))
    );
    // 4 ejecuciones × 0,7 × 1 por punto (entrada 2 × 0,7 + dos salidas de 1 × 0,7).
    assert_eq!(r.total_costs(), dec!(2.8));
}

#[test]
fn shares_pay_commission_on_every_execution() {
    let mut s = setup();
    s.costs.commission = Commission::PerShare {
        per_share: dec!(0.02),
        min: dec!(15),
    };
    let c = trade_candles(utc("2026-11-19T14:30:00Z"));
    let r = execute(
        &signal(Side::Long, dec!(98)),
        dec!(2),
        dec!(0.2),
        dec!(10),
        &c,
        &big_atr(4),
        &[],
        &s,
    )
    .unwrap();
    // Entrada y dos salidas parciales: 3 mínimos de 15.
    assert_eq!((r.commission, r.net_pnl), (dec!(45), dec!(-41)));
}

#[test]
fn an_overnight_trade_pays_funding_per_closed_part() {
    // Empieza 21:30 UTC: las dos salidas (22:15 y 22:30 UTC) cruzan las 22:00 UTC.
    let c = trade_candles(utc("2026-11-19T21:30:00Z"));
    let r = execute(
        &signal(Side::Long, dec!(98)),
        dec!(2),
        dec!(0.2),
        dec!(10),
        &c,
        &big_atr(4),
        &[],
        &setup(),
    )
    .unwrap();
    // 2 partes de 1 × 100 × 3,6 % / 360 = 0,01 cada una.
    assert_eq!((r.funding, r.net_pnl), (dec!(0.02), dec!(3.98)));
}

#[test]
fn a_gap_through_the_stop_at_the_entry_exits_at_that_open() {
    let t0 = utc("2026-11-19T14:30:00Z");
    let c = vec![
        bar(t0, 0, dec!(100), dec!(100.5), dec!(99.5), dec!(100)),
        bar(t0, 1, dec!(97), dec!(97.5), dec!(96), dec!(97)),
    ];
    let r = execute(
        &signal(Side::Long, dec!(98)),
        dec!(2),
        dec!(0.2),
        dec!(10),
        &c,
        &big_atr(2),
        &[],
        &setup(),
    )
    .unwrap();
    assert_eq!(r.exits.len(), 1);
    assert_eq!(
        (r.exits[0].index, r.exits[0].mid, r.exits[0].reason),
        (1, dec!(97), CloseReason::Exit(ExitReason::Stop))
    );
    // Entra a 97,7 y sale a 96,3: −1,4 por punto.
    assert_eq!(r.gross_pnl, dec!(-2.8));
}

#[test]
fn a_trade_still_open_closes_at_the_last_close() {
    let t0 = utc("2026-11-19T14:30:00Z");
    let c = vec![
        bar(t0, 0, dec!(100), dec!(100.5), dec!(99.5), dec!(100)),
        bar(t0, 1, dec!(100), dec!(101), dec!(99.5), dec!(100.5)),
        bar(t0, 2, dec!(100.5), dec!(101.5), dec!(100), dec!(101)),
    ];
    let r = execute(
        &signal(Side::Long, dec!(98)),
        dec!(2),
        dec!(0.2),
        dec!(10),
        &c,
        &big_atr(3),
        &[],
        &setup(),
    )
    .unwrap();
    assert_eq!(
        (r.exits[0].index, r.exits[0].mid, r.exits[0].reason),
        (2, dec!(101), CloseReason::EndOfData)
    );
}

#[test]
fn short_trades_mirror_the_long_ones() {
    let m = |x: Decimal| dec!(200) - x;
    let t0 = utc("2026-11-19T14:30:00Z");
    let c: Vec<Candle> = trade_candles(t0)
        .iter()
        .enumerate()
        .map(|(i, k)| bar(t0, i, m(k.open), m(k.low), m(k.high), m(k.close)))
        .collect();
    let r = execute(
        &signal(Side::Short, dec!(102)),
        dec!(2),
        dec!(0.2),
        dec!(10),
        &c,
        &big_atr(4),
        &[],
        &setup(),
    )
    .unwrap();
    assert_eq!(r.entry_price, dec!(99.3));
    // El TP1 de un corto está abajo: 100 − 2 × (102 − 100).
    assert_eq!(r.tp1, dec!(96));
    assert_eq!(
        (r.mid_pnl, r.gross_pnl, r.net_pnl),
        (dec!(6.8), dec!(4), dec!(4))
    );
}

// ---------- El recorrido ----------

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

#[test]
fn after_a_data_error_nothing_opens_until_the_warmup_is_complete() {
    // Spec §7.8 y §12, con la misma serie que el test de invariantes.
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let s = setup();
    let clean = run(&m15, &h1, &[], &[], &s).unwrap();
    let first = clean.trades[0].signal_index;
    let checked = run_checked(&m15, &h1, &[], &[], &[m15[first].start], &s).unwrap();
    assert!(
        checked.trades.iter().all(|t| t.signal_index >= first + 200),
        "ninguna entrada antes de 200 velas válidas después del error"
    );
    assert!(
        !checked.trades.is_empty(),
        "después del calentamiento vuelve a operar"
    );
    // Sin errores, `run_checked` es `run`.
    assert_eq!(run_checked(&m15, &h1, &[], &[], &[], &s).unwrap(), clean);
}

#[test]
fn a_trade_open_at_a_rollover_closes_before_it_and_nothing_opens_until_the_warmup() {
    // #267: con la fuente de futuros, el contrato cambia en una vela; la operación abierta se
    // cierra al cierre de la última vela del contrato viejo y el calentamiento vuelve a empezar.
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let s = setup();
    let clean = run(&m15, &h1, &[], &[], &s).unwrap();
    let open = clean
        .trades
        .iter()
        .find(|t| t.exits.last().unwrap().index >= t.entry_index + 2)
        .expect("una operación que dura más de dos velas");
    let roll_index = open.entry_index + 2;
    let roll = m15[roll_index].start;
    let (mut entries, mut force_close) = (vec![true; m15.len()], vec![false; m15.len()]);
    let mut errors = Vec::new();
    let closed = mark_rollovers(&m15, &[roll], &mut entries, &mut force_close, &mut errors);
    assert_eq!(closed, vec![roll_index - 1]);
    assert_eq!(errors, vec![roll]);
    assert!(force_close[roll_index - 1] && !entries[roll_index - 1]);
    // Un vencimiento fuera de la serie no marca nada.
    let (mut e2, mut f2, mut d2) = (vec![true; m15.len()], vec![false; m15.len()], Vec::new());
    let outside = m15[0].start - TimeDelta::days(1);
    assert!(mark_rollovers(&m15, &[outside], &mut e2, &mut f2, &mut d2).is_empty());
    assert!(d2.is_empty() && f2.iter().all(|f| !f));

    let r = run_checked(&m15, &h1, &entries, &force_close, &errors, &s).unwrap();
    let cut = r
        .trades
        .iter()
        .find(|t| t.entry_index == open.entry_index)
        .expect("la misma operación");
    let last = cut.exits.last().unwrap();
    assert_eq!(last.index, roll_index - 1);
    assert_eq!(last.reason, CloseReason::Exit(ExitReason::Forced));
    for t in &r.trades {
        // Ninguna operación cruza el vencimiento ni se decide en la última vela del contrato.
        let end = t.exits.last().unwrap().index;
        assert!(!(t.entry_index < roll_index && end >= roll_index), "{t:?}");
        assert_ne!(t.signal_index, roll_index - 1);
        // Después del vencimiento, nada hasta tener 200 velas del contrato nuevo.
        if t.signal_index >= roll_index {
            assert!(t.signal_index >= roll_index + 200);
        }
    }
}

#[test]
fn the_run_respects_its_invariants() {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let s = setup();
    let report = run(&m15, &h1, &[], &[], &s).unwrap();
    assert!(
        report.trades.len() > 5,
        "solo {} operaciones",
        report.trades.len()
    );
    let mut last_exit = 0;
    for tr in &report.trades {
        // Decisión en t, entrada en t+1, nunca antes de la salida anterior.
        assert_eq!(tr.entry_index, tr.signal_index + 1);
        assert!(tr.signal_index >= last_exit);
        assert_eq!(tr.entry_mid, m15[tr.entry_index].open);
        assert_eq!(
            tr.exits.iter().map(|e| e.fraction).sum::<Decimal>(),
            dec!(1)
        );
        assert!(tr.exits.iter().all(|e| e.index >= tr.entry_index));
        assert!(tr.leverage <= dec!(10));
        assert!(tr.score >= 5);
        // Los costos nunca son cero.
        assert!(tr.total_costs() > Decimal::ZERO);
        last_exit = tr.exits.last().unwrap().index;
    }
    let net: Decimal = report.trades.iter().map(|t| t.net_pnl).sum();
    assert_eq!(
        report.final_equity.round_dp(10),
        (s.initial_equity + net).round_dp(10)
    );
}

#[test]
fn the_drawdown_limit_stops_the_run() {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let mut s = setup();
    s.risk.max_drawdown_pct = dec!(0.01);
    s.risk.max_daily_loss_pct = dec!(99);
    let report = run(&m15, &h1, &[], &[], &s).unwrap();
    let at = report.kill_switch_at.expect("tendría que dispararse");
    assert_eq!(
        report.rejected.last().unwrap().reason,
        Rejection::DrawdownLimit
    );
    assert!(report.trades.iter().all(|t| t.signal_index < at));
    assert!(report.trades.iter().any(|t| t.net_pnl < Decimal::ZERO));
    // #273: la corrida lo puede mostrar y guardar, nunca pasa en silencio.
    assert_eq!(report.kill_switch_time(&m15), Some(m15[at].start));
    assert_eq!(report.rejections_by_reason().get("DrawdownLimit"), Some(&1));
    assert_eq!(
        report.rejections_by_reason().values().sum::<usize>(),
        report.rejected.len()
    );
    let mut loose = setup();
    loose.risk.max_drawdown_pct = dec!(99);
    let clean = run(&m15, &h1, &[], &[], &loose).unwrap();
    assert_eq!(clean.kill_switch_time(&m15), None);
}

#[test]
fn the_daily_loss_limit_blocks_the_rest_of_the_day() {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let mut s = setup();
    s.risk.max_daily_loss_pct = dec!(0.01);
    let report = run(&m15, &h1, &[], &[], &s).unwrap();
    let day = |i: usize| {
        m15[i]
            .end()
            .with_timezone(&chrono_tz::Europe::Madrid)
            .date_naive()
    };
    assert!(
        report
            .rejected
            .iter()
            .any(|r| r.reason == Rejection::DailyLossLimit)
    );
    // Después de un rechazo por pérdida diaria no hay más entradas ese día: sin operaciones, el
    // capital no cambia hasta el día siguiente.
    for r in report
        .rejected
        .iter()
        .filter(|r| r.reason == Rejection::DailyLossLimit)
    {
        assert!(
            report
                .trades
                .iter()
                .all(|t| t.signal_index < r.index || day(t.signal_index) != day(r.index)),
            "entrada el mismo día de un rechazo en {}",
            r.index
        );
    }
}

#[test]
fn an_unconfirmed_broker_minimum_blocks_every_trade() {
    let m15 = random_walk(1500, 11);
    let h1 = resample_h1(&m15);
    let mut s = setup();
    s.limits.min_deal_size = None;
    let report = run(&m15, &h1, &[], &[], &s).unwrap();
    assert!(report.trades.is_empty());
    assert!(!report.rejected.is_empty());
    // Todo lo que pasa los demás límites se rechaza por el mínimo desconocido.
    assert!(
        report
            .rejected
            .iter()
            .any(|r| r.reason == Rejection::MinDealSizeUnknown)
    );
    assert_eq!(report.final_equity, s.initial_equity);
}

#[test]
fn the_higher_score_wins_and_a_tie_means_no_trade() {
    let with = |side, total: i32| Signal {
        score: Score {
            total,
            breakdown: vec![],
            candidate: total >= 5,
        },
        ..signal(side, dec!(98))
    };
    let side = |s: Option<Signal>| s.map(|s| s.side);
    assert_eq!(
        side(pick(vec![with(Side::Long, 6), with(Side::Short, 7)])),
        Some(Side::Short)
    );
    assert_eq!(
        side(pick(vec![with(Side::Long, 7), with(Side::Short, 6)])),
        Some(Side::Long)
    );
    assert_eq!(
        side(pick(vec![with(Side::Long, 6), with(Side::Short, 6)])),
        None
    );
    // Una que no llega al mínimo no compite.
    assert_eq!(
        side(pick(vec![with(Side::Long, 4), with(Side::Short, 5)])),
        Some(Side::Short)
    );
    assert_eq!(side(pick(vec![with(Side::Long, 4)])), None);
    assert_eq!(side(pick(vec![])), None);
}

#[test]
fn buy_and_hold_of_shares_pays_commission_twice() {
    let mut s = setup();
    s.costs.commission = Commission::PerShare {
        per_share: dec!(0.02),
        min: dec!(15),
    };
    let t0 = utc("2026-11-19T14:30:00Z");
    let candles = vec![
        bar(t0, 0, dec!(100), dec!(103), dec!(99), dec!(102)),
        bar(t0, 1, dec!(102), dec!(111), dec!(101), dec!(110)),
    ];
    let b = buy_and_hold(&candles, dec!(2), &s).unwrap();
    assert_eq!((b.commission, b.funding), (dec!(30), dec!(0)));
    assert_eq!(b.net_pnl, dec!(17.2) - dec!(30));
}

// ---------- Cartera ----------

use cfds_backtest::{Market, PortfolioError, PortfolioTrade, run_portfolio};

/// Tres instrumentos con las mismas horas y precios distintos.
fn three(setup_for: impl Fn(&str) -> Setup) -> (Vec<Vec<Candle>>, Vec<Vec<Candle>>, Vec<Setup>) {
    let names = ["US100", "US500", "DE40"];
    let m15: Vec<Vec<Candle>> = [11, 7, 3].iter().map(|&s| random_walk(2500, s)).collect();
    let h1 = m15.iter().map(|c| resample_h1(c)).collect();
    (m15, h1, names.iter().map(|n| setup_for(n)).collect())
}

fn named(n: &str) -> Setup {
    Setup {
        instrument: n.parse().unwrap(),
        ..setup()
    }
}

fn markets<'a>(m15: &'a [Vec<Candle>], h1: &'a [Vec<Candle>], setups: &[Setup]) -> Vec<Market<'a>> {
    setups
        .iter()
        .enumerate()
        .map(|(i, s)| Market {
            setup: s.clone(),
            m15: &m15[i],
            h1: &h1[i],
            entries: &[],
            force_close: &[],
            data_errors: &[],
            news: &[],
        })
        .collect()
}

/// (instrumento, entrada, fin de la última salida) de cada operación.
fn intervals(trades: &[PortfolioTrade]) -> Vec<(String, DateTime<Utc>, DateTime<Utc>)> {
    trades
        .iter()
        .map(|t| {
            (
                t.instrument.to_string(),
                t.record.entry_time,
                t.record.exits.last().unwrap().time,
            )
        })
        .collect()
}

/// Operaciones abiertas cuando se decidió la entrada de `trades[i]` (el cierre anterior a su
/// entrada: decide al fin de la vela de la señal = inicio de la vela de entrada).
fn open_at_entry(trades: &[PortfolioTrade], i: usize) -> Vec<usize> {
    let iv = intervals(trades);
    let decided = iv[i].1;
    // Las operaciones están en el orden en que se abrieron: solo cuentan las anteriores.
    (0..i)
        .filter(|&j| iv[j].1 <= decided && iv[j].2 > decided)
        .collect()
}

#[test]
fn the_portfolio_shares_one_account() {
    let (m15, h1, setups) = three(named);
    let report = run_portfolio(&markets(&m15, &h1, &setups)).unwrap();
    let trades = &report.trades;
    let mut overlapped = false;
    for i in 0..trades.len() {
        let others = open_at_entry(trades, i);
        overlapped |= !others.is_empty();
        assert!(others.len() < 5, "más de 5 posiciones");
        assert!(
            others
                .iter()
                .all(|&j| trades[j].instrument != trades[i].instrument),
            "dos posiciones en el mismo instrumento"
        );
        // El margen libre descuenta el margen de las otras abiertas, y la nueva entra en él.
        let used: Decimal = others.iter().map(|&j| trades[j].record.margin).sum();
        assert_eq!(
            trades[i].free_margin_at_entry,
            trades[i].equity_at_entry - used
        );
        assert!(trades[i].record.margin <= trades[i].free_margin_at_entry);
    }
    assert!(overlapped, "la serie no probó posiciones simultáneas");
    let net: Decimal = trades.iter().map(|t| t.record.net_pnl).sum();
    assert_eq!(
        report.final_equity.round_dp(10),
        (dec!(1000) + net).round_dp(10)
    );
}

#[test]
fn with_one_position_at_most_trades_never_overlap() {
    let (m15, h1, setups) = three(|n| {
        let mut s = named(n);
        s.risk.max_open_positions = 1;
        s
    });
    let report = run_portfolio(&markets(&m15, &h1, &setups)).unwrap();
    assert!(report.trades.len() > 3);
    for i in 0..report.trades.len() {
        assert!(open_at_entry(&report.trades, i).is_empty());
    }
    assert!(
        report
            .rejected
            .iter()
            .any(|r| r.rejected.reason == Rejection::MaxOpenPositions)
    );
}

#[test]
fn margin_is_shared_between_positions() {
    // Margen del 25 %: una posición entra, pero varias juntas se quedan sin margen libre.
    let (m15, h1, setups) = three(|n| {
        let mut s = named(n);
        s.limits.margin_pct = dec!(25);
        s
    });
    let report = run_portfolio(&markets(&m15, &h1, &setups)).unwrap();
    assert!(
        report
            .rejected
            .iter()
            .any(|r| r.rejected.reason == Rejection::InsufficientMargin)
    );
    for t in &report.trades {
        assert!(t.record.margin <= t.free_margin_at_entry);
    }
}

#[test]
fn pnl_is_converted_to_the_account_currency() {
    let (m15, h1, setups) = three(|n| {
        let mut s = named(n);
        if n != "DE40" {
            s.account_to_instrument = dec!(1.25);
        }
        s
    });
    let report = run_portfolio(&markets(&m15, &h1, &setups)).unwrap();
    let net: Decimal = report
        .trades
        .iter()
        .map(|t| {
            let fx = if t.instrument.to_string() == "DE40" {
                dec!(1)
            } else {
                dec!(1.25)
            };
            t.record.net_pnl / fx
        })
        .sum();
    assert_eq!(
        report.final_equity.round_dp(10),
        (dec!(1000) + net).round_dp(10)
    );
}

#[test]
fn the_instruments_must_share_the_account_setup() {
    let (m15, h1, mut setups) = three(named);
    setups[1].initial_equity = dec!(2000);
    assert_eq!(
        run_portfolio(&markets(&m15, &h1, &setups)).unwrap_err(),
        PortfolioError::MixedSetup("capital inicial")
    );
    let (m15, h1, mut setups) = three(named);
    setups[2].risk.max_open_positions = 3;
    assert_eq!(
        run_portfolio(&markets(&m15, &h1, &setups)).unwrap_err(),
        PortfolioError::MixedSetup("riesgo")
    );
    let (m15, h1, mut setups) = three(named);
    setups[0].day_tz = chrono_tz::UTC;
    assert_eq!(
        run_portfolio(&markets(&m15, &h1, &setups)).unwrap_err(),
        PortfolioError::MixedSetup("zona del día")
    );
    assert_eq!(run_portfolio(&[]).unwrap_err(), PortfolioError::Empty);
}

#[test]
fn equity_includes_the_unrealized_pnl_of_open_positions() {
    let (m15, h1, setups) = three(named);
    let report = run_portfolio(&markets(&m15, &h1, &setups)).unwrap();
    let trades = &report.trades;
    let iv = intervals(trades);
    let mut with_open = 0;
    for i in 0..trades.len() {
        let decided = iv[i].1;
        // Realizado: inicial + netos de lo que ya cerró.
        let realized = dec!(1000)
            + (0..i)
                .filter(|&j| iv[j].2 <= decided)
                .map(|j| trades[j].record.net_pnl)
                .sum::<Decimal>();
        if open_at_entry(trades, i).is_empty() {
            assert_eq!(
                trades[i].equity_at_entry.round_dp(10),
                realized.round_dp(10),
                "operación {i}"
            );
        } else if trades[i].equity_at_entry.round_dp(10) != realized.round_dp(10) {
            with_open += 1;
        }
    }
    // Con posiciones abiertas, el capital se mueve con ellas.
    assert!(with_open > 0);
}

#[test]
fn a_position_never_blocks_its_own_instrument_after_closing() {
    // Una operación que cierra en la vela k libera el instrumento al cierre de k: nunca hay un
    // rechazo por "ya hay una posición en el instrumento" (el recorrido no decide mientras está
    // abierta, y al cerrar ya se realizó).
    let (m15, h1, setups) = three(named);
    let report = run_portfolio(&markets(&m15, &h1, &setups)).unwrap();
    assert!(
        report
            .rejected
            .iter()
            .all(|r| r.rejected.reason != Rejection::InstrumentAlreadyOpen)
    );
}

#[test]
fn equity_marks_open_positions_exactly() {
    // Con comisión, lo realizado (neto) se distingue de lo que todavía se valúa (bruto): así el
    // test también fija que una operación que cierra justo al decidir ya está realizada.
    let (m15, h1, setups) = three(|n| {
        let mut s = named(n);
        s.costs.commission = Commission::PerShare {
            per_share: dec!(0.02),
            min: dec!(0.5),
        };
        s
    });
    let report = run_portfolio(&markets(&m15, &h1, &setups)).unwrap();
    let trades = &report.trades;
    let iv = intervals(trades);
    let series = |name: &str| match name {
        "US100" => &m15[0],
        "US500" => &m15[1],
        _ => &m15[2],
    };
    // Valor de una operación abierta a `now`: salidas ya hechas + el resto al último cierre.
    let value = |t: &PortfolioTrade, now: DateTime<Utc>| -> Decimal {
        let r = &t.record;
        let s = if r.side == Side::Long {
            dec!(1)
        } else {
            dec!(-1)
        };
        let done: Vec<_> = r.exits.iter().filter(|e| e.time <= now).collect();
        let rest = dec!(1) - done.iter().map(|e| e.fraction).sum::<Decimal>();
        let c = series(&t.instrument.to_string());
        let last = c.iter().rposition(|k| k.end() <= now).unwrap();
        let mark = if last >= r.entry_index {
            r.size * rest * (c[last].close - r.entry_price) * s
        } else {
            dec!(0)
        };
        done.iter()
            .map(|e| r.size * e.fraction * (e.price - r.entry_price) * s)
            .sum::<Decimal>()
            + mark
    };
    let mut checked = 0;
    for i in 0..trades.len() {
        let decided = iv[i].1;
        let others = open_at_entry(trades, i);
        if others.is_empty() {
            continue;
        }
        let realized = dec!(1000)
            + (0..i)
                .filter(|&j| iv[j].2 <= decided)
                .map(|j| trades[j].record.net_pnl)
                .sum::<Decimal>();
        let marked: Decimal = others.iter().map(|&j| value(&trades[j], decided)).sum();
        assert_eq!(
            trades[i].equity_at_entry.round_dp(10),
            (realized + marked).round_dp(10),
            "operación {i}"
        );
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn a_trade_open_when_the_data_ends_is_realized() {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let full = run(&m15, &h1, &[], &[], &setup()).unwrap();
    // Se cortan los datos en medio de la tercera operación.
    let cut = full.trades[2].entry_index + 1;
    let short = run(&m15[..=cut], &h1, &[], &[], &setup()).unwrap();
    let last = short.trades.last().unwrap();
    assert_eq!(last.exits.last().unwrap().reason, CloseReason::EndOfData);
    // La curva diaria termina con el capital final, que ya incluye esa operación.
    assert_eq!(short.daily_equity.last().unwrap().1, short.final_equity);
    let net: Decimal = short.trades.iter().map(|t| t.net_pnl).sum();
    assert_eq!(
        short.final_equity.round_dp(10),
        (dec!(1000) + net).round_dp(10)
    );
}

#[test]
fn the_run_keeps_a_daily_equity_curve() {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let report = run(&m15, &h1, &[], &[], &setup()).unwrap();
    let curve = &report.daily_equity;
    // Un punto por día de Madrid con velas decididas, en orden, y el último es el capital final.
    assert!(curve.windows(2).all(|w| w[0].0 < w[1].0));
    assert_eq!(curve.last().unwrap().1, report.final_equity);
    let first = m15[0]
        .end()
        .with_timezone(&chrono_tz::Europe::Madrid)
        .date_naive();
    let last = m15[m15.len() - 2]
        .end()
        .with_timezone(&chrono_tz::Europe::Madrid)
        .date_naive();
    assert_eq!((curve[0].0, curve.last().unwrap().0), (first, last));
    assert_eq!(curve.len() as i64, (last - first).num_days() + 1);
    // El valor de cada día es el del último cierre decidido de ese día: si a esa hora no hay
    // posición abierta, es el capital realizado.
    let madrid = |t: DateTime<Utc>| t.with_timezone(&chrono_tz::Europe::Madrid).date_naive();
    let mut checked = 0;
    for &(d, value) in &curve[..curve.len() - 1] {
        let close = m15[..m15.len() - 1]
            .iter()
            .map(|c| c.end())
            .filter(|&e| madrid(e) == d)
            .max()
            .unwrap();
        let open = report
            .trades
            .iter()
            .any(|t| t.entry_time <= close && t.exits.last().unwrap().time > close);
        if open {
            continue;
        }
        let realized = dec!(1000)
            + report
                .trades
                .iter()
                .filter(|t| t.exits.last().unwrap().time <= close)
                .map(|t| t.net_pnl)
                .sum::<Decimal>();
        assert_eq!(value.round_dp(10), realized.round_dp(10), "{d}");
        checked += 1;
    }
    assert!(checked > 10);
}

#[test]
fn no_decision_outside_the_entry_window() {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let all = run(&m15, &h1, &[], &[], &setup()).unwrap();
    // Sin ninguna vela habilitada, no hay operaciones ni rechazos: ni siquiera se decide.
    let none = run(&m15, &h1, &vec![false; m15.len()], &[], &setup()).unwrap();
    assert!(none.trades.is_empty() && none.rejected.is_empty());
    // Habilitando solo las velas pares, ninguna señal sale de una impar.
    let even: Vec<bool> = (0..m15.len()).map(|i| i % 2 == 0).collect();
    let some = run(&m15, &h1, &even, &[], &setup()).unwrap();
    assert!(!some.trades.is_empty());
    assert!(some.trades.iter().all(|t| t.signal_index % 2 == 0));
    assert!(some.rejected.iter().all(|r| r.index % 2 == 0));
    assert_ne!(some.trades.len(), all.trades.len());
}

/// Interpretación de prueba con la forma del contrato `news_interpretation.v1`.
fn interpretation(
    k: u32,
    published_at: DateTime<Utc>,
    interpreted_at: DateTime<Utc>,
    sentiment: &str,
) -> cfds_core::NewsInterpretation {
    let z = |t: DateTime<Utc>| t.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
    serde_json::from_value(serde_json::json!({
        "interpretation_id": format!("{k:064x}"),
        "source": "fed",
        "item_id": format!("noticia-{k}"),
        "published_at": z(published_at),
        "interpreted_at": z(interpreted_at),
        "interpreter": {"name": "ollama", "version": "1", "model": null, "prompt_sha256": null},
        "summary_es": "resumen",
        "impacts": [{"instrument_id": "US100", "sentiment": sentiment, "relevance": "1"}],
    }))
    .unwrap()
}

fn news_points(t: &cfds_backtest::TradeRecord) -> i32 {
    t.breakdown
        .iter()
        .find(|(c, _)| *c == Component::News)
        .map_or(0, |(_, p)| *p)
}

#[test]
fn news_adjust_the_score_from_the_close_they_were_interpreted_at() {
    let m15 = random_walk(3000, 11);
    let h1 = resample_h1(&m15);
    let s = setup();
    let base = run(&m15, &h1, &[], &[], &s).unwrap();
    let first = base.trades.first().expect("hay operaciones").clone();
    let close = m15[first.signal_index].end();
    let long = first.side == Side::Long;
    // A favor de la operación: +2 (2 × 1 × 1, recortado a news_max).
    let favor = if long { "1" } else { "-1" };
    let cutoff = utc("2020-01-01T00:00:00Z");
    let published = close - TimeDelta::hours(1);

    // Sin interpretaciones, el recorrido es el mismo que sin noticias.
    let none = run_with_news(&m15, &h1, &[], &[], &[], &[], cutoff, &s).unwrap();
    assert_eq!(none, base);

    // Interpretada justo al cierre de la vela de la señal: ya cuenta.
    let at_close = [interpretation(1, published, close, favor)];
    let with = run_with_news(&m15, &h1, &[], &[], &[], &at_close, cutoff, &s).unwrap();
    let t = &with.trades[0];
    assert_eq!((t.signal_index, t.side), (first.signal_index, first.side));
    assert_eq!(news_points(&first), 0);
    assert_eq!(news_points(t), 2);
    assert_eq!(t.score, first.score + 2);

    // Un microsegundo después del cierre: todavía no existía (sin look-ahead).
    let late = [interpretation(
        2,
        published,
        close + TimeDelta::microseconds(1),
        favor,
    )];
    let after = run_with_news(&m15, &h1, &[], &[], &[], &late, cutoff, &s).unwrap();
    assert_eq!(after.trades[0], first);

    // Publicada antes del corte del modelo (ADR 0008 §5): no cuenta aunque esté en la lista.
    let model_cutoff = published + TimeDelta::seconds(1);
    let old = run_with_news(&m15, &h1, &[], &[], &[], &at_close, model_cutoff, &s).unwrap();
    assert_eq!(old, base);
}

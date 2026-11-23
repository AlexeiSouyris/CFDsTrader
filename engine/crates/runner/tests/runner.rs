//! Runner de paper trading (fase 8, #305): sin look-ahead, reinicio sin duplicados, huecos de
//! datos que activan el kill switch y la alerta, y la liquidación diaria del swap. Precios
//! sintéticos (un paseo al azar con semilla fija), mercado abierto 24 h.

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, MarketSetup, PaperBroker, restore_and_reconcile,
};
use cfds_core::{Candle, InstrumentId, OrderStatus, Timeframe};
use cfds_indicators::TrendConfig;
use cfds_liquidity::LiquidityConfig;
use cfds_market_data::{SessionCalendar, ValidationConfig};
use cfds_obs::{AlertConfig, Alerter, Logger, ReqwestPost};
use cfds_patterns::PatternsConfig;
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_runner::{Feed, Notice, Runner, RunnerConfig, Step, publish};
use cfds_strategy::{Configs, StrategyConfig};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

/// Ventana de velas que se le pasa al runner (alcanza para el calentamiento: 200 de 15m y 60 de
/// 1h).
const WINDOW: usize = 320;

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

/// Mercado abierto todo el tiempo: toda vela de la grilla tendría que existir.
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

fn runner() -> Runner {
    let strategy = Configs {
        patterns: PatternsConfig::from_toml(cfg!("patterns.toml")).unwrap(),
        liquidity: LiquidityConfig::from_toml(cfg!("liquidity.toml")).unwrap(),
        trend: TrendConfig::from_toml(cfg!("indicators.toml")).unwrap(),
        strategy: StrategyConfig::from_toml(cfg!("strategy.toml")).unwrap(),
    };
    Runner::new(
        RunnerConfig::from_toml(cfg!("runner.toml")).unwrap(),
        strategy,
        ValidationConfig::from_toml(cfg!("data_validation.toml")).unwrap(),
    )
}

fn broker_cfg() -> BrokerConfig {
    BrokerConfig::from_toml(
        cfg!("broker.toml"),
        RiskConfig::from_toml(cfg!("risk.toml")).unwrap(),
        CostConfig {
            slippage_spread_fraction: dec!(0.1),
            slippage_ticks: dec!(1),
        },
        ExitConfig::from_toml(cfg!("backtest.toml")).unwrap(),
        chrono_tz::Europe::Madrid,
    )
    .unwrap()
}

fn markets(candles: &[Candle]) -> BTreeMap<InstrumentId, MarketSetup> {
    let m = MarketSetup {
        limits: InstrumentLimits {
            max_leverage: dec!(10),
            margin_pct: dec!(5),
            min_deal_size: Some(dec!(0.01)),
        },
        costs: InstrumentCosts {
            spread: dec!(1),
            tick: dec!(0.1),
            commission: Commission::None,
            funding_admin_fee_pct: dec!(2.5),
            funding_day_count: dec!(365),
        },
        account_to_instrument: Decimal::ONE,
        candles: candles.to_vec(),
        funding: FundingSetup {
            cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            tz: chrono_tz::Europe::Madrid,
            rates: FundingRates::Constant(dec!(2)),
        },
    };
    [(id(), m)].into_iter().collect()
}

/// Un paso con la compuerta abierta, sin filtros de entrada ni noticias, viendo las velas
/// `m15[..=last]` (más las futuras que se pasen en `extra`, que no cerraron todavía).
fn step_at(
    r: &mut Runner,
    b: &mut PaperBroker,
    m15: &[Candle],
    last: usize,
    now: DateTime<Utc>,
) -> Step {
    let from = last + 1 - WINDOW.min(last + 1);
    let window = &m15[from..m15.len()];
    let h1 = resample_h1(&m15[from - from % 4..]);
    r.step(
        b,
        Feed {
            instrument: &id(),
            m15: window,
            h1: &h1,
        },
        &AlwaysOpen,
        true,
        |_, _| true,
        |_| 0,
        now,
    )
}

/// La primera vela `L` en la que el runner propone una orden (decisión al cierre de `L − 1`).
fn first_proposal(m15: &[Candle]) -> usize {
    let mut r = runner();
    let mut b = PaperBroker::new(broker_cfg(), markets(m15));
    for last in WINDOW..m15.len() {
        let s = step_at(&mut r, &mut b, &m15[..=last], last, m15[last].end());
        if s.order.is_some() {
            return last;
        }
    }
    panic!("el paseo al azar no dio ninguna señal");
}

#[test]
fn the_repo_config_is_valid_and_off_by_default() {
    let c = RunnerConfig::from_toml(cfg!("runner.toml")).unwrap();
    assert!(!c.enabled, "el runner arranca apagado");
    for (old, new) in [
        ("poll_secs = 30", "poll_secs = 0"),
        ("lookback_days = 15", "lookback_days = 0"),
        ("stale_after_minutes = 45", "stale_after_minutes = 5"),
        ("order_prefix = \"run\"", "order_prefix = \"a b\""),
        ("enabled = false", "enabled = false\nlive = true"),
    ] {
        let bad = cfg!("runner.toml").replacen(old, new, 1);
        assert_ne!(bad, cfg!("runner.toml"));
        assert!(RunnerConfig::from_toml(&bad).is_err(), "{new}");
    }
}

#[test]
fn a_decision_at_t_only_sees_t_and_is_filled_at_the_open_of_t_plus_1() {
    let m15 = random_walk(1500, 11);
    let l = first_proposal(&m15);
    let now = m15[l].end();

    // Con velas futuras en la base (todavía abiertas en `now`), la decisión es la misma.
    let mut b = PaperBroker::new(broker_cfg(), markets(&m15[..=l]));
    let with_future = step_at(
        &mut runner(),
        &mut b,
        &m15[..(l + 6).min(m15.len())],
        l,
        now,
    );
    let order = with_future.order.clone().expect("la misma propuesta");
    // Y cambiar la vela L (la de la ejecución) tampoco cambia la decisión.
    let mut changed = m15[..=l].to_vec();
    changed[l].close += dec!(0.5);
    changed[l].high += dec!(0.5);
    let mut b2 = PaperBroker::new(broker_cfg(), markets(&changed));
    let other = step_at(&mut runner(), &mut b2, &changed, l, now);
    assert_eq!(other.order, Some(order.clone()));

    // Decisión al cierre de t = L − 1; el broker la llena en la apertura de L.
    assert_eq!(order.created_at, m15[l - 1].end());
    assert_eq!(order.created_at, m15[l].start);
    let report = b.submit(&order);
    assert_eq!(report.at, m15[l].start, "{report:?}");
    assert_eq!(report.status, OrderStatus::Filled, "{report:?}");
    let px = report.fill_price.unwrap();
    // Apertura de L ± medio spread y slippage (0,5 + 0,1 + 0,1).
    assert_eq!((px - m15[l].open).abs(), dec!(0.7));
}

#[test]
fn a_restart_resumes_without_duplicate_orders() {
    let m15 = random_walk(1500, 11);
    let l = first_proposal(&m15);
    let now = m15[l].end();
    let mut r = runner();
    let mut b = PaperBroker::new(broker_cfg(), markets(&m15[..=l]));
    let s = step_at(&mut r, &mut b, &m15, l, now);
    let first = b.submit(s.order.as_ref().unwrap());
    // El mismo runner no vuelve a proponer en la misma vela.
    assert!(step_at(&mut r, &mut b, &m15, l, now).order.is_none());

    // Reinicio: broker restaurado desde el registro y runner nuevo, en la misma vela.
    let journal = b.journal().to_vec();
    // En paper, las posiciones "del broker" son las del simulador anterior (como el gateway).
    let positions = b.positions(now);
    let (mut b2, rec) =
        restore_and_reconcile(broker_cfg(), markets(&m15[..=l]), &journal, &positions, now);
    assert!(rec.is_clean(), "{rec:?}");
    assert!(!b2.kill_switch_active());
    let again = step_at(&mut runner(), &mut b2, &m15, l, now);
    let order = again.order.expect("vuelve a proponer la misma decisión");
    assert_eq!(order.order_id, s.order.unwrap().order_id, "mismo id");
    let report = b2.submit(&order);
    assert_eq!(report, first, "devuelve el informe guardado");
    assert_eq!(b2.journal().len(), 1, "sin orden duplicada");
}

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn obs() -> (Arc<Logger>, Alerter<ReqwestPost>, Captured) {
    let out = Captured::default();
    let log = Arc::new(Logger::new(Box::new(out.clone())));
    let alerts = Alerter::new(
        AlertConfig::from_toml(cfg!("alerts.toml")).unwrap(),
        log.clone(),
        None,
    );
    (log, alerts, out)
}

fn text(c: &Captured) -> String {
    String::from_utf8(c.0.lock().unwrap().clone()).unwrap()
}

#[tokio::test]
async fn a_missing_candle_activates_the_kill_switch_and_alerts() {
    let m15 = random_walk(600, 11);
    let mut r = runner();
    let mut b = PaperBroker::new(broker_cfg(), markets(&m15));
    // Arranque con datos completos: nada.
    let ok = step_at(&mut r, &mut b, &m15, 400, m15[400].end());
    assert!(
        !ok.notices
            .iter()
            .any(|n| matches!(n, Notice::DataGap { .. }))
    );
    assert!(!b.kill_switch_active());

    // Llega una vela nueva, pero falta una del medio (sesión abierta): hueco.
    let mut gap = m15[..=402].to_vec();
    gap.remove(395);
    let s = step_at(&mut r, &mut b, &gap, gap.len() - 1, m15[402].end());
    assert!(
        s.notices
            .iter()
            .any(|n| matches!(n, Notice::DataGap { .. })),
        "{:?}",
        s.notices
    );
    assert!(b.kill_switch_active());
    assert!(s.order.is_none());
    let (log, alerts, out) = obs();
    publish(&log, &alerts, &s.notices, m15[402].end()).await;
    let t = text(&out);
    assert!(t.contains("\"event\":\"data_gap\""));
    assert!(t.contains("\"kind\":\"data_error\""), "{t}");
}

#[tokio::test]
async fn candles_that_stop_arriving_in_session_are_a_gap_too() {
    let m15 = random_walk(600, 11);
    let mut r = runner();
    let mut b = PaperBroker::new(broker_cfg(), markets(&m15));
    step_at(&mut r, &mut b, &m15, 400, m15[400].end());
    // Dos horas sin velas nuevas con el mercado abierto.
    let late = m15[400].end() + TimeDelta::hours(2);
    let s = step_at(&mut r, &mut b, &m15[..=400], 400, late);
    assert!(b.kill_switch_active());
    assert!(matches!(
        s.notices.iter().find(|n| matches!(n, Notice::DataGap { .. })),
        Some(Notice::DataGap { detail, .. }) if detail.contains("datos en vivo")
    ));
    // Solo una vez por hueco.
    let again = step_at(
        &mut r,
        &mut b,
        &m15[..=400],
        400,
        late + TimeDelta::minutes(1),
    );
    assert!(
        !again
            .notices
            .iter()
            .any(|n| matches!(n, Notice::DataGap { .. }))
    );
}

#[test]
fn nothing_is_proposed_with_the_gate_closed_or_the_kill_switch_on() {
    let m15 = random_walk(1500, 11);
    let l = first_proposal(&m15);
    let now = m15[l].end();
    let window = &m15[l + 1 - WINDOW..=l];
    let h1 = resample_h1(&m15[(l + 1 - WINDOW) - (l + 1 - WINDOW) % 4..=l]);
    let mut r = runner();
    let mut b = PaperBroker::new(broker_cfg(), markets(&m15[..=l]));
    let closed = r.step(
        &mut b,
        Feed {
            instrument: &id(),
            m15: window,
            h1: &h1,
        },
        &AlwaysOpen,
        false,
        |_, _| true,
        |_| 0,
        now,
    );
    assert!(closed.order.is_none());
    // Al abrir la compuerta, decide sobre la misma vela.
    assert!(step_at(&mut r, &mut b, &m15, l, now).order.is_some());

    let mut r = runner();
    let mut b = PaperBroker::new(broker_cfg(), markets(&m15[..=l]));
    b.activate_kill_switch("prueba", now);
    let s = step_at(&mut r, &mut b, &m15, l, now);
    assert!(s.order.is_none());
    assert!(
        s.notices.iter().any(
            |n| matches!(n, Notice::Skipped { reason, .. } if *reason == "kill switch activo")
        )
    );
}

#[test]
fn swap_is_settled_once_a_day_after_the_funding_cutoff() {
    let m15 = random_walk(600, 11);
    let mut r = runner();
    let mut b = PaperBroker::new(broker_cfg(), markets(&m15));
    // 2026-01-07 22:00 UTC = 23:00 en Madrid (invierno): el corte.
    let i = m15
        .iter()
        .position(|c| c.end() == utc("2026-01-07T22:00:00Z"))
        .unwrap();
    let settled = |s: &Step| {
        s.notices
            .iter()
            .filter(|n| matches!(n, Notice::SwapSettled { .. }))
            .count()
    };
    let before = step_at(&mut r, &mut b, &m15, i - 1, m15[i - 1].end());
    let at_cutoff = step_at(&mut r, &mut b, &m15, i, m15[i].end());
    let after = step_at(&mut r, &mut b, &m15, i + 1, m15[i + 1].end());
    // El primer paso puede caer después del corte de otro día; lo que importa es el corte del 7.
    assert!(settled(&before) <= 1);
    assert_eq!(settled(&at_cutoff), 1);
    assert_eq!(settled(&after), 0, "una vez por día");
}

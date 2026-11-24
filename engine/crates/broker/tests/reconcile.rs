//! Reconciliación al arrancar (fase 7, #278). Precios sintéticos.

use std::collections::BTreeMap;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, KillSwitchChange, KillSwitchMark, MarketSetup, Mismatch,
    PaperBroker, kill_switch_stored_active, reconcile, restore_and_reconcile,
    restore_with_kill_switch,
};
use cfds_core::{
    Candle, InstrumentId, KillSwitch, Order, OrderRejection, OrderSide, OrderStatus, PaperMode,
    Position, Timeframe,
};
use cfds_risk::{InstrumentLimits, RiskConfig};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("fixtures/broker.toml");

fn at(i: i64) -> DateTime<Utc> {
    "2026-11-18T09:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(15 * i)
}

fn flat(id: &str, close: Decimal) -> Vec<Candle> {
    (0..30)
        .map(|i| Candle {
            instrument_id: id.parse().unwrap(),
            timeframe: Timeframe::M15,
            start: at(i),
            open: close,
            high: close + dec!(1),
            low: close - dec!(1),
            close,
            volume: None,
        })
        .collect()
}

fn setup(close: Decimal) -> (BrokerConfig, BTreeMap<InstrumentId, MarketSetup>) {
    let cfg = BrokerConfig::from_toml(
        BROKER,
        RiskConfig::from_toml(RISK).unwrap(),
        CostConfig {
            slippage_spread_fraction: dec!(0.1),
            slippage_ticks: dec!(1),
        },
        ExitConfig::from_toml(BACKTEST).unwrap(),
        chrono_tz::Europe::Madrid,
    )
    .unwrap();
    let markets = ["US100", "DE40"]
        .into_iter()
        .map(|id| {
            (
                id.parse().unwrap(),
                MarketSetup {
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
                    account_to_instrument: cfds_backtest::AccountFx::Same,
                    candles: flat(id, close),
                    funding: FundingSetup {
                        cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
                        tz: chrono_tz::Europe::Madrid,
                        rates: FundingRates::Constant(dec!(2)),
                    },
                },
            )
        })
        .collect();
    (cfg, markets)
}

fn order(id: &str, instrument: &str, stop: Decimal) -> Order {
    Order {
        order_id: id.parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: instrument.parse().unwrap(),
        side: OrderSide::Long,
        stop,
        tp1: None,
        proposal_id: None,
        created_at: at(20),
    }
}

/// Un broker que trabajó, su registro y sus posiciones (lo que se guarda al parar).
fn session() -> (PaperBroker, Vec<Position>) {
    let (cfg, markets) = setup(dec!(100));
    let mut b = PaperBroker::new(cfg, markets);
    b.submit(&order("a", "US100", dec!(98)));
    b.submit(&order("b", "DE40", dec!(98)));
    b.submit(&order("c", "DE40", dec!(97))); // rechazada: ya hay una en DE40
    let p = b.positions(at(29));
    (b, p)
}

#[test]
fn a_clean_restart_rebuilds_the_same_positions_without_kill_switch() {
    let (b, positions) = session();
    assert_eq!(positions.len(), 2);
    let (cfg, markets) = setup(dec!(100));
    let (mut restored, rec) = restore_and_reconcile(cfg, markets, b.journal(), &positions, at(29));
    assert!(rec.is_clean(), "{rec:?}");
    assert_eq!(rec.matched, 2);
    assert_eq!(restored.positions(at(29)), positions);
    assert!(!restored.kill_switch_active());
    assert_eq!(restored.journal(), b.journal());
}

#[test]
fn a_position_the_broker_does_not_have_activates_the_kill_switch() {
    let (b, positions) = session();
    let (cfg, markets) = setup(dec!(100));
    let (mut restored, rec) =
        restore_and_reconcile(cfg, markets, b.journal(), &positions[..1], at(29));
    assert_eq!(
        rec.mismatches,
        vec![Mismatch::MissingInBroker(positions[1].position_id.clone())]
    );
    assert!(matches!(
        restored.account(at(29)).kill_switch,
        KillSwitch::Active { ref reason, .. } if reason.contains("Reconciliación")
    ));
}

#[test]
fn unknown_or_different_positions_are_mismatches() {
    let (_, positions) = session();
    let mut other = positions[0].clone();
    other.position_id = "pos:extra".parse().unwrap();
    let mut changed = positions[1].clone();
    changed.size += dec!(1);
    let rec = reconcile(&positions, &[positions[0].clone(), changed, other]);
    assert_eq!(rec.matched, 1);
    assert_eq!(
        rec.mismatches,
        vec![
            Mismatch::Different(positions[1].position_id.clone()),
            Mismatch::UnknownInBroker("pos:extra".parse().unwrap()),
        ]
    );
}

#[test]
fn candles_that_changed_underneath_are_drift_and_stop_trading() {
    let (b, positions) = session();
    // Las velas guardadas cambiaron (repintado): el fill reconstruido sale a otro precio.
    let (cfg, markets) = setup(dec!(101));
    let (mut restored, rec) = restore_and_reconcile(cfg, markets, b.journal(), &positions, at(29));
    assert!(!rec.drifts.is_empty());
    assert!(restored.kill_switch_active());
    assert!(matches!(
        restored.account(at(29)).kill_switch,
        KillSwitch::Active { .. }
    ));
}

/// Un kill switch guardado (#441) después de la orden `a`: `b` sale rechazada por él.
fn session_with_kill_switch() -> (PaperBroker, Vec<KillSwitchMark>) {
    let (cfg, markets) = setup(dec!(100));
    let mut b = PaperBroker::new(cfg, markets);
    b.submit(&order("a", "US100", dec!(98)));
    b.activate_kill_switch("error de datos en DE40", at(20));
    let r = b.submit(&order("b", "DE40", dec!(98)));
    assert_eq!(r.rejection, Some(OrderRejection::KillSwitchActive));
    let marks = vec![KillSwitchMark {
        after_orders: 1,
        change: KillSwitchChange::Activated {
            reason: "error de datos en DE40".into(),
            at: at(20),
        },
    }];
    (b, marks)
}

#[test]
fn a_saved_kill_switch_survives_a_restart_at_the_same_point_of_the_journal() {
    let (mut b, marks) = session_with_kill_switch();
    let positions = b.positions(at(29));
    let (cfg, markets) = setup(dec!(100));
    let (mut restored, rec) =
        restore_with_kill_switch(cfg, markets, b.journal(), &marks, &positions, at(29));
    assert!(rec.is_clean(), "{rec:?}");
    assert!(kill_switch_stored_active(&marks));
    assert!(matches!(
        restored.account(at(29)).kill_switch,
        KillSwitch::Active { ref reason, since } if reason == "error de datos en DE40" && since == at(20)
    ));
}

#[test]
fn without_the_saved_kill_switch_the_restart_would_reopen_trading() {
    // Lo que pasaba antes de #441: sin el cambio guardado, la orden rechazada por el kill
    // switch se reconstruye distinta (drift) y el motivo del kill switch se pierde.
    let (mut b, _) = session_with_kill_switch();
    let positions = b.positions(at(29));
    let (cfg, markets) = setup(dec!(100));
    let (_, rec) = restore_and_reconcile(cfg, markets, b.journal(), &positions, at(29));
    assert_eq!(rec.drifts.len(), 1);
    assert_eq!(rec.drifts[0].order_id.as_str(), "b");
}

#[test]
fn only_a_saved_release_turns_the_kill_switch_off_and_later_orders_rebuild_the_same() {
    let (mut b, mut marks) = session_with_kill_switch();
    marks.push(KillSwitchMark {
        after_orders: 2,
        change: KillSwitchChange::Released,
    });
    // El broker en marcha no se libera solo: la liberación vale desde el reinicio.
    let before = b.positions(at(20));
    let (cfg, markets) = setup(dec!(100));
    let (mut after_vote, rec) =
        restore_with_kill_switch(cfg, markets, b.journal(), &marks, &before, at(20));
    assert!(rec.is_clean(), "{rec:?}");
    assert!(!after_vote.kill_switch_active());
    assert!(!kill_switch_stored_active(&marks));
    let r = after_vote.submit(&order("c", "DE40", dec!(98)));
    assert_eq!(r.status, OrderStatus::Filled);
    // Y se reconstruye igual: rechazada con el kill switch, llenada después de liberarlo.
    let positions = after_vote.positions(at(29));
    let (cfg, markets) = setup(dec!(100));
    let (again, rec) = restore_with_kill_switch(
        cfg,
        markets,
        after_vote.journal(),
        &marks,
        &positions,
        at(29),
    );
    assert!(rec.is_clean(), "{rec:?}");
    assert!(!again.kill_switch_active());
    assert_eq!(again.journal(), after_vote.journal());
}

/// 20 velas anchas (rango 20) y después 61 angostas (rango 2), desde `at(0)`. La última
/// (`at(80)`) es la del fill de una orden de `at(80)`.
fn wide_then_narrow(id: &str) -> Vec<Candle> {
    (0..81)
        .map(|i| {
            let half = if i < 20 { dec!(10) } else { dec!(1) };
            Candle {
                instrument_id: id.parse().unwrap(),
                timeframe: Timeframe::M15,
                start: at(i),
                open: dec!(100),
                high: dec!(100) + half,
                low: dec!(100) - half,
                close: dec!(100),
                volume: None,
            }
        })
        .collect()
}

fn markets_from(first: usize) -> BTreeMap<InstrumentId, MarketSetup> {
    let (_, mut markets) = setup(dec!(100));
    for (id, m) in &mut markets {
        m.candles = wide_then_narrow(id.as_str())[first..].to_vec();
    }
    markets
}

/// #438: el ATR de Wilder depende de la primera vela. Una sesión que arrancó con más historia
/// que la que carga la reconstrucción decide distinto (acá, el stop a 6,4 puntos pasa con el
/// ATR de toda la serie y es `stop_too_far` con el de las últimas 60 velas).
#[test]
fn the_candles_start_at_the_first_order_so_a_restart_rebuilds_the_same() {
    let (cfg, _) = setup(dec!(100));
    let mut o = order("a", "DE40", dec!(93.6));
    o.created_at = at(80);
    let window = TimeDelta::minutes(15 * 60);

    // Sin fijar el comienzo: la sesión tiene velas desde at(0), la reconstrucción desde at(20).
    let mut loose = PaperBroker::new(cfg.clone(), markets_from(0));
    let r = loose.submit(&o);
    let (_, rec) =
        restore_and_reconcile(cfg.clone(), markets_from(20), loose.journal(), &[], at(80));
    assert_eq!(rec.drifts.len(), 1, "{r:?}");

    // Fijando el comienzo antes de la primera orden: igual que la reconstrucción.
    let mut anchored = PaperBroker::new(cfg.clone(), markets_from(0));
    anchored.start_candles_at(o.created_at - window).unwrap();
    anchored.submit(&o);
    let positions = anchored.positions(at(80));
    let (_, rec) = restore_and_reconcile(
        cfg,
        markets_from(20),
        anchored.journal(),
        &positions,
        at(80),
    );
    assert!(rec.is_clean(), "{rec:?}");
    // Con órdenes ya procesadas las velas no se tocan.
    assert!(anchored.start_candles_at(at(40)).is_err());
    assert_eq!(anchored.markets()[&o.instrument_id].candles.len(), 61);
}

/// Las velas de `markets` que empiezan en `[from, to)`, por instrumento.
fn window_of(
    mut markets: BTreeMap<InstrumentId, MarketSetup>,
    id: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> BTreeMap<InstrumentId, MarketSetup> {
    let id: InstrumentId = id.parse().unwrap();
    for (k, m) in &mut markets {
        m.candles
            .retain(|c| c.start >= from && (k != &id || c.start < to));
    }
    markets
}

fn candle_of(markets: &BTreeMap<InstrumentId, MarketSetup>, id: &str, i: i64) -> Candle {
    markets[&id.parse::<InstrumentId>().unwrap()]
        .candles
        .iter()
        .find(|c| c.start == at(i))
        .unwrap()
        .clone()
}

/// #471: una orden manual de ahora (a mitad de una vela) no tiene todavía su vela del fill.
/// La sesión en marcha no la rechaza con `no_price`: queda pendiente, fuera del registro, y se
/// decide cuando llega esa vela. La reconstrucción, que ya tiene la vela, la decide igual.
#[test]
fn a_manual_order_waits_for_its_fill_candle_and_rebuilds_the_same() {
    let (cfg, full) = setup(dec!(100));
    let mut o = order("manual", "US100", dec!(98));
    o.created_at = at(25) + TimeDelta::minutes(7);
    // En marcha: velas de US100 cerradas hasta la 24 (termina en at(25)).
    let mut live = PaperBroker::new(cfg.clone(), window_of(full.clone(), "US100", at(0), at(25)));

    let r = live.submit(&o);
    assert_eq!(r.status, OrderStatus::Pending);
    assert_eq!(r.rejection, None);
    assert!(live.journal().is_empty());
    assert_eq!(live.pending().len(), 1);
    // Repetirla no la duplica.
    assert_eq!(live.submit(&o), r);
    assert_eq!(live.pending().len(), 1);

    // La vela 25 (la de created_at) no es la del fill: sigue pendiente.
    live.push_candle(candle_of(&full, "US100", 25)).unwrap();
    assert!(live.settle_pending().is_empty());
    // La 26 empieza después de created_at: se decide y se llena en su apertura.
    live.push_candle(candle_of(&full, "US100", 26)).unwrap();
    let settled = live.settle_pending();
    assert_eq!(settled.len(), 1);
    let filled = &settled[0].1;
    assert_eq!(filled.status, OrderStatus::Filled, "{filled:?}");
    assert_eq!(filled.at, at(26));
    assert!(live.pending().is_empty());
    assert_eq!(live.journal().len(), 1);
    // Ya decidida, repetirla devuelve el informe del registro.
    assert_eq!(&live.submit(&o), filled);

    // La reconstrucción (con todas las velas) decide lo mismo: sin diferencias.
    let positions = live.positions(at(27));
    assert_eq!(positions.len(), 1);
    let (mut restored, rec) = restore_and_reconcile(cfg, full, live.journal(), &positions, at(27));
    assert!(rec.is_clean(), "{rec:?}");
    assert_eq!(restored.journal(), live.journal());
    assert_eq!(restored.positions(at(27)), positions);
}

/// #471: si el riesgo la rechaza, el rechazo no depende de la vela del fill: se registra ya, y
/// la reconstrucción lo repite.
#[test]
fn a_manual_order_the_risk_rejects_is_recorded_at_once_and_rebuilds_the_same() {
    let (cfg, full) = setup(dec!(100));
    let mut o = order("wrong", "US100", dec!(101)); // stop del lado equivocado
    o.created_at = at(25) + TimeDelta::minutes(7);
    let mut live = PaperBroker::new(cfg.clone(), window_of(full.clone(), "US100", at(0), at(25)));
    let r = live.submit(&o);
    assert_eq!(r.status, OrderStatus::Rejected);
    assert_eq!(r.rejection, Some(OrderRejection::StopWrongSide));
    assert!(live.pending().is_empty());
    assert_eq!(live.journal().len(), 1);
    let (_, rec) = restore_and_reconcile(cfg, full, live.journal(), &[], at(27));
    assert!(rec.is_clean(), "{rec:?}");
}

/// #471 con #438: las velas se recortan antes de la primera orden que **entra al registro**,
/// no de la primera que llega. Una pendiente que llega antes que otra orden de fecha anterior
/// (la del runner, con `created_at` = cierre de la vela de la decisión) no fija el comienzo.
#[test]
fn the_candles_start_at_the_first_recorded_order_even_with_a_pending_one() {
    let (cfg, full) = setup(dec!(100));
    let window = TimeDelta::minutes(15 * 15);
    let mut manual = order("manual", "US100", dec!(98));
    manual.created_at = at(25) + TimeDelta::minutes(7);
    let mut runner = order("runner", "DE40", dec!(98));
    runner.created_at = at(20);

    let mut live = PaperBroker::new(cfg.clone(), window_of(full.clone(), "US100", at(0), at(25)));
    live.anchor_candles_before_first_order(window);
    assert_eq!(live.submit(&manual).status, OrderStatus::Pending);
    // Todavía nada en el registro: las velas no se tocaron.
    assert_eq!(
        live.markets()[&manual.instrument_id].candles[0].start,
        at(0)
    );
    assert_eq!(live.submit(&runner).status, OrderStatus::Filled);
    for m in live.markets().values() {
        assert_eq!(m.candles[0].start, runner.created_at - window);
    }
    live.push_candle(candle_of(&full, "US100", 25)).unwrap();
    live.push_candle(candle_of(&full, "US100", 26)).unwrap();
    let settled = live.settle_pending();
    assert_eq!(settled[0].1.status, OrderStatus::Filled, "{settled:?}");

    // La reconstrucción carga las velas desde la primera orden del registro (la del runner).
    let positions = live.positions(at(27));
    assert_eq!(positions.len(), 2);
    let rebuilt = window_of(full, "US100", runner.created_at - window, at(30));
    let (_, rec) = restore_and_reconcile(cfg, rebuilt, live.journal(), &positions, at(27));
    assert!(rec.is_clean(), "{rec:?}");
}

/// Las velas de todos los instrumentos que empiezan en `[from, to)`.
fn between(
    mut markets: BTreeMap<InstrumentId, MarketSetup>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> BTreeMap<InstrumentId, MarketSetup> {
    for m in markets.values_mut() {
        m.candles.retain(|c| c.start >= from && c.start < to);
    }
    markets
}

/// #479: reinicio con una orden pendiente guardada. Arranca como el gateway: reconstruye el
/// broker desde el registro y los cambios del kill switch guardados, con las velas desde
/// `from` hasta `to` (las cerradas al arrancar), fija el recorte de las velas y retoma las
/// pendientes, en orden.
fn restart(
    live: &mut PaperBroker,
    marks: &[KillSwitchMark],
    full: &BTreeMap<InstrumentId, MarketSetup>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    window: TimeDelta,
) -> PaperBroker {
    let (cfg, _) = setup(dec!(100));
    let positions = live.positions(to);
    let (mut restored, rec) = restore_with_kill_switch(
        cfg,
        between(full.clone(), from, to),
        live.journal(),
        marks,
        &positions,
        to,
    );
    assert!(rec.is_clean(), "{rec:?}");
    restored.anchor_candles_before_first_order(window);
    for o in live.pending() {
        restored.restore_pending(o.clone()).unwrap();
    }
    restored
}

/// #479: la sesión que se reinicia con una orden pendiente decide exactamente lo mismo que la
/// que no se reinició, y la reconstrucción desde el registro final también.
#[test]
fn a_restart_with_a_pending_order_decides_the_same_as_without_restart() {
    let (cfg, full) = setup(dec!(100));
    let window = TimeDelta::minutes(15 * 15);
    let mut runner = order("runner", "DE40", dec!(98));
    runner.created_at = at(20);
    let mut manual = order("manual", "US100", dec!(98));
    manual.created_at = at(25) + TimeDelta::minutes(7);
    let mut later = order("later", "DE40", dec!(97));
    later.created_at = at(27);

    let run = |restart_after_pending: bool| {
        let mut live = PaperBroker::new(cfg.clone(), between(full.clone(), at(0), at(25)));
        live.anchor_candles_before_first_order(window);
        assert_eq!(live.submit(&runner).status, OrderStatus::Filled);
        assert_eq!(live.submit(&manual).status, OrderStatus::Pending);
        if restart_after_pending {
            live = restart(
                &mut live,
                &[],
                &full,
                runner.created_at - window,
                at(25),
                window,
            );
            assert_eq!(live.pending(), [manual.clone()]);
            // Repetirla después del reinicio no la duplica.
            assert_eq!(live.submit(&manual).status, OrderStatus::Pending);
            assert_eq!(live.pending().len(), 1);
        }
        for i in 25..28 {
            for id in ["US100", "DE40"] {
                live.push_candle(candle_of(&full, id, i)).unwrap();
            }
            live.settle_pending();
        }
        // Ya en el registro: la orden siguiente se decide sobre el estado con la pendiente.
        live.submit(&later);
        live
    };
    let mut straight = run(false);
    let mut restarted = run(true);
    assert_eq!(restarted.journal(), straight.journal());
    assert_eq!(straight.journal().len(), 3);
    assert_eq!(straight.journal()[1].1.status, OrderStatus::Filled);
    assert_eq!(
        straight.journal()[2].1.rejection,
        Some(OrderRejection::InstrumentAlreadyOpen)
    );
    let positions = straight.positions(at(28));
    assert_eq!(restarted.positions(at(28)), positions);
    assert_eq!(restarted.account(at(28)), straight.account(at(28)));

    // Y la reconstrucción desde el registro final (con todas las velas desde la primera orden).
    let rebuilt = between(full, runner.created_at - window, at(30));
    let (_, rec) = restore_and_reconcile(cfg, rebuilt, restarted.journal(), &positions, at(28));
    assert!(rec.is_clean(), "{rec:?}");
}

/// #479 con #441: un kill switch que se activa mientras la orden espera su vela la rechaza
/// igual con o sin reinicio en el medio (el reinicio lo vuelve a activar en su punto del
/// registro, antes de decidirla), y la reconstrucción lo repite.
#[test]
fn the_kill_switch_rejects_a_restored_pending_order_as_strictly() {
    let (cfg, full) = setup(dec!(100));
    let window = TimeDelta::minutes(15 * 15);
    let mut runner = order("runner", "DE40", dec!(98));
    runner.created_at = at(20);
    let mut manual = order("manual", "US100", dec!(98));
    manual.created_at = at(25) + TimeDelta::minutes(7);
    let reason = "error de datos en US100";

    let run = |restart_after_pending: bool| {
        let mut live = PaperBroker::new(cfg.clone(), between(full.clone(), at(0), at(25)));
        live.anchor_candles_before_first_order(window);
        live.submit(&runner);
        assert_eq!(live.submit(&manual).status, OrderStatus::Pending);
        live.activate_kill_switch(reason, at(25) + TimeDelta::minutes(8));
        let marks = vec![KillSwitchMark {
            after_orders: live.journal().len(),
            change: KillSwitchChange::Activated {
                reason: reason.into(),
                at: at(25) + TimeDelta::minutes(8),
            },
        }];
        if restart_after_pending {
            live = restart(
                &mut live,
                &marks,
                &full,
                runner.created_at - window,
                at(25),
                window,
            );
            assert!(live.kill_switch_active());
        }
        for i in 25..27 {
            for id in ["US100", "DE40"] {
                live.push_candle(candle_of(&full, id, i)).unwrap();
            }
            live.settle_pending();
        }
        (live, marks)
    };
    let (straight, _) = run(false);
    let (mut restarted, marks) = run(true);
    assert_eq!(restarted.journal(), straight.journal());
    let last = &restarted.journal()[1];
    assert_eq!(last.0.order_id.as_str(), "manual");
    assert_eq!(last.1.rejection, Some(OrderRejection::KillSwitchActive));

    let positions = restarted.positions(at(27));
    let rebuilt = between(full, runner.created_at - window, at(30));
    let (again, rec) = restore_with_kill_switch(
        cfg,
        rebuilt,
        restarted.journal(),
        &marks,
        &positions,
        at(27),
    );
    assert!(rec.is_clean(), "{rec:?}");
    assert!(again.kill_switch_active());
}

/// #479 con #438: sin órdenes en el registro, el gateway carga velas desde la pendiente
/// guardada más vieja y fija el recorte al arrancar. Si la que entra primero al registro es
/// otra (su vela del fill llegó antes), las velas se recortan antes de esa, como en la sesión
/// sin reinicio.
#[test]
fn restored_pending_orders_trim_the_candles_like_the_session_without_restart() {
    let (cfg, full) = setup(dec!(100));
    let window = TimeDelta::minutes(15 * 20);
    let mut first = order("first", "US100", dec!(98));
    first.created_at = at(24) + TimeDelta::minutes(7);
    let mut second = order("second", "DE40", dec!(98));
    second.created_at = at(25) + TimeDelta::minutes(3);

    let run = |restart_after_pending: bool| {
        // US100 sin la vela 25 todavía (llega tarde); DE40 hasta la 24.
        let mut live = PaperBroker::new(cfg.clone(), between(full.clone(), at(0), at(25)));
        live.anchor_candles_before_first_order(window);
        assert_eq!(live.submit(&first).status, OrderStatus::Pending);
        live.push_candle(candle_of(&full, "DE40", 25)).unwrap();
        assert_eq!(live.submit(&second).status, OrderStatus::Pending);
        if restart_after_pending {
            // Registro vacío: velas desde la pendiente más vieja menos la ventana.
            let (cfg, _) = setup(dec!(100));
            let mut markets = between(full.clone(), first.created_at - window, at(25));
            markets
                .get_mut(&"DE40".parse::<InstrumentId>().unwrap())
                .unwrap()
                .candles
                .push(candle_of(&full, "DE40", 25));
            let (mut restored, rec) = restore_with_kill_switch(cfg, markets, &[], &[], &[], at(26));
            assert!(rec.is_clean(), "{rec:?}");
            restored.anchor_candles_before_first_order(window);
            for o in live.pending() {
                restored.restore_pending(o.clone()).unwrap();
            }
            live = restored;
        }
        // Llega primero la vela del fill de `second` (DE40, 26) y después la de `first`.
        live.push_candle(candle_of(&full, "DE40", 26)).unwrap();
        let settled = live.settle_pending();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].0.order_id.as_str(), "second");
        // La primera vela que empieza desde `second` menos la ventana (at(5) + 3 min), no
        // desde `first` (at(4) + 7 min), aunque esa llegó antes.
        for m in live.markets().values() {
            assert_eq!(m.candles[0].start, at(6));
        }
        live.push_candle(candle_of(&full, "US100", 25)).unwrap();
        live.push_candle(candle_of(&full, "US100", 26)).unwrap();
        assert_eq!(live.settle_pending().len(), 1);
        live
    };
    let mut straight = run(false);
    let mut restarted = run(true);
    assert_eq!(restarted.journal(), straight.journal());
    let positions = straight.positions(at(27));
    assert_eq!(positions.len(), 2, "{:?}", straight.journal());
    assert_eq!(restarted.positions(at(27)), positions);
    let rebuilt = between(full, second.created_at - window, at(30));
    let (_, rec) = restore_and_reconcile(cfg, rebuilt, restarted.journal(), &positions, at(27));
    assert!(rec.is_clean(), "{rec:?}");
}

/// `wide_then_narrow` corrida 20 velas: anchas de at(20) a at(40), angostas hasta at(100).
fn shifted(id: &str, from: i64, to: i64) -> Vec<Candle> {
    wide_then_narrow(id)
        .into_iter()
        .map(|mut c| {
            c.start += TimeDelta::minutes(15 * 20);
            c
        })
        .filter(|c| c.start >= at(from) && c.start < at(to))
        .collect()
}

fn shifted_markets(from: i64) -> BTreeMap<InstrumentId, MarketSetup> {
    let (_, mut markets) = setup(dec!(100));
    for (id, m) in &mut markets {
        m.candles = shifted(id.as_str(), from, 101);
    }
    markets
}

/// #481: con el registro vacío, el gateway carga las velas desde `now` menos la ventana. La
/// primera orden del runner tiene `created_at` = cierre de la vela de la decisión, antes de
/// `now`: sin las velas de `[created_at − ventana, now − ventana)` su ATR sale distinto del de la
/// reconstrucción, que las carga desde la orden. Con `prepend_candles` (lo que hace el gateway
/// antes de mandarla) la sesión tiene exactamente las mismas velas y decide igual.
#[test]
fn the_first_order_after_a_start_gets_the_same_candles_as_its_rebuild() {
    let (cfg, _) = setup(dec!(100));
    let window = TimeDelta::minutes(15 * 80);
    // La decisión se manda con `created_at` = at(100); el gateway arrancó 40 velas después
    // (datos atrasados) y cargó desde at(140) − ventana = at(60). La reconstrucción carga desde
    // at(100) − ventana = at(20), con las 20 velas anchas.
    let mut o = order("a", "DE40", dec!(93.6));
    o.created_at = at(100);
    let loaded_from = 60;

    // Sin completar: el ATR sale solo de velas angostas y el stop a 6,4 es `stop_too_far`; en
    // la reconstrucción pasa.
    let mut loose = PaperBroker::new(cfg.clone(), shifted_markets(loaded_from));
    loose.anchor_candles_before_first_order(window);
    let r = loose.submit(&o);
    assert_eq!(r.rejection, Some(OrderRejection::StopTooFar));
    let (_, rec) = restore_and_reconcile(
        cfg.clone(),
        shifted_markets(20),
        loose.journal(),
        &[],
        at(101),
    );
    assert_eq!(rec.drifts.len(), 1, "{rec:?}");

    // Completando antes de mandarla (como `cover_first_order` del gateway): igual que la
    // reconstrucción.
    let mut covered = PaperBroker::new(cfg.clone(), shifted_markets(loaded_from));
    covered.anchor_candles_before_first_order(window);
    for id in ["US100", "DE40"] {
        covered
            .prepend_candles(&id.parse().unwrap(), shifted(id, 20, loaded_from))
            .unwrap();
    }
    let r = covered.submit(&o);
    assert_eq!(r.status, OrderStatus::Filled, "{r:?}");
    for m in covered.markets().values() {
        assert_eq!(m.candles[0].start, at(20));
    }
    let positions = covered.positions(at(101));
    let (_, rec) = restore_and_reconcile(
        cfg,
        shifted_markets(20),
        covered.journal(),
        &positions,
        at(101),
    );
    assert!(rec.is_clean(), "{rec:?}");
}

/// #481: `prepend_candles` solo completa hacia atrás y solo con el registro vacío.
#[test]
fn prepending_candles_is_only_for_an_empty_journal_and_older_candles() {
    let (cfg, _) = setup(dec!(100));
    let de40: InstrumentId = "DE40".parse().unwrap();
    let all = wide_then_narrow("DE40");
    let mut b = PaperBroker::new(cfg, markets_from(40));
    // Que se pisan con las que ya están, desordenadas o de otro instrumento: error, sin cambios.
    assert!(b.prepend_candles(&de40, all[30..41].to_vec()).is_err());
    let mut swapped = all[10..12].to_vec();
    swapped.reverse();
    assert!(b.prepend_candles(&de40, swapped).is_err());
    assert!(
        b.prepend_candles(&de40, wide_then_narrow("US100")[10..12].to_vec())
            .is_err()
    );
    assert!(
        b.prepend_candles(&"XX".parse().unwrap(), all[10..12].to_vec())
            .is_err()
    );
    assert_eq!(b.markets()[&de40].candles[0].start, at(40));
    b.prepend_candles(&de40, all[30..40].to_vec()).unwrap();
    assert_eq!(b.markets()[&de40].candles[0].start, at(30));
    // Con una orden en el registro, nunca.
    let mut o = order("a", "DE40", dec!(98));
    o.created_at = at(80);
    b.submit(&o);
    assert!(!b.journal().is_empty());
    assert!(b.prepend_candles(&de40, all[20..30].to_vec()).is_err());
}

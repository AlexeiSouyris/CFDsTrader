//! Puntos de control del broker simulado (#463): reconstruir desde un punto de control da
//! exactamente lo mismo que desde la primera orden (informes, posiciones, cuenta, operaciones y
//! kill switch), el punto se verifica (cadena de hashes, registro y config) y el kill switch
//! sigue igual de estricto. Precios sintéticos, deterministas.

use std::collections::BTreeMap;

use cfds_backtest::{AccountFx, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerCheckpoint, BrokerConfig, FundingSetup, KillSwitchChange, KillSwitchMark,
    MarketSetup, PaperBroker, SealedCheckpoint, restore_from_checkpoint, restore_with_kill_switch,
    seal, verified_chain,
};
use cfds_core::{
    Candle, FxRate, InstrumentId, KillSwitch, Order, OrderSide, OrderStatus, PaperMode, Timeframe,
};
use cfds_indicators::wilder_atr;
use cfds_risk::{InstrumentLimits, RiskConfig};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("fixtures/broker.toml");

/// Lunes 16-11-2026 00:00 UTC.
fn t0() -> DateTime<Utc> {
    "2026-11-16T00:00:00Z".parse().unwrap()
}

fn at(i: i64) -> DateTime<Utc> {
    t0() + TimeDelta::minutes(15 * i)
}

const DAYS: i64 = 6;
const BARS: i64 = 96 * DAYS;

/// Velas de 15m, cinco días seguidos, con un paseo determinista (sin `float`).
fn walk(id: &str, start: Decimal, seed: u64) -> Vec<Candle> {
    let mut x = seed;
    let mut close = start;
    (0..BARS)
        .map(|i| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let step = Decimal::from((x >> 33) % 41) / dec!(10) - dec!(2);
            let open = close;
            close = (open + step).max(dec!(10));
            let wick = Decimal::from((x >> 20) % 7) / dec!(10);
            Candle {
                instrument_id: id.parse().unwrap(),
                timeframe: Timeframe::M15,
                start: at(i),
                open,
                high: open.max(close) + wick,
                low: open.min(close) - wick,
                close,
                volume: None,
            }
        })
        .collect()
}

fn market(candles: Vec<Candle>, fx: AccountFx) -> MarketSetup {
    MarketSetup {
        limits: InstrumentLimits {
            max_leverage: dec!(20),
            margin_pct: dec!(5),
            min_deal_size: Some(dec!(0.01)),
        },
        costs: InstrumentCosts {
            spread: dec!(0.4),
            tick: dec!(0.1),
            commission: cfds_backtest::Commission::None,
            funding_admin_fee_pct: dec!(2.5),
            funding_day_count: dec!(365),
        },
        account_to_instrument: fx,
        candles,
        funding: FundingSetup {
            cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            tz: chrono_tz::Europe::Madrid,
            rates: FundingRates::Constant(dec!(2)),
        },
    }
}

fn eurusd() -> AccountFx {
    let rates = (0..=DAYS)
        .map(|d| {
            let published = t0() - TimeDelta::days(1) + TimeDelta::days(d);
            FxRate {
                pair: "EURUSD".into(),
                ref_date: published.date_naive(),
                published_at: published,
                rate: dec!(1.10) + Decimal::from(d) / dec!(100),
            }
        })
        .collect();
    AccountFx::Daily(cfds_backtest::FxRates::new(rates).unwrap())
}

fn config(edit: impl FnOnce(&mut RiskConfig)) -> BrokerConfig {
    let mut risk = RiskConfig::from_toml(RISK).unwrap();
    edit(&mut risk);
    let costs = CostConfig {
        slippage_spread_fraction: dec!(0.1),
        slippage_ticks: dec!(1),
    };
    BrokerConfig::from_toml(
        BROKER,
        risk,
        costs,
        ExitConfig::from_toml(BACKTEST).unwrap(),
        chrono_tz::Europe::Madrid,
    )
    .unwrap()
}

fn markets() -> BTreeMap<InstrumentId, MarketSetup> {
    [
        (
            "US100",
            market(walk("US100", dec!(200), 7), AccountFx::Same),
        ),
        ("DE40", market(walk("DE40", dec!(150), 11), AccountFx::Same)),
        ("XAUUSD", market(walk("XAUUSD", dec!(120), 23), eurusd())),
    ]
    .into_iter()
    .map(|(id, m)| (id.parse().unwrap(), m))
    .collect()
}

/// Una orden en el cierre de la vela `i - 1`, con el stop a 1,5 ATR del cierre.
fn order(n: usize, id: &str, i: i64) -> Order {
    let m = markets();
    let candles = &m[&id.parse::<InstrumentId>().unwrap()].candles;
    let closed = &candles[..usize::try_from(i).unwrap()];
    let atr = wilder_atr(closed, 14).last().copied().flatten().unwrap();
    let close = closed.last().unwrap().close;
    let side = if n.is_multiple_of(2) {
        OrderSide::Long
    } else {
        OrderSide::Short
    };
    let stop = match side {
        OrderSide::Long => close - atr * dec!(1.5),
        OrderSide::Short => close + atr * dec!(1.5),
    };
    Order {
        order_id: format!("o{n}").parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: id.parse().unwrap(),
        side,
        stop: stop.round_dp(1),
        tp1: None,
        proposal_id: None,
        created_at: at(i),
    }
}

/// Órdenes repartidas en los cinco días, en los tres instrumentos.
fn orders() -> Vec<Order> {
    let ids = ["US100", "DE40", "XAUUSD"];
    (0..22)
        .map(|n| {
            let i = 30 + i64::try_from(n).unwrap() * 25;
            order(n, ids[n % 3], i)
        })
        .collect()
}

/// El punto de control: el martes 18 a las 13:00 (después de la orden 9).
fn cut() -> DateTime<Utc> {
    at(2 * 96 + 52)
}

fn split(all: &[Order]) -> (Vec<Order>, Vec<Order>) {
    let (a, b): (Vec<_>, Vec<_>) = all.iter().cloned().partition(|o| o.created_at <= cut());
    (a, b)
}

/// Todo lo que se ve de un broker en `t`.
fn view(b: &mut PaperBroker, t: DateTime<Utc>) -> String {
    format!(
        "{:?}\n{:?}\n{:?}\n{}",
        b.positions(t),
        b.account(t),
        b.trade_outcomes(t),
        b.kill_switch_active()
    )
}

fn checkpointed(full: &PaperBroker) -> BrokerCheckpoint {
    full.plan_checkpoint(cut(), 0)
        .expect("hay algo para sellar")
}

#[test]
fn rebuilding_from_a_checkpoint_gives_exactly_the_same_broker() {
    let (before, after) = split(&orders());
    assert!(before.len() >= 6 && after.len() >= 6);
    let mut full = PaperBroker::new(config(|_| {}), markets());
    for o in &before {
        full.submit(o);
    }
    let ck = checkpointed(&full);
    // El punto sirve: sella operaciones cerradas, suelta velas y sigue con el ATR guardado.
    let sealed_ids: Vec<_> = full
        .trade_outcomes(cut())
        .into_iter()
        .filter(|t| t.exits.iter().all(|e| !e.end_of_data))
        .collect();
    assert!(!sealed_ids.is_empty());
    assert!(
        ck.cuts.values().all(|c| c.atr_seed.is_some()),
        "{:?}",
        ck.cuts
    );
    assert!(ck.cuts.values().all(|c| c.first_start.unwrap() > at(96)));

    let mut from_ck =
        PaperBroker::from_checkpoint(config(|_| {}), markets(), &ck, full.journal()).unwrap();
    let mut applied = full.clone();
    applied.apply_checkpoint(&ck).unwrap();
    for b in [&from_ck, &applied] {
        let kept: usize = b.markets().values().map(|m| m.candles.len()).sum();
        assert!(kept + 400 < 3 * usize::try_from(BARS).unwrap(), "{kept}");
    }
    assert_eq!(view(&mut from_ck, cut()), view(&mut full.clone(), cut()));

    // Las órdenes siguientes dan los mismos informes, y todo se ve igual en cada momento.
    for o in &after {
        let r = full.submit(o);
        assert_eq!(from_ck.submit(o), r);
        assert_eq!(applied.submit(o), r);
        let t = o.created_at + TimeDelta::minutes(20);
        let v = view(&mut full, t);
        assert_eq!(view(&mut from_ck, t), v);
        assert_eq!(view(&mut applied, t), v);
    }
    let filled = full
        .journal()
        .iter()
        .filter(|(_, r)| r.status == OrderStatus::Filled)
        .count();
    assert!(filled >= 10, "{filled}");
    for b in [&mut from_ck, &mut applied] {
        for d in [0, 7, 40, 200] {
            let t = at(BARS - 1) + TimeDelta::minutes(d);
            assert_eq!(view(b, t), view(&mut full, t));
        }
        assert_eq!(b.journal(), full.journal());
        // Repetir una orden ya sellada devuelve su informe guardado (idempotencia).
        assert_eq!(b.submit(&before[0]), full.journal()[0].1);
    }

    // Un segundo punto, sobre el primero: el mismo que desde el broker completo, y de nuevo
    // idéntico al reconstruir.
    let end = at(BARS - 1);
    let second = from_ck.plan_checkpoint(end, 0).unwrap();
    assert_eq!(full.plan_checkpoint(end, 0).unwrap(), second);
    assert_eq!(applied.plan_checkpoint(end, 0).unwrap(), second);
    assert!(second.safe_from > ck.safe_from);
    let mut again =
        PaperBroker::from_checkpoint(config(|_| {}), markets(), &second, full.journal()).unwrap();
    for d in [15, 300] {
        let t = end + TimeDelta::minutes(d);
        assert_eq!(view(&mut again, t), view(&mut full, t));
    }
}

#[test]
fn restore_from_a_checkpoint_equals_the_full_restore_with_the_kill_switch_marks() {
    // Un kill switch guardado antes del punto, liberado por votación, y otro después.
    let ks = |i: i64| KillSwitchChange::Activated {
        reason: "prueba".into(),
        at: at(i),
    };
    let marks: Vec<KillSwitchMark> = [(3, ks(105)), (5, KillSwitchChange::Released), (15, ks(400))]
        .into_iter()
        .map(|(after_orders, change)| KillSwitchMark {
            after_orders,
            change,
        })
        .collect();
    let all = orders();
    // El registro que se habría guardado: la reconstrucción completa (es determinista; los
    // informes de entrada solo sirven para medir drift).
    let mut plain = PaperBroker::new(config(|_| {}), markets());
    for o in &all {
        plain.submit(o);
    }
    let (full, _) = restore_with_kill_switch(
        config(|_| {}),
        markets(),
        plain.journal(),
        &marks,
        &[],
        at(BARS),
    );
    let journal = full.journal().to_vec();
    assert_ne!(
        journal,
        plain.journal(),
        "los cambios del kill switch cambian informes"
    );
    let now = at(BARS);
    let mut full = full;
    let positions = full.positions(now);

    // El punto de control, tomado de la reconstrucción hasta el corte (con sus cambios).
    let n = split(&all).0.len();
    let applied_marks = marks.iter().filter(|m| m.after_orders <= n).count();
    let rebuild_until_cut = |positions: &[cfds_core::Position]| {
        restore_with_kill_switch(
            config(|_| {}),
            markets(),
            &journal[..n],
            &marks[..applied_marks],
            positions,
            cut(),
        )
    };
    let (mut probe, _) = rebuild_until_cut(&[]);
    let (part, rec) = rebuild_until_cut(&probe.positions(cut()));
    assert!(rec.is_clean(), "{rec:?}");
    let ck = part.plan_checkpoint(cut(), applied_marks).unwrap();
    let (mut rebuilt, rec) = restore_from_checkpoint(
        config(|_| {}),
        markets(),
        &ck,
        &journal,
        &marks,
        &positions,
        now,
    )
    .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    assert_eq!(rebuilt.journal(), full.journal());
    assert_eq!(view(&mut rebuilt, now), view(&mut full, now));
    assert!(rebuilt.kill_switch_active());
}

#[test]
fn a_checkpoint_is_verifiable_and_refuses_another_journal_or_config() {
    let (before, _) = split(&orders());
    let mut full = PaperBroker::new(config(|_| {}), markets());
    for o in &before {
        full.submit(o);
    }
    let ck = checkpointed(&full);
    let first = seal(&ck, None).unwrap();
    let second = seal(&ck, Some(&first.hash)).unwrap();
    let rows = vec![first.clone(), second.clone()];
    assert_eq!(verified_chain(&rows).len(), 2);
    assert_eq!(verified_chain(&rows)[1].0, ck);

    // Texto cambiado: el hash no verifica y desde ahí no se usa nada.
    let tampered = SealedCheckpoint {
        state: first
            .state
            .replace("\"peak_equity\":\"", "\"peak_equity\":\"9"),
        ..first.clone()
    };
    assert!(verified_chain(&[tampered, second.clone()]).is_empty());
    // Cadena rota (falta el primero).
    assert!(verified_chain(&[second]).is_empty());

    // Otro registro o otra config: no se usa (se reconstruye completo).
    let mut other = full.journal().to_vec();
    other[0].1.size = Some(dec!(999));
    assert!(PaperBroker::from_checkpoint(config(|_| {}), markets(), &ck, &other).is_err());
    let other_cfg = config(|r| r.max_drawdown_pct = dec!(14));
    assert!(PaperBroker::from_checkpoint(other_cfg, markets(), &ck, full.journal()).is_err());
    // Sin la primera vela del corte: tampoco.
    let mut late = markets();
    for m in late.values_mut() {
        m.candles.retain(|c| c.start > at(BARS - 10));
    }
    assert!(PaperBroker::from_checkpoint(config(|_| {}), late, &ck, full.journal()).is_err());
}

#[test]
fn the_kill_switch_stays_as_strict_after_a_checkpoint() {
    let (before, _) = split(&orders());
    let mut full = PaperBroker::new(config(|_| {}), markets());
    for o in &before {
        full.submit(o);
    }
    let ck = checkpointed(&full);
    assert!(
        !full.kill_switch_active(),
        "el punto se toma sin kill switch"
    );
    let mut from_ck =
        PaperBroker::from_checkpoint(config(|_| {}), markets(), &ck, full.journal()).unwrap();
    // Si falta el tipo de cambio del oro, valuar lo sellado también activa el kill switch,
    // como la valuación completa.
    let gold: InstrumentId = "XAUUSD".parse().unwrap();
    let missing = AccountFx::Missing {
        pair: "EURUSD".into(),
    };
    for b in [&mut full, &mut from_ck] {
        b.update_account_fx(&gold, missing.clone()).unwrap();
        let acc = b.account(cut() + TimeDelta::hours(1));
        assert!(
            matches!(acc.kill_switch, KillSwitch::Active { .. }),
            "{acc:?}"
        );
    }

    // Valuar antes de lo sellado no se adivina: kill switch.
    let mut early =
        PaperBroker::from_checkpoint(config(|_| {}), markets(), &ck, full.journal()).unwrap();
    let acc = early.account(at(100));
    assert!(
        matches!(acc.kill_switch, KillSwitch::Active { .. }),
        "{acc:?}"
    );
}

#[test]
fn nothing_to_seal_gives_no_checkpoint() {
    // Sin órdenes no hay punto: las velas todavía se anclan a la primera orden (#438).
    let mut b = PaperBroker::new(config(|_| {}), markets());
    assert!(b.plan_checkpoint(cut(), 0).is_none());
    b.submit(&orders()[0]);
    assert!(b.plan_checkpoint(cut(), 0).is_some());
    // Sin velas cerradas en ningún instrumento: tampoco.
    assert!(b.plan_checkpoint(at(0), 0).is_none());
    // Un instrumento sin velas no frena a los demás: queda sin cortar.
    let mut m = markets();
    m.get_mut(&"DE40".parse::<InstrumentId>().unwrap())
        .unwrap()
        .candles
        .clear();
    let mut b = PaperBroker::new(config(|_| {}), m.clone());
    b.submit(&orders()[0]);
    let ck = b.plan_checkpoint(cut(), 0).unwrap();
    assert_eq!(
        ck.cuts[&"DE40".parse::<InstrumentId>().unwrap()].first_start,
        None
    );
    let mut restored = PaperBroker::from_checkpoint(config(|_| {}), m, &ck, b.journal()).unwrap();
    assert_eq!(view(&mut restored, cut()), view(&mut b.clone(), cut()));
}

// ---------- Con PostgreSQL real (migración 0043) ----------

const DB: &str = "cfds_test_broker_ckpt";

/// Crea `cfds_test_broker_ckpt` desde cero, con las migraciones hechas. Devuelve una sesión
/// normal y una de solo lectura como la del gateway (que escribe solo lo que declara).
async fn fresh_store() -> (cfds_storage::PgStore, cfds_storage::PgStore) {
    use cfds_storage::PgStore;
    let url = cfds_storage::test_db::test_database_url();
    let admin = PgStore::connect(&url).await.unwrap();
    sqlx::query("DROP DATABASE IF EXISTS cfds_test_broker_ckpt WITH (FORCE)")
        .execute(admin.pool())
        .await
        .unwrap();
    sqlx::query("CREATE DATABASE cfds_test_broker_ckpt")
        .execute(admin.pool())
        .await
        .unwrap();
    let url = cfds_storage::test_db::with_database(&url, DB);
    let store = PgStore::connect(&url).await.unwrap();
    store.migrate().await.unwrap();
    let sep = if url.contains('?') { '&' } else { '?' };
    let ro = PgStore::connect(&format!(
        "{url}{sep}options=-c%20default_transaction_read_only%3Don"
    ))
    .await
    .unwrap();
    (store, ro)
}

/// Un solo test, en orden: la base propia se crea una vez y nada corre en paralelo sobre ella.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL con permiso para crear bases"]
async fn checkpoints_are_saved_chained_and_used_to_restart() {
    use cfds_broker::{
        ReconcileSchedule, reconcile_periodically, restore_from_store, stored_checkpoints,
        submit_and_record,
    };
    let (rw, store) = fresh_store().await;
    let (before, after) = split(&orders());
    let mut running = PaperBroker::new(config(|_| {}), markets());
    let mut full = PaperBroker::new(config(|_| {}), markets());
    for o in &before {
        submit_and_record(&mut running, &store, o).await.unwrap();
        full.submit(o);
    }

    // Reconciliación periódica limpia → se guarda un punto y el broker en marcha se corta.
    let mut schedule = ReconcileSchedule::default();
    let markets_now = running.markets().clone();
    let rec = reconcile_periodically(&store, &mut running, markets_now, &mut schedule, cut())
        .await
        .unwrap()
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    let chain = stored_checkpoints(&store).await.unwrap();
    assert_eq!(chain.len(), 1);
    let kept: usize = running.markets().values().map(|m| m.candles.len()).sum();
    assert!(kept + 400 < 3 * usize::try_from(BARS).unwrap(), "{kept}");

    for o in &after {
        let r = submit_and_record(&mut running, &store, o).await.unwrap();
        assert_eq!(full.submit(o), r);
    }
    let now = at(BARS);
    assert_eq!(view(&mut running, now), view(&mut full, now));

    // La reconciliación siguiente parte del punto guardado (velas del broker en marcha, ya
    // cortadas), da limpia y encadena otro punto.
    let mut schedule = ReconcileSchedule::default();
    let markets_now = running.markets().clone();
    let rec = reconcile_periodically(&store, &mut running, markets_now, &mut schedule, now)
        .await
        .unwrap()
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    assert_eq!(stored_checkpoints(&store).await.unwrap().len(), 2);
    assert_eq!(view(&mut running, now), view(&mut full, now));

    // Reinicio: desde el último punto, igual que el broker completo.
    let positions = full.positions(now);
    let (mut restored, rec) =
        restore_from_store(&store, config(|_| {}), markets(), &positions, now)
            .await
            .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    assert_eq!(view(&mut restored, now), view(&mut full, now));

    // Solo agregar, y un punto que no verifica no se usa (se sigue con el anterior).
    for sql in [
        "UPDATE paper_broker_checkpoints SET state = '{}'",
        "DELETE FROM paper_broker_checkpoints",
    ] {
        assert!(sqlx::query(sql).execute(rw.pool()).await.is_err(), "{sql}");
    }
    let last = stored_checkpoints(&store).await.unwrap().pop().unwrap().1;
    sqlx::query(
        "INSERT INTO paper_broker_checkpoints (after_orders, state, prev_hash, hash)
         VALUES (1, '{}', $1, $2)",
    )
    .bind(&last)
    .bind("0".repeat(64))
    .execute(rw.pool())
    .await
    .unwrap();
    assert_eq!(stored_checkpoints(&store).await.unwrap().len(), 2);
    let (mut restored, rec) =
        restore_from_store(&store, config(|_| {}), markets(), &positions, now)
            .await
            .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    assert_eq!(view(&mut restored, now), view(&mut full, now));
}

/// #691: el máximo del capital (valuado en cada vela cerrada) se guarda en el punto de control y
/// sigue igual al reiniciar. Con el límite de caída al 5 % (puntos de la caída gradual a la misma
/// proporción), el kill switch se activa recién después del corte; el broker que arranca del
/// punto guardado (JSON sellado y leído de nuevo) decide igual cada orden y lo activa en la
/// misma vela.
#[test]
fn the_peak_survives_a_restart_from_a_checkpoint() {
    let max = dec!(5);
    let edit = |r: &mut RiskConfig| {
        for p in &mut r.drawdown_scaling.points {
            p.drawdown_pct = p.drawdown_pct * max / dec!(15);
        }
        r.max_drawdown_pct = max;
    };
    let (before, after) = split(&orders());
    let mut full = PaperBroker::new(config(edit), markets());
    for o in &before {
        full.submit(o);
    }
    assert!(!full.kill_switch_active());
    let ck = checkpointed(&full);
    let row = seal(&ck, None).unwrap();
    let [(stored, _)] = verified_chain(std::slice::from_ref(&row))
        .try_into()
        .expect("el punto verifica");
    let mut from_ck =
        PaperBroker::from_checkpoint(config(edit), markets(), &stored, full.journal()).unwrap();
    let mut applied = full.clone();
    applied.apply_checkpoint(&stored).unwrap();
    for o in &after {
        let r = full.submit(o);
        assert_eq!(from_ck.submit(o), r);
        assert_eq!(applied.submit(o), r);
    }
    let since = full
        .kill_switch()
        .map(|(_, t)| t)
        .expect("el kill switch se activa");
    assert!(since > cut(), "{since}");
    for b in [&from_ck, &applied] {
        assert_eq!(b.kill_switch().map(|(_, t)| t), Some(since));
        assert_eq!(b.journal(), full.journal());
    }

    // Un punto de la versión anterior (máximo solo de las órdenes evaluadas) no se usa: se
    // reconstruye completo.
    let mut old = ck;
    old.version = 1;
    assert!(verified_chain(&[seal(&old, None).unwrap()]).is_empty());
}

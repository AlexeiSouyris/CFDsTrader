//! El kill switch del broker simulado guardado en PostgreSQL **real** (#441), en una base
//! propia, `cfds_test_broker_kill_kill`, que el test crea desde cero en cada corrida (la tabla es de
//! solo agregar). Necesita `DATABASE_URL` con permiso para crear bases; está marcado `#[ignore]`
//! y el CI lo corre con `-- --ignored`. Precios sintéticos.

use std::collections::BTreeMap;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, MarketSetup, PaperBroker, ReconcileSchedule,
    kill_switch_marks, kill_switch_stored_active, reconcile_periodically, restore_from_store,
    save_kill_switch, submit_and_record,
};
use cfds_core::{
    Candle, InstrumentId, KillSwitch, Order, OrderRejection, OrderSide, PaperMode, Timeframe,
};
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_storage::PgStore;
use cfds_storage::test_db::with_database;
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("../../../../config/broker.toml");
const DB: &str = "cfds_test_broker_kill";

/// Crea `cfds_test_broker_kill` desde cero y devuelve la conexión con las migraciones hechas.
async fn fresh_store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let admin = PgStore::connect(&url).await.unwrap();
    sqlx::query("DROP DATABASE IF EXISTS cfds_test_broker_kill WITH (FORCE)")
        .execute(admin.pool())
        .await
        .unwrap();
    sqlx::query("CREATE DATABASE cfds_test_broker_kill")
        .execute(admin.pool())
        .await
        .unwrap();
    let store = PgStore::connect(&with_database(&url, DB)).await.unwrap();
    store.migrate().await.unwrap();
    store
}

fn at(i: i64) -> DateTime<Utc> {
    "2026-11-18T09:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(15 * i)
}

fn markets() -> BTreeMap<InstrumentId, MarketSetup> {
    ["US100", "DE40"]
        .into_iter()
        .map(|id| {
            let candles = (0..40)
                .map(|i| Candle {
                    instrument_id: id.parse().unwrap(),
                    timeframe: Timeframe::M15,
                    start: at(i),
                    open: dec!(100),
                    high: dec!(101),
                    low: dec!(99),
                    close: dec!(100),
                    volume: None,
                })
                .collect();
            let setup = MarketSetup {
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
                candles,
                funding: FundingSetup {
                    cutoff: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
                    tz: chrono_tz::Europe::Madrid,
                    rates: FundingRates::Constant(dec!(2)),
                },
            };
            (id.parse().unwrap(), setup)
        })
        .collect()
}

fn config() -> BrokerConfig {
    BrokerConfig::from_toml(
        BROKER,
        RiskConfig::from_toml(RISK).unwrap(),
        CostConfig {
            slippage_spread_fraction: dec!(0.1),
            slippage_ticks: dec!(1),
        },
        ExitConfig::from_toml(BACKTEST).unwrap(),
        chrono_tz::Europe::Madrid,
    )
    .unwrap()
}

fn order(id: &str, instrument: &str, stop: Decimal, i: i64) -> Order {
    Order {
        order_id: id.parse().unwrap(),
        mode: PaperMode::Paper,
        instrument_id: instrument.parse().unwrap(),
        side: OrderSide::Long,
        stop,
        tp1: None,
        proposal_id: None,
        created_at: at(i),
    }
}

/// Un solo test, en orden: la base propia se crea una vez y nada corre en paralelo sobre ella.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL con permiso para crear bases"]
async fn the_kill_switch_is_saved_and_survives_a_restart() {
    let store = fresh_store().await;
    let mut broker = PaperBroker::new(config(), markets());
    submit_and_record(&mut broker, &store, &order("a", "US100", dec!(98), 20))
        .await
        .unwrap();

    // Sin kill switch no se guarda nada.
    assert!(!save_kill_switch(&mut broker, &store, false).await.unwrap());
    assert!(kill_switch_marks(&store).await.unwrap().is_empty());

    // Se activa (por ejemplo, un hueco de datos): se guarda una sola vez.
    broker.activate_kill_switch("faltan velas de DE40", at(21));
    assert!(save_kill_switch(&mut broker, &store, false).await.unwrap());
    assert!(save_kill_switch(&mut broker, &store, true).await.unwrap());
    let marks = kill_switch_marks(&store).await.unwrap();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].after_orders, 1);
    assert!(kill_switch_stored_active(&marks));

    // Una orden con el kill switch activo sale rechazada y se guarda así.
    let rb = submit_and_record(&mut broker, &store, &order("b", "DE40", dec!(98), 22))
        .await
        .unwrap();
    assert_eq!(rb.rejection, Some(OrderRejection::KillSwitchActive));

    // Reinicio: vuelve activo, con el mismo motivo, y la reconstrucción da limpia.
    let positions = broker.positions(at(30));
    let (mut restored, rec) = restore_from_store(&store, config(), markets(), &positions, at(30))
        .await
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    assert!(matches!(
        restored.account(at(30)).kill_switch,
        KillSwitch::Active { ref reason, since } if reason == "faltan velas de DE40" && since == at(21)
    ));
    // La reconciliación periódica también lo tiene en cuenta: sin diferencias.
    let mut schedule = ReconcileSchedule::default();
    let rec = reconcile_periodically(&store, &mut broker, markets(), &mut schedule, at(30))
        .await
        .unwrap()
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");

    // La tabla es de solo agregar.
    for sql in [
        "UPDATE paper_kill_switch SET reason = 'otra'",
        "DELETE FROM paper_kill_switch",
    ] {
        assert!(
            sqlx::query(sql).execute(store.pool()).await.is_err(),
            "{sql}"
        );
    }

    // Solo una liberación guardada (votación §1.2, la escribe una persona) lo apaga al reiniciar.
    sqlx::query(
        "INSERT INTO paper_kill_switch (change, after_orders, reason, at)
         VALUES ('released', 2, 'votación §1.2: 4 de 6 y los 2 responsables', $1)",
    )
    .bind(at(31))
    .execute(store.pool())
    .await
    .unwrap();
    let (mut restored, rec) = restore_from_store(&store, config(), markets(), &positions, at(31))
        .await
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    assert!(!restored.kill_switch_active());
    assert_eq!(restored.account(at(31)).kill_switch, KillSwitch::Inactive);
    assert!(!kill_switch_stored_active(
        &kill_switch_marks(&store).await.unwrap()
    ));
}

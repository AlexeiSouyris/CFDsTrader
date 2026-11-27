//! Registro de órdenes del broker simulado en PostgreSQL **real** (#278), en una base propia,
//! `cfds_test_broker`, que el test crea desde cero en cada corrida (las tablas son de solo
//! agregar). Necesita `DATABASE_URL` con permiso para crear bases; está marcado `#[ignore]` y el
//! CI lo corre con `-- --ignored`. Precios sintéticos.

use std::collections::BTreeMap;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, MarketSetup, Mismatch, PaperBroker, ReconcileSchedule,
    reconcile_periodically, restore_from_store, submit_and_record,
};
use cfds_core::{
    Candle, InstrumentId, KillSwitch, Order, OrderSide, OrderStatus, PaperMode, Timeframe,
};
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_storage::test_db::with_database;
use cfds_storage::{PgStore, StorageError};
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("fixtures/broker.toml");
const DB: &str = "cfds_test_broker";

/// Crea `cfds_test_broker` desde cero y devuelve la conexión con las migraciones hechas.
async fn fresh_store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let admin = PgStore::connect(&url).await.unwrap();
    sqlx::query("DROP DATABASE IF EXISTS cfds_test_broker WITH (FORCE)")
        .execute(admin.pool())
        .await
        .unwrap();
    sqlx::query("CREATE DATABASE cfds_test_broker")
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
async fn the_order_journal_lives_in_postgres_and_rebuilds_the_broker() {
    let store = fresh_store().await;
    let mut broker = PaperBroker::new(config(), markets());

    // Enviar y guardar: dos que se ejecutan y una que el riesgo rechaza.
    let a = order("a", "US100", dec!(98), 20);
    let ra = submit_and_record(&mut broker, &store, &a).await.unwrap();
    assert_eq!(ra.status, OrderStatus::Filled);
    submit_and_record(&mut broker, &store, &order("b", "DE40", dec!(98), 21))
        .await
        .unwrap();
    let rc = submit_and_record(&mut broker, &store, &order("c", "DE40", dec!(97), 22))
        .await
        .unwrap();
    assert_eq!(rc.status, OrderStatus::Rejected);

    // Idempotente: reenviar la misma orden no guarda nada nuevo.
    submit_and_record(&mut broker, &store, &a).await.unwrap();
    assert!(!store.insert_paper_order(&a, &ra).await.unwrap());
    // Otro informe para el mismo order_id es repintado: error, no se pisa.
    let mut other = ra.clone();
    other.fill_price = Some(dec!(1));
    assert!(matches!(
        store.insert_paper_order(&a, &other).await,
        Err(StorageError::PaperOrderConflict { .. })
    ));

    // El registro guardado es exactamente el del broker, en orden de llegada.
    let journal = store.paper_journal().await.unwrap();
    assert_eq!(journal.as_slice(), broker.journal());

    // Arranque: se reconstruye desde la base y coincide con las posiciones del broker.
    let positions = broker.positions(at(30));
    assert_eq!(positions.len(), 2);
    let (mut restored, rec) = restore_from_store(&store, config(), markets(), &positions, at(30))
        .await
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    assert_eq!(restored.positions(at(30)), positions);

    // Reconciliación periódica: la primera vez toca y da limpio; enseguida no toca.
    let mut schedule = ReconcileSchedule::default();
    let first = reconcile_periodically(&store, &mut broker, markets(), &mut schedule, at(30))
        .await
        .unwrap();
    assert!(first.unwrap().is_clean());
    let soon = reconcile_periodically(
        &store,
        &mut broker,
        markets(),
        &mut schedule,
        at(30) + TimeDelta::minutes(5),
    )
    .await
    .unwrap();
    assert!(
        soon.is_none(),
        "every_minutes = 15: a los 15 minutos todavía no toca"
    );

    // Una orden que el broker procesó pero no se guardó: a los 15 minutos, kill switch.
    broker.submit(&order("unsaved", "US100", dec!(98), 32));
    let later = reconcile_periodically(&store, &mut broker, markets(), &mut schedule, at(34))
        .await
        .unwrap()
        .unwrap();
    assert!(
        later
            .mismatches
            .contains(&Mismatch::UnsavedOrder("unsaved".parse().unwrap()))
    );
    assert!(matches!(
        broker.account(at(34)).kill_switch,
        KillSwitch::Active { ref reason, .. } if reason.contains("Reconciliación")
    ));

    // Las tablas son de solo agregar y solo admiten modo paper.
    for sql in [
        "UPDATE paper_orders SET created_at = now()",
        "DELETE FROM paper_orders",
        "UPDATE paper_order_reports SET at = now()",
        "DELETE FROM paper_order_reports",
    ] {
        assert!(
            sqlx::query(sql).execute(store.pool()).await.is_err(),
            "{sql}"
        );
    }
    let live = sqlx::query(
        "INSERT INTO paper_orders (order_id, instrument_id, created_at, document)
         VALUES ('live', 'US100', now(), '{\"order_id\": \"live\", \"mode\": \"live\"}')",
    )
    .execute(store.pool())
    .await;
    assert!(live.is_err(), "una orden real no entra al registro");
}

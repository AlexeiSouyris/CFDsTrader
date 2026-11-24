//! Las órdenes pendientes del broker simulado guardadas en PostgreSQL **real** (#479), en una
//! base propia, `cfds_test_broker_pending`, que el test crea desde cero en cada corrida (las
//! tablas son de solo agregar). Necesita `DATABASE_URL` con permiso para crear bases; está
//! marcado `#[ignore]` y el CI lo corre con `-- --ignored`. Precios sintéticos.

use std::collections::BTreeMap;

use cfds_backtest::{Commission, CostConfig, ExitConfig, FundingRates, InstrumentCosts};
use cfds_broker::{
    Broker, BrokerConfig, FundingSetup, MarketSetup, PaperBroker, ReconcileSchedule, flush_journal,
    reconcile_periodically, restore_from_store, restore_pending_from_store, settle_and_record,
    submit_and_record,
};
use cfds_core::{Candle, InstrumentId, Order, OrderSide, OrderStatus, PaperMode, Timeframe};
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_storage::PgStore;
use cfds_storage::test_db::with_database;
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

const RISK: &str = include_str!("../../../../config/risk.toml");
const BACKTEST: &str = include_str!("../../../../config/backtest.toml");
const BROKER: &str = include_str!("fixtures/broker.toml");
const DB: &str = "cfds_test_broker_pending";

/// Crea `cfds_test_broker_pending` desde cero y devuelve la conexión con las migraciones hechas.
async fn fresh_store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let admin = PgStore::connect(&url).await.unwrap();
    sqlx::query("DROP DATABASE IF EXISTS cfds_test_broker_pending WITH (FORCE)")
        .execute(admin.pool())
        .await
        .unwrap();
    sqlx::query("CREATE DATABASE cfds_test_broker_pending")
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

/// Los dos mercados con las velas `0..n` (las cerradas hasta `at(n)`).
fn markets(n: i64) -> BTreeMap<InstrumentId, MarketSetup> {
    ["US100", "DE40"]
        .into_iter()
        .map(|id| {
            let candles = (0..n)
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

fn candle(markets: &BTreeMap<InstrumentId, MarketSetup>, id: &str, i: i64) -> Candle {
    markets[&id.parse::<InstrumentId>().unwrap()]
        .candles
        .iter()
        .find(|c| c.start == at(i))
        .unwrap()
        .clone()
}

/// Llegan las velas `25..27` de los dos instrumentos.
fn push_candles(b: &mut PaperBroker) {
    let all = markets(40);
    for i in 25..27 {
        for id in ["US100", "DE40"] {
            b.push_candle(candle(&all, id, i)).unwrap();
        }
    }
}

/// Un solo test, en orden: la base propia se crea una vez y nada corre en paralelo sobre ella.
#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL con permiso para crear bases"]
async fn a_pending_order_is_saved_survives_a_restart_and_settles_the_same() {
    let store = fresh_store().await;
    let runner = order("a", "US100", dec!(98), 20);
    let mut manual = order("manual", "DE40", dec!(98), 25);
    manual.created_at += TimeDelta::minutes(7);

    // En marcha, con las velas cerradas hasta at(25): la manual queda pendiente y guardada.
    let mut live = PaperBroker::new(config(), markets(25));
    submit_and_record(&mut live, &store, &runner).await.unwrap();
    let r = submit_and_record(&mut live, &store, &manual).await.unwrap();
    assert_eq!(r.status, OrderStatus::Pending);
    assert_eq!(store.paper_pending().await.unwrap(), [manual.clone()]);
    assert_eq!(store.paper_journal().await.unwrap().len(), 1);
    // Repetirla no la duplica.
    let again = submit_and_record(&mut live, &store, &manual).await.unwrap();
    assert_eq!(again, r);
    assert_eq!(store.paper_pending().await.unwrap().len(), 1);
    // Otro contenido para la misma pendiente, o una pendiente con el id de una decidida: error.
    let mut other = manual.clone();
    other.stop = dec!(97);
    assert!(store.insert_paper_pending(&other).await.is_err());
    assert!(store.insert_paper_pending(&runner).await.is_err());
    // La reconciliación periódica no la ve como diferencia (no está en el registro).
    let mut schedule = ReconcileSchedule::default();
    let rec = reconcile_periodically(&store, &mut live, markets(25), &mut schedule, at(25))
        .await
        .unwrap()
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");

    // La misma sesión, sin reinicio, para comparar.
    let mut straight = live.clone();
    push_candles(&mut straight);
    let settled = straight.settle_pending();
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].1.status, OrderStatus::Filled);

    // Parada: la pendiente está guardada, no se pierde.
    let flush = flush_journal(&live, &store).await.unwrap();
    assert!(flush.is_clean());
    assert_eq!(flush.pending_kept, ["manual"]);
    assert!(flush.pending_dropped.is_empty());

    // Una pendiente guardada de un instrumento que en el próximo arranque no se opera.
    let mut ghost = order("ghost", "US500", dec!(98), 25);
    ghost.created_at += TimeDelta::minutes(8);
    assert!(store.insert_paper_pending(&ghost).await.unwrap());

    // Reinicio antes de la vela del fill: se reconstruye y se retoma la pendiente.
    let now = at(25) + TimeDelta::minutes(10);
    let positions = live.positions(now);
    drop(live);
    let (mut restored, rec) = restore_from_store(&store, config(), markets(25), &positions, now)
        .await
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    let resumed = restore_pending_from_store(&mut restored, &store, now)
        .await
        .unwrap();
    assert_eq!(resumed.restored, ["manual"]);
    assert_eq!(resumed.dropped, ["ghost"]);
    assert_eq!(restored.pending(), [manual.clone()]);
    assert_eq!(store.paper_pending().await.unwrap(), [manual.clone()]);

    // Llega su vela: se decide igual que sin reinicio, entra al registro y queda `settled`.
    push_candles(&mut restored);
    let reports = settle_and_record(&mut restored, &store).await.unwrap();
    assert_eq!(reports, [settled[0].1.clone()]);
    assert_eq!(restored.journal(), straight.journal());
    assert_eq!(store.paper_journal().await.unwrap(), straight.journal());
    assert!(store.paper_pending().await.unwrap().is_empty());
    let outcomes = store.paper_pending_outcomes().await.unwrap();
    assert_eq!(outcomes.len(), 2);
    assert_eq!(
        (
            outcomes[0].0.as_str(),
            outcomes[0].1.as_str(),
            &outcomes[0].2
        ),
        ("manual", "settled", &None)
    );
    assert_eq!(outcomes[1].0, "ghost");
    assert_eq!(outcomes[1].1, "dropped");
    assert!(outcomes[1].2.as_deref().unwrap().contains("US500"));
    assert_eq!(restored.positions(at(27)), straight.positions(at(27)));

    // Otro reinicio: nada pendiente, y la reconstrucción con todas las velas da limpia.
    let positions = restored.positions(at(27));
    let (mut again, rec) = restore_from_store(&store, config(), markets(27), &positions, at(27))
        .await
        .unwrap();
    assert!(rec.is_clean(), "{rec:?}");
    let resumed = restore_pending_from_store(&mut again, &store, at(27))
        .await
        .unwrap();
    assert!(resumed.restored.is_empty() && resumed.dropped.is_empty());
    assert!(again.pending().is_empty());

    // Las dos tablas son de solo agregar.
    for sql in [
        "UPDATE paper_pending_orders SET instrument_id = 'X'",
        "DELETE FROM paper_pending_orders",
        "UPDATE paper_pending_outcomes SET outcome = 'dropped', reason = 'otra'",
        "DELETE FROM paper_pending_outcomes",
    ] {
        assert!(
            sqlx::query(sql).execute(store.pool()).await.is_err(),
            "{sql}"
        );
    }
}

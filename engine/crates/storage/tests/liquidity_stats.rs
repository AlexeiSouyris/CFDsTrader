//! Estadísticas de liquidez en PostgreSQL **real** (#231), en una base propia: `cfds_test_stats`
//! (se crea si falta, en el mismo servidor de `DATABASE_URL`). Nunca en `cfds_test` ni en la base
//! real. Están marcados `#[ignore]` y el CI los corre con `-- --ignored`. Las tablas son de solo
//! agregar: cada test usa un instrumento propio. Los precios son inventados.

use std::num::NonZeroU32;

use cfds_core::{
    AssetClass, Currency, DailyCandle, Direction, DistMetric, DistStat, FvgEvent, FvgEventPoint,
    FvgReaction, GapSession, Imbalance, ImbalanceLabel, Instrument, InstrumentId, LiquidityStats,
    OpeningGap, RateMetric, RateStat, SizeBucket, SizeCuts, StatsSubject, StatsTimeframe,
    ZoneStatus,
};
use cfds_storage::{PgStore, StatsReport, StatsRunReport, StorageError};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::dec;

const TEST_DB: &str = "cfds_test_stats";

/// La base de prueba propia, con las migraciones aplicadas.
async fn store() -> PgStore {
    let url = cfds_storage::test_db::test_database_url();
    let admin = PgStore::connect(&url).await.unwrap();
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(TEST_DB)
            .fetch_one(admin.pool())
            .await
            .unwrap();
    if !exists {
        // Si otro test la creó al mismo tiempo, falla con "ya existe": da igual.
        let _ = sqlx::query("CREATE DATABASE cfds_test_stats")
            .execute(admin.pool())
            .await;
    }
    let s = PgStore::connect(&cfds_storage::test_db::with_database(&url, TEST_DB))
        .await
        .unwrap();
    s.migrate().await.unwrap();
    s
}

async fn fresh_instrument(s: &PgStore, prefix: &str) -> InstrumentId {
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id: InstrumentId = format!("{prefix}{nanos}").parse().unwrap();
    s.upsert_instrument(&Instrument {
        id: id.clone(),
        name: "Test estadísticas".into(),
        asset_class: AssetClass::Index,
        currency: Currency::USD,
        exchange_timezone: "America/New_York".into(),
    })
    .await
    .unwrap();
    id
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}

fn point(t: &str, bars: u32) -> Option<FvgEventPoint> {
    Some(FvgEventPoint {
        at: at(t),
        bars: nz(bars),
    })
}

/// FVG alcista de 15m creado a las 15:00, tocado a las 15:15 y abierto.
fn open_fvg(id: &InstrumentId) -> FvgEvent {
    FvgEvent {
        instrument_id: id.clone(),
        timeframe: StatsTimeframe::M15,
        direction: Direction::Bullish,
        created_at: at("2026-11-16T15:00:00Z"),
        bottom: dec!(101),
        top: dec!(103),
        atr: dec!(2.5),
        size_atr: dec!(0.8),
        touch: point("2026-11-16T15:15:00Z", 1),
        mid: None,
        fill: None,
        expiry: None,
        reactions: vec![
            FvgReaction {
                bars: nz(4),
                return_atr: None,
                at: None,
            },
            FvgReaction {
                bars: nz(8),
                return_atr: None,
                at: None,
            },
        ],
        status: ZoneStatus::Open,
        as_of: at("2026-11-16T15:30:00Z"),
    }
}

/// El mismo FVG con más velas: reacción a 4 velas y llenado a las 16:30.
fn filled_fvg(id: &InstrumentId) -> FvgEvent {
    FvgEvent {
        mid: point("2026-11-16T15:45:00Z", 3),
        fill: point("2026-11-16T16:30:00Z", 6),
        reactions: vec![
            FvgReaction {
                bars: nz(4),
                return_atr: Some(dec!(-0.4)),
                at: Some(at("2026-11-16T16:15:00Z")),
            },
            FvgReaction {
                bars: nz(8),
                return_atr: None,
                at: None,
            },
        ],
        status: ZoneStatus::Filled,
        as_of: at("2026-11-16T16:45:00Z"),
        ..open_fvg(id)
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn fvg_events_are_idempotent_read_as_of_and_refuse_to_repaint() {
    let s = store().await;
    let us = fresh_instrument(&s, "ITFV").await;
    let horizons = [nz(4), nz(8)];
    let first = s.insert_fvg_events(&[open_fvg(&us)]).await.unwrap();
    assert_eq!(
        first,
        StatsReport {
            new_rows: 1,
            new_events: 1,
            unchanged: 0
        }
    );
    let again = s.insert_fvg_events(&[open_fvg(&us)]).await.unwrap();
    assert_eq!(
        again,
        StatsReport {
            new_rows: 0,
            new_events: 0,
            unchanged: 1
        }
    );
    // Con más velas: mitad, reacción y llenado, una sola vez.
    let later = s.insert_fvg_events(&[filled_fvg(&us)]).await.unwrap();
    assert_eq!(
        later,
        StatsReport {
            new_rows: 0,
            new_events: 3,
            unchanged: 0
        }
    );

    let read = |as_of: &str| {
        s.fvg_events(
            &us,
            StatsTimeframe::M15,
            at("2026-11-01T00:00:00Z"),
            at(as_of),
            &horizons,
        )
    };
    assert_eq!(
        read("2026-11-16T16:45:00Z").await.unwrap(),
        vec![filled_fvg(&us)]
    );
    // Como se conocía a las 15:30: tocado y abierto (sin look-ahead).
    assert_eq!(
        read("2026-11-16T15:30:00Z").await.unwrap(),
        vec![open_fvg(&us)]
    );
    // Antes de su creación no existe.
    assert!(read("2026-11-16T14:45:00Z").await.unwrap().is_empty());

    // Repintado: otro llenado, o volver a abierto, es un error y no guarda nada del lote.
    let other = FvgEvent {
        created_at: at("2026-11-17T15:00:00Z"),
        touch: None,
        as_of: at("2026-11-17T15:00:00Z"),
        ..open_fvg(&us)
    };
    let moved_fill = FvgEvent {
        fill: point("2026-11-16T16:45:00Z", 7),
        ..filled_fvg(&us)
    };
    let changed_zone = FvgEvent {
        top: dec!(104),
        ..filled_fvg(&us)
    };
    for bad in [moved_fill, open_fvg(&us), changed_zone] {
        assert!(matches!(
            s.insert_fvg_events(&[other.clone(), bad]).await,
            Err(StorageError::LiquidityStatsConflict { .. })
        ));
    }
    assert_eq!(read("2026-11-30T00:00:00Z").await.unwrap().len(), 1);
}

fn daily(id: &InstrumentId, day: &str, o: rust_decimal::Decimal) -> DailyCandle {
    let d: NaiveDate = day.parse().unwrap();
    let start = d.and_hms_opt(5, 0, 0).unwrap().and_utc();
    DailyCandle {
        instrument_id: id.clone(),
        day: d,
        start,
        end: start + TimeDelta::days(1),
        open: o,
        high: o + dec!(2),
        low: o - dec!(2),
        close: o,
        volume: None,
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn opening_gaps_are_idempotent_and_read_as_of() {
    let s = store().await;
    let us = fresh_instrument(&s, "ITGP").await;
    s.insert_daily(&[
        daily(&us, "2026-11-13", dec!(100)),
        daily(&us, "2026-11-16", dec!(103)),
        daily(&us, "2026-11-17", dec!(101)),
    ])
    .await
    .unwrap();
    let open = OpeningGap {
        instrument_id: us.clone(),
        day: "2026-11-16".parse().unwrap(),
        direction: Direction::Bullish,
        prev_close: dec!(100),
        open: dec!(103),
        atr: dec!(2),
        size_atr: dec!(1.5),
        fill: None,
        same_session: false,
        expiry: None,
        status: ZoneStatus::Open,
        as_of: at("2026-11-17T05:00:00Z"),
    };
    let filled = OpeningGap {
        fill: Some(GapSession {
            day: "2026-11-17".parse().unwrap(),
            sessions: 1,
            at: Some(at("2026-11-18T05:00:00Z")),
        }),
        status: ZoneStatus::Filled,
        as_of: at("2026-11-18T05:00:00Z"),
        ..open.clone()
    };
    assert_eq!(
        s.insert_opening_gaps(std::slice::from_ref(&open))
            .await
            .unwrap(),
        StatsReport {
            new_rows: 1,
            new_events: 0,
            unchanged: 0
        }
    );
    assert_eq!(
        s.insert_opening_gaps(std::slice::from_ref(&filled))
            .await
            .unwrap(),
        StatsReport {
            new_rows: 0,
            new_events: 1,
            unchanged: 0
        }
    );
    assert_eq!(
        s.insert_opening_gaps(std::slice::from_ref(&filled))
            .await
            .unwrap(),
        StatsReport {
            new_rows: 0,
            new_events: 0,
            unchanged: 1
        }
    );
    let from: NaiveDate = "2026-11-01".parse().unwrap();
    assert_eq!(
        s.opening_gaps(&us, from, filled.as_of).await.unwrap(),
        vec![filled.clone()]
    );
    assert_eq!(
        s.opening_gaps(&us, from, open.as_of).await.unwrap(),
        vec![open.clone()]
    );
    // Antes del cierre de la sesión del gap no existe.
    assert!(
        s.opening_gaps(&us, from, at("2026-11-16T20:00:00Z"))
            .await
            .unwrap()
            .is_empty()
    );
    // Repintado: volver a abierto.
    assert!(matches!(
        s.insert_opening_gaps(&[open]).await,
        Err(StorageError::LiquidityStatsConflict { .. })
    ));
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn imbalances_are_idempotent_and_refuse_to_repaint() {
    let s = store().await;
    let us = fresh_instrument(&s, "ITIM").await;
    let x = Imbalance {
        instrument_id: us.clone(),
        timeframe: StatsTimeframe::H1,
        at: at("2026-11-16T16:00:00Z"),
        window_bars: nz(20),
        displacement: Some(Direction::Bearish),
        bullish_body_sum: dec!(0),
        bearish_body_sum: dec!(12.5),
        range_sum: dec!(50),
        index: dec!(-0.25),
        label: ImbalanceLabel::PriceApproximation,
    };
    let y = Imbalance {
        at: at("2026-11-16T17:00:00Z"),
        displacement: None,
        index: dec!(0),
        bearish_body_sum: dec!(0),
        ..x.clone()
    };
    let r = s.insert_imbalances(&[x.clone(), y.clone()]).await.unwrap();
    assert_eq!((r.new_rows, r.unchanged), (2, 0));
    let r = s.insert_imbalances(&[x.clone(), y.clone()]).await.unwrap();
    assert_eq!((r.new_rows, r.unchanged), (0, 2));
    let read = s
        .imbalances(
            &us,
            StatsTimeframe::H1,
            at("2026-11-16T00:00:00Z"),
            at("2026-11-17T00:00:00Z"),
        )
        .await
        .unwrap();
    assert_eq!(read, vec![x.clone(), y]);
    let repainted = Imbalance {
        index: dec!(-0.3),
        ..x
    };
    assert!(matches!(
        s.insert_imbalances(&[repainted]).await,
        Err(StorageError::LiquidityStatsConflict { .. })
    ));
}

fn stats(id: &InstrumentId, n: u32, as_of: &str) -> Vec<LiquidityStats> {
    [SizeBucket::Low, SizeBucket::All]
        .into_iter()
        .map(|size| LiquidityStats {
            instrument_id: id.clone(),
            subject: StatsSubject::Fvg,
            timeframe: StatsTimeframe::M15,
            direction: Direction::Bullish,
            size_tercile: size,
            size_cuts_atr: SizeCuts {
                low_max: dec!(0.5),
                mid_max: dec!(1),
            },
            min_cases: nz(30),
            n,
            rates: vec![RateStat {
                metric: RateMetric::Touch,
                successes: 7,
                n,
                insufficient: n < 30,
                rate: None,
                ci_low: None,
                ci_high: None,
            }],
            distributions: vec![DistStat {
                metric: DistMetric::Reaction,
                horizon_bars: Some(nz(4)),
                n: 0,
                insufficient: true,
                median: None,
                p25: None,
                p75: None,
                iqr: None,
            }],
            as_of: at(as_of),
        })
        .collect()
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn aggregates_are_kept_per_run_and_an_identical_run_is_not_saved_again() {
    let s = store().await;
    let us = fresh_instrument(&s, "ITAG").await;
    let run1 = stats(&us, 12, "2026-11-16T16:00:00Z");
    let t1 = at("2026-11-16T16:05:00Z");
    assert_eq!(
        s.insert_liquidity_stats(t1, &run1).await.unwrap(),
        StatsRunReport {
            new_groups: 1,
            unchanged_groups: 0
        }
    );
    // Misma entrada más tarde: no se guarda otra vez (idempotente).
    assert_eq!(
        s.insert_liquidity_stats(at("2026-11-16T16:10:00Z"), &run1)
            .await
            .unwrap(),
        StatsRunReport {
            new_groups: 0,
            unchanged_groups: 1
        }
    );
    let run2 = stats(&us, 13, "2026-11-16T17:00:00Z");
    let t2 = at("2026-11-16T17:05:00Z");
    assert_eq!(
        s.insert_liquidity_stats(t2, &run2).await.unwrap(),
        StatsRunReport {
            new_groups: 1,
            unchanged_groups: 0
        }
    );
    let latest = |when| s.latest_liquidity_stats(&us, when);
    let mut expected = run2.clone();
    expected.sort_by_key(|x| serde_json::to_string(&x.size_tercile).unwrap());
    assert_eq!(latest(t2).await.unwrap(), expected);
    // La corrida anterior no se pisó.
    let mut first = run1.clone();
    first.sort_by_key(|x| serde_json::to_string(&x.size_tercile).unwrap());
    assert_eq!(latest(t1).await.unwrap(), first);
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_tables_are_append_only() {
    let s = store().await;
    let us = fresh_instrument(&s, "ITAO").await;
    s.insert_fvg_events(&[filled_fvg(&us)]).await.unwrap();
    s.insert_liquidity_stats(
        at("2026-11-16T17:05:00Z"),
        &stats(&us, 3, "2026-11-16T17:00:00Z"),
    )
    .await
    .unwrap();
    for sql in [
        "UPDATE fvg_zones SET top = 1 WHERE instrument_id = $1",
        "DELETE FROM fvg_zones WHERE instrument_id = $1",
        "UPDATE fvg_zone_events SET bars = 9 WHERE zone_id IN
             (SELECT id FROM fvg_zones WHERE instrument_id = $1)",
        "DELETE FROM fvg_zone_events WHERE zone_id IN
             (SELECT id FROM fvg_zones WHERE instrument_id = $1)",
        "UPDATE liquidity_stats SET n = 0 WHERE instrument_id = $1",
        "DELETE FROM liquidity_stats WHERE instrument_id = $1",
    ] {
        let r = sqlx::query(sql).bind(us.as_str()).execute(s.pool()).await;
        assert!(r.is_err(), "{sql}");
    }
    // Un evento no puede ser anterior a la creación del FVG.
    let r = sqlx::query(
        "INSERT INTO fvg_zone_events (zone_id, event, at, bars)
         SELECT id, 'expiry', created_at, 1 FROM fvg_zones WHERE instrument_id = $1",
    )
    .bind(us.as_str())
    .execute(s.pool())
    .await;
    assert!(r.is_err());
}

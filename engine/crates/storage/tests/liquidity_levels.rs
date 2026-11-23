//! Mapa de liquidez en PostgreSQL **real** (#231). Necesita `DATABASE_URL`; está marcado
//! `#[ignore]` y el CI lo corre con `-- --ignored`. Las tablas son de solo agregar: cada corrida
//! usa un instrumento propio.

use cfds_core::{
    AssetClass, Currency, Instrument, InstrumentId, LiquidityLevel, LiquidityLevelKind as K,
    LiquidityLevelState as S, LiquiditySession,
};
use cfds_storage::{LevelsReport, PgStore, StorageError};
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, dec};

async fn store() -> PgStore {
    let url = std::env::var("DATABASE_URL").expect("falta DATABASE_URL (necesita PostgreSQL)");
    let s = PgStore::connect(&url).await.unwrap();
    s.migrate().await.unwrap();
    s
}

async fn fresh_instrument(s: &PgStore) -> InstrumentId {
    let nanos = Utc::now().timestamp_nanos_opt().unwrap() % 1_000_000_000_000;
    let id: InstrumentId = format!("ITLL{nanos}").parse().unwrap();
    s.upsert_instrument(&Instrument {
        id: id.clone(),
        name: "Test niveles".into(),
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

fn level(
    id: &InstrumentId,
    kind: K,
    price: Decimal,
    exists_from: &str,
    state: S,
    changed: Option<&str>,
) -> LiquidityLevel {
    LiquidityLevel {
        instrument_id: id.clone(),
        kind,
        session: kind.is_session().then_some(LiquiditySession::Ny),
        price,
        exists_from: at(exists_from),
        state,
        state_changed_at: changed.map(at),
        expired_at: None,
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn levels_are_idempotent_read_as_of_and_refuse_to_repaint() {
    let s = store().await;
    let us = fresh_instrument(&s).await;
    let pdh = level(
        &us,
        K::Pdh,
        dec!(110.50),
        "2026-11-16T21:00:00Z",
        S::Intact,
        None,
    );
    let mut pdl = level(
        &us,
        K::Pdl,
        dec!(90),
        "2026-11-16T21:00:00Z",
        S::Intact,
        None,
    );
    pdl.expired_at = Some(at("2026-11-23T10:00:00Z"));
    let eqh = level(
        &us,
        K::Eqh,
        dec!(105.25),
        "2026-11-17T10:00:00Z",
        S::Swept,
        Some("2026-11-17T15:15:00Z"),
    );
    let ny = level(
        &us,
        K::SessionHigh,
        dec!(108),
        "2026-11-17T20:00:00Z",
        S::Broken,
        Some("2026-11-18T14:45:00Z"),
    );
    let all = vec![pdh.clone(), pdl.clone(), eqh.clone(), ny.clone()];

    let first = s.insert_liquidity_levels(&all).await.unwrap();
    assert_eq!(
        first,
        LevelsReport {
            new_levels: 4,
            new_events: 3,
            unchanged: 0
        }
    );
    let again = s.insert_liquidity_levels(&all).await.unwrap();
    assert_eq!(
        again,
        LevelsReport {
            new_levels: 0,
            new_events: 0,
            unchanged: 4
        }
    );
    let from = at("2026-11-01T00:00:00Z");
    assert_eq!(
        s.liquidity_levels(&us, from, at("2026-12-01T00:00:00Z"))
            .await
            .unwrap(),
        all
    );

    // Como se conocía el martes a las 15:00: el EQH todavía estaba intacto, el de NY no existía
    // y el PDL no había vencido.
    let tuesday = s
        .liquidity_levels(&us, from, at("2026-11-17T15:00:00Z"))
        .await
        .unwrap();
    let intact = |l: &LiquidityLevel| LiquidityLevel {
        state: S::Intact,
        state_changed_at: None,
        expired_at: None,
        ..l.clone()
    };
    assert_eq!(tuesday, vec![pdh.clone(), intact(&pdl), intact(&eqh)]);

    // Con más velas, el PDH se barre: el evento se agrega una vez.
    let swept_pdh = LiquidityLevel {
        state: S::Swept,
        state_changed_at: Some(at("2026-11-19T15:00:00Z")),
        ..pdh.clone()
    };
    let later = vec![swept_pdh.clone(), pdl.clone(), eqh.clone(), ny.clone()];
    let r = s.insert_liquidity_levels(&later).await.unwrap();
    assert_eq!((r.new_levels, r.new_events, r.unchanged), (0, 1, 3));

    // Repintado: otro estado, o volver a intacto, es un error y no guarda nada del lote.
    let new_one = level(
        &us,
        K::Pwh,
        dec!(120),
        "2026-11-20T21:00:00Z",
        S::Intact,
        None,
    );
    let broken_eqh = LiquidityLevel {
        state: S::Broken,
        ..eqh.clone()
    };
    for bad in [broken_eqh, pdh.clone()] {
        assert!(matches!(
            s.insert_liquidity_levels(&[new_one.clone(), bad]).await,
            Err(StorageError::LiquidityLevelConflict { .. })
        ));
    }
    let stored = s
        .liquidity_levels(&us, from, at("2026-12-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(stored, vec![swept_pdh, pdl, eqh, ny]);

    // Solo agregar: ni UPDATE ni DELETE en las dos tablas.
    for sql in [
        "UPDATE liquidity_levels SET price = 1 WHERE instrument_id = $1",
        "DELETE FROM liquidity_levels WHERE instrument_id = $1",
        "UPDATE liquidity_level_events SET event = 'broken' WHERE level_id IN
             (SELECT id FROM liquidity_levels WHERE instrument_id = $1)",
        "DELETE FROM liquidity_level_events WHERE level_id IN
             (SELECT id FROM liquidity_levels WHERE instrument_id = $1)",
    ] {
        let r = sqlx::query(sql).bind(us.as_str()).execute(s.pool()).await;
        assert!(r.is_err(), "{sql}");
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn the_database_rejects_what_the_contract_rejects() {
    let s = store().await;
    let us = fresh_instrument(&s).await;
    let insert = |kind: &'static str, session: Option<&'static str>| {
        sqlx::query(
            "INSERT INTO liquidity_levels (instrument_id, kind, session, price, exists_from)
             VALUES ($1, $2, $3, 100, '2026-11-16T21:00:00Z') RETURNING id",
        )
        .bind(us.as_str())
        .bind(kind)
        .bind(session)
    };
    assert!(insert("pdh", Some("ny")).execute(s.pool()).await.is_err());
    assert!(insert("session_low", None).execute(s.pool()).await.is_err());
    assert!(insert("equal_highs", None).execute(s.pool()).await.is_err());
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO liquidity_levels (instrument_id, kind, session, price, exists_from)
         VALUES ($1, 'pdh', NULL, 100, '2026-11-16T21:00:00Z') RETURNING id",
    )
    .bind(us.as_str())
    .fetch_one(s.pool())
    .await
    .unwrap();
    // Sin repetir: el mismo nivel (también con sesión NULL) no entra dos veces.
    assert!(insert("pdh", None).execute(s.pool()).await.is_err());
    let event = |at: &'static str| {
        sqlx::query("INSERT INTO liquidity_level_events (level_id, event, at) VALUES ($1, 'swept', $2::timestamptz)")
            .bind(id)
            .bind(at)
    };
    // El evento no puede ser anterior (ni igual) a la existencia del nivel.
    assert!(
        event("2026-11-16T21:00:00Z")
            .execute(s.pool())
            .await
            .is_err()
    );
    event("2026-11-17T15:00:00Z")
        .execute(s.pool())
        .await
        .unwrap();
    // Un solo evento por nivel.
    assert!(
        event("2026-11-17T16:00:00Z")
            .execute(s.pool())
            .await
            .is_err()
    );
}

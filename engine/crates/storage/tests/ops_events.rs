//! `ops_events` (#337) contra PostgreSQL **real**: se guarda desde una sesión de solo lectura
//! (como el gateway), en orden, no se modifica y solo la retención borra. Necesita
//! `DATABASE_URL`; marcado `#[ignore]` (el CI lo corre con `-- --ignored`).

use cfds_storage::{OpsEventRecord, PgStore};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Map, json};

/// La sesión de solo lectura del gateway (`cfds_gateway::read_only_url`).
fn read_only(url: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}options=-c%20default_transaction_read_only%3Don")
}

fn event(ts: DateTime<Utc>, run: &str, n: i64) -> OpsEventRecord {
    let mut fields = Map::new();
    fields.insert("run".into(), json!(run));
    fields.insert("n".into(), json!(n));
    OpsEventRecord {
        ts,
        level: "warn".into(),
        target: "broker".into(),
        event: "risk_decision".into(),
        fields,
    }
}

#[tokio::test]
#[ignore = "necesita PostgreSQL: DATABASE_URL"]
async fn saved_from_a_read_only_session_in_order_append_only_and_purged_only_by_retention() {
    let url = cfds_storage::test_db::test_database_url();
    let rw = PgStore::connect(&url).await.unwrap();
    rw.migrate().await.unwrap();
    let ro = PgStore::connect(&read_only(&url)).await.unwrap();
    // Hora futura propia de esta corrida: estos eventos son los más nuevos de la tabla.
    let run = Utc::now().timestamp_nanos_opt().unwrap().to_string();
    let t = Utc::now() + TimeDelta::days(3650);
    let events = vec![
        event(t, &run, 0),
        event(t, &run, 1),
        event(t + TimeDelta::seconds(1), &run, 2),
    ];
    assert_eq!(ro.insert_ops_events(&events).await.unwrap(), 3);

    // El más nuevo primero; a igual hora, el último escrito primero.
    let latest = ro.latest_ops_events(3).await.unwrap();
    let ns: Vec<i64> = latest
        .iter()
        .map(|e| e.fields["n"].as_i64().unwrap())
        .collect();
    assert_eq!(ns, [2, 1, 0]);
    assert!(latest.iter().all(|e| e.fields["run"] == run.as_str()));
    assert_eq!(ro.latest_ops_events(1).await.unwrap().len(), 1);

    // Solo agregar: ni UPDATE ni DELETE sin el permiso de la retención.
    for sql in [
        "UPDATE ops_events SET level = 'info' WHERE fields->>'run' = $1",
        "DELETE FROM ops_events WHERE fields->>'run' = $1",
    ] {
        assert!(
            sqlx::query(sql)
                .bind(&run)
                .execute(rw.pool())
                .await
                .is_err(),
            "{sql}"
        );
    }

    // La retención borra solo lo anterior al corte.
    let old = Utc::now() - TimeDelta::days(3650);
    let old_run = format!("{run}-old");
    rw.insert_ops_events(&[event(old, &old_run, 0)])
        .await
        .unwrap();
    let removed = rw
        .purge_ops_events_before(old + TimeDelta::seconds(1))
        .await
        .unwrap();
    assert!(removed >= 1);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM ops_events WHERE fields->>'run' = $1")
        .bind(&old_run)
        .fetch_one(rw.pool())
        .await
        .unwrap();
    assert_eq!(left, 0);
    let mine: i64 = sqlx::query_scalar("SELECT count(*) FROM ops_events WHERE fields->>'run' = $1")
        .bind(&run)
        .fetch_one(rw.pool())
        .await
        .unwrap();
    assert_eq!(mine, 3);
}

//! Eventos de los trabajos del laboratorio en Redis **real** (#775): `lab_jobs.v1.<job_id>`.
//!
//! Necesitan `REDIS_URL`. Marcados `#[ignore]`; el CI los corre en el job de integración con
//! `-- --ignored`. Cada test usa un trabajo propio (id único) y vence solo.

use std::time::Duration;

use cfds_storage::test_db::unique_id;
use cfds_storage::{LabStream, lab_stream_key};

const NEEDS: &str = "necesita Redis: REDIS_URL";

fn url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| panic!("falta REDIS_URL ({NEEDS})"))
}

#[tokio::test]
#[ignore = "necesita Redis: REDIS_URL"]
async fn events_are_published_once_in_order_and_expire() {
    let job = unique_id("LABJOB");
    let ttl = Duration::from_secs(60);
    let mut w = LabStream::connect(&url()).await.unwrap();
    for (seq, data) in [(3, "a"), (5, "b"), (9, "c")] {
        w.publish(&job, seq, data, ttl, 100).await.unwrap();
    }
    // Repetido o viejo: no agrega nada.
    w.publish(&job, 5, "b", ttl, 100).await.unwrap();
    w.publish(&job, 4, "x", ttl, 100).await.unwrap();
    let mut r = LabStream::connect(&url()).await.unwrap();
    let all = r
        .read_after(&job, 0, Duration::from_millis(10))
        .await
        .unwrap();
    let want: Vec<(i64, String)> = vec![(3, "a".into()), (5, "b".into()), (9, "c".into())];
    assert_eq!(all, want);
    let rest = r
        .read_after(&job, 5, Duration::from_millis(10))
        .await
        .unwrap();
    assert_eq!(rest, vec![(9, "c".to_owned())]);
    // Nada nuevo: espera y vuelve vacío.
    let none = r
        .read_after(&job, 9, Duration::from_millis(50))
        .await
        .unwrap();
    assert!(none.is_empty());
    // Un trabajo sin stream (vencido o desconocido): vacío, sin error.
    let other = r
        .read_after(&unique_id("LABNONE"), 0, Duration::from_millis(10))
        .await
        .unwrap();
    assert!(other.is_empty());
    let client = redis::Client::open(url()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let left: i64 = redis::cmd("TTL")
        .arg(lab_stream_key(&job))
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!((1..=60).contains(&left), "ttl {left}");
}

#[tokio::test]
#[ignore = "necesita Redis: REDIS_URL"]
async fn a_blocked_reader_wakes_up_when_the_worker_publishes() {
    let job = unique_id("LABJOB");
    let mut r = LabStream::connect(&url()).await.unwrap();
    let reader = tokio::spawn({
        let job = job.clone();
        async move { r.read_after(&job, 0, Duration::from_secs(5)).await.unwrap() }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut w = LabStream::connect(&url()).await.unwrap();
    w.publish(&job, 1, "{}", Duration::from_secs(60), 100)
        .await
        .unwrap();
    assert_eq!(reader.await.unwrap(), vec![(1, "{}".to_owned())]);
}

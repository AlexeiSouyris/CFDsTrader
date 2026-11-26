//! Consumidor de `news_interpretations.v1` contra Redis **real** (#197).
//!
//! Necesitan `REDIS_URL`. Marcados `#[ignore]`; el CI los corre en el job de integración con
//! `-- --ignored`. Cada test usa su propio stream (nombre único) y lo borra al terminar.

use cfds_storage::{NewsBatch, NewsInterpretationConsumer, StartAt};
use redis::aio::MultiplexedConnection;

const NEEDS: &str = "necesita Redis: REDIS_URL";

fn url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| panic!("falta REDIS_URL ({NEEDS})"))
}

struct Stream {
    name: String,
    conn: MultiplexedConnection,
}

impl Stream {
    async fn new(tag: &str) -> Self {
        let client = redis::Client::open(url()).unwrap();
        Self {
            name: format!(
                "test.news_interpretations.v1.{}",
                cfds_storage::test_db::unique_id(tag)
            ),
            conn: client.get_multiplexed_async_connection().await.unwrap(),
        }
    }

    async fn add(&mut self, fields: &[(&str, &str)]) {
        let mut cmd = redis::cmd("XADD");
        cmd.arg(&self.name).arg("*");
        for (k, v) in fields {
            cmd.arg(*k).arg(*v);
        }
        let _: String = cmd.query_async(&mut self.conn).await.unwrap();
    }

    /// Publica como Python (#186): `data` + `interpretation_id`.
    async fn publish(&mut self, n: u32, summary: &str) {
        let id = interpretation_id(n);
        self.add(&[("data", &doc(&id, summary)), ("interpretation_id", &id)])
            .await;
    }

    async fn consumer(&self, name: &str, start: StartAt) -> NewsInterpretationConsumer {
        NewsInterpretationConsumer::connect(&url(), &self.name, "engine", name, start)
            .await
            .unwrap()
    }

    async fn cleanup(mut self) {
        let _: i64 = redis::cmd("DEL")
            .arg(&self.name)
            .arg(format!("{}.seen.engine", self.name))
            .query_async(&mut self.conn)
            .await
            .unwrap();
    }
}

fn interpretation_id(n: u32) -> String {
    format!("{n:064x}")
}

fn doc(id: &str, summary: &str) -> String {
    format!(
        r#"{{"interpretation_id":"{id}","source":"fed","item_id":"https://www.federalreserve.gov/x/{id}","published_at":"2026-11-18T19:00:00Z","interpreted_at":"2026-11-18T19:05:00Z","interpreter":{{"name":"ollama","version":"p2.t00000000.m000000000000","model":"m@d","prompt_sha256":null}},"summary_es":"{summary}","impacts":[{{"instrument_id":"US500","sentiment":"-0.15","relevance":"0.95"}}]}}"#
    )
}

fn ids(b: &NewsBatch) -> Vec<String> {
    b.items
        .iter()
        .map(|d| d.interpretation.interpretation_id.clone())
        .collect()
}

#[tokio::test]
#[ignore = "necesita Redis: REDIS_URL"]
async fn duplicates_are_delivered_once_even_across_batches() {
    let mut s = Stream::new("dup").await;
    s.publish(1, "Primera.").await;
    // Republicada (Python publica al menos una vez), con otro resumen: vale la primera.
    s.publish(1, "Otra corrida.").await;
    s.publish(2, "Segunda.").await;
    let mut c = s.consumer("a", StartAt::Beginning).await;
    let b = c.next_batch(10, None).await.unwrap();
    assert_eq!(ids(&b), [interpretation_id(1), interpretation_id(2)]);
    assert_eq!(b.items[0].interpretation.summary_es, "Primera.");
    assert_eq!((b.duplicates, b.rejected.len()), (1, 0));
    c.commit(&b).await.unwrap();

    s.publish(1, "Tarde.").await;
    let again = c.next_batch(10, None).await.unwrap();
    assert!(again.items.is_empty());
    assert_eq!(again.duplicates, 1);
    s.cleanup().await;
}

#[tokio::test]
#[ignore = "necesita Redis: REDIS_URL"]
async fn invalid_messages_are_rejected_without_stopping_and_never_come_back() {
    let mut s = Stream::new("bad").await;
    s.add(&[("data", "no es json")]).await;
    s.add(&[("otro", "campo")]).await;
    let out_of_range = doc(&interpretation_id(3), "x").replace("-0.15", "-1.5");
    s.add(&[("data", &out_of_range)]).await;
    let early = doc(&interpretation_id(4), "x").replace("19:05:00Z", "18:00:00Z");
    s.add(&[("data", &early)]).await;
    s.add(&[
        ("data", &doc(&interpretation_id(5), "x")),
        ("interpretation_id", "otro"),
    ])
    .await;
    s.publish(6, "Válida.").await;

    let mut c = s.consumer("a", StartAt::Beginning).await;
    let b = c.next_batch(10, None).await.unwrap();
    assert_eq!(ids(&b), [interpretation_id(6)]);
    assert_eq!(b.rejected.len(), 5);
    assert!(b.rejected[1].reason.contains("falta el campo data"));
    c.commit(&b).await.unwrap();

    // Reinicio: ni las rechazadas ni la confirmada vuelven.
    drop(c);
    let mut c = s.consumer("a", StartAt::Beginning).await;
    let after = c.next_batch(10, None).await.unwrap();
    assert_eq!(after, NewsBatch::default());
    s.cleanup().await;
}

#[tokio::test]
#[ignore = "necesita Redis: REDIS_URL"]
async fn restart_neither_loses_nor_repeats() {
    let mut s = Stream::new("restart").await;
    for n in 1..=3 {
        s.publish(n, "Resumen.").await;
    }
    let mut c = s.consumer("motor", StartAt::Beginning).await;
    let first = c.next_batch(2, None).await.unwrap();
    assert_eq!(ids(&first), [interpretation_id(1), interpretation_id(2)]);
    c.commit(&first).await.unwrap();
    // Se entrega la 3 y el motor se cae antes de confirmarla.
    let lost = c.next_batch(2, None).await.unwrap();
    assert_eq!(ids(&lost), [interpretation_id(3)]);
    drop(c);

    // Mismo consumidor después del reinicio: primero la pendiente, sin repetir 1 ni 2.
    let mut c = s.consumer("motor", StartAt::Beginning).await;
    let pending = c.next_batch(10, None).await.unwrap();
    assert_eq!(ids(&pending), [interpretation_id(3)]);
    c.commit(&pending).await.unwrap();
    s.publish(4, "Nueva.").await;
    let next = c.next_batch(10, Some(100)).await.unwrap();
    assert_eq!(ids(&next), [interpretation_id(4)]);
    c.commit(&next).await.unwrap();

    drop(c);
    let mut c = s.consumer("motor", StartAt::Beginning).await;
    assert_eq!(
        c.next_batch(10, Some(50)).await.unwrap(),
        NewsBatch::default()
    );
    s.cleanup().await;
}

#[tokio::test]
#[ignore = "necesita Redis: REDIS_URL"]
async fn a_new_group_can_skip_the_history() {
    let mut s = Stream::new("new").await;
    s.publish(1, "Vieja.").await;
    let mut c = s.consumer("a", StartAt::New).await;
    assert!(c.next_batch(10, None).await.unwrap().items.is_empty());
    s.publish(2, "Nueva.").await;
    let b = c.next_batch(10, None).await.unwrap();
    assert_eq!(ids(&b), [interpretation_id(2)]);
    s.cleanup().await;
}

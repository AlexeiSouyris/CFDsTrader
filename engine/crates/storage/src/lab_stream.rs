//! Eventos de los trabajos del laboratorio en Redis Streams (#775).
//!
//! - Stream: `lab_jobs.v1.<job_id>`, uno por trabajo.
//! - Cada entrada tiene un campo `data` con el JSON de `lab_job_event.v1`.
//! - El ID de la entrada es `<seq>-0`, con el `seq` del evento en `lab_job_events`: ordena y
//!   evita duplicados (publicar dos veces el mismo evento no agrega nada).
//!
//! PostgreSQL es la fuente de verdad; Redis es solo el aviso en vivo. Lo escribe el
//! `lab_worker` y lo lee el gateway con `XREAD BLOCK` para pasarlo a SSE. Cada stream vence solo
//! (`EXPIRE`): un trabajo viejo se lee de PostgreSQL.

use std::time::Duration;

use redis::aio::MultiplexedConnection;

use crate::StorageError;

pub fn lab_stream_key(job_id: &str) -> String {
    format!("lab_jobs.v1.{job_id}")
}

// Respuesta de XREAD: [(stream, [(id, [(campo, valor)])])], o nil si venció la espera.
type ReadReply = Option<Vec<(String, Vec<(String, Vec<(String, String)>)>)>>;

pub struct LabStream {
    conn: MultiplexedConnection,
}

impl LabStream {
    /// Una conexión propia: `XREAD BLOCK` la ocupa mientras espera.
    pub async fn connect(redis_url: &str) -> Result<Self, StorageError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self {
            conn: client.get_multiplexed_async_connection().await?,
        })
    }

    /// Publica el evento `seq` del trabajo (`data` = `lab_job_event.v1`). Si ya estaba (o hay uno
    /// posterior), no hace nada. Deja el stream con como mucho ~`maxlen` entradas y vencimiento
    /// en `ttl`.
    pub async fn publish(
        &mut self,
        job_id: &str,
        seq: i64,
        data: &str,
        ttl: Duration,
        maxlen: usize,
    ) -> Result<(), StorageError> {
        let key = lab_stream_key(job_id);
        let added: Result<String, redis::RedisError> = redis::cmd("XADD")
            .arg(&key)
            .arg("MAXLEN")
            .arg("~")
            .arg(maxlen)
            .arg(format!("{seq}-0"))
            .arg("data")
            .arg(data)
            .query_async(&mut self.conn)
            .await;
        match added {
            // Redis rechaza un ID menor o igual al último: ya se publicó.
            Ok(_) => {}
            Err(e) if e.code() == Some("ERR") => {}
            Err(e) => return Err(e.into()),
        }
        let secs = i64::try_from(ttl.as_secs().max(1)).unwrap_or(i64::MAX);
        let _: i64 = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(secs)
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }

    /// Las entradas del trabajo con `seq` mayor que `after`, en orden; si no hay, espera hasta
    /// `block` a que llegue alguna (vacío si no llegó). Cada una: `(seq, data)`.
    pub async fn read_after(
        &mut self,
        job_id: &str,
        after: i64,
        block: Duration,
    ) -> Result<Vec<(i64, String)>, StorageError> {
        let millis = u64::try_from(block.as_millis()).unwrap_or(u64::MAX).max(1);
        let reply: ReadReply = redis::cmd("XREAD")
            .arg("BLOCK")
            .arg(millis)
            .arg("STREAMS")
            .arg(lab_stream_key(job_id))
            .arg(format!("{}-0", after.max(0)))
            .query_async(&mut self.conn)
            .await?;
        let mut out = Vec::new();
        for (_, entries) in reply.unwrap_or_default() {
            for (id, fields) in entries {
                let seq = id
                    .split_once('-')
                    .and_then(|(ms, _)| ms.parse::<i64>().ok());
                let data = fields
                    .into_iter()
                    .find(|(k, _)| k == "data")
                    .map(|(_, v)| v);
                if let (Some(seq), Some(data)) = (seq, data) {
                    out.push((seq, data));
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_stream_per_job() {
        assert_eq!(lab_stream_key("abc"), "lab_jobs.v1.abc");
    }
}

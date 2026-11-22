//! Lectura de interpretaciones de noticias desde Redis Streams (#197).
//!
//! Python publica cada interpretación nueva en `news_interpretations.v1` (#186): una entrada con
//! `data` = JSON de `news_interpretation.v1` e `interpretation_id`. La entrega es **al menos una
//! vez**: la misma interpretación puede aparecer en más de una entrada.
//!
//! **Por qué consumer group** (`XREADGROUP` + `XACK`) y no un "último ID leído" guardado aparte:
//! - Redis guarda solo, y de forma atómica, qué se entregó a cada consumidor. Una entrada entregada
//!   y no confirmada queda **pendiente** y se vuelve a entregar después de un reinicio: no se
//!   pierde aunque el motor se caiga a mitad de un lote.
//! - Con un ID guardado habría que escribirlo nosotros después de procesar, y una caída entre
//!   procesar y guardar repetiría o perdería entradas según el orden.
//!
//! **Deduplicación por `interpretation_id`**: un set de Redis (`<stream>.seen.<grupo>`) con los ids
//! ya confirmados, que sobrevive a los reinicios. Se agrega al set y se confirma la entrada en una
//! misma transacción ([`NewsInterpretationConsumer::commit`]): si el motor se cae antes, la entrada
//! sigue pendiente y no figura como vista, así que se vuelve a entregar; después, ya no.
//!
//! **Mensajes inválidos**: si `data` falta o no cumple el contrato (`cfds_core::NewsInterpretation`,
//! que además exige `interpreted_at >= published_at`), la entrada se rechaza: se informa por
//! `stderr`, se devuelve en [`NewsBatch::rejected`] y se confirma para que no se entregue de nuevo.
//! El consumidor no se cae.

use std::collections::{HashMap, HashSet};

use cfds_core::NewsInterpretation;
use redis::aio::MultiplexedConnection;

use crate::StorageError;

/// Stream donde Python publica las interpretaciones (`contracts/README.md`).
pub const NEWS_INTERPRETATIONS_STREAM: &str = "news_interpretations.v1";

/// Desde dónde lee un grupo **nuevo**. Un grupo que ya existe sigue desde donde quedó.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartAt {
    /// Todo el historial del stream (la deduplicación evita repetir lo ya confirmado).
    Beginning,
    /// Solo lo que se publique desde ahora (el historial se carga de PostgreSQL, #194).
    New,
}

/// Una interpretación válida y nueva, con el ID de su entrada en el stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    pub stream_id: String,
    pub interpretation: NewsInterpretation,
}

/// Una entrada que no cumple el contrato. Ya está confirmada: no vuelve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    pub stream_id: String,
    pub reason: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NewsBatch {
    /// Interpretaciones nuevas, en el orden del stream. Hay que confirmarlas con `commit`.
    pub items: Vec<Delivery>,
    pub rejected: Vec<Rejected>,
    /// Entradas repetidas (mismo `interpretation_id` ya confirmado o ya en este lote).
    pub duplicates: usize,
}

pub struct NewsInterpretationConsumer {
    conn: MultiplexedConnection,
    stream: String,
    group: String,
    consumer: String,
    seen_key: String,
    /// Mientras quedan entradas pendientes de una corrida anterior, se leen desde acá.
    pending_from: Option<String>,
}

// Respuesta de XREADGROUP: [(stream, [(id, campos o nil si la entrada se borró)])].
type ReadReply = Option<Vec<(String, Vec<(String, Option<HashMap<String, String>>)>)>>;

impl NewsInterpretationConsumer {
    /// Se conecta y crea el grupo si no existe. `consumer` identifica a este proceso dentro del
    /// grupo: tiene que ser el mismo entre reinicios para recuperar sus pendientes.
    pub async fn connect(
        redis_url: &str,
        stream: &str,
        group: &str,
        consumer: &str,
        start: StartAt,
    ) -> Result<Self, StorageError> {
        let client = redis::Client::open(redis_url)?;
        let mut conn = client.get_multiplexed_async_connection().await?;
        let from = match start {
            StartAt::Beginning => "0",
            StartAt::New => "$",
        };
        let created: Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream)
            .arg(group)
            .arg(from)
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;
        match created {
            Ok(()) => {}
            // El grupo ya existe: sigue desde donde quedó.
            Err(e) if e.code() == Some("BUSYGROUP") => {}
            Err(e) => return Err(e.into()),
        }
        Ok(Self {
            conn,
            stream: stream.to_owned(),
            group: group.to_owned(),
            consumer: consumer.to_owned(),
            seen_key: format!("{stream}.seen.{group}"),
            pending_from: Some("0".to_owned()),
        })
    }

    /// Lee hasta `count` entradas: primero las pendientes de una corrida anterior y después las
    /// nuevas. Con `block_ms`, espera hasta ese tiempo si no hay nuevas. Rechazadas y repetidas ya
    /// quedan confirmadas; las de `items` se confirman con [`Self::commit`].
    pub async fn next_batch(
        &mut self,
        count: usize,
        block_ms: Option<u64>,
    ) -> Result<NewsBatch, StorageError> {
        let mut entries = Vec::new();
        if let Some(from) = self.pending_from.clone() {
            entries = self.read(count, None, &from).await?;
            match entries.last() {
                Some((id, _)) => self.pending_from = Some(id.clone()),
                None => self.pending_from = None,
            }
        }
        if self.pending_from.is_none() && entries.is_empty() {
            entries = self.read(count, block_ms, ">").await?;
        }

        let mut batch = NewsBatch::default();
        let mut in_batch = HashSet::new();
        let mut settle = Vec::new();
        for (stream_id, fields) in entries {
            let parsed = match fields {
                None => Err("la entrada ya no está en el stream (borrada o recortada)".to_owned()),
                Some(f) => parse(&f),
            };
            match parsed {
                Err(reason) => {
                    eprintln!(
                        "⚠️  {}: entrada {stream_id} rechazada: {reason}",
                        self.stream
                    );
                    batch.rejected.push(Rejected {
                        stream_id: stream_id.clone(),
                        reason,
                    });
                    settle.push(stream_id);
                }
                Ok(n) => {
                    let id = n.interpretation_id.clone();
                    let seen: bool = redis::cmd("SISMEMBER")
                        .arg(&self.seen_key)
                        .arg(&id)
                        .query_async(&mut self.conn)
                        .await?;
                    if seen || !in_batch.insert(id) {
                        batch.duplicates += 1;
                        settle.push(stream_id);
                    } else {
                        batch.items.push(Delivery {
                            stream_id,
                            interpretation: n,
                        });
                    }
                }
            }
        }
        if !settle.is_empty() {
            let _: i64 = redis::cmd("XACK")
                .arg(&self.stream)
                .arg(&self.group)
                .arg(&settle)
                .query_async(&mut self.conn)
                .await?;
        }
        Ok(batch)
    }

    /// Confirma las interpretaciones de `batch` ya procesadas: las marca como vistas y las saca
    /// de pendientes, en una sola transacción.
    pub async fn commit(&mut self, batch: &NewsBatch) -> Result<(), StorageError> {
        if batch.items.is_empty() {
            return Ok(());
        }
        let ids: Vec<&str> = batch
            .items
            .iter()
            .map(|d| d.interpretation.interpretation_id.as_str())
            .collect();
        let entries: Vec<&str> = batch.items.iter().map(|d| d.stream_id.as_str()).collect();
        redis::pipe()
            .atomic()
            .cmd("SADD")
            .arg(&self.seen_key)
            .arg(&ids)
            .ignore()
            .cmd("XACK")
            .arg(&self.stream)
            .arg(&self.group)
            .arg(&entries)
            .ignore()
            .query_async::<()>(&mut self.conn)
            .await?;
        Ok(())
    }

    async fn read(
        &mut self,
        count: usize,
        block_ms: Option<u64>,
        from: &str,
    ) -> Result<Vec<(String, Option<HashMap<String, String>>)>, StorageError> {
        let mut cmd = redis::cmd("XREADGROUP");
        cmd.arg("GROUP")
            .arg(&self.group)
            .arg(&self.consumer)
            .arg("COUNT")
            .arg(count);
        if let Some(ms) = block_ms {
            cmd.arg("BLOCK").arg(ms);
        }
        cmd.arg("STREAMS").arg(&self.stream).arg(from);
        let reply: ReadReply = cmd.query_async(&mut self.conn).await?;
        Ok(reply
            .into_iter()
            .flatten()
            .flat_map(|(_, entries)| entries)
            .collect())
    }
}

/// `data` → `NewsInterpretation`, con el `interpretation_id` de la entrada igual al del JSON.
fn parse(fields: &HashMap<String, String>) -> Result<NewsInterpretation, String> {
    let data = fields
        .get("data")
        .ok_or_else(|| "falta el campo data".to_owned())?;
    let n: NewsInterpretation =
        serde_json::from_str(data).map_err(|e| format!("no cumple news_interpretation.v1: {e}"))?;
    if let Some(id) = fields.get("interpretation_id")
        && *id != n.interpretation_id
    {
        return Err(format!(
            "interpretation_id de la entrada ({id}) distinto del de data ({})",
            n.interpretation_id
        ));
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"{"interpretation_id":"0000000000000000000000000000000000000000000000000000000000000001","source":"fed","item_id":"x","published_at":"2026-11-18T19:00:00Z","interpreted_at":"2026-11-18T19:05:00Z","interpreter":{"name":"ollama","version":"p2","model":null,"prompt_sha256":null},"summary_es":"Resumen.","impacts":[{"instrument_id":"US500","sentiment":"-0.15","relevance":"0.95"}]}"#;

    fn fields(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn parses_the_contract_and_checks_the_entry_id() {
        let n = parse(&fields(&[("data", DOC)])).unwrap();
        assert_eq!(n.impacts.len(), 1);
        let ok = parse(&fields(&[
            ("data", DOC),
            ("interpretation_id", &n.interpretation_id),
        ]));
        assert!(ok.is_ok());
        let other = parse(&fields(&[("data", DOC), ("interpretation_id", "otro")]));
        assert!(other.unwrap_err().contains("distinto"));
    }

    #[test]
    fn rejects_missing_or_invalid_data() {
        assert!(parse(&fields(&[])).unwrap_err().contains("falta"));
        let bad = DOC.replace("\"-0.15\"", "\"-1.5\"");
        assert!(
            parse(&fields(&[("data", &bad)]))
                .unwrap_err()
                .contains("news_interpretation.v1")
        );
        let early = DOC.replace("19:05:00Z", "18:00:00Z");
        assert!(parse(&fields(&[("data", &early)])).is_err());
    }
}

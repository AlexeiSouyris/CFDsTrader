//! Muestra las interpretaciones de noticias nuevas del stream `news_interpretations.v1` (#201).
//!
//! Sirve para verificar en el Mac mini que la cadena Python (`infra/scripts/interpret_news.sh`)
//! → Redis → Rust anda de punta a punta. No decide nada ni toca la estrategia.
//!
//! ```text
//! REDIS_URL=... cargo run -q -p cfds-storage --example news_stream_tail                # sigue esperando
//! REDIS_URL=... cargo run -q -p cfds-storage --example news_stream_tail -- --once      # lo que haya y sale
//! REDIS_URL=... cargo run -q -p cfds-storage --example news_stream_tail -- --from-start --once
//! ```
//!
//! - Usa su propio grupo (`news_stream_tail`), nunca el del motor: lo que muestra acá no le
//!   quita nada al motor. El grupo recuerda lo ya mostrado: otra corrida muestra solo lo nuevo.
//! - `--from-start`: si el grupo todavía no existe, lo crea desde el principio del stream (todo el
//!   historial). Sin la opción, un grupo nuevo empieza con lo que se publique desde ahora.
//! - `--group NOMBRE`: otro grupo (por ejemplo, para volver a ver todo con `--from-start`).
//! - Las entradas que no cumplen el contrato se muestran como rechazadas (las informa el
//!   consumidor por `stderr`) y no vuelven.

use cfds_storage::{NEWS_INTERPRETATIONS_STREAM, NewsInterpretationConsumer, StartAt};

const BATCH: usize = 50;
const BLOCK_MS: u64 = 5_000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut once = false;
    let mut start = StartAt::New;
    let mut group = "news_stream_tail".to_owned();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--once" => once = true,
            "--from-start" => start = StartAt::Beginning,
            "--group" => group = args.next().ok_or("--group necesita un nombre")?,
            other => return Err(format!("opción desconocida: {other}").into()),
        }
    }
    let url = std::env::var("REDIS_URL").map_err(|_| "falta REDIS_URL")?;
    let mut consumer = NewsInterpretationConsumer::connect(
        &url,
        NEWS_INTERPRETATIONS_STREAM,
        &group,
        "tail",
        start,
    )
    .await?;
    println!("Leyendo {NEWS_INTERPRETATIONS_STREAM} (grupo {group})…");

    let (mut shown, mut rejected, mut duplicates) = (0usize, 0usize, 0usize);
    loop {
        let batch = consumer
            .next_batch(BATCH, if once { None } else { Some(BLOCK_MS) })
            .await?;
        for d in &batch.items {
            let n = &d.interpretation;
            println!(
                "\n[{}] {} {} (publicada {}, interpretada {})",
                d.stream_id,
                n.source,
                n.item_id,
                n.published_at.format("%Y-%m-%d %H:%M UTC"),
                n.interpreted_at.format("%Y-%m-%d %H:%M UTC"),
            );
            println!(
                "  intérprete {} {} ({})",
                n.interpreter.name,
                n.interpreter.version,
                n.interpreter.model.as_deref().unwrap_or("sin modelo"),
            );
            println!("  {}", n.summary_es);
            if n.impacts.is_empty() {
                println!("  sin instrumentos afectados");
            }
            for i in &n.impacts {
                println!(
                    "  {:<6} sentimiento {:>7}  relevancia {:>6}",
                    i.instrument_id.as_str(),
                    i.sentiment,
                    i.relevance
                );
            }
        }
        consumer.commit(&batch).await?;
        let empty = batch.items.is_empty() && batch.rejected.is_empty() && batch.duplicates == 0;
        shown += batch.items.len();
        rejected += batch.rejected.len();
        duplicates += batch.duplicates;
        if once && empty {
            break;
        }
    }
    println!("\n{shown} nuevas, {rejected} rechazadas, {duplicates} repetidas");
    Ok(())
}

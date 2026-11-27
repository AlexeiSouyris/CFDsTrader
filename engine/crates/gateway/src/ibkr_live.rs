//! Vela en formación de IBKR para la UI (#558), **solo visual y solo lectura**:
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/ibkr/live?instrument=<id>&timeframe=1m\|5m\|15m\|1h` (SSE) | `ibkr_live_bar.v1` |
//!
//! [`LiveHub`] recibe lo que arma `cfds_ibkr::live_feed` (la conexión de streaming, que se abre
//! solo con `[ibkr] live = true`) y lo guarda en memoria: la vela en curso y la última cerrada de
//! cada instrumento y timeframe. Cada conexión SSE manda **como mucho un mensaje por segundo**: la
//! vela cerrada (una vez, con `forming = false`) antes que la siguiente en formación. El gateway no
//! arma velas ni calcula nada acá, y nada de esto llega al análisis ni al runner.

use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use cfds_core::InstrumentId;
use cfds_ibkr::Trip;
use cfds_ibkr::live::{BarUpdate, LiveBar, LiveTimeframe};
use cfds_ibkr::live_feed::{FeedStatus, LiveSink, QuoteInfo};
use chrono::{DateTime, SecondsFormat, Utc};
use hyper::body::{Bytes, Frame};
use serde::Serialize;
use tokio::sync::{mpsc, watch};

pub const LIVE: &str = "/api/v1/ibkr/live";

pub fn is_ibkr_live_route(path: &str) -> bool {
    path == LIVE
}

/// Lo que el hub le avisa a quien lo creó (para el log y las alertas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubEvent {
    Status(FeedStatus),
    Trip(Trip),
}

#[derive(Default)]
struct Slot {
    forming: Option<LiveBar>,
    closed: Option<LiveBar>,
}

struct HubState {
    status: FeedStatus,
    quotes: HashMap<InstrumentId, QuoteInfo>,
    slots: HashMap<(InstrumentId, LiveTimeframe), Slot>,
}

/// Lo último que mandó el streaming, en memoria. Implementa [`LiveSink`].
pub struct LiveHub {
    source: String,
    instruments: Vec<InstrumentId>,
    timeframes: Vec<LiveTimeframe>,
    state: Mutex<HubState>,
    version: watch::Sender<u64>,
    events: Option<mpsc::UnboundedSender<HubEvent>>,
    clients: AtomicUsize,
}

impl LiveHub {
    /// `instruments` y `timeframes`: los que se sirven (los de `[live]`). Sin streaming
    /// (`[ibkr] live = false`), el estado queda en `disabled`.
    pub fn new(
        source: &str,
        instruments: Vec<InstrumentId>,
        timeframes: Vec<LiveTimeframe>,
        status: FeedStatus,
        events: Option<mpsc::UnboundedSender<HubEvent>>,
    ) -> Self {
        Self {
            source: source.to_owned(),
            instruments,
            timeframes,
            state: Mutex::new(HubState {
                status,
                quotes: HashMap::new(),
                slots: HashMap::new(),
            }),
            version: watch::channel(0).0,
            events,
            clients: AtomicUsize::new(0),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HubState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn bump(&self) {
        self.version.send_modify(|v| *v = v.wrapping_add(1));
    }

    fn notify(&self, ev: HubEvent) {
        if let Some(tx) = &self.events {
            let _ = tx.send(ev);
        }
    }

    pub fn serves(&self, instrument: &InstrumentId, timeframe: LiveTimeframe) -> bool {
        self.instruments.contains(instrument) && self.timeframes.contains(&timeframe)
    }

    /// El mensaje que toca mandar a una conexión que ya mandó la vela cerrada que empieza en
    /// `closed_sent` (o ninguna): primero una cerrada nueva, si hay; si no, el estado de ahora.
    pub fn next_message(
        &self,
        instrument: &InstrumentId,
        timeframe: LiveTimeframe,
        closed_sent: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> LiveMessage {
        let s = self.lock();
        let quote = s.quotes.get(instrument).cloned().unwrap_or_default();
        let slot = s.slots.get(&(instrument.clone(), timeframe));
        let closed = slot.and_then(|x| x.closed.as_ref());
        let forming = slot.and_then(|x| x.forming.as_ref());
        let (bar, is_forming) = match (closed, forming) {
            (Some(c), _) if Some(c.start) != closed_sent => (Some(c), false),
            (_, Some(f)) => (Some(f), true),
            (Some(c), None) => (Some(c), false),
            (None, None) => (None, false),
        };
        let (status, reason) = match &s.status {
            FeedStatus::Connected => match (&quote.rejected, bar) {
                (Some(r), None) => ("rejected", Some(r.clone())),
                (_, Some(_)) => ("streaming", None),
                (None, None) => ("waiting", None),
            },
            other => (other.as_str(), other.reason().map(str::to_owned)),
        };
        LiveMessage {
            instrument_id: instrument.to_string(),
            timeframe: timeframe.as_str(),
            source: self.source.clone(),
            status,
            reason: reason.map(|r| r.chars().take(300).collect()),
            data_type: quote.data_type.map(|d| d.as_str()),
            delay_seconds: quote.delay.map(|d| d.num_seconds().max(0)),
            contract: quote.contract.clone(),
            forming: is_forming,
            bar: bar.map(bar_out),
            generated_at: now.to_rfc3339_opts(SecondsFormat::Secs, true),
        }
    }
}

fn utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn bar_out(b: &LiveBar) -> LiveBarOut {
    LiveBarOut {
        start: utc(b.start),
        end: utc(b.end()),
        open: b.open.to_string(),
        high: b.high.to_string(),
        low: b.low.to_string(),
        close: b.close.to_string(),
        partial: b.partial,
        ticks: b.ticks,
    }
}

impl LiveSink for LiveHub {
    fn status(&self, status: FeedStatus, _at: DateTime<Utc>) {
        {
            let mut s = self.lock();
            if s.status == status {
                return;
            }
            s.status = status.clone();
            if !matches!(status, FeedStatus::Connected) {
                // Sin conexión, las velas en curso ya no se mueven: se descartan.
                s.quotes.clear();
                for slot in s.slots.values_mut() {
                    slot.forming = None;
                }
            }
        }
        self.notify(HubEvent::Status(status));
        self.bump();
    }

    fn quote(&self, instrument: &InstrumentId, info: &QuoteInfo, _at: DateTime<Utc>) {
        self.lock().quotes.insert(instrument.clone(), info.clone());
        self.bump();
    }

    fn bar(&self, instrument: &InstrumentId, _info: &QuoteInfo, update: &BarUpdate) {
        {
            let mut s = self.lock();
            let slot = s
                .slots
                .entry((instrument.clone(), update.bar.timeframe))
                .or_default();
            if update.forming {
                slot.forming = Some(update.bar.clone());
            } else {
                if slot
                    .forming
                    .as_ref()
                    .is_some_and(|f| f.start == update.bar.start)
                {
                    slot.forming = None;
                }
                slot.closed = Some(update.bar.clone());
            }
        }
        self.bump();
    }

    fn trip(&self, trip: &Trip) {
        self.notify(HubEvent::Trip(trip.clone()));
    }
}

/// La vela de `ibkr_live_bar.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveBarOut {
    pub start: String,
    pub end: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub partial: bool,
    pub ticks: u32,
}

/// Un mensaje de `ibkr_live_bar.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveMessage {
    pub instrument_id: String,
    pub timeframe: &'static str,
    pub source: String,
    pub status: &'static str,
    pub reason: Option<String>,
    pub data_type: Option<&'static str>,
    pub delay_seconds: Option<i64>,
    pub contract: Option<String>,
    pub forming: bool,
    pub bar: Option<LiveBarOut>,
    pub generated_at: String,
}

impl LiveMessage {
    /// Igual a otro salvo la hora en que se armó (para no repetir el mismo mensaje).
    fn same_as(&self, other: &LiveMessage) -> bool {
        LiveMessage {
            generated_at: String::new(),
            ..self.clone()
        } == LiveMessage {
            generated_at: String::new(),
            ..other.clone()
        }
    }

    fn sse(&self) -> Bytes {
        let json = serde_json::to_string(self).unwrap_or_else(|_| "{}".into());
        Bytes::from(format!("data: {json}\n\n"))
    }
}

/// Cuerpo de una respuesta SSE: lo que manda la tarea de esa conexión.
pub struct SseBody(mpsc::Receiver<Bytes>);

impl SseBody {
    /// El cuerpo que lee lo que manda `rx` (también lo usa el SSE del laboratorio).
    pub fn new(rx: mpsc::Receiver<Bytes>) -> Self {
        Self(rx)
    }
}

impl hyper::body::Body for SseBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        self.0.poll_recv(cx).map(|o| o.map(|b| Ok(Frame::data(b))))
    }
}

/// Respuesta de `/api/v1/ibkr/live`.
pub enum LiveReply {
    /// Error del pedido (texto plano).
    Error { status: u16, body: String },
    /// SSE abierto.
    Stream(SseBody),
}

struct ClientGuard(Arc<LiveHub>);

impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.0.clients.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Atiende `GET /api/v1/ibkr/live`: valida el pedido y abre el SSE.
pub fn respond_ibkr_live(
    hub: &Arc<LiveHub>,
    method: &str,
    query: Option<&str>,
    max_clients: usize,
    heartbeat: Duration,
) -> LiveReply {
    let err = |status: u16, body: &str| LiveReply::Error {
        status,
        body: body.to_owned(),
    };
    if method != "GET" {
        return err(405, "solo GET");
    }
    let Some(instrument) = crate::param(query, "instrument").and_then(|s| s.parse().ok()) else {
        return err(400, "falta instrument (por ejemplo MNQ)");
    };
    let Some(timeframe) = crate::param(query, "timeframe").and_then(|s| s.parse().ok()) else {
        return err(400, "timeframe: 1m, 5m, 15m o 1h");
    };
    if !hub.serves(&instrument, timeframe) {
        return err(
            404,
            "ese instrumento o timeframe no tiene vela en formación",
        );
    }
    if hub.clients.fetch_add(1, Ordering::SeqCst) >= max_clients {
        hub.clients.fetch_sub(1, Ordering::SeqCst);
        return err(503, "demasiadas conexiones de vela en formación");
    }
    let (tx, rx) = mpsc::channel(8);
    let guard = ClientGuard(hub.clone());
    tokio::spawn(serve(guard, instrument, timeframe, tx, heartbeat));
    LiveReply::Stream(SseBody(rx))
}

/// Una conexión SSE: como mucho un mensaje por segundo, la cerrada antes que la siguiente.
async fn serve(
    guard: ClientGuard,
    instrument: InstrumentId,
    timeframe: LiveTimeframe,
    tx: mpsc::Sender<Bytes>,
    heartbeat: Duration,
) {
    let hub = guard.0.clone();
    let mut changes = hub.version.subscribe();
    // Si se corta, el navegador vuelve a conectar a los 3 s.
    if tx
        .send(Bytes::from_static(b"retry: 3000\n\n"))
        .await
        .is_err()
    {
        return;
    }
    let mut last: Option<LiveMessage> = None;
    let mut closed_sent: Option<DateTime<Utc>> = None;
    loop {
        changes.borrow_and_update();
        let msg = hub.next_message(&instrument, timeframe, closed_sent, Utc::now());
        if last.as_ref().is_none_or(|l| !l.same_as(&msg)) {
            if tx.send(msg.sse()).await.is_err() {
                return;
            }
            if !msg.forming
                && let Some(b) = &msg.bar
            {
                closed_sent = DateTime::parse_from_rfc3339(&b.start)
                    .ok()
                    .map(|d| d.with_timezone(&Utc));
            }
            last = Some(msg);
            // Tope: un mensaje por segundo. Después se mira de nuevo (puede haber otro pendiente).
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }
        tokio::select! {
            r = changes.changed() => if r.is_err() { return },
            () = tokio::time::sleep(heartbeat) => {
                if tx.send(Bytes::from_static(b": ping\n\n")).await.is_err() {
                    return;
                }
            }
        }
    }
}

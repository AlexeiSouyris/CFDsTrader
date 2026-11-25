//! Suscripción de precios por streaming para la vela en formación (#558), **solo lectura y solo
//! visual**. Abre su propia conexión (`[live] client_id`), con las mismas guardas que el resto del
//! crate ([`crate::connect`]: puerto, cuenta paper, nada grabando en disco, kill switch), se suscribe
//! a cada instrumento de `[live]` y manda a un [`LiveSink`] las velas que arma [`QuoteState`].
//!
//! **Tipo de dato:** con `first_try = "live"` pide en vivo; si IBKR contesta que la cuenta no tiene
//! permiso (354, 10089, ...), pide demorado para ese instrumento. Lo que manda IBKR (en vivo,
//! demorado, congelado) y el atraso medido van siempre en cada actualización.
//!
//! **Cortes (decisión, #558):** una desconexión activa el kill switch de esa sesión, que no se
//! reabre nunca. Este módulo puede abrir **otra** sesión más tarde (espera creciente entre
//! `reconnect_min` y `reconnect_max`), que vuelve a pasar por todas las guardas: el riesgo del
//! ADR 0017 ("después de un nuevo login la sesión podría ser otra") lo cubre la guarda de cuentas,
//! que se mira de nuevo en cada conexión. No usa ni guarda credenciales: el login de IB Gateway lo
//! hace el dueño. Si una guarda rechaza la sesión (otra cuenta, puerto, `.env`) o llega un dato
//! inválido, el streaming se detiene y no reintenta.

use std::sync::Arc;
use std::time::Duration;

use cfds_core::InstrumentId;
use cfds_obs::AlertKind;
use chrono::{DateTime, TimeDelta, Utc};
use tokio::sync::mpsc;

use crate::client::{ConnectError, connect};
use crate::config::{IbkrConfig, LiveConfig};
use crate::live::{BarUpdate, LiveDataType, QuoteState};
use crate::session::{ApiError, IbkrApi, ReadOnlySession, SessionError, Trip};

/// Estado de la conexión de streaming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedStatus {
    /// `[live]` no está o el gateway no la prendió.
    Disabled,
    Connecting,
    /// Sesión abierta y suscripciones pedidas.
    Connected,
    /// Sin conexión con IB Gateway (cerrado, reiniciando o cortado): reintenta solo.
    Disconnected {
        reason: String,
    },
    /// Detenido por una guarda o un dato inválido: no reintenta hasta reiniciar.
    Stopped {
        reason: String,
    },
}

impl FeedStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedStatus::Disabled => "disabled",
            FeedStatus::Connecting => "connecting",
            FeedStatus::Connected => "connected",
            FeedStatus::Disconnected { .. } => "disconnected",
            FeedStatus::Stopped { .. } => "stopped",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            FeedStatus::Disconnected { reason } | FeedStatus::Stopped { reason } => Some(reason),
            _ => None,
        }
    }
}

/// Lo que se sabe de los precios de un instrumento.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuoteInfo {
    pub contract: Option<String>,
    pub data_type: Option<LiveDataType>,
    /// Atraso medido (llegada − hora del último operado), si hay.
    pub delay: Option<TimeDelta>,
    /// La suscripción de este instrumento fue rechazada (y por qué).
    pub rejected: Option<String>,
}

impl QuoteInfo {
    fn of(state: &QuoteState) -> Self {
        Self {
            contract: state.contract.clone(),
            data_type: state.data_type,
            delay: state.delay,
            rejected: None,
        }
    }
}

/// A quién se le mandan las velas (el gateway las guarda en memoria para la UI).
pub trait LiveSink: Send + Sync + 'static {
    fn status(&self, status: FeedStatus, at: DateTime<Utc>);
    /// Cambió lo que se sabe de un instrumento (contrato, tipo de dato, atraso, rechazo).
    fn quote(&self, instrument: &InstrumentId, info: &QuoteInfo, at: DateTime<Utc>);
    fn bar(&self, instrument: &InstrumentId, info: &QuoteInfo, update: &BarUpdate);
    /// Se activó un kill switch: el que usa el feed manda la alerta.
    fn trip(&self, trip: &Trip);
}

/// Cómo terminó una sesión.
enum Ended {
    /// Se cortó la conexión: se puede volver a abrir otra sesión.
    Disconnected(String),
    /// Una guarda o un dato inválido: no se reintenta.
    Stop(String),
}

fn classify(e: &SessionError) -> Ended {
    match e {
        SessionError::KillSwitch(t) if t.kind == AlertKind::Disconnect => {
            Ended::Disconnected(t.reason.clone())
        }
        SessionError::KillSwitch(t) => Ended::Stop(t.reason.clone()),
        other => Ended::Stop(other.to_string()),
    }
}

/// Corre el streaming para siempre (hasta un error que lo detiene). `env` lee variables de entorno
/// (en producción, `std::env::var`).
pub async fn run<S: LiveSink>(
    cfg: IbkrConfig,
    env: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
    sink: Arc<S>,
) {
    let Some(live) = cfg.live.clone() else {
        sink.status(FeedStatus::Disabled, Utc::now());
        return;
    };
    let mut conn_cfg = cfg.clone();
    conn_cfg.client_id = live.client_id;
    let mut wait = live.reconnect_min;
    loop {
        sink.status(FeedStatus::Connecting, Utc::now());
        let ended = match connect(conn_cfg.clone(), &env).await {
            Ok(session) => {
                wait = live.reconnect_min;
                sink.status(FeedStatus::Connected, Utc::now());
                run_session(Arc::new(session), &live, sink.clone()).await
            }
            Err(ConnectError::Connect { address, reason }) => {
                Ended::Disconnected(format!("IB Gateway no responde en {address}: {reason}"))
            }
            Err(ConnectError::Tripped(t)) => {
                sink.trip(&t);
                if t.kind == AlertKind::Disconnect {
                    Ended::Disconnected(t.reason)
                } else {
                    Ended::Stop(t.reason)
                }
            }
            Err(e) => Ended::Stop(e.to_string()),
        };
        match ended {
            Ended::Stop(reason) => {
                sink.status(FeedStatus::Stopped { reason }, Utc::now());
                return;
            }
            Ended::Disconnected(reason) => {
                sink.status(FeedStatus::Disconnected { reason }, Utc::now());
                tokio::time::sleep(wait).await;
                wait = next_wait(wait, live.reconnect_max);
            }
        }
    }
}

/// La espera siguiente: el doble, con tope.
pub fn next_wait(wait: Duration, max: Duration) -> Duration {
    (wait * 2).min(max)
}

/// Una sesión: una tarea por instrumento. Termina cuando una de ellas corta toda la sesión
/// (desconexión o kill switch); las demás se cancelan y la sesión se desconecta.
async fn run_session<A: IbkrApi + 'static, S: LiveSink>(
    session: Arc<ReadOnlySession<A>>,
    live: &LiveConfig,
    sink: Arc<S>,
) -> Ended {
    let mut tasks = tokio::task::JoinSet::new();
    for id in live.instruments.clone() {
        let (session, sink, live) = (session.clone(), sink.clone(), live.clone());
        tasks.spawn(async move { stream_instrument(&session, &id, &live, sink.as_ref()).await });
    }
    let mut ended = Ended::Disconnected("todas las suscripciones terminaron".into());
    while let Some(r) = tasks.join_next().await {
        match r {
            // Un instrumento rechazado no corta a los demás.
            Ok(None) => continue,
            Ok(Some(e)) => {
                ended = e;
                break;
            }
            Err(e) => {
                ended = Ended::Stop(format!("tarea de streaming: {e}"));
                break;
            }
        }
    }
    tasks.abort_all();
    session.disconnect().await;
    if let Some(t) = session.trip_reason() {
        sink.trip(&t);
    }
    ended
}

/// Streaming de un instrumento hasta que se corta. `None`: este instrumento quedó afuera (rechazo
/// de IBKR) sin afectar a la sesión; `Some`: la sesión tiene que terminar.
async fn stream_instrument<A: IbkrApi, S: LiveSink + ?Sized>(
    session: &ReadOnlySession<A>,
    id: &InstrumentId,
    live: &LiveConfig,
    sink: &S,
) -> Option<Ended> {
    let mut data_type = live.first_try;
    loop {
        let (tx, mut rx) = mpsc::channel(512);
        let mut state = QuoteState::new(&live.timeframes, live.pending_wait);
        let mut info = QuoteInfo::default();
        let stream = session.stream_quotes(id, data_type, tx, Utc::now());
        tokio::pin!(stream);
        let mut clock = tokio::time::interval(Duration::from_secs(1));
        clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut publish = |state: &QuoteState, updates: Vec<BarUpdate>| {
            let now_info = QuoteInfo::of(state);
            if now_info != info {
                info = now_info;
                sink.quote(id, &info, Utc::now());
            }
            for u in &updates {
                sink.bar(id, &info, u);
            }
        };
        let result = loop {
            let updates = tokio::select! {
                r = &mut stream => break r,
                Some(ev) = rx.recv() => state.on_event(ev),
                _ = clock.tick() => state.on_clock(Utc::now()),
            };
            publish(&state, updates);
        };
        // Lo que llegó antes de que terminara la suscripción.
        while let Ok(ev) = rx.try_recv() {
            let updates = state.on_event(ev);
            publish(&state, updates);
        }
        match result {
            Ok(()) => return Some(Ended::Disconnected("la suscripción terminó".into())),
            Err(SessionError::Rejected(ApiError::NoPermission(m)))
                if data_type == LiveDataType::Live =>
            {
                // Sin permiso para en vivo (medido: 354 en MNQ, MES y QQQ): se piden los
                // demorados. El tipo que informe IBKR va en cada mensaje.
                let _ = m;
                data_type = LiveDataType::Delayed;
            }
            Err(e @ SessionError::Rejected(_)) | Err(e @ SessionError::UnknownInstrument(_)) => {
                sink.quote(
                    id,
                    &QuoteInfo {
                        rejected: Some(e.to_string()),
                        ..QuoteInfo::default()
                    },
                    Utc::now(),
                );
                return None;
            }
            Err(e @ SessionError::KillSwitch(_)) => return Some(classify(&e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use rust_decimal::dec;

    use super::*;
    use crate::account::PaperAccountId;
    use crate::live::QuoteEvent;
    use crate::session::{BarsRequest, BarsResponse, QuoteRequest, SummaryItem};

    const CONFIG: &str = include_str!("../../../../config/ibkr.toml");

    /// IB Gateway falso: en vivo responde 354 (como la cuenta medida) y demorados manda unos
    /// precios; con `cut`, después de los precios se corta la conexión.
    #[derive(Default)]
    struct Fake {
        down: AtomicBool,
        cut: bool,
        asked: Mutex<Vec<LiveDataType>>,
    }

    impl IbkrApi for Fake {
        fn is_connected(&self) -> bool {
            !self.down.load(Ordering::SeqCst)
        }
        async fn managed_accounts(&self) -> Result<Vec<String>, ApiError> {
            Ok(vec!["DU0000001".into()])
        }
        async fn account_summary(&self) -> Result<Vec<SummaryItem>, ApiError> {
            Ok(Vec::new())
        }
        async fn bars(&self, _: &BarsRequest) -> Result<BarsResponse, ApiError> {
            Err(ApiError::Rejected("sin velas".into()))
        }
        async fn head_timestamp(
            &self,
            _: &BarsRequest,
        ) -> Result<(String, DateTime<Utc>), ApiError> {
            Err(ApiError::Rejected("no".into()))
        }
        async fn stream_quotes(
            &self,
            req: &QuoteRequest,
            out: mpsc::Sender<QuoteEvent>,
        ) -> Result<(), ApiError> {
            self.asked.lock().unwrap().push(req.data_type);
            if req.data_type == LiveDataType::Live {
                return Err(ApiError::NoPermission("354: not subscribed".into()));
            }
            let now = Utc::now();
            let evs = [
                QuoteEvent::Contract("MNQZ6".into()),
                QuoteEvent::DataType(LiveDataType::Delayed),
                QuoteEvent::LastTime {
                    at: now - TimeDelta::seconds(700),
                    received: now,
                },
                QuoteEvent::Last {
                    price: dec!(26000),
                    received: now,
                },
                QuoteEvent::LastTime {
                    at: now - TimeDelta::seconds(600),
                    received: now,
                },
            ];
            for ev in evs {
                let _ = out.send(ev).await;
            }
            if self.cut {
                self.down.store(true, Ordering::SeqCst);
                return Err(ApiError::ConnectionLost("socket cerrado".into()));
            }
            out.closed().await;
            Ok(())
        }
        async fn disconnect(&self) {
            self.down.store(true, Ordering::SeqCst);
        }
    }

    #[derive(Default)]
    struct Sink {
        quotes: Mutex<Vec<QuoteInfo>>,
        bars: Mutex<Vec<BarUpdate>>,
        trips: Mutex<Vec<Trip>>,
    }

    impl LiveSink for Sink {
        fn status(&self, _: FeedStatus, _: DateTime<Utc>) {}
        fn quote(&self, _: &InstrumentId, info: &QuoteInfo, _: DateTime<Utc>) {
            self.quotes.lock().unwrap().push(info.clone());
        }
        fn bar(&self, _: &InstrumentId, _: &QuoteInfo, u: &BarUpdate) {
            self.bars.lock().unwrap().push(u.clone());
        }
        fn trip(&self, t: &Trip) {
            self.trips.lock().unwrap().push(t.clone());
        }
    }

    async fn session(fake: Fake) -> ReadOnlySession<Fake> {
        let cfg = IbkrConfig::from_toml(CONFIG).unwrap();
        let paper = PaperAccountId::from_env("X", |_| Some("DU0000001".into())).unwrap();
        ReadOnlySession::open(fake, cfg, paper, Utc::now())
            .await
            .map_err(|t| t.reason)
            .unwrap()
    }

    fn live_cfg() -> LiveConfig {
        IbkrConfig::from_toml(CONFIG).unwrap().live.expect("[live]")
    }

    #[tokio::test]
    async fn no_live_permission_falls_back_to_delayed_and_reports_it() {
        let s = session(Fake {
            cut: true,
            ..Fake::default()
        })
        .await;
        let sink = Sink::default();
        let id: InstrumentId = "MNQ".parse().unwrap();
        let ended = stream_instrument(&s, &id, &live_cfg(), &sink).await;
        assert_eq!(
            *s.api_for_tests().asked.lock().unwrap(),
            vec![LiveDataType::Live, LiveDataType::Delayed]
        );
        // Un corte de conexión: la sesión termina con kill switch de desconexión → se reintenta.
        assert!(matches!(ended, Some(Ended::Disconnected(_))));
        assert!(
            s.trip_reason()
                .is_some_and(|t| t.kind == AlertKind::Disconnect)
        );
        let quotes = sink.quotes.lock().unwrap();
        let last = quotes.last().unwrap();
        assert_eq!(last.data_type, Some(LiveDataType::Delayed));
        assert_eq!(last.contract.as_deref(), Some("MNQZ6"));
        assert_eq!(last.delay, Some(TimeDelta::seconds(600)));
        let bars = sink.bars.lock().unwrap();
        assert!(!bars.is_empty() && bars.iter().all(|u| u.bar.close == dec!(26000)));
    }

    #[tokio::test]
    async fn unknown_instrument_only_drops_that_instrument() {
        let s = session(Fake::default()).await;
        let sink = Sink::default();
        let id: InstrumentId = "NOPE".parse().unwrap();
        assert!(
            stream_instrument(&s, &id, &live_cfg(), &sink)
                .await
                .is_none()
        );
        assert!(sink.quotes.lock().unwrap()[0].rejected.is_some());
        assert!(s.trip_reason().is_none());
    }

    #[test]
    fn classify_and_backoff() {
        let trip = |kind| {
            SessionError::KillSwitch(Trip {
                kind,
                reason: "x".into(),
                at: Utc::now(),
            })
        };
        assert!(matches!(
            classify(&trip(AlertKind::Disconnect)),
            Ended::Disconnected(_)
        ));
        // Otra cuenta o un dato inválido: no se reintenta.
        assert!(matches!(
            classify(&trip(AlertKind::KillSwitch)),
            Ended::Stop(_)
        ));
        assert!(matches!(
            classify(&trip(AlertKind::DataError)),
            Ended::Stop(_)
        ));
        let max = Duration::from_secs(300);
        assert_eq!(
            next_wait(Duration::from_secs(5), max),
            Duration::from_secs(10)
        );
        assert_eq!(next_wait(Duration::from_secs(200), max), max);
    }
}

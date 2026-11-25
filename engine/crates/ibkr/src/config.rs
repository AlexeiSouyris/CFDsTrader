//! `config/ibkr.toml`: conexión a IB Gateway paper, puertos prohibidos e instrumentos. Se valida
//! al arrancar (ADR 0017 §2.1).

use std::net::IpAddr;
use std::time::Duration;

use cfds_core::{InstrumentId, Timeframe};
use serde::Deserialize;

/// Puertos de sesión real que la lista de `forbidden_ports` tiene que incluir siempre: TWS real
/// (7496) e IB Gateway real (4001). Sacarlos de la configuración no los habilita.
pub const REQUIRED_FORBIDDEN_PORTS: [u16; 2] = [7496, 4001];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IbkrConfigError {
    #[error("config/ibkr.toml no es TOML válido: {0}")]
    Parse(String),
    #[error("config/ibkr.toml, {field}: {problem}")]
    Invalid {
        field: &'static str,
        problem: String,
    },
}

/// Qué precio piden las velas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatToShow {
    Trades,
    Midpoint,
}

/// Qué tipo de contrato es un instrumento de `config/ibkr.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractKind {
    /// Futuro con vencimiento: se usa el vigente más cercano (o el mes pedido).
    Future,
    /// Futuro continuo de IBKR (`CONTFUT`), solo para historia.
    ContinuousFuture,
    /// Acción o ETF (por ejemplo QQQ).
    Stock,
    /// Divisa en IDEALPRO: `symbol` es la moneda base y `currency` la cotizada.
    Forex,
}

impl ContractKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ContractKind::Future => "future",
            ContractKind::ContinuousFuture => "continuous_future",
            ContractKind::Stock => "stock",
            ContractKind::Forex => "forex",
        }
    }
}

/// Un instrumento de `config/ibkr.toml` (el nombre viene de la etapa 1, cuando eran solo futuros).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutureSpec {
    pub id: InstrumentId,
    pub kind: ContractKind,
    pub symbol: String,
    pub exchange: String,
    pub currency: String,
    /// Precio propio del instrumento; si no, el de `[history]` (en IDEALPRO no hay `TRADES`).
    pub what_to_show: Option<WhatToShow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrConfig {
    pub host: String,
    pub port: u16,
    pub client_id: i32,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub watch_interval: Duration,
    pub forbidden_ports: Vec<u16>,
    pub paper_account_env: String,
    pub timeframe: Timeframe,
    pub lookback: chrono::TimeDelta,
    pub what_to_show: WhatToShow,
    pub regular_hours_only: bool,
    pub source: String,
    pub instruments: Vec<FutureSpec>,
    pub load: crate::load::LoadConfig,
    /// `[load_health]`: cuándo avisar que la carga se trabó (#607). `None` si no está.
    pub load_health: Option<crate::health::HealthConfig>,
    /// `[live]`: precios por streaming para la vela en formación (#558). `None` si no está.
    pub live: Option<LiveConfig>,
}

/// `[live]` de `config/ibkr.toml`: la suscripción de precios para la vela en formación (#558).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveConfig {
    /// Identificador propio de esta conexión (distinto del de `[gateway]`).
    pub client_id: i32,
    /// Instrumentos con precios por streaming (futuros o acciones: los de IDEALPRO no tienen
    /// último operado).
    pub instruments: Vec<InstrumentId>,
    pub timeframes: Vec<crate::live::LiveTimeframe>,
    /// Qué se pide primero: en vivo (si no hay permiso, pasa a demorado) o directamente demorado.
    pub first_try: crate::live::LiveDataType,
    /// Cuánto espera un precio a su hora antes de usar la estimada.
    pub pending_wait: chrono::TimeDelta,
    /// Espera antes de volver a abrir el socket después de un corte, y su máximo (se duplica).
    pub reconnect_min: Duration,
    pub reconnect_max: Duration,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    gateway: RawGateway,
    account: RawAccount,
    history: RawHistory,
    load: RawLoad,
    #[serde(default)]
    load_health: Option<RawHealth>,
    #[serde(default)]
    live: Option<RawLive>,
    instruments: Vec<RawInstrument>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLive {
    client_id: i32,
    instruments: Vec<String>,
    timeframes: Vec<String>,
    first_try: String,
    pending_wait_ms: u64,
    reconnect_min_secs: u64,
    reconnect_max_secs: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLoad {
    source: String,
    pause_secs: u64,
    max_requests: usize,
    backfill_days: u32,
    window_1m_hours: u32,
    window_15m_hours: u32,
    window_1h_hours: u32,
    data_delay_secs: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHealth {
    min_failures: u32,
    stall_minutes: u32,
    forget_after_hours: u32,
    notify_macos: bool,
    state_file: String,
}

/// Valida `[load_health]` (#607).
fn parse_health(h: RawHealth) -> Result<crate::health::HealthConfig, IbkrConfigError> {
    if !(2..=100).contains(&h.min_failures) {
        return Err(invalid(
            "load_health.min_failures",
            "tiene que estar entre 2 y 100",
        ));
    }
    if !(5..=1440).contains(&h.stall_minutes) {
        return Err(invalid(
            "load_health.stall_minutes",
            "tiene que estar entre 5 y 1440",
        ));
    }
    let stall = chrono::TimeDelta::minutes(i64::from(h.stall_minutes));
    let forget_after = chrono::TimeDelta::hours(i64::from(h.forget_after_hours));
    if h.forget_after_hours > 168 || forget_after <= stall {
        return Err(invalid(
            "load_health.forget_after_hours",
            "tiene que ser más que stall_minutes y como mucho 168",
        ));
    }
    let path = std::path::Path::new(&h.state_file);
    if h.state_file.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || path.extension().is_none_or(|e| e != "json")
    {
        return Err(invalid(
            "load_health.state_file",
            "ruta relativa a la raíz del repo, sin '..', terminada en .json",
        ));
    }
    Ok(crate::health::HealthConfig {
        min_failures: h.min_failures,
        stall,
        forget_after,
        notify_macos: h.notify_macos,
        state_file: h.state_file,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGateway {
    host: String,
    port: u16,
    client_id: i32,
    connect_timeout_secs: u64,
    request_timeout_secs: u64,
    watch_interval_secs: u64,
    forbidden_ports: Vec<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAccount {
    paper_account_env: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHistory {
    timeframe: String,
    lookback_hours: u32,
    what_to_show: String,
    regular_hours_only: bool,
    source: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInstrument {
    id: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    what_to_show: Option<String>,
    symbol: String,
    exchange: String,
    currency: String,
}

fn invalid(field: &'static str, problem: impl Into<String>) -> IbkrConfigError {
    IbkrConfigError::Invalid {
        field,
        problem: problem.into(),
    }
}

fn secs(field: &'static str, v: u64, max: u64) -> Result<Duration, IbkrConfigError> {
    if (1..=max).contains(&v) {
        Ok(Duration::from_secs(v))
    } else {
        Err(invalid(field, format!("tiene que estar entre 1 y {max}")))
    }
}

fn plain(field: &'static str, v: &str) -> Result<String, IbkrConfigError> {
    let ok = (1..=16).contains(&v.len()) && v.chars().all(|c| c.is_ascii_alphanumeric());
    if ok {
        Ok(v.to_owned())
    } else {
        Err(invalid(field, format!("{v:?}: 1-16 letras o números")))
    }
}

fn parse_what(field: &'static str, v: &str) -> Result<WhatToShow, IbkrConfigError> {
    match v {
        "TRADES" => Ok(WhatToShow::Trades),
        "MIDPOINT" => Ok(WhatToShow::Midpoint),
        other => Err(invalid(
            field,
            format!("{other:?}: solo \"TRADES\" o \"MIDPOINT\""),
        )),
    }
}

fn parse_live(
    l: RawLive,
    gateway_client_id: i32,
    instruments: &[FutureSpec],
) -> Result<LiveConfig, IbkrConfigError> {
    use crate::live::{LiveDataType, LiveTimeframe};
    if l.client_id < 0 || l.client_id == gateway_client_id {
        return Err(invalid(
            "live.client_id",
            format!("no negativo y distinto de gateway.client_id ({gateway_client_id})"),
        ));
    }
    if l.instruments.is_empty() {
        return Err(invalid("live.instruments", "hace falta al menos uno"));
    }
    let mut ids: Vec<InstrumentId> = Vec::with_capacity(l.instruments.len());
    for raw in &l.instruments {
        let id = raw
            .parse::<InstrumentId>()
            .map_err(|e| invalid("live.instruments", e.to_string()))?;
        let spec = instruments.iter().find(|s| s.id == id).ok_or_else(|| {
            invalid(
                "live.instruments",
                format!("{id} no está en [[instruments]]"),
            )
        })?;
        if !matches!(spec.kind, ContractKind::Future | ContractKind::Stock) {
            return Err(invalid(
                "live.instruments",
                format!("{id}: solo futuros o acciones (tienen último operado)"),
            ));
        }
        if ids.contains(&id) {
            return Err(invalid("live.instruments", format!("{id} repetido")));
        }
        ids.push(id);
    }
    if l.timeframes.is_empty() {
        return Err(invalid("live.timeframes", "hace falta al menos uno"));
    }
    let mut timeframes = Vec::with_capacity(l.timeframes.len());
    for t in &l.timeframes {
        let tf = t
            .parse::<LiveTimeframe>()
            .map_err(|e| invalid("live.timeframes", e))?;
        if timeframes.contains(&tf) {
            return Err(invalid("live.timeframes", format!("{t} repetido")));
        }
        timeframes.push(tf);
    }
    let first_try = match l.first_try.as_str() {
        "live" => LiveDataType::Live,
        "delayed" => LiveDataType::Delayed,
        other => {
            return Err(invalid(
                "live.first_try",
                format!("{other:?}: solo \"live\" o \"delayed\""),
            ));
        }
    };
    if !(100..=5000).contains(&l.pending_wait_ms) {
        return Err(invalid(
            "live.pending_wait_ms",
            "tiene que estar entre 100 y 5000",
        ));
    }
    let reconnect_min = secs("live.reconnect_min_secs", l.reconnect_min_secs, 300)?;
    let reconnect_max = secs("live.reconnect_max_secs", l.reconnect_max_secs, 3600)?;
    if reconnect_max < reconnect_min {
        return Err(invalid(
            "live.reconnect_max_secs",
            "no puede ser menor que reconnect_min_secs",
        ));
    }
    Ok(LiveConfig {
        client_id: l.client_id,
        instruments: ids,
        timeframes,
        first_try,
        pending_wait: chrono::TimeDelta::milliseconds(l.pending_wait_ms as i64),
        reconnect_min,
        reconnect_max,
    })
}

/// Puerto prohibido: está en la lista de la configuración o es uno de los reales fijos.
pub fn is_forbidden_port(port: u16, forbidden: &[u16]) -> bool {
    forbidden.contains(&port) || REQUIRED_FORBIDDEN_PORTS.contains(&port)
}

impl IbkrConfig {
    pub fn from_toml(text: &str) -> Result<Self, IbkrConfigError> {
        let r: Raw = toml::from_str(text).map_err(|e| IbkrConfigError::Parse(e.to_string()))?;
        let g = r.gateway;
        let loopback =
            g.host == "localhost" || g.host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
        if !loopback {
            return Err(invalid(
                "gateway.host",
                "solo se permite esta máquina (127.0.0.1, ::1 o localhost)",
            ));
        }
        for p in REQUIRED_FORBIDDEN_PORTS {
            if !g.forbidden_ports.contains(&p) {
                return Err(invalid(
                    "gateway.forbidden_ports",
                    format!("tiene que incluir {p} (sesión real)"),
                ));
            }
        }
        if is_forbidden_port(g.port, &g.forbidden_ports) {
            return Err(invalid(
                "gateway.port",
                format!("{} es un puerto de sesión real: no se conecta", g.port),
            ));
        }
        if g.port == 0 {
            return Err(invalid("gateway.port", "no puede ser 0"));
        }
        if g.client_id < 0 {
            return Err(invalid("gateway.client_id", "no puede ser negativo"));
        }
        let env = r.account.paper_account_env;
        let env_ok = !env.is_empty()
            && env
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !env_ok {
            return Err(invalid(
                "account.paper_account_env",
                "nombre de variable: mayúsculas, números y '_'",
            ));
        }
        let h = r.history;
        let timeframe = match h.timeframe.as_str() {
            "15m" => Timeframe::M15,
            "1h" => Timeframe::H1,
            other => {
                return Err(invalid(
                    "history.timeframe",
                    format!("{other:?}: solo \"15m\" o \"1h\""),
                ));
            }
        };
        let what_to_show = parse_what("history.what_to_show", &h.what_to_show)?;
        if !(1..=24 * 30).contains(&h.lookback_hours) {
            return Err(invalid(
                "history.lookback_hours",
                "tiene que estar entre 1 y 720",
            ));
        }
        if h.source.trim().is_empty() {
            return Err(invalid("history.source", "no puede estar vacío"));
        }
        if r.instruments.is_empty() {
            return Err(invalid("instruments", "hace falta al menos uno"));
        }
        let mut instruments = Vec::with_capacity(r.instruments.len());
        for i in r.instruments {
            let id =
                i.id.parse::<InstrumentId>()
                    .map_err(|e| invalid("instruments.id", e.to_string()))?;
            if instruments.iter().any(|s: &FutureSpec| s.id == id) {
                return Err(invalid("instruments.id", format!("{id} repetido")));
            }
            let kind = match i.kind.as_deref().unwrap_or("future") {
                "future" => ContractKind::Future,
                "continuous_future" => ContractKind::ContinuousFuture,
                "stock" => ContractKind::Stock,
                "forex" => ContractKind::Forex,
                other => {
                    return Err(invalid(
                        "instruments.kind",
                        format!("{other:?}: future, continuous_future, stock o forex"),
                    ));
                }
            };
            let what_to_show = match i.what_to_show.as_deref() {
                None => None,
                Some(w) => Some(parse_what("instruments.what_to_show", w)?),
            };
            instruments.push(FutureSpec {
                id,
                kind,
                what_to_show,
                symbol: plain("instruments.symbol", &i.symbol)?,
                exchange: plain("instruments.exchange", &i.exchange)?,
                currency: plain("instruments.currency", &i.currency)?,
            });
        }
        let l = r.load;
        if !(1..=32).contains(&l.source.len())
            || !l.source.starts_with(|c: char| c.is_ascii_lowercase())
            || !l
                .source
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(invalid("load.source", "minúsculas, números y '_' (1-32)"));
        }
        // 60 pedidos en 10 minutos (límite de IBKR para velas chicas): nunca menos de 10 s.
        if !(10..=600).contains(&l.pause_secs) {
            return Err(invalid("load.pause_secs", "tiene que estar entre 10 y 600"));
        }
        if !(1..=60).contains(&l.max_requests) {
            return Err(invalid("load.max_requests", "tiene que estar entre 1 y 60"));
        }
        if !(1..=3650).contains(&l.backfill_days) {
            return Err(invalid(
                "load.backfill_days",
                "tiene que estar entre 1 y 3650",
            ));
        }
        // Tabla de duraciones de IBKR: 1 min hasta 1 día, 15 min hasta 1 semana, 1 h hasta 1 mes.
        let win = |field, v: u32, max: u32| {
            if (1..=max).contains(&v) {
                Ok(chrono::TimeDelta::hours(i64::from(v)))
            } else {
                Err(invalid(
                    field,
                    format!("tiene que estar entre 1 y {max} horas"),
                ))
            }
        };
        // Atraso de los datos de IBKR sin suscripción (~10 min, #541/#563): cota para la última vela.
        if !(60..=7200).contains(&l.data_delay_secs) {
            return Err(invalid(
                "load.data_delay_secs",
                "tiene que estar entre 60 y 7200",
            ));
        }
        let load = crate::load::LoadConfig {
            data_delay: chrono::TimeDelta::seconds(l.data_delay_secs as i64),
            pause: Duration::from_secs(l.pause_secs),
            max_requests: l.max_requests,
            backfill: chrono::TimeDelta::days(i64::from(l.backfill_days)),
            window_1m: win("load.window_1m_hours", l.window_1m_hours, 24)?,
            window_15m: win("load.window_15m_hours", l.window_15m_hours, 24 * 7)?,
            window_1h: win("load.window_1h_hours", l.window_1h_hours, 24 * 30)?,
            source: l.source,
        };
        let live = match r.live {
            None => None,
            Some(l) => Some(parse_live(l, g.client_id, &instruments)?),
        };
        let load_health = r.load_health.map(parse_health).transpose()?;
        Ok(Self {
            load,
            load_health,
            live,
            host: g.host,
            port: g.port,
            client_id: g.client_id,
            connect_timeout: secs("gateway.connect_timeout_secs", g.connect_timeout_secs, 120)?,
            request_timeout: secs("gateway.request_timeout_secs", g.request_timeout_secs, 600)?,
            watch_interval: secs("gateway.watch_interval_secs", g.watch_interval_secs, 300)?,
            forbidden_ports: g.forbidden_ports,
            paper_account_env: env,
            timeframe,
            lookback: chrono::TimeDelta::hours(i64::from(h.lookback_hours)),
            what_to_show,
            regular_hours_only: h.regular_hours_only,
            source: h.source,
            instruments,
        })
    }

    /// Vuelve a mirar el puerto justo antes de abrir el socket (la configuración es pública y se
    /// puede haber cambiado después de validarla).
    pub fn check_port(&self) -> Result<(), IbkrConfigError> {
        if is_forbidden_port(self.port, &self.forbidden_ports) {
            Err(invalid(
                "gateway.port",
                format!("{} es un puerto de sesión real: no se conecta", self.port),
            ))
        } else {
            Ok(())
        }
    }

    pub fn instrument(&self, id: &InstrumentId) -> Option<&FutureSpec> {
        self.instruments.iter().find(|s| &s.id == id)
    }
}

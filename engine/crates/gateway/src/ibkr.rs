//! Velas de IBKR para la terminal de la app (#551), **solo lectura**:
//!
//! | Ruta | Contrato |
//! |---|---|
//! | `GET /api/v1/ibkr/bars?instrument=<id>&timeframe=1m\|15m\|1h[&limit=<n>][&before=<UTC>]` | `gateway_ibkr_bars.v1` |
//! | `GET /api/v1/ibkr/watchlist` | `gateway_ibkr_watchlist.v1` |
//!
//! Lee `ibkr_bars` (#536) tal como está guardada: solo velas **cerradas** y de una sola serie (un
//! contrato, un tamaño y un precio), nunca mezcla contratos. El resumen de la watchlist es
//! presentación de dos precios (último y cierre de una sesión anterior); el día de sesión es el de
//! [`cfds_market_data::session_day`], el mismo de la terminal `bolsa`. No calcula indicadores ni
//! señales, no envía órdenes ni escribe en la base.

use std::future::Future;

use cfds_core::InstrumentId;
use cfds_market_data::SessionOffsets;
use cfds_market_data::session_day::{change, change_pct, session_date, session_start};
use cfds_storage::{IbkrBarRow, IbkrSeries, PgStore};
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::Reply;

const BARS: &str = "/api/v1/ibkr/bars";
const WATCHLIST: &str = "/api/v1/ibkr/watchlist";

/// Tamaños de vela que se sirven, en el orden en que se ofrecen.
pub const TIMEFRAMES: [&str; 3] = ["1m", "15m", "1h"];

/// Duración de una vela de ese tamaño.
pub fn bar_length(size: &str) -> Option<TimeDelta> {
    match size {
        "1m" => Some(TimeDelta::minutes(1)),
        "15m" => Some(TimeDelta::minutes(15)),
        "1h" => Some(TimeDelta::hours(1)),
        _ => None,
    }
}

/// `[ibkr]` de `config/gateway.toml`, más el día de sesión de `[session]` de
/// `config/terminal.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IbkrViewConfig {
    pub source: String,
    pub instruments: Vec<InstrumentId>,
    pub default_bars: u32,
    pub max_bars: u32,
    pub session: SessionOffsets,
    /// Vela en formación (#558): con `true` el gateway abre su conexión de streaming a IB Gateway
    /// (`[live]` de `config/ibkr.toml`). Con `false`, `/api/v1/ibkr/live` responde "disabled".
    pub live: bool,
    /// Conexiones SSE abiertas a la vez, como máximo.
    pub live_max_clients: usize,
    /// Cada cuánto va un comentario SSE para que la conexión no se corte sin mensajes.
    pub live_heartbeat: std::time::Duration,
}

impl IbkrViewConfig {
    pub fn from_toml(gateway: &str, terminal: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        struct Raw {
            ibkr: RawIbkr,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawIbkr {
            source: String,
            instruments: Vec<String>,
            default_bars: u32,
            max_bars: u32,
            live: bool,
            live_max_clients: usize,
            live_heartbeat_secs: u64,
        }
        let r = toml::from_str::<Raw>(gateway)
            .map_err(|e| e.to_string())?
            .ibkr;
        let source_ok = r.source.len() <= 32
            && r.source.starts_with(|c: char| c.is_ascii_lowercase())
            && r.source
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !source_ok {
            return Err("ibkr.source: minúsculas, dígitos o _ (como ibkr_bars.source)".into());
        }
        if r.instruments.is_empty() {
            return Err("ibkr.instruments no puede estar vacío".into());
        }
        let mut instruments = Vec::with_capacity(r.instruments.len());
        for s in &r.instruments {
            let id: InstrumentId = s.parse().map_err(|e| format!("ibkr.instruments: {e}"))?;
            if instruments.contains(&id) {
                return Err(format!("ibkr.instruments: {id} repetido"));
            }
            instruments.push(id);
        }
        if r.default_bars == 0 || r.max_bars > 20_000 || r.default_bars > r.max_bars {
            return Err("ibkr: 0 < default_bars <= max_bars <= 20000".into());
        }
        if !(1..=256).contains(&r.live_max_clients) {
            return Err("ibkr.live_max_clients: entre 1 y 256".into());
        }
        if !(1..=60).contains(&r.live_heartbeat_secs) {
            return Err("ibkr.live_heartbeat_secs: entre 1 y 60".into());
        }
        Ok(Self {
            live: r.live,
            live_max_clients: r.live_max_clients,
            live_heartbeat: std::time::Duration::from_secs(r.live_heartbeat_secs),
            source: r.source,
            instruments,
            default_bars: r.default_bars,
            max_bars: r.max_bars,
            session: SessionOffsets::from_toml(terminal)
                .map_err(|e| format!("config/terminal.toml {e}"))?,
        })
    }
}

/// De dónde salen las velas. En producción, PostgreSQL (`ibkr_bars`).
pub trait IbkrBarSource: Sync {
    /// La última vela cerrada a `at` (de ese tamaño, o de cualquiera con `None`).
    fn latest_closed(
        &self,
        source: &str,
        instrument: &str,
        bar_size: Option<&str>,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<Option<IbkrBarRow>, String>> + Send;
    /// Las últimas `limit` velas de la serie cerradas a `at`, en orden.
    fn closed_by(
        &self,
        series: &IbkrSeries<'_>,
        at: DateTime<Utc>,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<IbkrBarRow>, String>> + Send;
    /// La última vela de la serie que empieza antes de `before`.
    fn starting_before(
        &self,
        series: &IbkrSeries<'_>,
        before: DateTime<Utc>,
    ) -> impl Future<Output = Result<Option<IbkrBarRow>, String>> + Send;
}

impl IbkrBarSource for PgStore {
    async fn latest_closed(
        &self,
        source: &str,
        instrument: &str,
        bar_size: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, String> {
        self.latest_closed_ibkr_bar(source, instrument, bar_size, at)
            .await
            .map_err(|e| e.to_string())
    }
    async fn closed_by(
        &self,
        series: &IbkrSeries<'_>,
        at: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<IbkrBarRow>, String> {
        self.ibkr_bars_closed_by(series, at, limit)
            .await
            .map_err(|e| e.to_string())
    }
    async fn starting_before(
        &self,
        series: &IbkrSeries<'_>,
        before: DateTime<Utc>,
    ) -> Result<Option<IbkrBarRow>, String> {
        self.last_ibkr_bar_starting_before(series, before)
            .await
            .map_err(|e| e.to_string())
    }
}

fn series(row: &IbkrBarRow) -> IbkrSeries<'_> {
    IbkrSeries {
        source: &row.source,
        instrument_id: &row.instrument_id,
        contract: &row.contract,
        bar_size: &row.bar_size,
        what_to_show: &row.what_to_show,
    }
}

fn utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

/// Una vela como `contracts/ibkr_bar.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BarOut {
    pub source: String,
    pub instrument_id: String,
    pub kind: String,
    pub contract: String,
    pub bar_size: String,
    pub what_to_show: String,
    pub start: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: Option<String>,
}

impl From<IbkrBarRow> for BarOut {
    fn from(r: IbkrBarRow) -> Self {
        Self {
            start: r.start.to_rfc3339_opts(SecondsFormat::Secs, true),
            open: r.open.to_string(),
            high: r.high.to_string(),
            low: r.low.to_string(),
            close: r.close.to_string(),
            volume: r.volume.map(|v| v.to_string()),
            source: r.source,
            instrument_id: r.instrument_id,
            kind: r.kind,
            contract: r.contract,
            bar_size: r.bar_size,
            what_to_show: r.what_to_show,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SeriesOut {
    pub contract: String,
    pub kind: String,
    pub what_to_show: String,
}

/// `contracts/gateway_ibkr_bars.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BarsReply {
    pub instrument_id: String,
    pub timeframe: String,
    pub source: String,
    pub as_of: String,
    pub generated_at: String,
    pub series: Option<SeriesOut>,
    pub bars: Vec<BarOut>,
}

/// El último precio de un instrumento (`quote` de `contracts/gateway_ibkr_watchlist.v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuoteOut {
    pub contract: String,
    pub kind: String,
    pub what_to_show: String,
    pub timeframe: String,
    pub last: String,
    pub bar_start: String,
    pub bar_end: String,
    pub delay_seconds: i64,
    pub session: String,
    pub previous_session: Option<String>,
    pub previous_close: Option<String>,
    pub change: Option<String>,
    pub change_pct: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchEntry {
    pub instrument_id: String,
    pub quote: Option<QuoteOut>,
}

/// `contracts/gateway_ibkr_watchlist.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchlistReply {
    pub generated_at: String,
    pub source: String,
    pub instruments: Vec<WatchEntry>,
}

/// Arma el último precio a partir de la última vela cerrada y la de referencia (la última de la
/// misma serie con un día de sesión anterior). Función pura.
pub fn quote(
    last: &IbkrBarRow,
    reference: Option<&IbkrBarRow>,
    session: &SessionOffsets,
    now: DateTime<Utc>,
) -> Option<QuoteOut> {
    let len = bar_length(&last.bar_size)?;
    let offset = session.offset(last.kind == "stock");
    let day = session_date(last.start, offset);
    let end = last.start + len;
    // La referencia tiene que ser de una sesión anterior y de la misma serie.
    let reference = reference.filter(|r| {
        session_date(r.start, offset) < day
            && r.contract == last.contract
            && r.bar_size == last.bar_size
            && r.what_to_show == last.what_to_show
    });
    Some(QuoteOut {
        contract: last.contract.clone(),
        kind: last.kind.clone(),
        what_to_show: last.what_to_show.clone(),
        timeframe: last.bar_size.clone(),
        last: last.close.to_string(),
        bar_start: utc(last.start),
        bar_end: utc(end),
        delay_seconds: (now - end).num_seconds().max(0),
        session: day.to_string(),
        previous_session: reference.map(|r| session_date(r.start, offset).to_string()),
        previous_close: reference.map(|r| r.close.to_string()),
        change: reference.map(|r| change(last.close, r.close).to_string()),
        change_pct: reference
            .and_then(|r| change_pct(last.close, r.close))
            .map(|p| p.to_string()),
    })
}

/// `true` si la ruta es de este módulo.
pub fn is_ibkr_route(path: &str) -> bool {
    matches!(path, BARS | WATCHLIST)
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

fn json<T: Serialize>(v: &T) -> Reply {
    match serde_json::to_string(v) {
        Ok(body) => Reply {
            status: 200,
            content_type: "application/json",
            body,
        },
        Err(e) => {
            eprintln!("gateway: no se pudieron serializar las velas de IBKR: {e}");
            text(500, "error interno")
        }
    }
}

fn internal(e: String) -> Reply {
    eprintln!("gateway: velas de IBKR: {e}");
    text(500, "error interno")
}

/// Atiende las rutas de IBKR. Solo `GET`.
pub async fn respond_ibkr<S: IbkrBarSource>(
    store: &S,
    cfg: &IbkrViewConfig,
    method: &str,
    path: &str,
    query: Option<&str>,
    now: DateTime<Utc>,
) -> Reply {
    if !is_ibkr_route(path) {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: las velas de IBKR son de solo lectura");
    }
    if path == WATCHLIST {
        return watchlist(store, cfg, now).await;
    }
    let Some(Ok(id)) = crate::param(query, "instrument").map(str::parse::<InstrumentId>) else {
        return text(400, "falta ?instrument=<id> válido");
    };
    let Some(tf) = crate::param(query, "timeframe").filter(|t| TIMEFRAMES.contains(t)) else {
        return text(400, "timeframe: 1m, 15m o 1h");
    };
    let limit = match crate::param(query, "limit") {
        None => cfg.default_bars,
        Some(s) => match s.parse::<u32>() {
            Ok(n) if (1..=cfg.max_bars).contains(&n) => n,
            _ => {
                return text(400, &format!("limit: entero entre 1 y {}", cfg.max_bars));
            }
        },
    };
    // `before`: las velas tal como se conocían en ese momento, nunca después de ahora.
    let at = match crate::param(query, "before") {
        None => now,
        Some(s) => match crate::parse_before(s) {
            Some(t) => t.min(now),
            None => return text(400, "before: fecha RFC 3339 en UTC (…Z)"),
        },
    };
    if !cfg.instruments.contains(&id) {
        return text(404, "instrumento fuera de [ibkr] instruments");
    }
    let last = match store
        .latest_closed(&cfg.source, id.as_str(), Some(tf), at)
        .await
    {
        Ok(l) => l,
        Err(e) => return internal(e),
    };
    let (series_out, bars) = match &last {
        None => (None, Vec::new()),
        Some(l) => match store.closed_by(&series(l), at, limit).await {
            Ok(rows) => (
                Some(SeriesOut {
                    contract: l.contract.clone(),
                    kind: l.kind.clone(),
                    what_to_show: l.what_to_show.clone(),
                }),
                rows.into_iter().map(BarOut::from).collect(),
            ),
            Err(e) => return internal(e),
        },
    };
    json(&BarsReply {
        instrument_id: id.as_str().to_owned(),
        timeframe: tf.to_owned(),
        source: cfg.source.clone(),
        as_of: utc(at),
        generated_at: utc(now),
        series: series_out,
        bars,
    })
}

async fn watchlist<S: IbkrBarSource>(store: &S, cfg: &IbkrViewConfig, now: DateTime<Utc>) -> Reply {
    let mut instruments = Vec::with_capacity(cfg.instruments.len());
    for id in &cfg.instruments {
        let last = match store
            .latest_closed(&cfg.source, id.as_str(), None, now)
            .await
        {
            Ok(l) => l,
            Err(e) => return internal(e),
        };
        let quote_out = match &last {
            None => None,
            Some(l) => {
                let offset = cfg.session.offset(l.kind == "stock");
                let from = session_start(session_date(l.start, offset), offset);
                let reference = match store.starting_before(&series(l), from).await {
                    Ok(r) => r,
                    Err(e) => return internal(e),
                };
                quote(l, reference.as_ref(), &cfg.session, now)
            }
        };
        instruments.push(WatchEntry {
            instrument_id: id.as_str().to_owned(),
            quote: quote_out,
        });
    }
    json(&WatchlistReply {
        generated_at: utc(now),
        source: cfg.source.clone(),
        instruments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GATEWAY: &str = include_str!("../../../../config/gateway.toml");
    const TERMINAL: &str = include_str!("../../../../config/terminal.toml");
    const IBKR: &str = include_str!("../../../../config/ibkr.toml");

    #[test]
    fn reads_the_repo_config_and_matches_config_ibkr() {
        let c = IbkrViewConfig::from_toml(GATEWAY, TERMINAL).unwrap();
        let ibkr: toml::Table = toml::from_str(IBKR).unwrap();
        // La fuente es la que guarda la carga (`[load] source`).
        assert_eq!(
            ibkr["load"]["source"].as_str(),
            Some(c.source.as_str()),
            "[ibkr] source de gateway.toml tiene que ser [load] source de ibkr.toml"
        );
        let known: Vec<&str> = ibkr["instruments"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i.get("kind").and_then(|k| k.as_str()) != Some("continuous_future"))
            .map(|i| i["id"].as_str().unwrap())
            .collect();
        for id in &c.instruments {
            assert!(known.contains(&id.as_str()), "{id} no está en ibkr.toml");
        }
        assert_eq!(c.session.offset(true), TimeDelta::hours(-5));
    }

    #[test]
    fn rejects_bad_config() {
        let ok = IbkrViewConfig::from_toml(GATEWAY, TERMINAL).unwrap();
        for (from, to) in [
            ("max_bars = 5000", "max_bars = 0"),
            ("default_bars = 500", "default_bars = 0"),
            ("source = \"ibkr\"", "source = \"IBKR\""),
            (
                "instruments = [\"MNQ\", \"MES\", \"QQQ\"]",
                "instruments = []",
            ),
            (
                "instruments = [\"MNQ\", \"MES\", \"QQQ\"]",
                "instruments = [\"MNQ\", \"MNQ\"]",
            ),
            ("max_bars = 5000", "max_bars = 5000\nextra = 1"),
        ] {
            let bad = GATEWAY.replace(from, to);
            assert_ne!(bad, GATEWAY, "{from}");
            assert!(IbkrViewConfig::from_toml(&bad, TERMINAL).is_err(), "{to}");
        }
        assert_eq!(ok.max_bars, 5000);
        let bad_session = TERMINAL.replace("stock_offset_hours = -5", "stock_offset_hours = 20");
        assert!(IbkrViewConfig::from_toml(GATEWAY, &bad_session).is_err());
    }
}

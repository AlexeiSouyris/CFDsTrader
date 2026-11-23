//! Gateway: API **de solo lectura** del motor para la UI (#231). Hoy expone una sola ruta,
//! `GET /api/v1/liquidity?instrument=<id>`, que devuelve `contracts/liquidity_view.v1`: lo que
//! ya está guardado en PostgreSQL (velas, niveles, FVG, desequilibrio y agregados), sin calcular
//! nada nuevo. No envía órdenes ni escribe en la base.

use std::collections::BTreeSet;
use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroU32;

use cfds_core::{
    Candle, FvgEvent, Imbalance, InstrumentId, LiquidityLevel, LiquidityStats, StatsTimeframe,
    Timeframe,
};
use cfds_storage::{PgStore, StorageError};
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

/// `config/gateway.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayConfig {
    pub bind: SocketAddr,
    pub window: LiquidityWindow,
}

/// Ventana de la vista de liquidez, hacia atrás desde `as_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiquidityWindow {
    pub candles: TimeDelta,
    pub fvgs: TimeDelta,
    pub levels: TimeDelta,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    server: RawServer,
    liquidity: RawLiquidity,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawServer {
    bind: SocketAddr,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLiquidity {
    candles_hours: NonZeroU32,
    fvg_days: NonZeroU32,
    levels_days: NonZeroU32,
}

impl GatewayConfig {
    /// Lee y valida `config/gateway.toml`: todos los campos, positivos y sin campos de más.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
        let l = raw.liquidity;
        Ok(Self {
            bind: raw.server.bind,
            window: LiquidityWindow {
                candles: TimeDelta::hours(i64::from(l.candles_hours.get())),
                fvgs: TimeDelta::days(i64::from(l.fvg_days.get())),
                levels: TimeDelta::days(i64::from(l.levels_days.get())),
            },
        })
    }
}

/// `contracts/liquidity_view.v1`: mensajes de otros contratos, sin cambios.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LiquidityView {
    pub instrument_id: InstrumentId,
    pub generated_at: String,
    pub as_of: Option<String>,
    pub candles: Vec<Candle>,
    pub levels: Vec<LiquidityLevel>,
    pub fvgs: Vec<FvgEvent>,
    pub imbalance: Vec<Imbalance>,
    pub stats: Vec<LiquidityStats>,
}

fn utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

impl LiquidityView {
    /// La vista de un instrumento sin velas guardadas: "sin datos todavía".
    pub fn empty(instrument: InstrumentId, now: DateTime<Utc>) -> Self {
        Self {
            instrument_id: instrument,
            generated_at: utc(now),
            as_of: None,
            candles: Vec::new(),
            levels: Vec::new(),
            fvgs: Vec::new(),
            imbalance: Vec::new(),
            stats: Vec::new(),
        }
    }
}

/// De dónde sale la vista. En producción, PostgreSQL ([`PgSource`]); en los tests, uno falso.
pub trait LiquiditySource: Sync {
    fn view(
        &self,
        instrument: &InstrumentId,
        now: DateTime<Utc>,
    ) -> impl Future<Output = Result<LiquidityView, StorageError>> + Send;
}

/// La vista leída de PostgreSQL con las lecturas de `cfds-storage`, todas "tal como se conocían
/// en `as_of`" (sin mirar el futuro).
pub struct PgSource {
    pub store: PgStore,
    pub window: LiquidityWindow,
    /// `[fvg_stats] reaction_bars` de `config/liquidity.toml`.
    pub reaction_bars: Vec<NonZeroU32>,
}

impl LiquiditySource for PgSource {
    async fn view(
        &self,
        id: &InstrumentId,
        now: DateTime<Utc>,
    ) -> Result<LiquidityView, StorageError> {
        let tf = Timeframe::M15;
        // Solo velas cerradas: la última que empezó a más tardar `now − 15m`.
        let Some(last) = self
            .store
            .last_candle_start(id, tf, now - tf.duration())
            .await?
        else {
            return Ok(LiquidityView::empty(id.clone(), now));
        };
        let as_of = last + tf.duration();
        let w = self.window;
        let candles = self.store.candles(id, tf, as_of - w.candles, as_of).await?;
        let first = candles.first().map_or(as_of, |c| c.start);
        let s = &self.store;
        let levels = s.liquidity_levels(id, as_of - w.levels, as_of).await?;
        let fvgs = s
            .fvg_events(
                id,
                StatsTimeframe::M15,
                as_of - w.fvgs,
                as_of,
                &self.reaction_bars,
            )
            .await?;
        let imbalance = s.imbalances(id, StatsTimeframe::M15, first, as_of).await?;
        let stats = s.latest_liquidity_stats(id, as_of).await?;
        Ok(LiquidityView {
            instrument_id: id.clone(),
            generated_at: utc(now),
            as_of: Some(utc(as_of)),
            candles,
            levels,
            fvgs,
            imbalance,
            stats,
        })
    }
}

/// Una respuesta HTTP ya armada: estado, tipo de contenido y cuerpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain; charset=utf-8",
        body: body.to_owned(),
    }
}

/// Atiende un pedido. Solo `GET /api/v1/liquidity?instrument=<id>` con un instrumento del
/// catálogo; todo lo demás es 4xx. Los errores de la base no se muestran al cliente.
pub async fn respond<S: LiquiditySource>(
    source: &S,
    catalog: &BTreeSet<InstrumentId>,
    method: &str,
    path: &str,
    query: Option<&str>,
    now: DateTime<Utc>,
) -> Reply {
    if path != "/api/v1/liquidity" {
        return text(404, "ruta desconocida");
    }
    if method != "GET" {
        return text(405, "solo GET: el gateway es de solo lectura");
    }
    let instrument = query
        .unwrap_or("")
        .split('&')
        .find_map(|kv| kv.strip_prefix("instrument="));
    let Some(Ok(id)) = instrument.map(str::parse::<InstrumentId>) else {
        return text(400, "falta ?instrument=<id> válido");
    };
    if !catalog.contains(&id) {
        return text(404, "instrumento fuera del catálogo");
    }
    match source.view(&id, now).await {
        Ok(view) => match serde_json::to_string(&view) {
            Ok(body) => Reply {
                status: 200,
                content_type: "application/json",
                body,
            },
            Err(e) => {
                eprintln!("gateway: no se pudo serializar la vista de {id}: {e}");
                text(500, "error interno")
            }
        },
        Err(e) => {
            eprintln!("gateway: error leyendo la vista de {id}: {e}");
            text(500, "error interno")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = include_str!("../../../../config/gateway.toml");

    struct Fake(Option<LiquidityView>);

    impl LiquiditySource for Fake {
        async fn view(
            &self,
            id: &InstrumentId,
            now: DateTime<Utc>,
        ) -> Result<LiquidityView, StorageError> {
            Ok(self
                .0
                .clone()
                .unwrap_or_else(|| LiquidityView::empty(id.clone(), now)))
        }
    }

    fn now() -> DateTime<Utc> {
        "2026-11-23T10:00:00Z".parse().unwrap()
    }

    fn catalog() -> BTreeSet<InstrumentId> {
        ["US100".parse().unwrap()].into()
    }

    #[test]
    fn reads_the_repo_config() {
        let cfg = GatewayConfig::from_toml(CONFIG).unwrap();
        assert!(
            cfg.bind.ip().is_loopback(),
            "por defecto solo la máquina local"
        );
        assert_eq!(cfg.window.candles, TimeDelta::hours(72));
    }

    #[test]
    fn rejects_zero_and_unknown_fields() {
        let zero = CONFIG.replace("candles_hours = 72", "candles_hours = 0");
        assert!(GatewayConfig::from_toml(&zero).is_err());
        let extra = format!("{CONFIG}\nnope = 1\n");
        assert!(GatewayConfig::from_toml(&extra).is_err());
    }

    #[tokio::test]
    async fn empty_database_is_sin_datos() {
        let r = respond(
            &Fake(None),
            &catalog(),
            "GET",
            "/api/v1/liquidity",
            Some("instrument=US100"),
            now(),
        )
        .await;
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
        assert_eq!(v["as_of"], serde_json::Value::Null);
        assert_eq!(v["candles"], serde_json::json!([]));
        assert_eq!(v["generated_at"], "2026-11-23T10:00:00Z");
    }

    #[tokio::test]
    async fn only_get_on_the_route_with_a_catalog_instrument() {
        let f = Fake(None);
        let c = catalog();
        let q = Some("instrument=US100");
        let path = "/api/v1/liquidity";
        assert_eq!(respond(&f, &c, "POST", path, q, now()).await.status, 405);
        assert_eq!(respond(&f, &c, "GET", "/otra", q, now()).await.status, 404);
        assert_eq!(respond(&f, &c, "GET", path, None, now()).await.status, 400);
        let bad = Some("instrument=us100;drop");
        assert_eq!(respond(&f, &c, "GET", path, bad, now()).await.status, 400);
        let other = Some("instrument=DE40");
        assert_eq!(respond(&f, &c, "GET", path, other, now()).await.status, 404);
    }
}

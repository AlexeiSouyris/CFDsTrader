//! Sesión de solo lectura con las guardas del ADR 0017 §2: cuentas de la sesión, desconexión y
//! errores de datos activan el kill switch. Trabaja contra [`IbkrApi`] (el cliente real está en
//! [`crate::client`]; los tests usan uno falso, sin red).
//!
//! El kill switch es el de `paper_account.v1` ([`cfds_core::KillSwitch`]): una vez activo, la
//! sesión no lee nada más y **no se desactiva desde acá** (spec §1.2). Las alertas las manda
//! quien usa la sesión, con [`cfds_obs::Alerter`] y el [`Trip`] que devuelve.

use std::future::Future;
use std::sync::Mutex;

use cfds_core::{Candle, InstrumentId, KillSwitch, Timeframe};
use cfds_market_data::{MarketDataProvider, ProviderError};
use cfds_obs::AlertKind;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::account::{PaperAccountId, check_session_accounts, mask};
use crate::config::{FutureSpec, IbkrConfig, WhatToShow};

/// Error del cliente de IBKR.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// Se cortó la conexión con IB Gateway (o nunca se abrió).
    #[error("conexión con IB Gateway perdida: {0}")]
    ConnectionLost(String),
    /// Llegó algo que no se puede convertir (un precio no finito, una hora sin fecha, ...).
    #[error("dato inválido de IBKR: {0}")]
    Data(String),
    /// IBKR rechazó el pedido (contrato desconocido, sin permiso de datos, ...).
    #[error("IBKR rechazó el pedido: {0}")]
    Rejected(String),
}

/// Pedido de velas de un futuro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarsRequest {
    pub spec: FutureSpec,
    pub timeframe: Timeframe,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub what_to_show: WhatToShow,
    pub regular_hours_only: bool,
}

/// Una vela de IBKR ya convertida a decimales exactos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawBar {
    pub start: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
}

/// Velas de un contrato concreto (por ejemplo `MNQZ6`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarsResponse {
    pub contract: String,
    pub bars: Vec<RawBar>,
}

/// Un valor del resumen de cuenta, tal como lo manda IBKR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryItem {
    pub account: String,
    pub tag: String,
    pub value: String,
    pub currency: String,
}

/// Lo que el adaptador necesita de IBKR, **solo lectura**. No hay ningún método para órdenes.
pub trait IbkrApi: Send + Sync {
    fn is_connected(&self) -> bool;
    /// Todas las cuentas de la sesión.
    fn managed_accounts(&self) -> impl Future<Output = Result<Vec<String>, ApiError>> + Send;
    fn account_summary(&self) -> impl Future<Output = Result<Vec<SummaryItem>, ApiError>> + Send;
    fn bars(
        &self,
        req: &BarsRequest,
    ) -> impl Future<Output = Result<BarsResponse, ApiError>> + Send;
    fn disconnect(&self) -> impl Future<Output = ()> + Send;
}

/// Por qué se activó el kill switch, para la alerta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trip {
    pub kind: AlertKind,
    pub reason: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    /// El kill switch está activo (se activó ahora o antes): no se lee nada más.
    #[error("kill switch activo: {}", .0.reason)]
    KillSwitch(Trip),
    #[error("{0} no está en config/ibkr.toml")]
    UnknownInstrument(InstrumentId),
    #[error(transparent)]
    Rejected(ApiError),
}

/// Resumen de la cuenta paper, en decimales exactos.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountSummary {
    /// Cuenta, tapada.
    pub account: String,
    pub account_type: Option<String>,
    pub currency: Option<String>,
    pub net_liquidation: Option<Decimal>,
    pub total_cash: Option<Decimal>,
    pub available_funds: Option<Decimal>,
}

/// Velas cerradas con su fuente y el contrato del que salen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcedCandles {
    pub source: String,
    pub contract: String,
    pub candles: Vec<Candle>,
}

struct State {
    kill_switch: KillSwitch,
    trip: Option<Trip>,
}

pub struct ReadOnlySession<A: IbkrApi> {
    api: A,
    cfg: IbkrConfig,
    paper: PaperAccountId,
    state: Mutex<State>,
}

/// Desconecta y arma el motivo (para los rechazos antes de tener una sesión).
async fn refuse<A: IbkrApi>(api: &A, kind: AlertKind, reason: String, at: DateTime<Utc>) -> Trip {
    api.disconnect().await;
    Trip { kind, reason, at }
}

impl<A: IbkrApi> ReadOnlySession<A> {
    /// Abre la sesión: solo si la conexión está viva y **todas** las cuentas de la sesión son la
    /// cuenta paper de `.env`. Si no, desconecta y devuelve el motivo (kill switch + alerta).
    pub async fn open(
        api: A,
        cfg: IbkrConfig,
        paper: PaperAccountId,
        now: DateTime<Utc>,
    ) -> Result<Self, Trip> {
        if !api.is_connected() {
            let reason = "IB Gateway no está conectado".to_owned();
            return Err(refuse(&api, AlertKind::Disconnect, reason, now).await);
        }
        let accounts = match api.managed_accounts().await {
            Ok(a) => a,
            Err(ApiError::ConnectionLost(e)) => {
                let reason = format!("se cortó la conexión al leer las cuentas: {e}");
                return Err(refuse(&api, AlertKind::Disconnect, reason, now).await);
            }
            Err(e) => {
                let reason = format!("no se pudieron leer las cuentas de la sesión: {e}");
                return Err(refuse(&api, AlertKind::KillSwitch, reason, now).await);
            }
        };
        if let Err(reason) = check_session_accounts(&accounts, &paper) {
            return Err(refuse(&api, AlertKind::KillSwitch, reason, now).await);
        }
        Ok(Self {
            api,
            cfg,
            paper,
            state: Mutex::new(State {
                kill_switch: KillSwitch::Inactive,
                trip: None,
            }),
        })
    }

    pub fn config(&self) -> &IbkrConfig {
        &self.cfg
    }

    pub fn paper_account(&self) -> &PaperAccountId {
        &self.paper
    }

    pub fn kill_switch(&self) -> KillSwitch {
        self.lock().kill_switch.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        // Un pánico con el lock tomado no deja el estado a medias (solo se asignan campos).
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn active_trip(&self) -> Option<Trip> {
        self.lock().trip.clone()
    }

    /// Activa el kill switch (si no estaba), desconecta y devuelve el error.
    async fn trip(&self, kind: AlertKind, reason: String, at: DateTime<Utc>) -> SessionError {
        let trip = {
            let mut s = self.lock();
            if s.trip.is_none() {
                s.kill_switch = KillSwitch::Active {
                    reason: reason.chars().take(300).collect(),
                    since: at,
                };
                s.trip = Some(Trip { kind, reason, at });
            }
            s.trip.clone()
        };
        self.api.disconnect().await;
        // `trip` siempre está: se acaba de poner si faltaba.
        SessionError::KillSwitch(trip.unwrap_or(Trip {
            kind,
            reason: String::new(),
            at,
        }))
    }

    async fn api_error(&self, e: ApiError, at: DateTime<Utc>) -> SessionError {
        match e {
            ApiError::ConnectionLost(m) => {
                self.trip(
                    AlertKind::Disconnect,
                    format!("conexión con IB Gateway perdida: {m}"),
                    at,
                )
                .await
            }
            ApiError::Data(m) => {
                self.trip(
                    AlertKind::DataError,
                    format!("dato inválido de IBKR: {m}"),
                    at,
                )
                .await
            }
            e @ ApiError::Rejected(_) => SessionError::Rejected(e),
        }
    }

    /// Antes de cada lectura: kill switch inactivo y conexión viva. Una desconexión activa el kill
    /// switch (ADR 0017 §2.6); no hay reconexión automática.
    pub async fn check_connection(&self, now: DateTime<Utc>) -> Result<(), SessionError> {
        if let Some(t) = self.active_trip() {
            return Err(SessionError::KillSwitch(t));
        }
        if !self.api.is_connected() {
            return Err(self
                .trip(
                    AlertKind::Disconnect,
                    "se perdió la conexión con IB Gateway".into(),
                    now,
                )
                .await);
        }
        Ok(())
    }

    /// Resumen de la cuenta paper.
    pub async fn account_summary(
        &self,
        now: DateTime<Utc>,
    ) -> Result<AccountSummary, SessionError> {
        self.check_connection(now).await?;
        let items = match self.api.account_summary().await {
            Ok(i) => i,
            Err(e) => return Err(self.api_error(e, now).await),
        };
        let mut out = AccountSummary {
            account: mask(self.paper.as_str()),
            ..AccountSummary::default()
        };
        for it in items {
            if it.account.trim() != self.paper.as_str() {
                let reason = format!(
                    "el resumen trae otra cuenta ({}): ¿sesión real?",
                    mask(it.account.trim())
                );
                return Err(self.trip(AlertKind::KillSwitch, reason, now).await);
            }
            let num = |v: &str| v.trim().parse::<Decimal>();
            let parsed = match it.tag.as_str() {
                "AccountType" => {
                    out.account_type = Some(it.value.clone());
                    Ok(())
                }
                "NetLiquidation" => num(&it.value).map(|d| out.net_liquidation = Some(d)),
                "TotalCashValue" => num(&it.value).map(|d| out.total_cash = Some(d)),
                "AvailableFunds" => num(&it.value).map(|d| out.available_funds = Some(d)),
                _ => Ok(()),
            };
            if parsed.is_err() {
                let reason = format!("{} no es un número: {:?}", it.tag, it.value);
                return Err(self.trip(AlertKind::DataError, reason, now).await);
            }
            if !it.currency.is_empty() {
                out.currency = Some(it.currency);
            }
        }
        Ok(out)
    }

    /// Velas **cerradas** en `now` de `instrument`, con inicio en `[from, to)`, con su fuente.
    pub async fn closed_candles(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<SourcedCandles, SessionError> {
        let spec = self
            .cfg
            .instrument(instrument)
            .cloned()
            .ok_or_else(|| SessionError::UnknownInstrument(instrument.clone()))?;
        self.check_connection(now).await?;
        let req = BarsRequest {
            spec,
            timeframe,
            from,
            to,
            what_to_show: self.cfg.what_to_show,
            regular_hours_only: self.cfg.regular_hours_only,
        };
        let resp = match self.api.bars(&req).await {
            Ok(r) => r,
            Err(e) => return Err(self.api_error(e, now).await),
        };
        let mut candles: Vec<Candle> = Vec::with_capacity(resp.bars.len());
        for b in resp.bars {
            if let Err(problem) = sane(&b, candles.last().map(|c| c.start)) {
                let reason = format!("{instrument} {}: {problem}", b.start.to_rfc3339());
                return Err(self.trip(AlertKind::DataError, reason, now).await);
            }
            let c = Candle {
                instrument_id: instrument.clone(),
                timeframe,
                start: b.start,
                open: b.open,
                high: b.high,
                low: b.low,
                close: b.close,
                volume: b.volume,
            };
            // Solo velas cerradas (CLAUDE.md §5b): la vela en curso nunca sale de acá.
            if from <= c.start && c.start < to && c.is_closed_at(now) {
                candles.push(c);
            }
        }
        Ok(SourcedCandles {
            source: self.cfg.source.clone(),
            contract: resp.contract,
            candles,
        })
    }

    /// Cierra la conexión (sin activar el kill switch).
    pub async fn close(self) {
        self.api.disconnect().await;
    }
}

/// Controles mínimos de una vela de IBKR: precios positivos, máximo y mínimo coherentes, volumen
/// no negativo y en orden estricto.
fn sane(b: &RawBar, prev_start: Option<DateTime<Utc>>) -> Result<(), &'static str> {
    if [b.open, b.high, b.low, b.close]
        .iter()
        .any(|p| *p <= Decimal::ZERO)
    {
        return Err("precio no positivo");
    }
    if b.low > b.open.min(b.close) || b.high < b.open.max(b.close) || b.low > b.high {
        return Err("máximo o mínimo incoherente");
    }
    if b.volume.is_some_and(|v| v < Decimal::ZERO) {
        return Err("volumen negativo");
    }
    if prev_start.is_some_and(|p| b.start <= p) {
        return Err("velas fuera de orden o repetidas");
    }
    Ok(())
}

impl<A: IbkrApi> MarketDataProvider for ReadOnlySession<A> {
    /// Solo velas cerradas a la hora actual. Un kill switch activo o un error de IBKR es un error
    /// del proveedor.
    async fn history(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Candle>, ProviderError> {
        match self
            .closed_candles(instrument, timeframe, from, to, Utc::now())
            .await
        {
            Ok(s) => Ok(s.candles),
            Err(SessionError::UnknownInstrument(i)) => Err(ProviderError::NotFound(i.to_string())),
            Err(e @ SessionError::KillSwitch(_)) => Err(ProviderError::Transport(e.to_string())),
            Err(SessionError::Rejected(e)) => Err(ProviderError::InvalidResponse(e.to_string())),
        }
    }
}

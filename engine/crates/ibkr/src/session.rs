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
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;

use crate::account::{PaperAccountId, check_session_accounts, mask};
use crate::config::{FutureSpec, IbkrConfig, WhatToShow};
use crate::live::{LiveDataType, QuoteEvent};

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
    /// La cuenta no tiene permiso para ese tipo de precios (por ejemplo, en vivo sin suscripción:
    /// errores 354, 420, 10089-10091, 10168). Es un rechazo: no activa el kill switch.
    #[error("sin permiso de datos: {0}")]
    NoPermission(String),
}

/// Tamaño de vela que se le pide a IBKR (más que los `Timeframe` del motor: para medir y guardar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BarSize {
    M1,
    M15,
    H1,
    /// Diaria: IBKR la fecha por día de sesión, sin hora (se toma la medianoche UTC de esa fecha).
    D1,
}

impl BarSize {
    pub const ALL: [BarSize; 4] = [BarSize::M1, BarSize::M15, BarSize::H1, BarSize::D1];

    pub fn as_str(self) -> &'static str {
        match self {
            BarSize::M1 => "1m",
            BarSize::M15 => "15m",
            BarSize::H1 => "1h",
            BarSize::D1 => "1d",
        }
    }

    pub fn duration(self) -> TimeDelta {
        match self {
            BarSize::M1 => TimeDelta::minutes(1),
            BarSize::M15 => TimeDelta::minutes(15),
            BarSize::H1 => TimeDelta::hours(1),
            BarSize::D1 => TimeDelta::days(1),
        }
    }
}

impl From<Timeframe> for BarSize {
    fn from(t: Timeframe) -> Self {
        match t {
            Timeframe::M15 => BarSize::M15,
            Timeframe::H1 => BarSize::H1,
        }
    }
}

impl std::str::FromStr for BarSize {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        BarSize::ALL
            .into_iter()
            .find(|b| b.as_str() == s)
            .ok_or_else(|| format!("tamaño de vela desconocido: {s:?} (1m, 15m, 1h o 1d)"))
    }
}

/// De qué contrato de un futuro: el vigente más cercano o un mes concreto (`AAAAMM`), que puede
/// estar vencido. Para los demás tipos se ignora.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ContractChoice {
    #[default]
    Front,
    Month(String),
}

/// Pedido de velas (o de la fecha más vieja con datos) de un instrumento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarsRequest {
    pub spec: FutureSpec,
    pub contract: ContractChoice,
    pub bar_size: BarSize,
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
    /// Fecha de la primera vela que tiene IBKR (`reqHeadTimeStamp`) y el contrato. Usa `spec`,
    /// `contract`, `what_to_show` y `regular_hours_only` del pedido.
    fn head_timestamp(
        &self,
        req: &BarsRequest,
    ) -> impl Future<Output = Result<(String, DateTime<Utc>), ApiError>> + Send;
    /// Precios por streaming (#558) del contrato de `req`, con el tipo de dato pedido: manda los
    /// eventos a `out` hasta que se corta (error) o `out` se cierra (`Ok`). Solo lectura.
    fn stream_quotes(
        &self,
        req: &QuoteRequest,
        out: tokio::sync::mpsc::Sender<QuoteEvent>,
    ) -> impl Future<Output = Result<(), ApiError>> + Send {
        let _ = (req, out);
        async {
            Err(ApiError::Rejected(
                "este cliente no tiene precios por streaming".into(),
            ))
        }
    }
    fn disconnect(&self) -> impl Future<Output = ()> + Send;
}

/// Pedido de precios por streaming de un instrumento (#558): siempre el contrato vigente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuoteRequest {
    pub spec: FutureSpec,
    pub data_type: LiveDataType,
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

/// Velas cerradas de cualquier [`BarSize`], con su fuente y el contrato del que salen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcedBars {
    pub source: String,
    pub contract: String,
    pub bar_size: BarSize,
    pub bars: Vec<RawBar>,
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

    #[cfg(test)]
    pub(crate) fn api_for_tests(&self) -> &A {
        &self.api
    }

    /// Por qué se activó el kill switch, si se activó.
    pub fn trip_reason(&self) -> Option<Trip> {
        self.active_trip()
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
            e @ (ApiError::Rejected(_) | ApiError::NoPermission(_)) => SessionError::Rejected(e),
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

    fn request(
        &self,
        instrument: &InstrumentId,
        contract: ContractChoice,
        bar_size: BarSize,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<BarsRequest, SessionError> {
        let spec = self
            .cfg
            .instrument(instrument)
            .cloned()
            .ok_or_else(|| SessionError::UnknownInstrument(instrument.clone()))?;
        Ok(BarsRequest {
            what_to_show: spec.what_to_show.unwrap_or(self.cfg.what_to_show),
            spec,
            contract,
            bar_size,
            from,
            to,
            regular_hours_only: self.cfg.regular_hours_only,
        })
    }

    /// Fecha de la primera vela que tiene IBKR para `instrument` (y de qué contrato).
    pub async fn head_timestamp(
        &self,
        instrument: &InstrumentId,
        contract: ContractChoice,
        now: DateTime<Utc>,
    ) -> Result<(String, DateTime<Utc>), SessionError> {
        let req = self.request(instrument, contract, BarSize::H1, now, now)?;
        self.check_connection(now).await?;
        match self.api.head_timestamp(&req).await {
            Ok(r) => Ok(r),
            Err(e) => Err(self.api_error(e, now).await),
        }
    }

    /// Velas **cerradas** en `now` de `instrument`, con inicio en `[from, to)`, con su fuente.
    pub async fn closed_bars(
        &self,
        instrument: &InstrumentId,
        contract: ContractChoice,
        bar_size: BarSize,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<SourcedBars, SessionError> {
        let req = self.request(instrument, contract, bar_size, from, to)?;
        self.check_connection(now).await?;
        let resp = match self.api.bars(&req).await {
            Ok(r) => r,
            Err(e) => return Err(self.api_error(e, now).await),
        };
        let mut bars: Vec<RawBar> = Vec::with_capacity(resp.bars.len());
        for b in resp.bars {
            if let Err(problem) = sane(&b, bars.last().map(|c| c.start)) {
                let reason = format!("{instrument} {}: {problem}", b.start.to_rfc3339());
                return Err(self.trip(AlertKind::DataError, reason, now).await);
            }
            // Solo velas cerradas (CLAUDE.md §5b): la vela en curso nunca sale de acá.
            if from <= b.start && b.start < to && b.start + bar_size.duration() <= now {
                bars.push(b);
            }
        }
        Ok(SourcedBars {
            source: self.cfg.source.clone(),
            contract: resp.contract,
            bar_size,
            bars,
        })
    }

    /// Velas **cerradas** en `now` de `instrument`, con inicio en `[from, to)`, con su fuente,
    /// como velas del motor (solo 15m y 1h, del contrato vigente).
    pub async fn closed_candles(
        &self,
        instrument: &InstrumentId,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<SourcedCandles, SessionError> {
        let got = self
            .closed_bars(
                instrument,
                ContractChoice::Front,
                timeframe.into(),
                from,
                to,
                now,
            )
            .await?;
        let candles = got
            .bars
            .into_iter()
            .map(|b| Candle {
                instrument_id: instrument.clone(),
                timeframe,
                start: b.start,
                open: b.open,
                high: b.high,
                low: b.low,
                close: b.close,
                volume: b.volume,
            })
            .collect();
        Ok(SourcedCandles {
            source: got.source,
            contract: got.contract,
            candles,
        })
    }

    /// Precios por streaming de `instrument` (#558), con el tipo de dato pedido, hasta que se corta
    /// o `out` se cierra. Antes, kill switch inactivo y conexión viva; una desconexión o un dato
    /// inválido activan el kill switch, igual que en las velas. Solo lectura.
    pub async fn stream_quotes(
        &self,
        instrument: &InstrumentId,
        data_type: LiveDataType,
        out: tokio::sync::mpsc::Sender<QuoteEvent>,
        now: DateTime<Utc>,
    ) -> Result<(), SessionError> {
        let spec = self
            .cfg
            .instrument(instrument)
            .cloned()
            .ok_or_else(|| SessionError::UnknownInstrument(instrument.clone()))?;
        self.check_connection(now).await?;
        match self
            .api
            .stream_quotes(&QuoteRequest { spec, data_type }, out)
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => Err(self.api_error(e, Utc::now()).await),
        }
    }

    /// Desconecta sin activar el kill switch, sin consumir la sesión (compartida entre tareas).
    pub async fn disconnect(&self) {
        self.api.disconnect().await;
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

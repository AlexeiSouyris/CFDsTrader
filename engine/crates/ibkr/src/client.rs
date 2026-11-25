//! Cliente real sobre el crate `ibapi` (=5.7.0), **solo lectura**: cuentas, resumen, detalles de
//! contrato y velas históricas. Ninguna función de este archivo usa el módulo de órdenes de `ibapi`
//! (un test lo verifica en el código fuente).
//!
//! - Conexión: primero el ID de la cuenta paper de `.env` y el puerto, **después** el socket. Sin
//!   reconexión automática (`max_reconnect_attempts(0)`): si se corta, se activa el kill switch y
//!   no se vuelve a entrar solo (después de un nuevo login la sesión podría ser otra).
//! - Los precios pasan de `f64` a `Decimal` acá, al entrar, por su representación decimal más
//!   corta (la misma que muestra IBKR). Un valor no finito es un error de datos.

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, NaiveDate, Utc};
use ibapi::accounts::types::AccountGroup;
use ibapi::accounts::{AccountSummaryResult, AccountSummaryTags};
use ibapi::contracts::Contract;
use ibapi::contracts::ContractMonth;
use ibapi::contracts::tick_types::TickType;
use ibapi::market_data::MarketDataType;
use ibapi::market_data::TradingHours;
use ibapi::market_data::historical::{
    Bar, BarSize as IbSize, BarTimestamp, Duration as IbDuration, WhatToShow as IbWhat,
};
use ibapi::market_data::realtime::TickTypes;
use ibapi::prelude::{StreamExt, SubscriptionItem, SubscriptionItemStreamExt};
use ibapi::{Client, Error as IbError};
use rust_decimal::Decimal;
use time::OffsetDateTime;

use crate::account::{PaperAccountError, PaperAccountId};
use crate::config::{ContractKind, IbkrConfig, IbkrConfigError, WhatToShow};
use crate::live::{LiveDataType, QuoteEvent};
use crate::session::{
    ApiError, BarSize, BarsRequest, BarsResponse, ContractChoice, DetailsQuery, FutureDetails,
    IbkrApi, QuoteRequest, RawBar, ReadOnlySession, SummaryItem, Trip,
};

/// Variables de `ibapi` que graban en disco los mensajes crudos (con cuentas y saldos). Con
/// cualquiera de ellas puesta, no se conecta.
pub const RECORDING_ENV_VARS: [&str; 2] = ["IBAPI_RECORDING_DIR", "IBAPI_RAW_CAPTURE_DIR"];

/// Avisos de IB Gateway que dicen que perdió la conexión con IBKR (el socket local sigue vivo):
/// 1100 conectividad perdida, 1300 el puerto del socket se reinició.
const LOST_CONNECTIVITY_CODES: [i32; 2] = [1100, 1300];

/// Errores de IBKR por falta de permiso de datos (no son una falla): 354 sin suscripción, 420 sin
/// permiso para ese mercado, 10089-10091 hace falta una suscripción, 10168 sin datos demorados.
pub const NO_PERMISSION_CODES: [i32; 6] = [354, 420, 10089, 10090, 10091, 10168];

#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error(transparent)]
    PaperAccount(#[from] PaperAccountError),
    #[error(transparent)]
    Config(#[from] IbkrConfigError),
    #[error("{0} está puesta: ibapi grabaría en disco los mensajes crudos; no se conecta")]
    RecordingEnabled(&'static str),
    #[error("no se pudo conectar a IB Gateway en {address}: {reason}")]
    Connect { address: String, reason: String },
    /// Se conectó, pero una guarda lo rechazó: kill switch activo y alerta.
    #[error("kill switch: {}", .0.reason)]
    Tripped(Trip),
}

/// El cliente de `ibapi`, envuelto: no expone el `Client` (que también sabe mandar órdenes).
pub struct IbapiClient {
    client: Arc<Client>,
    request_timeout: std::time::Duration,
    /// IB Gateway avisó que perdió la conexión con IBKR.
    lost: Arc<AtomicBool>,
    /// El tipo de dato (`reqMarketDataType`) es de toda la conexión: se cambia y se suscribe sin
    /// que otra suscripción se meta en el medio.
    quotes_setup: tokio::sync::Mutex<()>,
}

fn address(cfg: &IbkrConfig) -> String {
    if cfg.host.contains(':') {
        format!("[{}]:{}", cfg.host, cfg.port)
    } else {
        format!("{}:{}", cfg.host, cfg.port)
    }
}

/// Conecta a IB Gateway paper y abre la sesión de solo lectura. El orden importa: ID de cuenta
/// paper de `.env`, puerto permitido y nada grabando en disco **antes** de abrir el socket; después,
/// las cuentas de la sesión. `env` lee una variable (en producción, `std::env::var`).
pub async fn connect(
    cfg: IbkrConfig,
    env: impl Fn(&str) -> Option<String>,
) -> Result<ReadOnlySession<IbapiClient>, ConnectError> {
    let paper = PaperAccountId::from_env(&cfg.paper_account_env, &env)?;
    cfg.check_port()?;
    for var in RECORDING_ENV_VARS {
        if env(var).is_some_and(|v| !v.trim().is_empty()) {
            return Err(ConnectError::RecordingEnabled(var));
        }
    }
    let addr = address(&cfg);
    let fail = |reason: String| ConnectError::Connect {
        address: addr.clone(),
        reason,
    };
    let builder = Client::builder()
        .address(addr.clone())
        .client_id(cfg.client_id)
        .max_reconnect_attempts(0)
        .one_shot_timeout(cfg.request_timeout);
    let (client, mut notices) =
        match tokio::time::timeout(cfg.connect_timeout, builder.connect_with_notice_stream()).await
        {
            Err(_) => return Err(fail("no respondió a tiempo".into())),
            Ok(Err(e)) => return Err(fail(e.to_string())),
            Ok(Ok(c)) => c,
        };
    let lost = Arc::new(AtomicBool::new(false));
    let flag = lost.clone();
    tokio::spawn(async move {
        while let Some(n) = notices.next().await {
            if LOST_CONNECTIVITY_CODES.contains(&n.code) {
                flag.store(true, Ordering::SeqCst);
            }
        }
    });
    let api = IbapiClient {
        client: Arc::new(client),
        request_timeout: cfg.request_timeout,
        lost,
        quotes_setup: tokio::sync::Mutex::new(()),
    };
    ReadOnlySession::open(api, cfg, paper, Utc::now())
        .await
        .map_err(ConnectError::Tripped)
}

fn map_err(e: IbError) -> ApiError {
    match e {
        IbError::Notice(n) if NO_PERMISSION_CODES.contains(&n.code) => {
            ApiError::NoPermission(format!("{}: {}", n.code, n.message))
        }
        IbError::ConnectionFailed | IbError::Shutdown | IbError::ConnectionReset => {
            ApiError::ConnectionLost(e.to_string())
        }
        e if e.is_connection_lost() => ApiError::ConnectionLost(e.to_string()),
        e => ApiError::Rejected(e.to_string()),
    }
}

/// `f64` de IBKR → `Decimal` exacto por su representación decimal más corta.
pub fn to_decimal(v: f64, field: &str) -> Result<Decimal, ApiError> {
    if !v.is_finite() {
        return Err(ApiError::Data(format!("{field} no es finito")));
    }
    Decimal::from_str(&v.to_string()).map_err(|e| ApiError::Data(format!("{field} = {v}: {e}")))
}

fn to_utc(t: OffsetDateTime) -> Result<DateTime<Utc>, ApiError> {
    let nanos = t.unix_timestamp_nanos();
    i64::try_from(nanos)
        .ok()
        .map(DateTime::<Utc>::from_timestamp_nanos)
        .ok_or_else(|| ApiError::Data(format!("hora fuera de rango: {t}")))
}

fn to_offset(t: DateTime<Utc>) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::from_unix_timestamp(t.timestamp())
        .map_err(|e| ApiError::Data(format!("hora fuera de rango: {e}")))
}

/// Una vela de `ibapi` a decimales y UTC. Volumen negativo (IBKR usa -1 cuando no hay) → `None`.
/// Una vela diaria (`daily`) puede venir solo con fecha: se toma la medianoche UTC de esa fecha.
pub fn to_raw_bar(b: &Bar, daily: bool) -> Result<RawBar, ApiError> {
    let start = match b.date {
        BarTimestamp::DateTime(t) => to_utc(t)?,
        BarTimestamp::Date(d) if daily => to_utc(d.midnight().assume_utc())?,
        BarTimestamp::Date(_) => {
            return Err(ApiError::Data("vela intradía sin hora".into()));
        }
    };
    let volume = if b.volume.is_finite() && b.volume < 0.0 {
        None
    } else {
        Some(to_decimal(b.volume, "volume")?)
    };
    Ok(RawBar {
        start,
        open: to_decimal(b.open, "open")?,
        high: to_decimal(b.high, "high")?,
        low: to_decimal(b.low, "low")?,
        close: to_decimal(b.close, "close")?,
        volume,
    })
}

fn ib_data_type(t: LiveDataType) -> MarketDataType {
    match t {
        LiveDataType::Live => MarketDataType::Realtime,
        LiveDataType::Frozen => MarketDataType::Frozen,
        LiveDataType::Delayed => MarketDataType::Delayed,
        LiveDataType::DelayedFrozen => MarketDataType::DelayedFrozen,
    }
}

/// Un tick de `ibapi` → el evento que usa la vela en formación (o nada, si no interesa). Solo el
/// último precio operado y su hora, en vivo o demorados, y el tipo de dato.
pub fn quote_event(t: &TickTypes, received: DateTime<Utc>) -> Result<Option<QuoteEvent>, ApiError> {
    let last = |tick: TickType, price: f64| -> Result<Option<QuoteEvent>, ApiError> {
        if !matches!(tick, TickType::Last | TickType::DelayedLast) {
            return Ok(None);
        }
        Ok(Some(QuoteEvent::Last {
            price: to_decimal(price, "last")?,
            received,
        }))
    };
    match t {
        TickTypes::Price(p) => last(p.tick_type, p.price),
        TickTypes::PriceSize(p) => last(p.price_tick_type, p.price),
        TickTypes::String(s)
            if matches!(
                s.tick_type,
                TickType::LastTimestamp | TickType::DelayedLastTimestamp
            ) =>
        {
            let secs: i64 =
                s.value.trim().parse().map_err(|_| {
                    ApiError::Data(format!("hora del último inválida: {:?}", s.value))
                })?;
            let at = DateTime::<Utc>::from_timestamp(secs, 0)
                .ok_or_else(|| ApiError::Data(format!("hora del último fuera de rango: {secs}")))?;
            Ok(Some(QuoteEvent::LastTime { at, received }))
        }
        TickTypes::MarketDataType(m) => {
            Ok(LiveDataType::from_code(*m as i32).map(QuoteEvent::DataType))
        }
        _ => Ok(None),
    }
}

/// Mes `AAAAMM` → `ContractMonth`.
pub fn parse_month(m: &str) -> Result<ContractMonth, ApiError> {
    let bad = || ApiError::Rejected(format!("mes de contrato inválido: {m:?} (AAAAMM)"));
    if m.len() != 6 || !m.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad());
    }
    let year: u16 = m[..4].parse().map_err(|_| bad())?;
    let month: u8 = m[4..].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&month) {
        return Err(bad());
    }
    Ok(ContractMonth::new(year, month))
}

/// Duración de IBKR que cubre `span` (como `between` de `ibapi`): segundos hasta un día, días hasta
/// 365 y si no años. Para el futuro continuo, que no acepta fecha de fin.
pub fn covering(span: chrono::TimeDelta) -> IbDuration {
    let secs = span.num_seconds().max(1);
    if secs <= 86_400 {
        return IbDuration::seconds(secs as i32);
    }
    let days = secs.div_euclid(86_400) + 2;
    if days <= 365 {
        IbDuration::days(days as i32)
    } else {
        IbDuration::years(secs.div_euclid(365 * 86_400) as i32 + 1)
    }
}

/// El vencimiento vigente más cercano: la menor fecha `AAAAMMDD` que no es anterior a `today`.
pub fn pick_front(expiries: &[String], today: NaiveDate) -> Option<usize> {
    expiries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            NaiveDate::parse_from_str(e.get(..8)?, "%Y%m%d")
                .ok()
                .map(|d| (i, d))
        })
        .filter(|(_, d)| *d >= today)
        .min_by_key(|(_, d)| *d)
        .map(|(i, _)| i)
}

impl IbapiClient {
    async fn with_timeout<T>(
        &self,
        f: impl std::future::Future<Output = Result<T, IbError>>,
    ) -> Result<T, ApiError> {
        match tokio::time::timeout(self.request_timeout, f).await {
            Err(_) => Err(ApiError::Rejected("IBKR no respondió a tiempo".into())),
            Ok(r) => r.map_err(map_err),
        }
    }

    async fn front_contract(&self, req: &BarsRequest) -> Result<Contract, ApiError> {
        let query = Contract::futures(req.spec.symbol.as_str())
            .on_exchange(req.spec.exchange.as_str())
            .in_currency(req.spec.currency.as_str())
            .any_month()
            .build();
        let details = self
            .with_timeout(self.client.contract_details(&query))
            .await?;
        let expiries: Vec<String> = details
            .iter()
            .map(|d| {
                if d.real_expiration_date.is_empty() {
                    d.contract.last_trade_date_or_contract_month.clone()
                } else {
                    d.real_expiration_date.clone()
                }
            })
            .collect();
        let i = pick_front(&expiries, Utc::now().date_naive()).ok_or_else(|| {
            ApiError::Rejected(format!(
                "IBKR no informó un vencimiento vigente de {} en {}",
                req.spec.symbol, req.spec.exchange
            ))
        })?;
        Ok(details[i].contract.clone())
    }

    /// El contrato del pedido, y su nombre para guardar junto a las velas.
    async fn contract(&self, req: &BarsRequest) -> Result<(Contract, String), ApiError> {
        let s = &req.spec;
        let c = match (s.kind, &req.contract) {
            (ContractKind::Future, ContractChoice::Front) => self.front_contract(req).await?,
            (ContractKind::Future, ContractChoice::Month(m)) => {
                let mut query = Contract::futures(s.symbol.as_str())
                    .on_exchange(s.exchange.as_str())
                    .in_currency(s.currency.as_str())
                    .expires_in(parse_month(m)?)
                    .build();
                query.include_expired = true;
                let details = self
                    .with_timeout(self.client.contract_details(&query))
                    .await?;
                let mut c = details
                    .into_iter()
                    .next()
                    .ok_or_else(|| ApiError::Rejected(format!("IBKR no conoce {} {m}", s.symbol)))?
                    .contract;
                c.include_expired = true;
                c
            }
            (ContractKind::ContinuousFuture, _) => Contract::continuous_futures(s.symbol.as_str())
                .on_exchange(s.exchange.as_str())
                .in_currency(s.currency.as_str())
                .build(),
            (ContractKind::Stock, _) => Contract::stock(s.symbol.as_str())
                .on_exchange(s.exchange.as_str())
                .in_currency(s.currency.as_str())
                .build(),
            (ContractKind::Forex, _) => Contract::forex(s.symbol.as_str(), s.currency.as_str())
                .on_exchange(s.exchange.as_str())
                .build(),
        };
        let name = match s.kind {
            ContractKind::Future if !c.local_symbol.is_empty() => c.local_symbol.clone(),
            ContractKind::ContinuousFuture => format!("{}.CONTFUT", s.symbol),
            ContractKind::Forex => format!("{}.{}", s.symbol, s.currency),
            _ => s.symbol.clone(),
        };
        Ok((c, name))
    }
}

fn ib_what(w: WhatToShow) -> IbWhat {
    match w {
        WhatToShow::Trades => IbWhat::Trades,
        WhatToShow::Midpoint => IbWhat::MidPoint,
    }
}

fn ib_hours(regular_only: bool) -> TradingHours {
    if regular_only {
        TradingHours::Regular
    } else {
        TradingHours::Extended
    }
}

impl IbkrApi for IbapiClient {
    fn is_connected(&self) -> bool {
        self.client.is_connected() && !self.lost.load(Ordering::SeqCst)
    }

    async fn managed_accounts(&self) -> Result<Vec<String>, ApiError> {
        // Las del saludo inicial y las que se piden: se revisan todas.
        let mut all: Vec<String> = self.client.handshake_managed_accounts().to_vec();
        for a in self.with_timeout(self.client.managed_accounts()).await? {
            if !all.contains(&a) {
                all.push(a);
            }
        }
        Ok(all)
    }

    async fn account_summary(&self) -> Result<Vec<SummaryItem>, ApiError> {
        let tags = [
            AccountSummaryTags::ACCOUNT_TYPE,
            AccountSummaryTags::NET_LIQUIDATION,
            AccountSummaryTags::TOTAL_CASH_VALUE,
            AccountSummaryTags::AVAILABLE_FUNDS,
        ];
        let read = async {
            let sub = self
                .client
                .account_summary(&AccountGroup("All".to_owned()), &tags)
                .await?;
            let mut sub = sub.filter_data();
            let mut out = Vec::new();
            while let Some(item) = sub.next().await {
                match item? {
                    AccountSummaryResult::Summary(s) => out.push(SummaryItem {
                        account: s.account,
                        tag: s.tag,
                        value: s.value,
                        currency: s.currency,
                    }),
                    AccountSummaryResult::End => break,
                }
            }
            Ok::<_, IbError>(out)
        };
        self.with_timeout(read).await
    }

    async fn bars(&self, req: &BarsRequest) -> Result<BarsResponse, ApiError> {
        let (contract, name) = self.contract(req).await?;
        let size = match req.bar_size {
            BarSize::M1 => IbSize::Min,
            BarSize::M15 => IbSize::Min15,
            BarSize::H1 => IbSize::Hour,
            BarSize::D1 => IbSize::Day,
        };
        let builder = self
            .client
            .historical_data(&contract, size)
            .what_to_show(ib_what(req.what_to_show))
            .trading_hours(ib_hours(req.regular_hours_only));
        // El futuro continuo no acepta fecha de fin: se pide hasta ahora y se recorta después.
        let builder = if req.spec.kind == ContractKind::ContinuousFuture {
            builder.duration(covering(Utc::now() - req.from))
        } else {
            builder.between(to_offset(req.from)?, to_offset(req.to)?)
        };
        let data = self.with_timeout(builder.fetch()).await?;
        let daily = req.bar_size == BarSize::D1;
        let bars = data
            .bars
            .iter()
            .map(|b| to_raw_bar(b, daily))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BarsResponse {
            contract: name,
            bars,
        })
    }

    async fn head_timestamp(&self, req: &BarsRequest) -> Result<(String, DateTime<Utc>), ApiError> {
        let (contract, name) = self.contract(req).await?;
        let t = self
            .with_timeout(self.client.head_timestamp(
                &contract,
                ib_what(req.what_to_show),
                ib_hours(req.regular_hours_only),
            ))
            .await?;
        Ok((name, to_utc(t)?))
    }

    async fn stream_quotes(
        &self,
        req: &QuoteRequest,
        out: tokio::sync::mpsc::Sender<QuoteEvent>,
    ) -> Result<(), ApiError> {
        let now = Utc::now();
        let contract_req = BarsRequest {
            spec: req.spec.clone(),
            contract: ContractChoice::Front,
            bar_size: BarSize::M1,
            from: now,
            to: now,
            what_to_show: WhatToShow::Trades,
            regular_hours_only: false,
        };
        let (contract, name) = self.contract(&contract_req).await?;
        let mut sub = {
            let _setup = self.quotes_setup.lock().await;
            self.with_timeout(
                self.client
                    .switch_market_data_type(ib_data_type(req.data_type)),
            )
            .await?;
            self.with_timeout(self.client.market_data(&contract).subscribe())
                .await?
        };
        if out.send(QuoteEvent::Contract(name)).await.is_err() {
            sub.cancel().await;
            return Ok(());
        }
        loop {
            let item = tokio::select! {
                i = sub.next() => i,
                () = out.closed() => {
                    sub.cancel().await;
                    return Ok(());
                }
            };
            if self.lost.load(Ordering::SeqCst) {
                return Err(ApiError::ConnectionLost(
                    "IB Gateway perdió la conexión con IBKR".into(),
                ));
            }
            let tick = match item {
                None if !self.client.is_connected() => {
                    return Err(ApiError::ConnectionLost("la suscripción se cortó".into()));
                }
                None => return Err(ApiError::Rejected("IBKR terminó la suscripción".into())),
                Some(Err(e)) => return Err(map_err(e)),
                // Avisos (por ejemplo 10167 "datos demorados"): no cortan la suscripción.
                Some(Ok(SubscriptionItem::Notice(_))) => continue,
                Some(Ok(SubscriptionItem::Data(t))) => t,
            };
            if let Some(ev) = quote_event(&tick, Utc::now())?
                && out.send(ev).await.is_err()
            {
                sub.cancel().await;
                return Ok(());
            }
        }
    }

    async fn future_details(&self, query: &DetailsQuery) -> Result<Vec<FutureDetails>, ApiError> {
        let q = Contract::futures(query.symbol.as_str())
            .on_exchange(query.exchange.as_str())
            .in_currency(query.currency.as_str())
            .any_month()
            .build();
        let details = self.with_timeout(self.client.contract_details(&q)).await?;
        details
            .into_iter()
            .map(|d| {
                Ok(FutureDetails {
                    local_symbol: d.contract.local_symbol.clone(),
                    trading_class: d.contract.trading_class.clone(),
                    exchange: d.contract.exchange.to_string(),
                    currency: d.contract.currency.to_string(),
                    multiplier: d.contract.multiplier.clone(),
                    min_tick: to_decimal(d.min_tick, "min_tick")?,
                    contract_month: d.contract_month,
                    last_trade_date: d.contract.last_trade_date_or_contract_month,
                    long_name: d.long_name,
                    time_zone_id: d.time_zone_id,
                    trading_hours: d.trading_hours,
                    market_rule_ids: d.market_rule_ids,
                })
            })
            .collect()
    }

    async fn disconnect(&self) {
        self.client.disconnect().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_from_ibkr_float_is_exact() {
        assert_eq!(to_decimal(21345.25, "x").unwrap().to_string(), "21345.25");
        assert_eq!(to_decimal(6012.5, "x").unwrap().to_string(), "6012.5");
        assert!(to_decimal(f64::NAN, "x").is_err());
        assert!(to_decimal(f64::INFINITY, "x").is_err());
    }

    #[test]
    fn bar_to_utc_and_decimals() {
        let t = OffsetDateTime::from_unix_timestamp(1_764_000_000).unwrap();
        let b = Bar {
            date: BarTimestamp::DateTime(t),
            open: 100.25,
            high: 101.0,
            low: 99.75,
            close: 100.5,
            volume: -1.0,
            wap: 0.0,
            count: -1,
        };
        let r = to_raw_bar(&b, false).unwrap();
        assert_eq!(r.start.timestamp(), 1_764_000_000);
        assert_eq!(r.low.to_string(), "99.75");
        assert_eq!(r.volume, None);
        let daily = Bar {
            date: BarTimestamp::Date(t.date()),
            ..b
        };
        assert!(matches!(to_raw_bar(&daily, false), Err(ApiError::Data(_))));
        let d = to_raw_bar(&daily, true).unwrap();
        assert_eq!(d.start.timestamp() % 86_400, 0);
    }

    #[test]
    fn contract_month_and_covering_duration() {
        assert!(parse_month("202609").is_ok());
        for bad in ["2026-09", "202613", "20260", "abcdef"] {
            assert!(parse_month(bad).is_err(), "{bad}");
        }
        let d = |td: chrono::TimeDelta| covering(td).to_string();
        assert_eq!(d(chrono::TimeDelta::hours(2)), "7200 S");
        assert_eq!(d(chrono::TimeDelta::days(3)), "5 D");
        assert_eq!(d(chrono::TimeDelta::days(800)), "3 Y");
    }

    #[test]
    fn front_is_nearest_unexpired() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 25).unwrap();
        let e = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            pick_front(&e(&["20270319", "20261218", "20260918"]), today),
            Some(1)
        );
        assert_eq!(pick_front(&e(&["20261125"]), today), Some(0));
        assert_eq!(pick_front(&e(&["20260918", "basura"]), today), None);
    }
}

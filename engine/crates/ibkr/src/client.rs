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
use ibapi::market_data::TradingHours;
use ibapi::market_data::historical::{Bar, BarSize, BarTimestamp, WhatToShow as IbWhat};
use ibapi::prelude::{StreamExt, SubscriptionItemStreamExt};
use ibapi::{Client, Error as IbError};
use rust_decimal::Decimal;
use time::OffsetDateTime;

use cfds_core::Timeframe;

use crate::account::{PaperAccountError, PaperAccountId};
use crate::config::{IbkrConfig, IbkrConfigError, WhatToShow};
use crate::session::{
    ApiError, BarsRequest, BarsResponse, IbkrApi, RawBar, ReadOnlySession, SummaryItem, Trip,
};

/// Variables de `ibapi` que graban en disco los mensajes crudos (con cuentas y saldos). Con
/// cualquiera de ellas puesta, no se conecta.
pub const RECORDING_ENV_VARS: [&str; 2] = ["IBAPI_RECORDING_DIR", "IBAPI_RAW_CAPTURE_DIR"];

/// Avisos de IB Gateway que dicen que perdió la conexión con IBKR (el socket local sigue vivo):
/// 1100 conectividad perdida, 1300 el puerto del socket se reinició.
const LOST_CONNECTIVITY_CODES: [i32; 2] = [1100, 1300];

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
    };
    ReadOnlySession::open(api, cfg, paper, Utc::now())
        .await
        .map_err(ConnectError::Tripped)
}

fn map_err(e: IbError) -> ApiError {
    match e {
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
pub fn to_raw_bar(b: &Bar) -> Result<RawBar, ApiError> {
    let start = match b.date {
        BarTimestamp::DateTime(t) => to_utc(t)?,
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
        let contract = self.front_contract(req).await?;
        let size = match req.timeframe {
            Timeframe::M15 => BarSize::Min15,
            Timeframe::H1 => BarSize::Hour,
        };
        let what = match req.what_to_show {
            WhatToShow::Trades => IbWhat::Trades,
            WhatToShow::Midpoint => IbWhat::MidPoint,
        };
        let hours = if req.regular_hours_only {
            TradingHours::Regular
        } else {
            TradingHours::Extended
        };
        let (from, to) = (to_offset(req.from)?, to_offset(req.to)?);
        let data = self
            .with_timeout(
                self.client
                    .historical_data(&contract, size)
                    .what_to_show(what)
                    .trading_hours(hours)
                    .between(from, to)
                    .fetch(),
            )
            .await?;
        let bars = data
            .bars
            .iter()
            .map(to_raw_bar)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BarsResponse {
            contract: contract.local_symbol.clone(),
            bars,
        })
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
        let r = to_raw_bar(&b).unwrap();
        assert_eq!(r.start.timestamp(), 1_764_000_000);
        assert_eq!(r.low.to_string(), "99.75");
        assert_eq!(r.volume, None);
        let daily = Bar {
            date: BarTimestamp::Date(t.date()),
            ..b
        };
        assert!(matches!(to_raw_bar(&daily), Err(ApiError::Data(_))));
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

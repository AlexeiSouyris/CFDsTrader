//! IBKR como segundo broker, **solo cuenta paper y solo lectura** (ADR 0017, etapa 1, #534).
//!
//! Se conecta a IB Gateway en esta máquina (login hecho a mano por el dueño, modo paper) y lee:
//! las cuentas de la sesión, el resumen de la cuenta paper y velas **cerradas** de futuros micro
//! (MNQ, MES), con su fuente. Precios a `Decimal` al entrar.
//!
//! Guardas (ADR 0017 §2), todas con tests sin red:
//! 1. Puertos de sesión real (7496, 4001 y los de `config/ibkr.toml`) rechazados antes de abrir el
//!    socket ([`config`]).
//! 2. Sin el ID de la cuenta paper en `.env` no se conecta; si alguna cuenta de la sesión es otra,
//!    desconecta y activa el kill switch ([`account`], [`session`]).
//! 3. Solo lectura: no hay código para enviar, modificar ni cancelar órdenes ([`IbkrApi`] no tiene
//!    esos métodos y un test revisa el código fuente).
//! 4. Desconexión de IB Gateway → kill switch, sin reconexión automática ([`session`], [`client`]).
//!
//! Vela en formación (#558, **solo visual**): [`live`] arma, con funciones puras, la vela en curso
//! de 1m, 5m, 15m y 1h a partir de los precios por streaming (en vivo si la cuenta tiene permiso,
//! si no demorados, siempre informado), y [`live_feed`] mantiene la suscripción con las mismas
//! guardas. Nada del motor que decide usa esas velas.
//!
//! Como fuente de datos (#536), [`load`] planea la carga incremental de velas cerradas; el
//! ejemplo `ibkr_load` las guarda en `ibkr_bars` (`ibkr_bar.v1`). [`health`] cuenta las corridas
//! en que IBKR no responde y avisa una vez cuando la carga se traba y otra cuando vuelve (#607).

pub mod account;
pub mod client;
pub mod config;
pub mod health;
pub mod live;
pub mod live_feed;
pub mod load;
pub mod session;

pub use account::{PaperAccountError, PaperAccountId, check_session_accounts, mask};
pub use client::{ConnectError, IbapiClient, connect};
pub use config::{
    ContractKind, FutureSpec, IbkrConfig, IbkrConfigError, LiveConfig, REQUIRED_FORBIDDEN_PORTS,
    WhatToShow,
};
pub use live::{BarUpdate, LiveBar, LiveDataType, LiveTimeframe, QuoteEvent, QuoteState};
pub use session::{
    AccountSummary, ApiError, BarSize, BarsRequest, BarsResponse, ContractChoice, DetailsQuery,
    FutureDetails, IbkrApi, QuoteRequest, RawBar, ReadOnlySession, SessionError, SourcedBars,
    SourcedCandles, SummaryItem, Trip,
};

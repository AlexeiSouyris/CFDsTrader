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
//! No guarda nada todavía: la persistencia de las velas (contrato y tabla) va en otro Issue.

pub mod account;
pub mod client;
pub mod config;
pub mod session;

pub use account::{PaperAccountError, PaperAccountId, check_session_accounts, mask};
pub use client::{ConnectError, IbapiClient, connect};
pub use config::{
    ContractKind, FutureSpec, IbkrConfig, IbkrConfigError, REQUIRED_FORBIDDEN_PORTS, WhatToShow,
};
pub use session::{
    AccountSummary, ApiError, BarSize, BarsRequest, BarsResponse, ContractChoice, IbkrApi, RawBar,
    ReadOnlySession, SessionError, SourcedBars, SourcedCandles, SummaryItem, Trip,
};

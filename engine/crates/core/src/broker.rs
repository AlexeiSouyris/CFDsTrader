//! Mensajes del broker (fase 7, #278). Contratos: `order.v1`, `order_report.v1`,
//! `ticket_verdict.v1`, `position.v1` y `paper_account.v1` de `contracts/`.
//!
//! Solo existe el modo paper ([`crate::Mode`]): ninguno de estos tipos puede describir una orden
//! real (CLAUDE.md §5, fase 11).

use std::fmt;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

/// Id propio de una orden, una posición o una propuesta: `^[A-Za-z0-9._:-]{1,200}$`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ClientId(String);

impl ClientId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for ClientId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ok = (1..=200).contains(&s.len())
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'));
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(format!("id inválido: {s:?}"))
        }
    }
}

impl<'de> Deserialize<'de> for ClientId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Dirección de una orden o posición.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Long,
    Short,
}

/// El único modo de los mensajes del broker: `"paper"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperMode {
    #[default]
    Paper,
}

/// `order.v1`: una orden de mercado. No lleva tamaño: lo calcula solo el riesgo (spec §8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Order {
    pub order_id: ClientId,
    pub mode: PaperMode,
    pub instrument_id: InstrumentId,
    pub side: OrderSide,
    #[serde(with = "crate::decimal_str")]
    pub stop: Decimal,
    #[serde(with = "crate::decimal_str::option")]
    pub tp1: Option<Decimal>,
    pub proposal_id: Option<ClientId>,
    #[serde(with = "utc_z")]
    pub created_at: DateTime<Utc>,
}

/// Motivo de rechazo de una orden: los del riesgo más los del broker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderRejection {
    InvalidInput,
    KillSwitchActive,
    DrawdownLimit,
    DailyLossLimit,
    MaxOpenPositions,
    InstrumentAlreadyOpen,
    StopTooClose,
    StopTooFar,
    InsufficientMargin,
    BelowMinDealSize,
    MinDealSizeUnknown,
    /// No hay vela para ejecutar o valuar.
    NoPrice,
    UnknownInstrument,
    /// El stop está del lado equivocado de la entrada (arriba en un largo, abajo en un corto).
    StopWrongSide,
    /// Hay vela de referencia, pero no alcanzan las velas cerradas recientes para el ATR del
    /// riesgo (calentamiento), por ejemplo porque el modo en vivo está en pausa sin cupo (#361).
    NoRecentCandles,
    /// El contrato está en otra moneda que la cuenta (el oro, en USD) y no hay tipo de cambio
    /// publicado a la hora de la señal (ADR 0013, #418): no se inventa uno.
    MissingFxRate,
}

impl OrderRejection {
    /// Todos, en el orden del contrato.
    pub const ALL: [OrderRejection; 16] = [
        Self::InvalidInput,
        Self::KillSwitchActive,
        Self::DrawdownLimit,
        Self::DailyLossLimit,
        Self::MaxOpenPositions,
        Self::InstrumentAlreadyOpen,
        Self::StopTooClose,
        Self::StopTooFar,
        Self::InsufficientMargin,
        Self::BelowMinDealSize,
        Self::MinDealSizeUnknown,
        Self::NoPrice,
        Self::UnknownInstrument,
        Self::StopWrongSide,
        Self::NoRecentCandles,
        Self::MissingFxRate,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Rejected,
    Filled,
    /// Aceptable por el riesgo, esperando la vela del fill (#471). Solo en la respuesta de
    /// `submit`: nunca entra al registro guardado.
    Pending,
}

/// `order_report.v1`: qué pasó con una orden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderReport {
    pub order_id: ClientId,
    pub status: OrderStatus,
    pub rejection: Option<OrderRejection>,
    pub position_id: Option<ClientId>,
    #[serde(with = "crate::decimal_str::option")]
    pub size: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub fill_price: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub commission: Option<Decimal>,
    #[serde(with = "utc_z")]
    pub at: DateTime<Utc>,
}

/// `ticket_verdict.v1`: la respuesta del riesgo a una orden, sin enviarla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TicketVerdict {
    pub order_id: ClientId,
    pub accepted: bool,
    pub rejection: Option<OrderRejection>,
    #[serde(with = "crate::decimal_str::option")]
    pub reference: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub size: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub risk: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub risk_pct: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub leverage: Option<Decimal>,
    #[serde(with = "crate::decimal_str::option")]
    pub margin: Option<Decimal>,
    pub leverage_capped: bool,
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
}

/// `position.v1`: una posición abierta, valuada al cierre de `as_of`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub position_id: ClientId,
    pub order_id: ClientId,
    pub instrument_id: InstrumentId,
    pub side: OrderSide,
    #[serde(with = "crate::decimal_str")]
    pub size: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub entry: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub stop: Decimal,
    #[serde(with = "crate::decimal_str::option")]
    pub tp1: Option<Decimal>,
    #[serde(with = "utc_z")]
    pub opened_at: DateTime<Utc>,
    #[serde(with = "crate::decimal_str")]
    pub last: Decimal,
    #[serde(with = "crate::decimal_str::signed")]
    pub unrealized_pl: Decimal,
    #[serde(with = "crate::decimal_str::signed")]
    pub unrealized_r: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub margin: Decimal,
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
}

/// Moneda de la cuenta del fondo: solo EUR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AccountCurrency {
    #[default]
    #[serde(rename = "EUR")]
    Eur,
}

/// Estado del kill switch en `paper_account.v1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KillSwitch {
    Inactive,
    Active {
        reason: String,
        since: DateTime<Utc>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KillSwitchDoc {
    active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(
        default,
        with = "crate::liquidity_stats::utc_z_option",
        skip_serializing_if = "Option::is_none"
    )]
    since: Option<DateTime<Utc>>,
}

impl Serialize for KillSwitch {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let doc = match self {
            KillSwitch::Inactive => KillSwitchDoc {
                active: false,
                reason: None,
                since: None,
            },
            KillSwitch::Active { reason, since } => KillSwitchDoc {
                active: true,
                reason: Some(reason.clone()),
                since: Some(*since),
            },
        };
        doc.serialize(s)
    }
}

impl<'de> Deserialize<'de> for KillSwitch {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let doc = KillSwitchDoc::deserialize(d)?;
        match (doc.active, doc.reason, doc.since) {
            (false, None, None) => Ok(KillSwitch::Inactive),
            (true, Some(reason), Some(since))
                if !reason.is_empty() && reason.chars().count() <= 300 =>
            {
                Ok(KillSwitch::Active { reason, since })
            }
            _ => Err(serde::de::Error::custom(
                "kill_switch: activo con motivo y hora, o inactivo sin nada más",
            )),
        }
    }
}

/// `paper_account.v1`: la cuenta del broker simulado al cierre de `as_of`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaperAccount {
    pub mode: PaperMode,
    pub currency: AccountCurrency,
    #[serde(with = "utc_z")]
    pub as_of: DateTime<Utc>,
    #[serde(with = "crate::decimal_str::signed")]
    pub equity: Decimal,
    #[serde(with = "crate::decimal_str::signed")]
    pub unrealized_pl: Decimal,
    #[serde(with = "crate::decimal_str::signed")]
    pub realized_pl_today: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub margin_used: Decimal,
    #[serde(with = "crate::decimal_str::signed")]
    pub margin_free: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub leverage: Decimal,
    #[serde(with = "crate::decimal_str")]
    pub daily_loss_used_pct: Decimal,
    pub open_positions: u32,
    pub max_positions: u32,
    pub kill_switch: KillSwitch,
}

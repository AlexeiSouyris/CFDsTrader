//! Nivel del mapa de liquidez (spec §5.4.1, #231). Contrato:
//! `contracts/liquidity_level.v1.schema.json`.
//!
//! Lo calcula el motor (`cfds-liquidity`, `levels`) y nadie más lo recalcula (CLAUDE.md §1).
//! Acepta exactamente los mismos documentos que el contrato y además exige
//! `state_changed_at > exists_from` y `expired_at > exists_from`, que el JSON Schema no puede
//! expresar.
//!
//! `expired_at` es un campo **opcional** agregado a v1 sin romperlo (#231): un documento sin él
//! sigue siendo válido y se lee como "no venció".

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::candle::utc_z;
use crate::instrument::InstrumentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquidityLevelKind {
    Eqh,
    Eql,
    Pdh,
    Pdl,
    Pwh,
    Pwl,
    SessionHigh,
    SessionLow,
    /// Número redondo por encima del cierre (múltiplo de un paso por instrumento, #497).
    RoundHigh,
    /// Número redondo por debajo del cierre (#497).
    RoundLow,
}

impl LiquidityLevelKind {
    /// `true` si es un nivel alto (los stops están por encima).
    pub fn is_high(self) -> bool {
        matches!(
            self,
            Self::Eqh | Self::Pdh | Self::Pwh | Self::SessionHigh | Self::RoundHigh
        )
    }

    pub fn is_session(self) -> bool {
        matches!(self, Self::SessionHigh | Self::SessionLow)
    }
}

/// Ventana UTC de `config/liquidity.toml` `[sessions_utc]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquiditySession {
    Asia,
    London,
    Ny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquidityLevelState {
    /// Ninguna vela cerrada lo superó.
    Intact,
    /// Barrido con reversión: lo superó y cerró de vuelta del lado original.
    Swept,
    /// Roto: lo superó y cerró del otro lado.
    Broken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Raw", into = "Raw")]
pub struct LiquidityLevel {
    pub instrument_id: InstrumentId,
    pub kind: LiquidityLevelKind,
    /// Solo en `session_high` y `session_low`.
    pub session: Option<LiquiditySession>,
    pub price: Decimal,
    /// Desde cuándo existe el nivel (nunca antes).
    pub exists_from: DateTime<Utc>,
    pub state: LiquidityLevelState,
    /// Cierre de la vela que barrió o rompió el nivel; `None` si está intacto.
    pub state_changed_at: Option<DateTime<Utc>>,
    /// Cierre de la vela número `max_age_bars` desde que existe, si llegó intacto hasta ahí:
    /// desde ese momento ya no está activo. `None` si no venció (o si se barrió o rompió antes).
    pub expired_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    instrument_id: InstrumentId,
    kind: LiquidityLevelKind,
    // Obligatorios aunque sean `null`, como en el contrato.
    #[serde(deserialize_with = "Option::deserialize")]
    session: Option<LiquiditySession>,
    #[serde(with = "crate::decimal_str")]
    price: Decimal,
    #[serde(with = "utc_z")]
    exists_from: DateTime<Utc>,
    state: LiquidityLevelState,
    #[serde(with = "optional_utc_z")]
    state_changed_at: Option<DateTime<Utc>>,
    // Opcional: puede faltar (documentos anteriores) y no se escribe cuando es `None`.
    #[serde(
        default,
        with = "optional_utc_z",
        skip_serializing_if = "Option::is_none"
    )]
    expired_at: Option<DateTime<Utc>>,
}

/// `utc_z` para un campo obligatorio que puede ser `null`.
mod optional_utc_z {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(t: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
        match t {
            Some(t) => crate::candle::utc_z::serialize(t, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
        #[derive(Deserialize)]
        struct Wrap(#[serde(with = "crate::candle::utc_z")] DateTime<Utc>);
        Ok(Option::<Wrap>::deserialize(d)?.map(|w| w.0))
    }
}

impl TryFrom<Raw> for LiquidityLevel {
    type Error = String;

    fn try_from(r: Raw) -> Result<Self, String> {
        if r.kind.is_session() != r.session.is_some() {
            return Err("session va solo (y siempre) en session_high y session_low".into());
        }
        match (r.state, r.state_changed_at) {
            (LiquidityLevelState::Intact, None) => {}
            (LiquidityLevelState::Intact, Some(_)) => {
                return Err("un nivel intacto no tiene state_changed_at".into());
            }
            (_, None) => return Err("un nivel barrido o roto necesita state_changed_at".into()),
            (_, Some(t)) if t <= r.exists_from => {
                return Err("state_changed_at tiene que ser posterior a exists_from".into());
            }
            _ => {}
        }
        match r.expired_at {
            Some(_) if r.state != LiquidityLevelState::Intact => {
                return Err("solo un nivel intacto puede vencer".into());
            }
            Some(t) if t <= r.exists_from => {
                return Err("expired_at tiene que ser posterior a exists_from".into());
            }
            _ => {}
        }
        Ok(Self {
            instrument_id: r.instrument_id,
            kind: r.kind,
            session: r.session,
            price: r.price,
            exists_from: r.exists_from,
            state: r.state,
            state_changed_at: r.state_changed_at,
            expired_at: r.expired_at,
        })
    }
}

impl From<LiquidityLevel> for Raw {
    fn from(l: LiquidityLevel) -> Self {
        Self {
            instrument_id: l.instrument_id,
            kind: l.kind,
            session: l.session,
            price: l.price,
            exists_from: l.exists_from,
            state: l.state,
            state_changed_at: l.state_changed_at,
            expired_at: l.expired_at,
        }
    }
}

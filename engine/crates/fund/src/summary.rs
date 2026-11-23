//! Resumen del fondo para la UI (`contracts/fund_summary.v1`, #312): NAV, valor total, unidades
//! y la cuenta de cada miembro, todo calculado acá desde el historial (`fund_movement.v1`).
//!
//! **Valuación:** el historial guarda cada liquidación con su NAV, pero no el valor del fondo
//! entre una liquidación y otra. El resumen da el valor **al cerrar la última liquidación**:
//! lo que valían las unidades que había antes, al NAV de esa liquidación (redondeado a
//! céntimos), más los aportes y menos los retiros de ese día. Lo dice `basis` (`last_settlement`)
//! y `valued_at`. Si más adelante el motor publica el valor del fondo entre liquidaciones, será
//! otra `basis`.
//!
//! La suma de lo que vale cada miembro es el valor total, al céntimo (`Fund::statement`).

use cfds_core::candle::utc_z;
use cfds_core::decimal_str;
use chrono::{DateTime, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Serialize, Serializer};

use crate::{CENTS, Fund, FundConfig, FundError, MoveKind, Record};

/// De dónde sale el valor del fondo del resumen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// Al cerrar la última liquidación del historial (ver el encabezado del módulo).
    LastSettlement,
}

/// La cuenta de un miembro en el resumen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemberSummary {
    pub member_id: String,
    #[serde(with = "decimal_str")]
    pub units: Decimal,
    /// Lo que valen sus unidades, en céntimos.
    #[serde(with = "decimal_str")]
    pub value: Decimal,
    /// Aportes − retiros (puede ser negativo si retiró ganancias).
    #[serde(with = "decimal_str::signed")]
    pub net_contributions: Decimal,
    /// `value − net_contributions`.
    #[serde(with = "decimal_str::signed")]
    pub pnl: Decimal,
}

/// `contracts/fund_summary.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub currency: String,
    pub basis: Basis,
    /// Hora de la última liquidación; `None` si el historial está vacío.
    #[serde(serialize_with = "opt_utc_z")]
    pub valued_at: Option<DateTime<Utc>>,
    #[serde(with = "decimal_str")]
    pub nav: Decimal,
    #[serde(with = "decimal_str")]
    pub total_value: Decimal,
    #[serde(with = "decimal_str")]
    pub total_units: Decimal,
    pub members: Vec<MemberSummary>,
}

fn opt_utc_z<S: Serializer>(t: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
    match t {
        Some(t) => utc_z::serialize(t, s),
        None => s.serialize_none(),
    }
}

/// Arma el resumen desde el historial completo, en orden de liquidación (`PgFund::load`).
pub fn summary(records: &[Record], cfg: &FundConfig) -> Result<Summary, FundError> {
    let fund = Fund::replay(records, cfg)?;
    let Some(last) = records.last() else {
        return Ok(Summary {
            currency: cfg.currency.clone(),
            basis: Basis::LastSettlement,
            valued_at: None,
            nav: cfg.initial_nav,
            total_value: Decimal::ZERO,
            total_units: Decimal::ZERO,
            members: Vec::new(),
        });
    };
    let split = records
        .iter()
        .rposition(|r| r.settled_at != last.settled_at)
        .map_or(0, |i| i + 1);
    let (before, settlement) = records.split_at(split);
    if let Some(r) = settlement.iter().find(|r| r.nav != last.nav) {
        return Err(FundError::BadHistory {
            id: r.id.clone(),
            reason: "una misma liquidación con dos NAV distintos".into(),
        });
    }
    let units_before = Fund::replay(before, cfg)?.total_units();
    let mut value = (units_before * last.nav)
        .round_dp_with_strategy(CENTS, RoundingStrategy::MidpointAwayFromZero);
    for r in settlement {
        match r.kind {
            MoveKind::Contribution => value += r.amount,
            MoveKind::Withdrawal => value -= r.amount,
        }
    }
    let nav = fund.nav(value, cfg)?.normalize();
    let members = fund
        .statement(value)?
        .into_iter()
        .map(|l| MemberSummary {
            member_id: l.member,
            units: l.units,
            value: l.value,
            net_contributions: l.net_contributions,
            pnl: l.pnl,
        })
        .collect();
    Ok(Summary {
        currency: cfg.currency.clone(),
        basis: Basis::LastSettlement,
        valued_at: Some(last.settled_at),
        nav,
        total_value: value,
        total_units: fund.total_units(),
        members,
    })
}

//! Fondo del grupo: contabilidad por unidades (spec §14, glosario "unidades" y "NAV", #177).
//!
//! - NAV = valor total del fondo ÷ unidades emitidas. Sin unidades, vale `initial_nav`.
//! - Cada aporte compra unidades y cada retiro las vende, todos al mismo NAV: el del cierre del
//!   viernes (la liquidación del lunes). El valor del fondo lo da el motor; acá no se calcula.
//! - P&L de un miembro = lo que valen sus unidades − (aportes − retiros).
//! - Invariante: la suma de lo que vale cada miembro es el valor total del fondo, al céntimo
//!   (reparto por el método del mayor resto).
//!
//! Funciones puras y decimales exactos. Cada liquidación devuelve el registro de cada
//! movimiento para el historial auditable.

use std::collections::BTreeMap;

use rust_decimal::{Decimal, RoundingStrategy};
use serde::Deserialize;

/// Decimales del dinero (céntimos).
const CENTS: u32 = 2;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FundError {
    #[error("config/fund.toml: {0}")]
    Config(String),
    #[error("{member}: el aporte de {amount} es menor que el mínimo de {min}")]
    BelowMinimum {
        member: String,
        amount: Decimal,
        min: Decimal,
    },
    #[error("{member}: el monto {amount} tiene que ser positivo y en céntimos")]
    InvalidAmount { member: String, amount: Decimal },
    #[error("{member}: pide retirar {amount} y sus unidades valen {available}")]
    OverWithdrawal {
        member: String,
        amount: Decimal,
        available: Decimal,
    },
    #[error("{0} no tiene unidades en el fondo")]
    UnknownMember(String),
    #[error("el valor del fondo ({0}) no puede ser negativo")]
    NegativeValue(Decimal),
    #[error("el fondo no tiene unidades pero vale {0}: ese valor no es de nadie")]
    ValueWithoutUnits(Decimal),
    #[error("el NAV es cero: no se pueden comprar ni vender unidades")]
    ZeroNav,
}

/// `config/fund.toml`, validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundConfig {
    pub currency: String,
    pub min_contribution: Decimal,
    pub initial_nav: Decimal,
    pub unit_decimals: u32,
}

impl FundConfig {
    pub fn from_toml(text: &str) -> Result<Self, FundError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            currency: String,
            min_contribution: String,
            initial_nav: String,
            unit_decimals: u32,
        }
        let bad = |m: &str| FundError::Config(m.to_string());
        let raw: Raw = toml::from_str(text).map_err(|e| FundError::Config(e.to_string()))?;
        if raw.currency.len() != 3 || !raw.currency.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(bad("currency: código de 3 letras mayúsculas (ISO 4217)"));
        }
        let dec = |s: &str, name: &str| -> Result<Decimal, FundError> {
            s.parse()
                .map_err(|_| FundError::Config(format!("{name} tiene que ser un decimal")))
        };
        let min_contribution = dec(&raw.min_contribution, "min_contribution")?;
        let initial_nav = dec(&raw.initial_nav, "initial_nav")?;
        if min_contribution <= Decimal::ZERO || initial_nav <= Decimal::ZERO {
            return Err(bad(
                "min_contribution e initial_nav tienen que ser positivos",
            ));
        }
        if !(2..=10).contains(&raw.unit_decimals) {
            return Err(bad("unit_decimals tiene que estar entre 2 y 10"));
        }
        Ok(Self {
            currency: raw.currency,
            min_contribution,
            initial_nav,
            unit_decimals: raw.unit_decimals,
        })
    }
}

/// Lo de un miembro: sus unidades y lo que aportó y retiró en total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Account {
    pub units: Decimal,
    pub contributed: Decimal,
    pub withdrawn: Decimal,
}

/// Un pedido para la liquidación del lunes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Contribute {
        member: String,
        amount: Decimal,
    },
    Withdraw {
        member: String,
        amount: Decimal,
    },
    /// Retirar todo: vende todas las unidades del miembro.
    WithdrawAll {
        member: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    Contribution,
    Withdrawal,
}

/// Registro de un movimiento liquidado (historial auditable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movement {
    pub member: String,
    pub kind: MoveKind,
    /// Dinero que entra o sale del fondo, en céntimos.
    pub amount: Decimal,
    /// Unidades compradas o vendidas.
    pub units: Decimal,
    pub nav: Decimal,
}

/// Resultado de una liquidación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    pub nav: Decimal,
    pub movements: Vec<Movement>,
    /// Valor del fondo después de los movimientos.
    pub value_after: Decimal,
}

/// Una línea del estado de cuenta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub member: String,
    pub units: Decimal,
    /// Lo que valen sus unidades, en céntimos.
    pub value: Decimal,
    /// Aportes − retiros.
    pub net_contributions: Decimal,
    /// `value − net_contributions`.
    pub pnl: Decimal,
}

/// El fondo: las cuentas de los miembros. El valor total lo da el motor en cada llamada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fund {
    accounts: BTreeMap<String, Account>,
}

fn cents_ok(x: Decimal) -> bool {
    x > Decimal::ZERO && x.normalize().scale() <= CENTS
}

impl Fund {
    pub fn accounts(&self) -> &BTreeMap<String, Account> {
        &self.accounts
    }

    pub fn total_units(&self) -> Decimal {
        self.accounts.values().map(|a| a.units).sum()
    }

    /// NAV con el valor total `value`. Sin unidades, `initial_nav` (y el fondo tiene que valer 0).
    pub fn nav(&self, value: Decimal, cfg: &FundConfig) -> Result<Decimal, FundError> {
        if value < Decimal::ZERO {
            return Err(FundError::NegativeValue(value));
        }
        let units = self.total_units();
        if units.is_zero() {
            if !value.is_zero() {
                return Err(FundError::ValueWithoutUnits(value));
            }
            return Ok(cfg.initial_nav);
        }
        Ok(value / units)
    }

    /// Liquida los pedidos, todos al NAV de `value` (cierre del viernes), en el orden dado.
    /// Si un pedido no es válido, no se liquida ninguno.
    pub fn settle(
        &self,
        value: Decimal,
        requests: &[Request],
        cfg: &FundConfig,
    ) -> Result<(Fund, Settlement), FundError> {
        let nav = self.nav(value, cfg)?;
        if nav.is_zero() && !requests.is_empty() {
            return Err(FundError::ZeroNav);
        }
        let dp = cfg.unit_decimals;
        let mut next = self.clone();
        let mut value_after = value;
        let mut movements = Vec::with_capacity(requests.len());
        for r in requests {
            let m = match r {
                Request::Contribute { member, amount } => {
                    if !cents_ok(*amount) {
                        return Err(FundError::InvalidAmount {
                            member: member.clone(),
                            amount: *amount,
                        });
                    }
                    if *amount < cfg.min_contribution {
                        return Err(FundError::BelowMinimum {
                            member: member.clone(),
                            amount: *amount,
                            min: cfg.min_contribution,
                        });
                    }
                    let units = (amount / nav).round_dp_with_strategy(dp, RoundingStrategy::ToZero);
                    let acc = next.accounts.entry(member.clone()).or_default();
                    acc.units += units;
                    acc.contributed += amount;
                    value_after += amount;
                    Movement {
                        member: member.clone(),
                        kind: MoveKind::Contribution,
                        amount: *amount,
                        units,
                        nav,
                    }
                }
                Request::Withdraw { member, amount } => {
                    if !cents_ok(*amount) {
                        return Err(FundError::InvalidAmount {
                            member: member.clone(),
                            amount: *amount,
                        });
                    }
                    let acc = next
                        .accounts
                        .get_mut(member)
                        .filter(|a| !a.units.is_zero())
                        .ok_or_else(|| FundError::UnknownMember(member.clone()))?;
                    let available =
                        (acc.units * nav).round_dp_with_strategy(CENTS, RoundingStrategy::ToZero);
                    if *amount > available {
                        return Err(FundError::OverWithdrawal {
                            member: member.clone(),
                            amount: *amount,
                            available,
                        });
                    }
                    let units = (amount / nav)
                        .round_dp_with_strategy(dp, RoundingStrategy::AwayFromZero)
                        .min(acc.units);
                    acc.units -= units;
                    acc.withdrawn += amount;
                    value_after -= amount;
                    Movement {
                        member: member.clone(),
                        kind: MoveKind::Withdrawal,
                        amount: *amount,
                        units,
                        nav,
                    }
                }
                Request::WithdrawAll { member } => {
                    let acc = next
                        .accounts
                        .get_mut(member)
                        .filter(|a| !a.units.is_zero())
                        .ok_or_else(|| FundError::UnknownMember(member.clone()))?;
                    let units = acc.units;
                    let amount =
                        (units * nav).round_dp_with_strategy(CENTS, RoundingStrategy::ToZero);
                    acc.units = Decimal::ZERO;
                    acc.withdrawn += amount;
                    value_after -= amount;
                    Movement {
                        member: member.clone(),
                        kind: MoveKind::Withdrawal,
                        amount,
                        units,
                        nav,
                    }
                }
            };
            movements.push(m);
        }
        Ok((
            next,
            Settlement {
                nav,
                movements,
                value_after,
            },
        ))
    }

    /// Estado de cuenta con el valor total `value`, redondeado a céntimos. Lo de cada miembro se
    /// reparte por el método del mayor resto: la suma da exactamente el total, al céntimo.
    /// Los miembros sin unidades aparecen con valor 0 (su P&L es lo que retiraron de más o de menos).
    pub fn statement(&self, value: Decimal) -> Result<Vec<Line>, FundError> {
        if value < Decimal::ZERO {
            return Err(FundError::NegativeValue(value));
        }
        let total = value.round_dp_with_strategy(CENTS, RoundingStrategy::MidpointAwayFromZero);
        let units = self.total_units();
        if units.is_zero() && !total.is_zero() {
            return Err(FundError::ValueWithoutUnits(total));
        }
        let cent = Decimal::new(1, CENTS);
        // (miembro, valor exacto, valor redondeado hacia abajo)
        let mut shares: Vec<(&String, Decimal, Decimal)> = self
            .accounts
            .iter()
            .map(|(m, a)| {
                let exact = if units.is_zero() {
                    Decimal::ZERO
                } else {
                    a.units * total / units
                };
                (
                    m,
                    exact,
                    exact.round_dp_with_strategy(CENTS, RoundingStrategy::ToZero),
                )
            })
            .collect();
        let floor_sum: Decimal = shares.iter().map(|s| s.2).sum();
        let mut left = ((total - floor_sum) / cent).round();
        // Más resto primero; a igual resto, por id de miembro (orden estable y reproducible).
        let mut order: Vec<usize> = (0..shares.len()).collect();
        order.sort_by(|&i, &j| {
            (shares[j].1 - shares[j].2)
                .cmp(&(shares[i].1 - shares[i].2))
                .then_with(|| shares[i].0.cmp(shares[j].0))
        });
        for i in order {
            if left <= Decimal::ZERO {
                break;
            }
            if self.accounts[shares[i].0].units.is_zero() {
                continue;
            }
            shares[i].2 += cent;
            left -= Decimal::ONE;
        }
        Ok(shares
            .into_iter()
            .map(|(m, _, v)| {
                let a = self.accounts[m];
                let net = a.contributed - a.withdrawn;
                Line {
                    member: m.clone(),
                    units: a.units,
                    value: v,
                    net_contributions: net,
                    pnl: v - net,
                }
            })
            .collect())
    }
}

//! Armar el [`Setup`] de un recorrido **solo** desde la configuración (#76): catálogo de
//! instrumentos (`config/instruments.toml`), estrategia, riesgo y backtest. Si falta cualquier
//! dato del broker, se devuelven todos los pendientes y no se simula nada (spec §11).

use cfds_core::InstrumentId;
use cfds_market_data::{Catalog, PendingField, Sourced};
use cfds_risk::{InstrumentLimits, RiskConfig};
use cfds_strategy::Configs;
use chrono::NaiveTime;
use chrono_tz::Tz;
use rust_decimal::Decimal;

use crate::{AccountFx, BacktestConfig, FundingRates, InstrumentCosts, Setup};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupError {
    /// El instrumento no está en el catálogo.
    Unknown(InstrumentId),
    /// Faltan datos del broker: no se simula con valores inventados.
    Pending(Vec<PendingField>),
    /// El capital inicial tiene que ser mayor que 0.
    InvalidEquity(Decimal),
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SetupError::Unknown(id) => write!(f, "{id} no está en config/instruments.toml"),
            SetupError::Pending(p) => {
                write!(f, "faltan datos del broker:")?;
                for x in p {
                    write!(f, " {}.{} ({})", x.item, x.field, x.reason)?;
                }
                Ok(())
            }
            SetupError::InvalidEquity(e) => write!(f, "capital inicial inválido: {e}"),
        }
    }
}

impl std::error::Error for SetupError {}

fn pending<T>(item: &str, field: &'static str, v: &Sourced<T>) -> Option<PendingField> {
    match v {
        Sourced::Pending { reason } => Some(PendingField {
            item: item.to_owned(),
            field,
            reason: reason.clone(),
        }),
        Sourced::Known { .. } => None,
    }
}

/// Lo que el catálogo dice de un instrumento para operarlo: límites, costos, tipo de cambio y
/// financiación. Lo usan el backtest ([`setup_from_catalog`]) y el broker simulado
/// (`cfds-broker`): una sola fuente de estos datos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketTerms {
    pub limits: InstrumentLimits,
    pub costs: InstrumentCosts,
    /// Moneda del instrumento por unidad de la moneda de la cuenta, en cada momento. Con otra
    /// moneda que la cuenta queda [`AccountFx::Missing`]: quien tiene la base la reemplaza por
    /// [`AccountFx::Daily`] (serie `fx_rates`, ADR 0013 §4); sin eso, el recorrido frena con
    /// `RunError::MissingFxRate`.
    pub account_to_instrument: AccountFx,
    /// Constante (`[funding_benchmarks]`); para la serie diaria guardada (#79), reemplazarla por
    /// [`FundingRates::Daily`].
    pub funding_rates: FundingRates,
    pub funding_cutoff: NaiveTime,
    pub funding_tz: Tz,
}

/// [`MarketTerms`] de `instrument` con los valores del catálogo. Si falta cualquier dato del
/// broker, devuelve todos los pendientes.
pub fn market_terms_from_catalog(
    catalog: &Catalog,
    instrument: &InstrumentId,
) -> Result<MarketTerms, SetupError> {
    let spec = catalog
        .instruments
        .get(instrument)
        .ok_or_else(|| SetupError::Unknown(instrument.clone()))?;
    let costs = InstrumentCosts::from_spec(spec);
    let mut missing = costs.clone().err().unwrap_or_default();
    missing.extend(pending("account", "currency", &catalog.account_currency));
    missing.extend(pending(
        "account",
        "funding_cutoff",
        &catalog.funding_cutoff,
    ));
    // El nombre del tipo de referencia ya cuenta en los costos; acá, su valor.
    let rate = spec.terms.funding_benchmark.value().and_then(|name| {
        let v = catalog.funding_benchmarks.get(name)?;
        missing.extend(pending("funding_benchmarks", "rate", v));
        v.value().copied()
    });
    // Sin margen, `from_spec` da `None`; el pendiente ya está en `missing_costs`.
    let limits = InstrumentLimits::from_spec(spec);
    match (
        costs,
        limits,
        rate,
        catalog.account_currency.value(),
        catalog.funding_cutoff.value(),
    ) {
        // Cada pendiente deja un `Err` o un `None` acá: no hace falta otro chequeo.
        (Ok(costs), Some(limits), Some(benchmark_pct), Some(account), Some(&cutoff)) => {
            let account_to_instrument = if *account == spec.instrument.currency {
                AccountFx::Same
            } else {
                AccountFx::Missing {
                    pair: format!("{account}{}", spec.instrument.currency),
                }
            };
            Ok(MarketTerms {
                limits,
                costs,
                account_to_instrument,
                funding_rates: FundingRates::Constant(benchmark_pct),
                funding_cutoff: cutoff,
                funding_tz: catalog.funding_cutoff_timezone,
            })
        }
        _ => Err(SetupError::Pending(missing)),
    }
}

/// [`Setup`] de `instrument` con los valores del catálogo. El tipo de referencia de la
/// financiación queda **constante** (`[funding_benchmarks]`); para usar la serie diaria
/// guardada (#79), reemplazar `funding_rates` por [`FundingRates::Daily`].
pub fn setup_from_catalog(
    catalog: &Catalog,
    instrument: &InstrumentId,
    strategy: Configs,
    risk: RiskConfig,
    backtest: BacktestConfig,
    initial_equity: Decimal,
) -> Result<Setup, SetupError> {
    if initial_equity <= Decimal::ZERO {
        return Err(SetupError::InvalidEquity(initial_equity));
    }
    let m = market_terms_from_catalog(catalog, instrument)?;
    Ok(Setup {
        instrument: instrument.clone(),
        strategy,
        risk,
        limits: m.limits,
        costs: m.costs,
        day_tz: backtest.day_tz,
        backtest,
        initial_equity,
        account_to_instrument: m.account_to_instrument,
        funding_rates: m.funding_rates,
        funding_cutoff: m.funding_cutoff,
        funding_tz: m.funding_tz,
        futures_basis: None,
    })
}

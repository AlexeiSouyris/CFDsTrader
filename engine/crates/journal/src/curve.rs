//! La curva de una corrida (`contracts/backtest_run_curve.v1`, #299) y el detalle que lee el
//! gateway (`contracts/backtest_run_detail.v1`). La curva diaria y comprar y mantener los calcula
//! el backtest y se perdían al terminar; acá se guardan para que la UI los dibuje sin calcular.

use cfds_backtest::BuyAndHold;
use cfds_backtest::metrics::drawdown_curve;
use cfds_core::InstrumentId;
use cfds_core::decimal_str;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{BacktestRun, Outcome, Proposal};

/// `contracts/backtest_run_curve.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCurve {
    pub run_id: String,
    /// Un punto por día, en orden.
    pub points: Vec<CurvePoint>,
    /// Comprar y mantener de cada instrumento; vacío si la corrida no lo calculó.
    pub buy_and_hold: Vec<RunBuyAndHold>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurvePoint {
    pub date: NaiveDate,
    /// Capital realizado + no realizado al cierre del día, en la moneda de la cuenta.
    #[serde(with = "decimal_str::signed")]
    pub equity: Decimal,
    /// Fracción bajo el pico anterior (0 en un pico nuevo).
    #[serde(with = "decimal_str")]
    pub drawdown: Decimal,
}

/// Comprar y mantener un instrumento con los mismos costos (benchmark de la Fase 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunBuyAndHold {
    pub instrument_id: InstrumentId,
    /// Moneda del instrumento por punto.
    #[serde(with = "decimal_str")]
    pub size: Decimal,
    #[serde(with = "decimal_str")]
    pub entry_price: Decimal,
    #[serde(with = "decimal_str")]
    pub exit_price: Decimal,
    pub nights: u32,
    /// En la moneda de la cuenta.
    #[serde(with = "decimal_str")]
    pub commission: Decimal,
    /// En la moneda de la cuenta (positivo: pagó).
    #[serde(with = "decimal_str::signed")]
    pub funding: Decimal,
    /// En la moneda de la cuenta, con costos.
    #[serde(with = "decimal_str::signed")]
    pub net_pnl: Decimal,
    /// `net_pnl` / capital inicial, como fracción.
    #[serde(with = "decimal_str::signed")]
    pub total_return: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CurveInvalid {
    #[error("los días de la curva no están en orden o se repiten ({0})")]
    Unordered(NaiveDate),
    #[error("drawdown fuera de [0, 1] el {0}")]
    Drawdown(NaiveDate),
    #[error("capital inicial no positivo")]
    InitialEquity,
}

impl RunBuyAndHold {
    /// Desde el `BuyAndHold` del backtest (montos en la moneda del instrumento), pasado a la
    /// moneda de la cuenta como el resto de la corrida.
    pub fn from_backtest(
        instrument_id: InstrumentId,
        size: Decimal,
        bh: &BuyAndHold,
        account_to_instrument: Decimal,
        initial_equity: Decimal,
    ) -> Self {
        let net_pnl = bh.net_pnl / account_to_instrument;
        Self {
            instrument_id,
            size,
            entry_price: bh.entry_price,
            exit_price: bh.exit_price,
            nights: bh.nights,
            commission: bh.commission / account_to_instrument,
            funding: bh.funding / account_to_instrument,
            net_pnl,
            total_return: net_pnl / initial_equity,
        }
    }
}

impl RunCurve {
    /// La curva desde el capital diario del backtest: el drawdown de cada día lo calcula
    /// `cfds_backtest::metrics::drawdown_curve` (el mismo pico que `max_drawdown`).
    pub fn new(
        run_id: &str,
        initial_equity: Decimal,
        daily_equity: &[(NaiveDate, Decimal)],
        buy_and_hold: Vec<RunBuyAndHold>,
    ) -> Result<Self, CurveInvalid> {
        if initial_equity <= Decimal::ZERO {
            return Err(CurveInvalid::InitialEquity);
        }
        let points = daily_equity
            .iter()
            .zip(drawdown_curve(initial_equity, daily_equity))
            .map(|(&(date, equity), (_, drawdown))| CurvePoint {
                date,
                equity,
                drawdown,
            })
            .collect();
        let curve = Self {
            run_id: run_id.to_owned(),
            points,
            buy_and_hold,
        };
        curve.validate()?;
        Ok(curve)
    }

    pub fn validate(&self) -> Result<(), CurveInvalid> {
        for w in self.points.windows(2) {
            if w[1].date <= w[0].date {
                return Err(CurveInvalid::Unordered(w[1].date));
            }
        }
        if let Some(p) = self
            .points
            .iter()
            .find(|p| p.drawdown < Decimal::ZERO || p.drawdown > Decimal::ONE)
        {
            return Err(CurveInvalid::Drawdown(p.date));
        }
        Ok(())
    }
}

/// Una operación de la corrida: la propuesta y su resultado simulado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunTrade {
    pub proposal: Proposal,
    pub outcome: Option<Outcome>,
}

/// `contracts/backtest_run_detail.v1`: lo que devuelve `GET /api/v1/backtest_runs/{run_id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunDetail {
    pub run: BacktestRun,
    pub curve: Option<RunCurve>,
    pub trades: Vec<RunTrade>,
}

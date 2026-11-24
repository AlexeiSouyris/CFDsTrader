//! Comprar y mantener el CFD: la vara del criterio 5 de la familia A (#466 §4.3, pre-registro de
//! la ronda 2 §2.2).
//!
//! ```text
//! en [from, to): largo de 1 unidad (1 × el capital = el precio de entrada, en la moneda del
//!   instrumento) desde la apertura de la primera vela de la sesión de contado hasta el cierre
//!   de la última; se mantiene los fines de semana; en los vencimientos se encadena sin costo
//! costos: spread + slippage + comisión en la entrada y la salida; financiación de cada corte
//!   sobre el cierre de la última vela anterior, menos la base del futuro (#470)
//! retorno de cada mes (UTC) = valor a fin de mes / valor a fin del mes anterior − 1
//! ```
//! Mismas funciones de costos que el resto del backtest (`fill_price`, `commission`,
//! `funding_over`, `basis_carry`).

use cfds_core::Candle;
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, Months, TimeZone, Utc};
use rust_decimal::Decimal;

use crate::{
    Execution, RunError, Setup, basis_carry, commission, fill_price, funding_cutoffs, funding_over,
};

fn in_session(session: &Session, c: &Candle) -> bool {
    matches!(
        session.hours(session.trading_day(c.start)),
        Some(Some((open, close))) if c.start >= open && c.end() <= close
    )
}

/// Retorno de un mes UTC: `((año, mes), retorno)`.
pub type MonthReturn = ((i32, u32), Decimal);

/// Retornos mensuales `((año, mes), retorno)` de comprar y mantener en `[from, to)`, con los
/// meses UTC que empiezan en ese tramo. Vacío si no hay velas de sesión en el tramo.
pub fn buy_and_hold_returns(
    m15: &[Candle],
    session: &Session,
    rolls: &[DateTime<Utc>],
    setup: &Setup,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<MonthReturn>, RunError> {
    let lo = m15.partition_point(|c| c.start < from);
    let Some(entry) = (lo..m15.len()).find(|&i| m15[i].end() <= to && in_session(session, &m15[i]))
    else {
        return Ok(Vec::new());
    };
    let hi = m15.partition_point(|c| c.end() <= to);
    let Some(exit) = (entry + 1..hi)
        .rev()
        .find(|&i| in_session(session, &m15[i]))
    else {
        return Ok(Vec::new());
    };
    let size = Decimal::ONE;
    let cfg = &setup.backtest.costs;
    let entry_mid = m15[entry].open;
    let exit_mid = m15[exit].close;
    let exit_end = m15[exit].end();
    let entry_price = fill_price(Side::Long, Execution::Entry, entry_mid, &setup.costs, cfg);
    let exit_price = fill_price(Side::Long, Execution::Exit, exit_mid, &setup.costs, cfg);
    let entry_cost = entry_price - entry_mid + commission(&setup.costs, size, entry_price);
    let exit_cost = exit_mid - exit_price + commission(&setup.costs, size, exit_price);

    // P/L de precio al cierre de cada vela, encadenado en los vencimientos.
    let mut pnl = Vec::with_capacity(exit - entry + 1);
    let (mut realized, mut base) = (Decimal::ZERO, entry_mid);
    for k in entry..=exit {
        if k > entry
            && rolls
                .iter()
                .any(|&r| m15[k - 1].start < r && r <= m15[k].start)
        {
            realized += m15[k - 1].close - base;
            base = m15[k].open;
        }
        pnl.push(realized + m15[k].close - base);
    }
    // Índice (en `pnl`) de la última vela cerrada en `t`, si hay alguna.
    let last_closed = |t: DateTime<Utc>| -> Option<usize> {
        let n = m15[entry..=exit].partition_point(|c| c.end() <= t);
        n.checked_sub(1)
    };
    // Financiación neta de cada corte, sobre el cierre de la última vela anterior.
    let mut nights: Vec<(DateTime<Utc>, Decimal)> = Vec::new();
    for c in funding_cutoffs(
        m15[entry].start,
        exit_end,
        setup.funding_cutoff,
        setup.funding_tz,
    ) {
        let price = last_closed(c).map_or(entry_mid, |k| m15[entry + k].close);
        let mut cost = funding_over(
            Side::Long,
            price,
            size,
            &[c],
            &setup.funding_rates,
            &setup.costs,
        )
        .map_err(RunError::MissingFundingRate)?;
        if let Some(basis) = &setup.futures_basis {
            cost -= basis_carry(Side::Long, price, size, &[c], basis, &setup.costs)
                .map_err(RunError::MissingFundingRate)?;
        }
        nights.push((c, cost));
    }
    let value = |t: DateTime<Utc>| -> Decimal {
        let price = last_closed(t).map_or(Decimal::ZERO, |k| pnl[k]);
        let mut costs: Decimal = nights
            .iter()
            .filter(|(c, _)| *c <= t)
            .map(|(_, x)| *x)
            .sum();
        if t > m15[entry].start {
            costs += entry_cost;
        }
        if t >= exit_end {
            costs += exit_cost;
        }
        entry_mid + price - costs
    };
    // Fronteras: `from`, el inicio de cada mes UTC dentro del tramo y `to`.
    let mut bounds = vec![from];
    let mut m = Utc
        .with_ymd_and_hms(from.year(), from.month(), 1, 0, 0, 0)
        .single()
        .and_then(|d| d.checked_add_months(Months::new(1)));
    while let Some(b) = m.filter(|b| *b < to) {
        bounds.push(b);
        m = b.checked_add_months(Months::new(1));
    }
    bounds.push(to);
    Ok(bounds
        .windows(2)
        .map(|w| {
            let (v0, v1) = (value(w[0]), value(w[1]));
            ((w[0].year(), w[0].month()), v1 / v0 - Decimal::ONE)
        })
        .collect())
}

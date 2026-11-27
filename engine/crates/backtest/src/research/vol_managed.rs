//! Familia V: exposición manejada por volatilidad (Moreira y Muir, 2017), con la definición del
//! pre-registro de la ronda 3 §3 (`docs/analysis/2026-11-ronda3-preregistro.md`, #493).
//!
//! ```text
//! revisión mensual r_k = la de la familia A (`monthly_reviews`)
//! RV_k = Σ g², g = retornos encadenados entre r_{k−1}, los cierres de sesión entre medio y r_k
//! c    = desvío(R_k) / desvío(R_k / RV_{k−1}), solo con las revisiones del entrenamiento
//! w_k  = min(tope, c / RV_k), solo largos
//! en cada ventana: entra con el peso de la última revisión anterior, rebalancea en t + 1 de cada
//!   revisión, sale al cierre de la última vela de sesión; fines de semana incluidos, encadenado
//!   en los vencimientos; costos sobre las unidades que cambian y financiación − base cada noche
//! ```
//! Comprar y mantener es el mismo cálculo con un solo peso de 1 y sin rebalanceos.

use cfds_core::Candle;
use cfds_market_data::Session;
use cfds_strategy::Side;
use chrono::{DateTime, Datelike, Months, NaiveDate, TimeZone, Utc};
use rust_decimal::{Decimal, MathematicalOps};

use super::buy_and_hold::{MonthReturn, in_session};
use super::chained_return;
use crate::{
    Execution, RunError, Setup, basis_carry, commission, fill_price, funding_cutoffs, funding_over,
};

/// Índice de la última vela de 15m dentro de la sesión de contado de cada día con sesión, en
/// orden. La de un día solo sirve para decidir después de su cierre.
pub fn session_close_indices(m15: &[Candle], session: &Session) -> Vec<usize> {
    let mut out: Vec<(NaiveDate, usize)> = Vec::new();
    for (i, c) in m15.iter().enumerate() {
        if !in_session(session, c) {
            continue;
        }
        let day = session.trading_day(c.start);
        match out.last_mut() {
            Some((d, k)) if *d == day => *k = i,
            _ => out.push((day, i)),
        }
    }
    out.into_iter().map(|(_, i)| i).collect()
}

/// Varianza realizada de cada revisión (alineada con `reviews`, índices de vela en orden): suma
/// de los cuadrados de los retornos encadenados entre la revisión anterior, los cierres de sesión
/// estrictamente entre las dos y la revisión. `None` en la primera revisión, si falta un retorno
/// o si la suma es cero. Usa solo velas cerradas hasta la revisión.
pub fn review_variances(
    m15: &[Candle],
    rolls: &[DateTime<Utc>],
    reviews: &[usize],
    closes: &[usize],
) -> Vec<Option<Decimal>> {
    let mut out = Vec::with_capacity(reviews.len());
    for (k, &r) in reviews.iter().enumerate() {
        let Some(&prev) = k.checked_sub(1).and_then(|p| reviews.get(p)) else {
            out.push(None);
            continue;
        };
        let lo = closes.partition_point(|&i| i <= prev);
        let hi = closes.partition_point(|&i| i < r);
        let mut points = vec![prev];
        points.extend_from_slice(&closes[lo..hi.max(lo)]);
        points.push(r);
        let mut sum = Some(Decimal::ZERO);
        for w in points.windows(2) {
            sum = sum.and_then(|s| chained_return(m15, rolls, w[0], w[1]).map(|g| s + g * g));
        }
        out.push(sum.filter(|s| !s.is_zero()));
    }
    out
}

pub(crate) fn sample_sd(xs: &[Decimal]) -> Option<Decimal> {
    let n = Decimal::from(xs.len());
    if xs.len() < 2 {
        return None;
    }
    let m = xs.iter().sum::<Decimal>() / n;
    let v = xs.iter().map(|x| (*x - m) * (*x - m)).sum::<Decimal>() / (n - Decimal::ONE);
    v.sqrt()
}

/// Constante `c` de una ventana: con las revisiones `k ≥ 1` cuyo cierre cae en `[from, to)` y que
/// tienen `RV_{k−1}`, `c = desvío(R_k) / desvío(R_k / RV_{k−1})`, donde `R_k` es el retorno
/// encadenado de `r_{k−1}` a `r_k`. `None` si hay menos de `min_pairs` pares o un desvío es 0.
pub fn vol_constant(
    m15: &[Candle],
    rolls: &[DateTime<Utc>],
    reviews: &[usize],
    rv: &[Option<Decimal>],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    min_pairs: usize,
) -> Option<Decimal> {
    let mut raw = Vec::new();
    let mut scaled = Vec::new();
    for k in 1..reviews.len() {
        let end = m15.get(reviews[k])?.end();
        if end < from || end >= to {
            continue;
        }
        let (Some(Some(v)), Some(r)) = (
            rv.get(k - 1),
            chained_return(m15, rolls, reviews[k - 1], reviews[k]),
        ) else {
            continue;
        };
        raw.push(r);
        scaled.push(r / *v);
    }
    if raw.len() < min_pairs {
        return None;
    }
    let (a, b) = (sample_sd(&raw)?, sample_sd(&scaled)?);
    (!b.is_zero()).then(|| a / b)
}

/// Peso de cada revisión: `min(tope, c / RV_k)`; `None` sin varianza.
pub fn vol_weights(rv: &[Option<Decimal>], c: Decimal, cap: Decimal) -> Vec<Option<Decimal>> {
    rv.iter().map(|v| v.map(|v| (c / v).min(cap))).collect()
}

/// Retornos mensuales `((año, mes), retorno)` de mantener un largo con peso variable en
/// `[from, to)`. `schedule` = `(índice de la vela de la decisión, peso)` en orden: se entra en la
/// apertura de la primera vela de sesión del tramo con el peso de la última decisión cerrada
/// antes, y cada decisión dentro del tramo rebalancea en la apertura de la vela siguiente.
/// Todos los costos se multiplican por `stress`. Vacío si no hay velas de sesión o si no hay
/// ninguna decisión antes de la entrada.
pub fn managed_returns(
    m15: &[Candle],
    session: &Session,
    rolls: &[DateTime<Utc>],
    setup: &Setup,
    range: (DateTime<Utc>, DateTime<Utc>),
    schedule: &[(usize, Decimal)],
    stress: Decimal,
) -> Result<Vec<MonthReturn>, RunError> {
    managed_returns_with_spreads(m15, session, rolls, setup, range, schedule, stress, &[])
}

/// Lo mismo que [`managed_returns`], con otro spread (en puntos) para las ejecuciones en la
/// apertura de algunas velas: `spreads` = `(índice de la vela de la ejecución, spread)`. El
/// slippage de esa ejecución se calcula con ese spread (`fill_price`). Lo usa H2 del programa
/// Nasdaq (pre-registro §4.2): la entrada de las 08:00 ET cae fuera de la sesión de contado.
#[allow(
    clippy::too_many_arguments,
    reason = "la misma firma que managed_returns más los spreads"
)]
pub fn managed_returns_with_spreads(
    m15: &[Candle],
    session: &Session,
    rolls: &[DateTime<Utc>],
    setup: &Setup,
    (from, to): (DateTime<Utc>, DateTime<Utc>),
    schedule: &[(usize, Decimal)],
    stress: Decimal,
    spreads: &[(usize, Decimal)],
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
    let Some(&(_, w0)) = schedule
        .iter()
        .rev()
        .find(|(i, _)| m15.get(*i).is_some_and(|c| c.end() <= m15[entry].start))
    else {
        return Ok(Vec::new());
    };
    // Rebalanceos: decididos al cierre de `i` en [entry, exit), ejecutados en la apertura de i + 1.
    let rebalances: Vec<(usize, Decimal)> = schedule
        .iter()
        .filter(|(i, _)| *i >= entry && *i < exit)
        .map(|&(i, w)| (i + 1, w))
        .collect();
    let cfg = &setup.backtest.costs;
    // Costo de ejecutar `delta` unidades (con signo) al precio medio `mid`, en la vela `k`.
    let trade_cost = |delta: Decimal, mid: Decimal, k: usize| -> Decimal {
        let size = delta.abs();
        if size.is_zero() {
            return Decimal::ZERO;
        }
        let mut costs = setup.costs;
        if let Some(&(_, s)) = spreads.iter().find(|(at, _)| *at == k) {
            costs.spread = s;
        }
        let (exec, slip) = if delta > Decimal::ZERO {
            let p = fill_price(Side::Long, Execution::Entry, mid, &costs, cfg);
            (p, p - mid)
        } else {
            let p = fill_price(Side::Long, Execution::Exit, mid, &costs, cfg);
            (p, mid - p)
        };
        stress * (slip * size + commission(&costs, size, exec))
    };
    let capital = m15[entry].open;
    let cutoffs = funding_cutoffs(
        m15[entry].start,
        m15[exit].end(),
        setup.funding_cutoff,
        setup.funding_tz,
    );
    // Cambios del valor de la cuenta (en la moneda del instrumento) y cuándo cuentan.
    let mut events: Vec<(DateTime<Utc>, Decimal)> = Vec::new();
    let mut value = capital;
    let mut units = w0 * capital / m15[entry].open;
    let c = trade_cost(units, m15[entry].open, entry);
    events.push((m15[entry].end(), -c));
    value -= c;
    let mut next_cutoff = 0usize;
    let mut next_rebalance = 0usize;
    let mut last_close: Option<Decimal> = None;
    let charge_nights = |until: DateTime<Utc>,
                         units: Decimal,
                         last_close: Option<Decimal>,
                         value: &mut Decimal,
                         events: &mut Vec<(DateTime<Utc>, Decimal)>,
                         next_cutoff: &mut usize|
     -> Result<(), RunError> {
        while let Some(&cut) = cutoffs.get(*next_cutoff).filter(|c| **c <= until) {
            let price = last_close.unwrap_or(capital);
            let mut cost = funding_over(
                Side::Long,
                price,
                units,
                &[cut],
                &setup.funding_rates,
                &setup.costs,
            )
            .map_err(RunError::MissingFundingRate)?;
            if let Some(basis) = &setup.futures_basis {
                cost -= basis_carry(Side::Long, price, units, &[cut], basis, &setup.costs)
                    .map_err(RunError::MissingFundingRate)?;
            }
            events.push((cut, -stress * cost));
            *value -= stress * cost;
            *next_cutoff += 1;
        }
        Ok(())
    };
    for k in entry..=exit {
        let candle = &m15[k];
        charge_nights(
            candle.start,
            units,
            last_close,
            &mut value,
            &mut events,
            &mut next_cutoff,
        )?;
        let rolled = k > entry
            && rolls
                .iter()
                .any(|&r| m15[k - 1].start < r && r <= candle.start);
        // Del cierre anterior a la apertura, con las unidades viejas (sin el salto del vencimiento).
        let mut pnl = match last_close {
            Some(prev) if !rolled => units * (candle.open - prev),
            _ => Decimal::ZERO,
        };
        let mut cost = Decimal::ZERO;
        if let Some(&(_, w)) = rebalances.get(next_rebalance).filter(|(at, _)| *at == k) {
            next_rebalance += 1;
            let target = w * (value + pnl) / candle.open;
            cost = trade_cost(target - units, candle.open, k);
            units = target;
        }
        pnl += units * (candle.close - candle.open);
        if k == exit {
            cost += trade_cost(-units, candle.close, usize::MAX); // al cierre: spread de siempre
        }
        value += pnl - cost;
        events.push((candle.end(), pnl - cost));
        last_close = Some(candle.close);
    }
    charge_nights(
        m15[exit].end(),
        units,
        last_close,
        &mut value,
        &mut events,
        &mut next_cutoff,
    )?;
    let value_at = |t: DateTime<Utc>| -> Decimal {
        capital
            + events
                .iter()
                .filter(|(at, _)| *at <= t)
                .map(|(_, x)| *x)
                .sum::<Decimal>()
    };
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
            let (v0, v1) = (value_at(w[0]), value_at(w[1]));
            ((w[0].year(), w[0].month()), v1 / v0 - Decimal::ONE)
        })
        .collect())
}

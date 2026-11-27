//! Vista previa de riesgo del laboratorio (#777, `docs/design/2026-11-laboratorio.md` §4.1,
//! `contracts/risk_preview.v1`): con un capital hipotético, qué tamaño daría el módulo de riesgo
//! para un stop dado, cuánto se pierde si salta y qué límite recorta o rechaza.
//!
//! **Un solo cálculo:** el mismo `cfds_risk::check` del broker simulado, con una cuenta sin
//! historia (sin posiciones abiertas, sin caída, sin kill switch) y el riesgo por operación
//! recortado al de `config/risk.toml`. Los costos de entrar y salir son los del backtest
//! (`fill_price`, `commission`). Función pura: el gateway le pasa las velas cerradas, el
//! instrumento del catálogo y el tipo de cambio.

use cfds_backtest::{CostConfig, Execution, InstrumentCosts, commission, fill_price};
use cfds_broker::paper::rejection_code;
use cfds_core::broker::OrderRejection;
use cfds_core::{Candle, InstrumentId};
use cfds_indicators::wilder_atr;
use cfds_market_data::InstrumentSpec;
use cfds_risk::{Account, InstrumentLimits, RiskConfig};
use cfds_strategy::Side;
use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// El stop pedido: a k × ATR de la referencia o un precio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StopRequest {
    Atr {
        #[serde(with = "cfds_core::decimal_str")]
        k: Decimal,
    },
    Price {
        #[serde(with = "cfds_core::decimal_str")]
        price: Decimal,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Long,
    Short,
}

/// `risk_preview.v1#/$defs/request`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    pub instrument_id: InstrumentId,
    pub timeframe: String,
    pub direction: Direction,
    #[serde(with = "cfds_core::decimal_str")]
    pub equity: Decimal,
    #[serde(with = "cfds_core::decimal_str")]
    pub risk_pct: Decimal,
    pub atr_period: usize,
    pub stop: StopRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewLimits {
    pub max_risk_pct: String,
    pub min_stop_atr: String,
    pub max_stop_atr: String,
    pub gap_atr: String,
    pub max_leverage: String,
    pub margin_pct: Option<String>,
}

/// `risk_preview.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RiskPreview {
    pub instrument_id: String,
    pub timeframe: String,
    pub direction: Direction,
    pub as_of: Option<String>,
    pub reference: Option<String>,
    pub atr: Option<String>,
    pub atr_period: usize,
    pub stop: Option<String>,
    pub stop_atr: Option<String>,
    pub equity: String,
    pub risk_pct_requested: String,
    pub risk_pct_applied: String,
    pub accepted: bool,
    pub rejection: Option<OrderRejection>,
    pub size: Option<String>,
    pub notional: Option<String>,
    pub leverage: Option<String>,
    pub margin: Option<String>,
    pub risk: Option<String>,
    pub risk_pct: Option<String>,
    pub leverage_capped: bool,
    pub group_capped: bool,
    pub round_trip_cost: Option<String>,
    pub account_currency: String,
    pub instrument_currency: String,
    pub fx_rate: Option<String>,
    pub limits: PreviewLimits,
    pub notes: Vec<String>,
}

/// Lo que necesita la vista previa además del pedido.
pub struct PreviewInputs<'a> {
    /// Velas **cerradas** del timeframe del pedido, de la más vieja a la más nueva.
    pub candles: &'a [Candle],
    pub spec: &'a InstrumentSpec,
    pub risk: &'a RiskConfig,
    pub costs: &'a CostConfig,
    pub account_currency: &'a str,
    /// Moneda del instrumento por unidad de la de la cuenta; `None` si no hay tipo publicado.
    pub fx: Option<Decimal>,
}

/// Texto decimal para mostrar: 6 decimales como mucho, sin ceros de más.
fn text(d: Decimal) -> String {
    let d = d.round_dp(6).normalize();
    if d.is_zero() {
        "0".into()
    } else {
        d.to_string()
    }
}

fn side(d: Direction) -> Side {
    match d {
        Direction::Long => Side::Long,
        Direction::Short => Side::Short,
    }
}

/// Arma la vista previa (ver el módulo).
pub fn preview(req: &PreviewRequest, i: &PreviewInputs) -> RiskPreview {
    let applied = req.risk_pct.min(i.risk.risk_per_trade_pct);
    let limits = InstrumentLimits::from_spec(i.spec);
    let last = i.candles.last();
    let reference = last.map(|c| c.close);
    let atr = (req.atr_period > 0)
        .then(|| {
            wilder_atr(i.candles, req.atr_period)
                .last()
                .copied()
                .flatten()
        })
        .flatten();
    let stop = match (req.stop, reference, atr) {
        (StopRequest::Price { price }, _, _) => Some(price),
        (StopRequest::Atr { k }, Some(r), Some(a)) => Some(match req.direction {
            Direction::Long => r - k * a,
            Direction::Short => r + k * a,
        }),
        _ => None,
    };
    let mut notes = vec![format!(
        "precio de referencia: cierre de la última vela cerrada de {} guardada (futuros, la fuente \
         del laboratorio); cuenta hipotética sin posiciones abiertas",
        req.timeframe
    )];
    if applied < req.risk_pct {
        notes.push(format!(
            "el riesgo pedido ({} %) pasa el de config/risk.toml: se usa {} %",
            text(req.risk_pct),
            text(applied)
        ));
    }
    let same_currency = i.spec.instrument.currency.as_str() == i.account_currency;
    let fx = if same_currency {
        Some(Decimal::ONE)
    } else {
        i.fx
    };
    let checked: Result<cfds_risk::Sizing, OrderRejection> = match (reference, atr, stop, &limits) {
        (None, _, _, _) => Err(OrderRejection::NoPrice),
        (_, None, _, _) => Err(OrderRejection::NoRecentCandles),
        (_, _, None, _) | (_, _, _, None) => Err(OrderRejection::InvalidInput),
        _ if req.equity <= Decimal::ZERO || req.risk_pct <= Decimal::ZERO => {
            Err(OrderRejection::InvalidInput)
        }
        (Some(r), _, Some(s), _)
            if (req.direction == Direction::Long && s >= r)
                || (req.direction == Direction::Short && s <= r) =>
        {
            Err(OrderRejection::StopWrongSide)
        }
        _ if fx.is_none() => Err(OrderRejection::MissingFxRate),
        (Some(r), Some(a), Some(s), Some(l)) => {
            let mut cfg = i.risk.clone();
            cfg.risk_per_trade_pct = applied;
            let account = Account {
                equity: req.equity,
                peak_equity: req.equity,
                day_start_equity: req.equity,
                free_margin: req.equity,
                kill_switch: false,
                open_positions: Vec::new(),
                realized_vol_pct: None,
            };
            let request = cfds_risk::Request {
                instrument: req.instrument_id.clone(),
                entry: r,
                stop: s,
                atr: a,
                account_to_instrument: fx.unwrap_or(Decimal::ONE),
            };
            cfds_risk::check(&request, &account, l, &cfg).map_err(|e| rejection_code(&e))
        }
    };
    if limits.is_none() {
        notes.push("al catálogo le falta el margen de este instrumento".into());
    }
    let fx_value = fx.unwrap_or(Decimal::ONE);
    let round_trip = match (&checked, reference, InstrumentCosts::from_spec(i.spec)) {
        (Ok(sz), Some(r), Ok(c)) => {
            let s = side(req.direction);
            let entry = fill_price(s, Execution::Entry, r, &c, i.costs);
            let exit = fill_price(s, Execution::Exit, r, &c, i.costs);
            let fees = commission(&c, sz.size, entry) + commission(&c, sz.size, exit);
            Some(((entry - exit).abs() * sz.size + fees) / fx_value)
        }
        (Ok(_), _, Err(_)) => {
            notes.push("al catálogo le falta algún costo: sin costo de ida y vuelta".into());
            None
        }
        _ => None,
    };
    if let Ok(sz) = &checked {
        if sz.leverage_capped {
            notes.push("el tope de apalancamiento redujo el tamaño".into());
        }
        if sz.group_capped {
            notes.push("el tope de riesgo de un grupo correlacionado redujo el tamaño".into());
        }
    }
    notes.push(
        "la financiación nocturna depende del tipo de referencia de cada noche: la muestra el \
         informe de una corrida, no esta vista"
            .into(),
    );
    let ok = checked.as_ref().ok();
    let risk_account = ok.map(|sz| sz.risk / fx_value);
    RiskPreview {
        instrument_id: req.instrument_id.as_str().to_owned(),
        timeframe: req.timeframe.clone(),
        direction: req.direction,
        as_of: last.map(|c| c.end().to_rfc3339_opts(SecondsFormat::AutoSi, true)),
        reference: reference.map(text),
        atr: atr.map(text),
        atr_period: req.atr_period,
        stop: stop.map(text),
        stop_atr: match (reference, stop, atr) {
            (Some(r), Some(s), Some(a)) if !a.is_zero() => Some(text((r - s).abs() / a)),
            _ => None,
        },
        equity: text(req.equity),
        risk_pct_requested: text(req.risk_pct),
        risk_pct_applied: text(applied),
        accepted: ok.is_some(),
        rejection: checked.as_ref().err().copied(),
        size: ok.map(|sz| text(sz.size)),
        notional: ok.map(|sz| text(sz.notional)),
        leverage: ok.map(|sz| text(sz.leverage)),
        margin: ok.map(|sz| text(sz.margin)),
        risk: risk_account.map(text),
        risk_pct: risk_account.map(|r| text(r / req.equity * Decimal::ONE_HUNDRED)),
        leverage_capped: ok.is_some_and(|sz| sz.leverage_capped),
        group_capped: ok.is_some_and(|sz| sz.group_capped),
        round_trip_cost: round_trip.map(text),
        account_currency: i.account_currency.to_owned(),
        instrument_currency: i.spec.instrument.currency.as_str().to_owned(),
        fx_rate: fx.map(text),
        limits: PreviewLimits {
            max_risk_pct: text(i.risk.risk_per_trade_pct),
            min_stop_atr: text(i.risk.min_stop_atr),
            max_stop_atr: text(i.risk.max_stop_atr),
            gap_atr: text(i.risk.gap_atr),
            max_leverage: text(i.spec.max_leverage),
            margin_pct: limits.map(|l| text(l.margin_pct)),
        },
        notes,
    }
}

/// Las velas que entran en la vista: solo las cerradas a `now`.
pub fn closed_at(candles: Vec<Candle>, now: DateTime<Utc>) -> Vec<Candle> {
    candles
        .into_iter()
        .filter(|c| c.is_closed_at(now))
        .collect()
}

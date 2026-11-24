//! Riesgo (spec §7, §8 y §10). Módulo **independiente**: toda propuesta pasa por `check` y no
//! hay otro camino para obtener un tamaño de posición. Funciones puras: misma entrada, misma
//! salida, sin estado oculto. La persistencia del kill switch es de quien llama.

pub mod config;

use cfds_core::InstrumentId;
use cfds_market_data::InstrumentSpec;
use rust_decimal::Decimal;

pub use config::RiskConfig;

/// Lo que el módulo necesita saber de un instrumento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstrumentLimits {
    /// Tope propio de apalancamiento (índices 10:1, acciones 3:1, oro 10:1; ADR 0013).
    pub max_leverage: Decimal,
    /// Margen requerido por el broker, en % del nocional.
    pub margin_pct: Decimal,
    /// Tamaño mínimo del broker. `None` mientras no esté confirmado (#8): entonces no se abre.
    pub min_deal_size: Option<Decimal>,
}

impl InstrumentLimits {
    /// Desde el catálogo. `None` si el margen todavía no tiene fuente.
    pub fn from_spec(spec: &InstrumentSpec) -> Option<Self> {
        Some(Self {
            max_leverage: spec.max_leverage,
            margin_pct: *spec.terms.margin_retail_pct.value()?,
            min_deal_size: spec.terms.min_deal_size.value().copied(),
        })
    }
}

/// Estado de la cuenta al cierre de la vela, en la moneda de la cuenta (EUR).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Capital actual (incluye el P/L no realizado).
    pub equity: Decimal,
    /// Máximo histórico del capital (para el drawdown).
    pub peak_equity: Decimal,
    /// Capital a las 00:00 de Madrid de hoy.
    pub day_start_equity: Decimal,
    pub free_margin: Decimal,
    pub kill_switch: bool,
    /// Instrumento de cada posición abierta.
    pub open_positions: Vec<InstrumentId>,
}

/// Una operación que la estrategia quiere abrir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub instrument: InstrumentId,
    pub entry: Decimal,
    pub stop: Decimal,
    pub atr: Decimal,
    /// Unidades de la moneda del instrumento por 1 unidad de la moneda de la cuenta
    /// (por ejemplo, USD por EUR). 1 si son la misma.
    pub account_to_instrument: Decimal,
}

/// Tamaño aprobado. Montos en la moneda del instrumento, salvo que se indique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sizing {
    /// Moneda del instrumento por punto de movimiento.
    pub size: Decimal,
    pub notional: Decimal,
    /// Nocional / capital, ya con el tope aplicado.
    pub leverage: Decimal,
    pub margin: Decimal,
    /// Lo que se pierde si salta el stop (sin gaps ni costos).
    pub risk: Decimal,
    /// `true` si el tope de apalancamiento redujo el tamaño (el riesgo queda por debajo del 1 %).
    pub leverage_capped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// Datos imposibles (capital, precios, ATR o tipo de cambio no positivos, stop en la entrada).
    InvalidInput(&'static str),
    KillSwitchActive,
    /// El drawdown llegó al límite: quien llama tiene que **activar** el kill switch y alertar.
    DrawdownLimit,
    DailyLossLimit,
    MaxOpenPositions,
    InstrumentAlreadyOpen,
    StopTooClose,
    StopTooFar,
    InsufficientMargin,
    BelowMinDealSize,
    /// El mínimo del broker no está confirmado todavía: no se abre (no se inventa).
    MinDealSizeUnknown,
}

fn pct(x: Decimal) -> Decimal {
    x / Decimal::ONE_HUNDRED
}

/// Valida la operación contra todos los límites y, si pasa, devuelve su tamaño.
pub fn check(
    req: &Request,
    account: &Account,
    limits: &InstrumentLimits,
    cfg: &RiskConfig,
) -> Result<Sizing, Rejection> {
    let positive = [
        (account.equity, "capital"),
        (account.peak_equity, "máximo del capital"),
        (account.day_start_equity, "capital al inicio del día"),
        (req.entry, "entrada"),
        (req.stop, "stop"),
        (req.atr, "ATR"),
        (req.account_to_instrument, "tipo de cambio"),
        (limits.max_leverage, "tope de apalancamiento"),
        (limits.margin_pct, "margen del instrumento"),
    ];
    if let Some((_, what)) = positive.iter().find(|(v, _)| *v <= Decimal::ZERO) {
        return Err(Rejection::InvalidInput(what));
    }

    // Límites de la cuenta (§10), del más grave al menos grave.
    if account.kill_switch {
        return Err(Rejection::KillSwitchActive);
    }
    let drawdown_floor = account.peak_equity * (Decimal::ONE - pct(cfg.max_drawdown_pct));
    if account.equity <= drawdown_floor {
        return Err(Rejection::DrawdownLimit);
    }
    let daily_floor = account.day_start_equity * (Decimal::ONE - pct(cfg.max_daily_loss_pct));
    if account.equity <= daily_floor {
        return Err(Rejection::DailyLossLimit);
    }
    if account.open_positions.len() >= cfg.max_open_positions {
        return Err(Rejection::MaxOpenPositions);
    }
    let same = account
        .open_positions
        .iter()
        .filter(|i| **i == req.instrument)
        .count();
    if same >= cfg.max_positions_per_instrument {
        return Err(Rejection::InstrumentAlreadyOpen);
    }

    // Distancia al stop (§7.7).
    let r = (req.entry - req.stop).abs();
    if r.is_zero() {
        return Err(Rejection::InvalidInput("stop en la entrada"));
    }
    if r < cfg.min_stop_atr * req.atr {
        return Err(Rejection::StopTooClose);
    }
    if r > cfg.max_stop_atr * req.atr {
        return Err(Rejection::StopTooFar);
    }

    // Tamaño (§8), todo en la moneda del instrumento.
    let fx = req.account_to_instrument;
    let equity = account.equity * fx;
    let mut size = pct(cfg.risk_per_trade_pct) * equity / r;
    let max_size = limits.max_leverage * equity / req.entry;
    let leverage_capped = size > max_size;
    if leverage_capped {
        size = max_size;
    }
    let notional = size * req.entry;
    let margin = notional * pct(limits.margin_pct);
    if margin > account.free_margin * fx {
        return Err(Rejection::InsufficientMargin);
    }
    match limits.min_deal_size {
        None => return Err(Rejection::MinDealSizeUnknown),
        Some(min) if size < min => return Err(Rejection::BelowMinDealSize),
        Some(_) => {}
    }
    Ok(Sizing {
        size,
        notional,
        leverage: notional / equity,
        margin,
        risk: size * r,
        leverage_capped,
    })
}
